//! SQLite storage — sessions, raw samples, per-minute rollups, daily totals.
//! Ported from backend/db.py. Single connection guarded by a Mutex (our write
//! rate is ~3 Hz, reads are light), WAL mode, foreign keys on.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_ts REAL NOT NULL,
    ended_ts REAL,
    local_date TEXT NOT NULL,
    display_unit TEXT NOT NULL,
    start_steps INTEGER,
    start_duration_s INTEGER,
    steps_end INTEGER,
    duration_s_end INTEGER,
    distance_raw_end INTEGER,
    calories_end INTEGER,
    speed_raw_last INTEGER,
    closed_reason TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_date ON sessions(local_date);
CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_ts);

CREATE TABLE IF NOT EXISTS samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER,
    ts REAL NOT NULL,
    steps INTEGER,
    duration_s INTEGER,
    speed_raw INTEGER,
    distance_raw INTEGER,
    calories INTEGER,
    status INTEGER,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
CREATE INDEX IF NOT EXISTS idx_samples_session ON samples(session_id);
CREATE INDEX IF NOT EXISTS idx_samples_ts ON samples(ts);

CREATE TABLE IF NOT EXISTS sample_rollups_1m (
    bucket_ts INTEGER NOT NULL,
    session_id INTEGER,
    steps_delta INTEGER NOT NULL DEFAULT 0,
    distance_raw_delta INTEGER NOT NULL DEFAULT 0,
    calories_delta INTEGER NOT NULL DEFAULT 0,
    duration_s_delta INTEGER NOT NULL DEFAULT 0,
    speed_raw_min INTEGER,
    speed_raw_avg REAL,
    speed_raw_max INTEGER,
    running_samples INTEGER NOT NULL DEFAULT 0,
    total_samples INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket_ts, session_id),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
CREATE INDEX IF NOT EXISTS idx_rollups_1m_ts ON sample_rollups_1m(bucket_ts);

CREATE TABLE IF NOT EXISTS rollup_state (
    kind TEXT PRIMARY KEY,
    last_rolled_ts REAL NOT NULL DEFAULT 0,
    last_run_ts REAL
);

CREATE TABLE IF NOT EXISTS speed_marks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts REAL NOT NULL,
    set_speed REAL NOT NULL,
    unit TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_speed_marks_ts ON speed_marks(ts);
"#;

const ROLLUP_RESOLUTION_S: i64 = 60;
const ROLLUP_KIND: &str = "samples_1m";
/// How far before `last_rolled` the de-glitch walk re-reads samples purely to
/// establish the previous-value context (so the first new bucket's increment
/// and any boundary spike are judged correctly). Those older buckets are not
/// re-written.
const ROLLUP_DEGLITCH_LOOKBACK_S: f64 = 180.0;

pub fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

fn local_date(ts: f64) -> String {
    // chrono local time, matching Python datetime.fromtimestamp(ts).strftime("%Y-%m-%d")
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(ts as i64, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// De-glitched cumulative total over the SC110's noisy free-running odometer.
///
/// The console reports a counter that (a) carries across BLE reconnects, (b)
/// resets to ~0 on a power-cycle, and (c) occasionally emits a single garbage
/// sample. We:
///   - discard isolated spike-and-revert outliers (a value far from BOTH
///     neighbours in the same direction — the signature of a stale frame),
///   - sum genuine positive increments (so real steps walked across a
///     reconnect gap are kept),
///   - treat a drop to `<= reset_max` as a power-cycle reset (the post-reset
///     climb is then counted incrementally), and
///   - ignore any other decrease (a non-reset dip).
///
/// This replaces a naive LAG accumulator that added the full counter value on
/// every decrease, which turned a stale `…1800, 346, 1891…` read into ~1500
/// phantom steps after a reconnect.
///
/// Core pass: calls `emit(i, delta)` once per *accepted* positive increment at
/// sample index `i` (including the first-sample baseline). Day totals and the
/// hourly breakdown both drive off this so they always reconcile.
fn deglitch_walk(values: &[i64], spike: i64, reset_max: i64, mut emit: impl FnMut(usize, i64)) {
    let n = values.len();
    let mut prev: Option<i64> = None;
    for i in 0..n {
        let v = values[i];
        // Drop an isolated outlier: far from both neighbours in the same
        // direction (spikes up or down that immediately revert).
        if i > 0 && i + 1 < n {
            let p = values[i - 1];
            let nx = values[i + 1];
            let spike_up = v - p > spike && v - nx > spike;
            let spike_down = p - v > spike && nx - v > spike;
            if spike_up || spike_down {
                continue;
            }
        }
        match prev {
            None => {
                // First accepted reading already reflects steps walked today.
                let base = v.max(0);
                if base > 0 {
                    emit(i, base);
                }
                prev = Some(v);
            }
            Some(pv) => {
                let d = v - pv;
                if d > 0 {
                    emit(i, d);
                    prev = Some(v);
                } else if d < 0 && (v <= reset_max || v * 2 < pv) {
                    // Genuine counter reset: dropped to ~0, OR fell by more than
                    // half — the SC110 zeroes its step counter between sessions and
                    // we often catch it after it has already climbed a little
                    // (e.g. 488 -> 42). The post-reset climb is counted from here.
                    prev = Some(v);
                }
                // else: shallow non-reset dip — keep prev, add nothing.
            }
        }
    }
}

/// De-glitched cumulative total — sum of every accepted increment.
fn deglitch_total(values: &[i64], spike: i64, reset_max: i64) -> i64 {
    let mut total: i64 = 0;
    deglitch_walk(values, spike, reset_max, |_, d| total += d);
    total
}

/// De-glitched increments per (bucket_ts, session_id) for the rollup writer.
/// Walks the continuous cross-session stream (samples must be ordered by ts,id
/// with NULLs pre-filtered) so a stale frame at a session boundary still has
/// neighbour context. Increments only — the starting baseline is not a "step
/// added", matching the analytics range semantics.
fn deglitch_bucketed(
    samples: &[(i64, i64, i64)], // (ts, session_id, value)
    resolution_s: i64,
    spike: i64,
    reset_max: i64,
) -> std::collections::HashMap<(i64, i64), i64> {
    let n = samples.len();
    let mut out: std::collections::HashMap<(i64, i64), i64> = std::collections::HashMap::new();
    let mut prev: Option<i64> = None;
    for i in 0..n {
        let (ts, sess, v) = samples[i];
        if i > 0 && i + 1 < n {
            let p = samples[i - 1].2;
            let nx = samples[i + 1].2;
            if (v - p > spike && v - nx > spike) || (p - v > spike && nx - v > spike) {
                continue;
            }
        }
        match prev {
            None => prev = Some(v),
            Some(pv) => {
                let d = v - pv;
                if d > 0 {
                    let bucket = (ts / resolution_s) * resolution_s;
                    *out.entry((bucket, sess)).or_insert(0) += d;
                    prev = Some(v);
                } else if d < 0 && (v <= reset_max || v * 2 < pv) {
                    prev = Some(v); // reset to ~0 or a drop of more than half
                }
            }
        }
    }
    out
}

/// De-glitched total for one metric: use the sampled odometer when samples
/// exist, otherwise fall back to the session end-minus-start sum. `start_col`
/// empty means the metric has no per-session start (count from 0).
fn metric_total(
    c: &Connection,
    local_date_s: &str,
    values: &[i64],
    spike: i64,
    reset_max: i64,
    end_col: &str,
    start_col: &str,
) -> Result<i64> {
    if !values.is_empty() {
        return Ok(deglitch_total(values, spike, reset_max));
    }
    let start_expr = if start_col.is_empty() {
        "0".to_string()
    } else {
        format!("COALESCE(se.{start_col}, 0)")
    };
    let sql = format!(
        "SELECT COALESCE(SUM(COALESCE(se.{end_col}, 0) - {start_expr}), 0)
         FROM sessions se WHERE se.local_date = ?"
    );
    Ok(c.query_row(&sql, params![local_date_s], |r| r.get(0))?)
}

/// Human-readable local timestamp for diagnostic dumps ("YYYY-MM-DD HH:MM:SS").
fn iso_local(ts: f64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(ts as i64, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: i64,
    pub started_ts: f64,
    pub ended_ts: Option<f64>,
    pub local_date: String,
    pub display_unit: String,
    pub start_steps: Option<i64>,
    pub steps_end: Option<i64>,
    pub duration_s_end: Option<i64>,
    pub distance_raw_end: Option<i64>,
    pub calories_end: Option<i64>,
    pub speed_raw_last: Option<i64>,
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    /// Lock the connection, recovering from a poisoned mutex (a prior panic
    /// while holding the lock) instead of cascading the panic into every call.
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // --- sessions --------------------------------------------------------

    pub fn open_session(
        &self,
        ts: f64,
        display_unit: &str,
        start_steps: Option<u32>,
        start_duration_s: Option<u32>,
    ) -> Result<i64> {
        let c = self.conn();
        c.execute(
            "INSERT INTO sessions(started_ts, local_date, display_unit, start_steps, start_duration_s)
             VALUES (?, ?, ?, ?, ?)",
            params![ts, local_date(ts), display_unit, start_steps, start_duration_s],
        )?;
        Ok(c.last_insert_rowid())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn close_session(
        &self,
        session_id: i64,
        ts: f64,
        steps: Option<u32>,
        duration_s: Option<u32>,
        distance_raw: Option<u32>,
        calories: Option<u32>,
        speed_raw: Option<u32>,
        reason: &str,
    ) -> Result<()> {
        let c = self.conn();
        c.execute(
            "UPDATE sessions SET ended_ts=?, steps_end=?, duration_s_end=?, distance_raw_end=?,
                                 calories_end=?, speed_raw_last=?, closed_reason=?
             WHERE id=? AND ended_ts IS NULL",
            params![ts, steps, duration_s, distance_raw, calories, speed_raw, reason, session_id],
        )?;
        Ok(())
    }

    pub fn update_active_session(
        &self,
        session_id: i64,
        steps: Option<u32>,
        duration_s: Option<u32>,
        distance_raw: Option<u32>,
        calories: Option<u32>,
        speed_raw: Option<u32>,
    ) -> Result<()> {
        let c = self.conn();
        c.execute(
            "UPDATE sessions SET steps_end=?, duration_s_end=?, distance_raw_end=?,
                                 calories_end=?, speed_raw_last=? WHERE id=?",
            params![steps, duration_s, distance_raw, calories, speed_raw, session_id],
        )?;
        Ok(())
    }

    pub fn close_stale_active(&self, reason: &str) -> Result<usize> {
        let c = self.conn();
        let n = c.execute(
            "UPDATE sessions SET ended_ts=?, closed_reason=? WHERE ended_ts IS NULL",
            params![now_ts(), reason],
        )?;
        Ok(n)
    }

    pub fn list_sessions(&self, limit: i64) -> Result<Vec<Session>> {
        let c = self.conn();
        let mut stmt =
            c.prepare("SELECT * FROM sessions ORDER BY started_ts DESC LIMIT ?")?;
        let rows = stmt
            .query_map(params![limit], row_to_session)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_session(&self, id: i64) -> Result<Option<Session>> {
        let c = self.conn();
        let row = c
            .query_row("SELECT * FROM sessions WHERE id=?", params![id], row_to_session)
            .optional()?;
        Ok(row)
    }

    // --- samples ---------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn insert_sample(
        &self,
        session_id: Option<i64>,
        ts: f64,
        steps: Option<u32>,
        duration_s: Option<u32>,
        speed_raw: Option<u32>,
        distance_raw: Option<u32>,
        calories: Option<u32>,
        status: Option<u8>,
    ) -> Result<()> {
        let c = self.conn();
        c.execute(
            "INSERT INTO samples(session_id, ts, steps, duration_s, speed_raw, distance_raw, calories, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![session_id, ts, steps, duration_s, speed_raw, distance_raw, calories, status],
        )?;
        Ok(())
    }

    // --- aggregates ------------------------------------------------------

    /// Per-day totals from the raw samples via a de-glitched odometer
    /// accumulator, falling back to SUM(end-start) from session rows for any
    /// metric that has no samples. See `deglitch_total` for why the naive
    /// LAG/`ELSE value` accumulator over-counted: a single stale BLE frame
    /// (e.g. a steps reading of 346 wedged between 1800 and 1891 after a
    /// reconnect) injected ~1500 phantom steps.
    pub fn day_totals(&self, local_date_s: &str) -> Result<Value> {
        let c = self.conn();

        let sessions: i64 = c.query_row(
            "SELECT COUNT(*) FROM sessions WHERE local_date = ? AND ended_ts IS NOT NULL",
            params![local_date_s],
            |r| r.get(0),
        )?;

        // Pull the day's samples once, in the order the odometer advanced.
        let mut stmt = c.prepare(
            "SELECT s.steps, s.duration_s, s.distance_raw, s.calories
             FROM samples s JOIN sessions se ON se.id = s.session_id
             WHERE se.local_date = ? ORDER BY s.ts, s.id",
        )?;
        let mut steps_v: Vec<i64> = Vec::new();
        let mut dur_v: Vec<i64> = Vec::new();
        let mut dist_v: Vec<i64> = Vec::new();
        let mut cal_v: Vec<i64> = Vec::new();
        let mut rows = stmt.query(params![local_date_s])?;
        while let Some(r) = rows.next()? {
            if let Some(x) = r.get::<_, Option<i64>>(0)? {
                steps_v.push(x);
            }
            if let Some(x) = r.get::<_, Option<i64>>(1)? {
                dur_v.push(x);
            }
            if let Some(x) = r.get::<_, Option<i64>>(2)? {
                dist_v.push(x);
            }
            if let Some(x) = r.get::<_, Option<i64>>(3)? {
                cal_v.push(x);
            }
        }
        drop(rows);
        drop(stmt);

        // Per-metric (spike, reset_max). Spike = max plausible single-sample jump
        // above BOTH neighbours (a stale read that immediately reverts); the
        // result is insensitive to its exact value. reset_max = a drop to at or
        // below this counts as a genuine power-cycle reset to ~0.
        let steps = metric_total(&c, local_date_s, &steps_v, 50, 10, "steps_end", "start_steps")?;
        let duration_s =
            metric_total(&c, local_date_s, &dur_v, 600, 10, "duration_s_end", "start_duration_s")?;
        let calories = metric_total(&c, local_date_s, &cal_v, 100, 10, "calories_end", "")?;
        let distance_raw =
            metric_total(&c, local_date_s, &dist_v, 200, 10, "distance_raw_end", "")?;

        Ok(json!({
            "sessions": sessions,
            "steps": steps,
            "duration_s": duration_s,
            "calories": calories,
            "distance_raw": distance_raw,
        }))
    }


    /// Mean speed_raw across moving samples (>0) for the day.
    pub fn day_avg_speed_raw(&self, local_date_s: &str) -> Result<Option<f64>> {
        let c = self.conn();
        let row: Option<(Option<f64>, i64)> = c
            .query_row(
                "SELECT AVG(s.speed_raw) AS avg_raw, COUNT(*) AS n
                 FROM samples s JOIN sessions se ON se.id = s.session_id
                 WHERE se.local_date = ? AND s.speed_raw IS NOT NULL AND s.speed_raw > 0",
                params![local_date_s],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(avg), n)) if n > 0 => Some(avg),
            _ => None,
        })
    }

    /// Steps added per hour on the given date (24 buckets, "00".."23").
    ///
    /// Drives off the same `deglitch_walk` as `day_totals`, attributing each
    /// accepted increment to the local hour of its sample — so the bars sum to
    /// the day's step total and a stale frame can't spike a single hour (this is
    /// what made one afternoon hour show the day's max after a reconnect).
    pub fn hourly_steps(&self, local_date_s: &str) -> Result<Vec<Value>> {
        let mut hours: Vec<usize> = Vec::new();
        let mut steps: Vec<i64> = Vec::new();
        {
            let c = self.conn();
            let mut stmt = c.prepare(
                "SELECT CAST(strftime('%H', datetime(s.ts, 'unixepoch', 'localtime')) AS INTEGER) AS hour,
                        s.steps
                 FROM samples s JOIN sessions se ON se.id = s.session_id
                 WHERE se.local_date = ? AND s.steps IS NOT NULL
                 ORDER BY s.ts, s.id",
            )?;
            let mut rows = stmt.query(params![local_date_s])?;
            while let Some(r) = rows.next()? {
                let h: i64 = r.get(0)?;
                hours.push(h.clamp(0, 23) as usize);
                steps.push(r.get(1)?);
            }
        }

        let mut buckets = [0i64; 24];
        // Same (spike, reset_max) as the steps metric in day_totals so totals reconcile.
        deglitch_walk(&steps, 50, 10, |i, d| buckets[hours[i]] += d);

        Ok((0..24)
            .map(|h| json!({"hour": format!("{h:02}"), "steps": buckets[h]}))
            .collect())
    }

    /// Wipe every data table (sessions, samples, rollups, speed marks, rollup
    /// state). Used by the "reset to empty" flow after a snapshot has been saved.
    pub fn wipe_all(&self) -> Result<()> {
        let c = self.conn();
        c.execute_batch(
            "DELETE FROM samples;
             DELETE FROM sample_rollups_1m;
             DELETE FROM speed_marks;
             DELETE FROM sessions;
             DELETE FROM rollup_state;",
        )?;
        Ok(())
    }

    /// Record the speed the user has dialed on the treadmill, timestamped, so a
    /// human-known set speed can be correlated against the device's averaged
    /// `0x82` reading (the SC110 doesn't broadcast the instantaneous set speed).
    pub fn insert_speed_mark(&self, set_speed: f64, unit: &str) -> Result<i64> {
        let c = self.conn();
        c.execute(
            "INSERT INTO speed_marks(ts, set_speed, unit) VALUES (?, ?, ?)",
            params![now_ts(), set_speed, unit],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Most recent speed marks (newest first) for display + diagnostics.
    pub fn recent_speed_marks(&self, limit: i64) -> Result<Vec<Value>> {
        let c = self.conn();
        let mut stmt = c
            .prepare("SELECT ts, set_speed, unit FROM speed_marks ORDER BY ts DESC LIMIT ?")?;
        let rows = stmt
            .query_map(params![limit], |r| {
                let ts: f64 = r.get(0)?;
                Ok(json!({
                    "ts": ts,
                    "iso": iso_local(ts),
                    "set_speed": r.get::<_, f64>(1)?,
                    "unit": r.get::<_, String>(2)?,
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Diagnostic dump for a single local date: the raw sessions, every sample
    /// in accumulator order, and the per-minute rollups, plus the computed day
    /// totals and hourly buckets. Read-only support tool — lets us reconstruct
    /// exactly what the device counter reported across the day (e.g. to find a
    /// double-count after a crash/reconnect).
    pub fn diag_day(&self, local_date_s: &str) -> Result<Value> {
        let sessions: Vec<Value>;
        let samples: Vec<Value>;
        let rollups: Vec<Value>;
        {
            let c = self.conn();

            let mut sstmt = c.prepare(
                "SELECT id, started_ts, ended_ts, local_date, display_unit, start_steps,
                        start_duration_s, steps_end, duration_s_end, distance_raw_end,
                        calories_end, speed_raw_last, closed_reason
                 FROM sessions WHERE local_date = ? ORDER BY started_ts, id",
            )?;
            sessions = sstmt
                .query_map(params![local_date_s], |r| {
                    let started: f64 = r.get(1)?;
                    let ended: Option<f64> = r.get(2)?;
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "started_ts": started,
                        "started_iso": iso_local(started),
                        "ended_ts": ended,
                        "ended_iso": ended.map(iso_local),
                        "local_date": r.get::<_, String>(3)?,
                        "display_unit": r.get::<_, String>(4)?,
                        "start_steps": r.get::<_, Option<i64>>(5)?,
                        "start_duration_s": r.get::<_, Option<i64>>(6)?,
                        "steps_end": r.get::<_, Option<i64>>(7)?,
                        "duration_s_end": r.get::<_, Option<i64>>(8)?,
                        "distance_raw_end": r.get::<_, Option<i64>>(9)?,
                        "calories_end": r.get::<_, Option<i64>>(10)?,
                        "speed_raw_last": r.get::<_, Option<i64>>(11)?,
                        "closed_reason": r.get::<_, Option<String>>(12)?,
                    }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            // Every raw sample for the day, ordered exactly as day_totals walks them.
            let mut smt = c.prepare(
                "SELECT s.id, s.session_id, s.ts, s.steps, s.duration_s, s.speed_raw,
                        s.distance_raw, s.calories, s.status
                 FROM samples s JOIN sessions se ON se.id = s.session_id
                 WHERE se.local_date = ? ORDER BY s.ts, s.id",
            )?;
            samples = smt
                .query_map(params![local_date_s], |r| {
                    let ts: f64 = r.get(2)?;
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "session_id": r.get::<_, Option<i64>>(1)?,
                        "ts": ts,
                        "iso": iso_local(ts),
                        "steps": r.get::<_, Option<i64>>(3)?,
                        "duration_s": r.get::<_, Option<i64>>(4)?,
                        "speed_raw": r.get::<_, Option<i64>>(5)?,
                        "distance_raw": r.get::<_, Option<i64>>(6)?,
                        "calories": r.get::<_, Option<i64>>(7)?,
                        "status": r.get::<_, Option<i64>>(8)?,
                    }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut rmt = c.prepare(
                "SELECT r.bucket_ts, r.session_id, r.steps_delta, r.distance_raw_delta,
                        r.calories_delta, r.duration_s_delta, r.speed_raw_min,
                        r.speed_raw_avg, r.speed_raw_max, r.running_samples, r.total_samples
                 FROM sample_rollups_1m r JOIN sessions se ON se.id = r.session_id
                 WHERE se.local_date = ? ORDER BY r.bucket_ts",
            )?;
            rollups = rmt
                .query_map(params![local_date_s], |r| {
                    let bucket: i64 = r.get(0)?;
                    Ok(json!({
                        "bucket_ts": bucket,
                        "bucket_iso": iso_local(bucket as f64),
                        "session_id": r.get::<_, Option<i64>>(1)?,
                        "steps_delta": r.get::<_, i64>(2)?,
                        "distance_raw_delta": r.get::<_, i64>(3)?,
                        "calories_delta": r.get::<_, i64>(4)?,
                        "duration_s_delta": r.get::<_, i64>(5)?,
                        "speed_raw_min": r.get::<_, Option<i64>>(6)?,
                        "speed_raw_avg": r.get::<_, Option<f64>>(7)?,
                        "speed_raw_max": r.get::<_, Option<i64>>(8)?,
                        "running_samples": r.get::<_, i64>(9)?,
                        "total_samples": r.get::<_, i64>(10)?,
                    }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
        } // release the connection lock before re-locking in day_totals/hourly_steps

        let day_totals = self.day_totals(local_date_s)?;
        let hourly_steps = self.hourly_steps(local_date_s)?;

        Ok(json!({
            "date": local_date_s,
            "day_totals": day_totals,
            "hourly_steps": hourly_steps,
            "sessions": sessions,
            "samples": samples,
            "rollups": rollups,
        }))
    }

    // --- analytics timeseries -------------------------------------------

    fn bucket_expr(ts_col: &str, resolution_s: i64) -> String {
        if resolution_s >= 86400 {
            format!("CAST(strftime('%s', date({ts_col}, 'unixepoch', 'localtime')) AS INTEGER)")
        } else {
            format!("(CAST({ts_col} AS INTEGER) / {resolution_s}) * {resolution_s}")
        }
    }

    /// Bucketed timeseries for charting, merging raw samples + per-minute rollups.
    /// Ported from Python `timeseries`.
    pub fn timeseries(
        &self,
        metric: &str,
        resolution_s: i64,
        start_ts: f64,
        end_ts: f64,
    ) -> Result<Vec<Value>> {
        let raw_bucket = Self::bucket_expr("s.ts", resolution_s);
        let roll_bucket = Self::bucket_expr("r.bucket_ts", resolution_s);

        let c = self.conn();
        let raw_floor: f64 = c
            .query_row(
                "SELECT last_rolled_ts FROM rollup_state WHERE kind=?",
                params![ROLLUP_KIND],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0.0);
        let effective_start = start_ts.max(raw_floor);

        // merge helpers
        let mut merged: std::collections::BTreeMap<i64, (f64, f64)> = std::collections::BTreeMap::new();

        match metric {
            "steps" | "calories" | "distance_raw" => {
                let (col, delta_col) = match metric {
                    "steps" => ("s.steps", "r.steps_delta"),
                    "calories" => ("s.calories", "r.calories_delta"),
                    _ => ("s.distance_raw", "r.distance_raw_delta"),
                };
                let raw_sql = format!(
                    "SELECT inner_bucket AS bucket_ts, SUM(per_session_delta) AS value FROM (
                        SELECT s.session_id, {raw_bucket} AS inner_bucket,
                               (MAX({col}) - MIN({col})) AS per_session_delta
                        FROM samples s
                        WHERE s.ts >= ? AND s.ts < ? AND {col} IS NOT NULL AND s.session_id IS NOT NULL
                        GROUP BY s.session_id, inner_bucket
                    ) GROUP BY inner_bucket"
                );
                let roll_sql = format!(
                    "SELECT {roll_bucket} AS bucket_ts, SUM({delta_col}) AS value
                     FROM sample_rollups_1m r WHERE r.bucket_ts >= ? AND r.bucket_ts < ? GROUP BY bucket_ts"
                );
                Self::accumulate_sum(&c, &raw_sql, effective_start, end_ts, &mut merged)?;
                Self::accumulate_sum(&c, &roll_sql, start_ts, end_ts, &mut merged)?;
                Ok(merged
                    .into_iter()
                    .map(|(ts, (v, _))| json!({"bucket_ts": ts, "value": v}))
                    .collect())
            }
            "speed_raw" => {
                let raw_sql = format!(
                    "SELECT {raw_bucket} AS bucket_ts, SUM(speed_raw) AS sum_v, COUNT(*) AS n
                     FROM samples s WHERE s.ts >= ? AND s.ts < ? AND speed_raw IS NOT NULL AND speed_raw > 0
                     GROUP BY bucket_ts"
                );
                let roll_sql = format!(
                    "SELECT {roll_bucket} AS bucket_ts, SUM(speed_raw_avg * running_samples) AS sum_v,
                            SUM(running_samples) AS n
                     FROM sample_rollups_1m r WHERE r.bucket_ts >= ? AND r.bucket_ts < ?
                       AND speed_raw_avg IS NOT NULL GROUP BY bucket_ts"
                );
                Self::accumulate_avg(&c, &raw_sql, effective_start, end_ts, &mut merged)?;
                Self::accumulate_avg(&c, &roll_sql, start_ts, end_ts, &mut merged)?;
                Ok(merged
                    .into_iter()
                    .map(|(ts, (s, n))| {
                        json!({"bucket_ts": ts, "value": if n != 0.0 { s / n } else { 0.0 }})
                    })
                    .collect())
            }
            "duration_running_s" => {
                let raw_sql = format!(
                    "SELECT {raw_bucket} AS bucket_ts, SUM(CASE WHEN status = 3 THEN 1 ELSE 0 END) * 2.5 AS value
                     FROM samples s WHERE s.ts >= ? AND s.ts < ? GROUP BY bucket_ts"
                );
                let roll_sql = format!(
                    "SELECT {roll_bucket} AS bucket_ts, SUM(running_samples) * 2.5 AS value
                     FROM sample_rollups_1m r WHERE r.bucket_ts >= ? AND r.bucket_ts < ? GROUP BY bucket_ts"
                );
                Self::accumulate_sum(&c, &raw_sql, effective_start, end_ts, &mut merged)?;
                Self::accumulate_sum(&c, &roll_sql, start_ts, end_ts, &mut merged)?;
                Ok(merged
                    .into_iter()
                    .map(|(ts, (v, _))| json!({"bucket_ts": ts, "value": v}))
                    .collect())
            }
            other => anyhow::bail!("unknown metric: {other}"),
        }
    }

    fn accumulate_sum(
        c: &Connection,
        sql: &str,
        a: f64,
        b: f64,
        merged: &mut std::collections::BTreeMap<i64, (f64, f64)>,
    ) -> Result<()> {
        let mut stmt = c.prepare(sql)?;
        let rows = stmt.query_map(params![a, b], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<f64>>(1)?.unwrap_or(0.0)))
        })?;
        for row in rows {
            let (ts, v) = row?;
            merged.entry(ts).or_insert((0.0, 0.0)).0 += v;
        }
        Ok(())
    }

    fn accumulate_avg(
        c: &Connection,
        sql: &str,
        a: f64,
        b: f64,
        merged: &mut std::collections::BTreeMap<i64, (f64, f64)>,
    ) -> Result<()> {
        let mut stmt = c.prepare(sql)?;
        let rows = stmt.query_map(params![a, b], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            ))
        })?;
        for row in rows {
            let (ts, s, n) = row?;
            let e = merged.entry(ts).or_insert((0.0, 0.0));
            e.0 += s;
            e.1 += n;
        }
        Ok(())
    }

    // --- rollups / retention --------------------------------------------

    pub fn rollup_status(&self) -> Result<Value> {
        let c = self.conn();
        let state: Option<(f64, Option<f64>)> = c
            .query_row(
                "SELECT last_rolled_ts, last_run_ts FROM rollup_state WHERE kind=?",
                params![ROLLUP_KIND],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (last_rolled, last_run) = state.unwrap_or((0.0, None));
        let raw_count: i64 = c.query_row("SELECT COUNT(*) FROM samples", [], |r| r.get(0))?;
        let rollup_count: i64 =
            c.query_row("SELECT COUNT(*) FROM sample_rollups_1m", [], |r| r.get(0))?;
        let oldest_raw: Option<f64> =
            c.query_row("SELECT MIN(ts) FROM samples", [], |r| r.get(0))?;
        Ok(json!({
            "last_rolled_ts": last_rolled,
            "last_run_ts": last_run,
            "raw_samples": raw_count,
            "rollup_buckets": rollup_count,
            "oldest_raw_ts": oldest_raw,
        }))
    }

    /// Aggregate unprocessed raw samples into per-minute buckets. Idempotent via
    /// rollup_state.last_rolled_ts. Returns buckets_written.
    pub fn rollup_samples(&self) -> Result<Value> {
        let now = now_ts();
        let cutoff = now - ROLLUP_RESOLUTION_S as f64;
        let mut c = self.conn();
        let last_rolled: f64 = c
            .query_row(
                "SELECT last_rolled_ts FROM rollup_state WHERE kind=?",
                params![ROLLUP_KIND],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0.0);

        if cutoff <= last_rolled {
            c.execute(
                "INSERT INTO rollup_state(kind, last_rolled_ts, last_run_ts) VALUES (?, ?, ?)
                 ON CONFLICT(kind) DO UPDATE SET last_run_ts=excluded.last_run_ts",
                params![ROLLUP_KIND, last_rolled, now],
            )?;
            return Ok(json!({"buckets_written": 0, "last_rolled_ts": last_rolled, "cutoff_ts": cutoff}));
        }

        let tx = c.transaction()?;
        let mut written = 0i64;
        let mut max_bucket_ts = last_rolled;
        {
            let res_s = ROLLUP_RESOLUTION_S;

            // De-glitched per-(bucket,session) metric deltas. Read with a small
            // lookback so the walk has prior-value context across the window edge.
            let lookback_start = (last_rolled - ROLLUP_DEGLITCH_LOOKBACK_S).max(0.0);
            let mut steps_s: Vec<(i64, i64, i64)> = Vec::new();
            let mut dist_s: Vec<(i64, i64, i64)> = Vec::new();
            let mut cal_s: Vec<(i64, i64, i64)> = Vec::new();
            let mut dur_s: Vec<(i64, i64, i64)> = Vec::new();
            {
                let mut q = tx.prepare(
                    "SELECT CAST(s.ts AS INTEGER), s.session_id, s.steps, s.distance_raw,
                            s.calories, s.duration_s
                     FROM samples s WHERE s.ts > ? AND s.ts < ? AND s.session_id IS NOT NULL
                     ORDER BY s.ts, s.id",
                )?;
                let mut rows = q.query(params![lookback_start, cutoff])?;
                while let Some(r) = rows.next()? {
                    let ts: i64 = r.get(0)?;
                    let sess: i64 = r.get(1)?;
                    if let Some(v) = r.get::<_, Option<i64>>(2)? {
                        steps_s.push((ts, sess, v));
                    }
                    if let Some(v) = r.get::<_, Option<i64>>(3)? {
                        dist_s.push((ts, sess, v));
                    }
                    if let Some(v) = r.get::<_, Option<i64>>(4)? {
                        cal_s.push((ts, sess, v));
                    }
                    if let Some(v) = r.get::<_, Option<i64>>(5)? {
                        dur_s.push((ts, sess, v));
                    }
                }
            }
            let steps_d = deglitch_bucketed(&steps_s, res_s, 50, 10);
            let dist_d = deglitch_bucketed(&dist_s, res_s, 200, 10);
            let cal_d = deglitch_bucketed(&cal_s, res_s, 100, 10);
            let dur_d = deglitch_bucketed(&dur_s, res_s, 600, 10);

            // Stateless speed/running/total aggregates per (bucket,session) over
            // the strict (last_rolled, cutoff) window — the authoritative bucket set.
            let agg_sql = format!(
                "SELECT (CAST(s.ts AS INTEGER) / {res_s}) * {res_s} AS bucket_ts, s.session_id,
                        MIN(CASE WHEN s.speed_raw > 0 THEN s.speed_raw END) AS speed_raw_min,
                        AVG(CASE WHEN s.speed_raw > 0 THEN s.speed_raw END) AS speed_raw_avg,
                        MAX(CASE WHEN s.speed_raw > 0 THEN s.speed_raw END) AS speed_raw_max,
                        SUM(CASE WHEN s.status = 3 THEN 1 ELSE 0 END) AS running_samples,
                        COUNT(*) AS total_samples
                 FROM samples s WHERE s.ts > ? AND s.ts < ? AND s.session_id IS NOT NULL
                 GROUP BY bucket_ts, s.session_id"
            );
            let mut agg = tx.prepare(&agg_sql)?;
            let groups: Vec<RollupRow> = agg
                .query_map(params![last_rolled, cutoff], |r| {
                    let bucket_ts: i64 = r.get(0)?;
                    let session_id: Option<i64> = r.get(1)?;
                    let key = (bucket_ts, session_id.unwrap_or(0));
                    Ok(RollupRow {
                        bucket_ts,
                        session_id,
                        steps_delta: *steps_d.get(&key).unwrap_or(&0),
                        distance_raw_delta: *dist_d.get(&key).unwrap_or(&0),
                        calories_delta: *cal_d.get(&key).unwrap_or(&0),
                        duration_s_delta: *dur_d.get(&key).unwrap_or(&0),
                        speed_raw_min: r.get(2)?,
                        speed_raw_avg: r.get(3)?,
                        speed_raw_max: r.get(4)?,
                        running_samples: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                        total_samples: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(agg);

            for r in &groups {
                tx.execute(
                    "INSERT INTO sample_rollups_1m(bucket_ts, session_id, steps_delta, distance_raw_delta,
                        calories_delta, duration_s_delta, speed_raw_min, speed_raw_avg, speed_raw_max,
                        running_samples, total_samples)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(bucket_ts, session_id) DO UPDATE SET
                        steps_delta=excluded.steps_delta, distance_raw_delta=excluded.distance_raw_delta,
                        calories_delta=excluded.calories_delta, duration_s_delta=excluded.duration_s_delta,
                        speed_raw_min=excluded.speed_raw_min, speed_raw_avg=excluded.speed_raw_avg,
                        speed_raw_max=excluded.speed_raw_max, running_samples=excluded.running_samples,
                        total_samples=excluded.total_samples",
                    params![
                        r.bucket_ts, r.session_id, r.steps_delta, r.distance_raw_delta,
                        r.calories_delta, r.duration_s_delta, r.speed_raw_min, r.speed_raw_avg,
                        r.speed_raw_max, r.running_samples, r.total_samples
                    ],
                )?;
                written += 1;
                max_bucket_ts = max_bucket_ts.max((r.bucket_ts + res_s) as f64);
            }
        }
        let new_mark = max_bucket_ts.max(last_rolled);
        tx.execute(
            "INSERT INTO rollup_state(kind, last_rolled_ts, last_run_ts) VALUES (?, ?, ?)
             ON CONFLICT(kind) DO UPDATE SET last_rolled_ts=excluded.last_rolled_ts, last_run_ts=excluded.last_run_ts",
            params![ROLLUP_KIND, new_mark, now],
        )?;
        tx.commit()?;
        Ok(json!({"buckets_written": written, "last_rolled_ts": new_mark, "cutoff_ts": cutoff}))
    }

    /// Wipe all rollups and recompute them from the raw samples still on disk,
    /// using the de-glitched aggregation. Repairs buckets that the old
    /// MAX-MIN writer inflated from stale frames. Buckets older than the raw
    /// retention window are gone but were already final, so nothing is lost.
    pub fn rebuild_rollups(&self) -> Result<Value> {
        {
            let c = self.conn();
            c.execute("DELETE FROM sample_rollups_1m", [])?;
            c.execute(
                "INSERT INTO rollup_state(kind, last_rolled_ts, last_run_ts) VALUES (?, 0, NULL)
                 ON CONFLICT(kind) DO UPDATE SET last_rolled_ts = 0",
                params![ROLLUP_KIND],
            )?;
        }
        self.rollup_samples()
    }

    pub fn prune_raw_samples(&self, retention_s: f64) -> Result<usize> {
        let now = now_ts();
        let cutoff = now - retention_s;
        let c = self.conn();
        let last_rolled: f64 = c
            .query_row(
                "SELECT last_rolled_ts FROM rollup_state WHERE kind=?",
                params![ROLLUP_KIND],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0.0);
        let effective_cutoff = cutoff.min(last_rolled);
        if effective_cutoff <= 0.0 {
            return Ok(0);
        }
        let n = c.execute("DELETE FROM samples WHERE ts < ?", params![effective_cutoff])?;
        Ok(n)
    }

    // --- export / import -------------------------------------------------

    pub fn export_all(&self) -> Result<Value> {
        let c = self.conn();
        let sessions = rows_as_json(&c, "SELECT * FROM sessions ORDER BY id")?;
        let samples = rows_as_json(&c, "SELECT * FROM samples ORDER BY id")?;
        let rollups = rows_as_json(
            &c,
            "SELECT * FROM sample_rollups_1m ORDER BY bucket_ts, session_id",
        )?;
        Ok(json!({
            "format": "lifespan-sc110-dump",
            "version": 2,
            "exported_at": now_ts(),
            "sessions": sessions,
            "samples": samples,
            "rollups_1m": rollups,
        }))
    }

    /// Load a previous export back in. mode="merge" skips sessions whose
    /// started_ts already exists (idempotent re-import); mode="replace" wipes
    /// first. Ported from Python `import_dump`.
    pub fn import_dump(&self, dump: &Value, mode: &str) -> Result<Value> {
        if dump.get("format").and_then(|v| v.as_str()) != Some("lifespan-sc110-dump") {
            anyhow::bail!("not a lifespan-sc110 dump");
        }
        match dump.get("version").and_then(|v| v.as_i64()) {
            Some(1) | Some(2) => {}
            other => anyhow::bail!("unsupported dump version: {other:?}"),
        }
        if mode != "merge" && mode != "replace" {
            anyhow::bail!("mode must be 'merge' or 'replace', got {mode}");
        }
        let empty: Vec<Value> = Vec::new();
        let sessions = dump.get("sessions").and_then(|v| v.as_array()).unwrap_or(&empty);
        let samples = dump.get("samples").and_then(|v| v.as_array()).unwrap_or(&empty);
        let rollups = dump.get("rollups_1m").and_then(|v| v.as_array()).unwrap_or(&empty);

        let mut counts = serde_json::Map::new();
        for k in ["sessions", "samples", "rollups", "skipped_sessions", "skipped_samples", "skipped_rollups"] {
            counts.insert(k.into(), json!(0));
        }
        fn bump(m: &mut serde_json::Map<String, Value>, k: &str) {
            let n = m.get(k).and_then(|v| v.as_i64()).unwrap_or(0) + 1;
            m.insert(k.into(), json!(n));
        }

        let f64_of = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_f64());
        let i64_of = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_i64());
        let str_of = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());

        let mut c = self.conn();
        let tx = c.transaction()?;
        if mode == "replace" {
            tx.execute("DELETE FROM sample_rollups_1m", [])?;
            tx.execute("DELETE FROM samples", [])?;
            tx.execute("DELETE FROM sessions", [])?;
            tx.execute("DELETE FROM rollup_state", [])?;
        }

        let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for s in sessions {
            let started_ts = match f64_of(s, "started_ts") {
                Some(t) => t,
                None => continue,
            };
            let old_id = i64_of(s, "id");
            if mode == "merge" {
                let existing: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM sessions WHERE started_ts = ?",
                        params![started_ts],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(eid) = existing {
                    if let Some(oid) = old_id {
                        id_map.insert(oid, eid);
                    }
                    bump(&mut counts, "skipped_sessions");
                    continue;
                }
            }
            tx.execute(
                "INSERT INTO sessions(started_ts, ended_ts, local_date, display_unit, start_steps,
                    start_duration_s, steps_end, duration_s_end, distance_raw_end, calories_end,
                    speed_raw_last, closed_reason) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    started_ts,
                    f64_of(s, "ended_ts"),
                    str_of(s, "local_date").unwrap_or_else(|| local_date(started_ts)),
                    str_of(s, "display_unit").unwrap_or_else(|| "km/h".into()),
                    i64_of(s, "start_steps"),
                    i64_of(s, "start_duration_s"),
                    i64_of(s, "steps_end"),
                    i64_of(s, "duration_s_end"),
                    i64_of(s, "distance_raw_end"),
                    i64_of(s, "calories_end"),
                    i64_of(s, "speed_raw_last"),
                    str_of(s, "closed_reason"),
                ],
            )?;
            if let Some(oid) = old_id {
                id_map.insert(oid, tx.last_insert_rowid());
            }
            bump(&mut counts, "sessions");
        }

        for sm in samples {
            let ts = match f64_of(sm, "ts") {
                Some(t) => t,
                None => continue,
            };
            let new_sid = i64_of(sm, "session_id").and_then(|o| id_map.get(&o).copied());
            if mode == "merge" {
                let dup: Option<i64> = tx
                    .query_row(
                        "SELECT 1 FROM samples WHERE ts = ? AND (session_id IS ? OR session_id = ?) LIMIT 1",
                        params![ts, new_sid, new_sid],
                        |r| r.get(0),
                    )
                    .optional()?;
                if dup.is_some() {
                    bump(&mut counts, "skipped_samples");
                    continue;
                }
            }
            tx.execute(
                "INSERT INTO samples(session_id, ts, steps, duration_s, speed_raw, distance_raw, calories, status)
                 VALUES (?,?,?,?,?,?,?,?)",
                params![
                    new_sid, ts, i64_of(sm, "steps"), i64_of(sm, "duration_s"),
                    i64_of(sm, "speed_raw"), i64_of(sm, "distance_raw"),
                    i64_of(sm, "calories"), i64_of(sm, "status")
                ],
            )?;
            bump(&mut counts, "samples");
        }

        for rr in rollups {
            let bucket_ts = match i64_of(rr, "bucket_ts") {
                Some(t) => t,
                None => continue,
            };
            let new_sid = i64_of(rr, "session_id").and_then(|o| id_map.get(&o).copied());
            if mode == "merge" {
                let dup: Option<i64> = tx
                    .query_row(
                        "SELECT 1 FROM sample_rollups_1m WHERE bucket_ts=? AND (session_id IS ? OR session_id = ?) LIMIT 1",
                        params![bucket_ts, new_sid, new_sid],
                        |r| r.get(0),
                    )
                    .optional()?;
                if dup.is_some() {
                    bump(&mut counts, "skipped_rollups");
                    continue;
                }
            }
            tx.execute(
                "INSERT INTO sample_rollups_1m(bucket_ts, session_id, steps_delta, distance_raw_delta,
                    calories_delta, duration_s_delta, speed_raw_min, speed_raw_avg, speed_raw_max,
                    running_samples, total_samples) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    bucket_ts, new_sid,
                    i64_of(rr, "steps_delta").unwrap_or(0),
                    i64_of(rr, "distance_raw_delta").unwrap_or(0),
                    i64_of(rr, "calories_delta").unwrap_or(0),
                    i64_of(rr, "duration_s_delta").unwrap_or(0),
                    i64_of(rr, "speed_raw_min"),
                    f64_of(rr, "speed_raw_avg"),
                    i64_of(rr, "speed_raw_max"),
                    i64_of(rr, "running_samples").unwrap_or(0),
                    i64_of(rr, "total_samples").unwrap_or(0),
                ],
            )?;
            bump(&mut counts, "rollups");
        }

        tx.commit()?;
        Ok(Value::Object(counts))
    }
}

struct RollupRow {
    bucket_ts: i64,
    session_id: Option<i64>,
    steps_delta: i64,
    distance_raw_delta: i64,
    calories_delta: i64,
    duration_s_delta: i64,
    speed_raw_min: Option<i64>,
    speed_raw_avg: Option<f64>,
    speed_raw_max: Option<i64>,
    running_samples: i64,
    total_samples: i64,
}

fn row_to_session(r: &rusqlite::Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: r.get("id")?,
        started_ts: r.get("started_ts")?,
        ended_ts: r.get("ended_ts")?,
        local_date: r.get("local_date")?,
        display_unit: r.get("display_unit")?,
        start_steps: r.get("start_steps")?,
        steps_end: r.get("steps_end")?,
        duration_s_end: r.get("duration_s_end")?,
        distance_raw_end: r.get("distance_raw_end")?,
        calories_end: r.get("calories_end")?,
        speed_raw_last: r.get("speed_raw_last")?,
    })
}

/// Generic "dump a SELECT to a JSON array of objects" using column names.
fn rows_as_json(c: &Connection, sql: &str) -> Result<Vec<Value>> {
    let mut stmt = c.prepare(sql)?;
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut out = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let mut obj = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            let v = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(x) => json!(x),
                rusqlite::types::ValueRef::Real(x) => json!(x),
                rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                rusqlite::types::ValueRef::Blob(b) => json!(b),
            };
            obj.insert(name.clone(), v);
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Db {
        Db::open(":memory:").unwrap()
    }

    #[test]
    fn day_totals_accumulates_across_counter_reset() {
        let db = mem();
        let today = local_date(now_ts());
        let sid = db.open_session(now_ts(), "km/h", Some(0), Some(0)).unwrap();
        let base = now_ts();
        // Walk 1: steps climb 0->10, then a reset (new walk) 0->5 => total 15.
        for (i, steps) in [0u32, 4, 10, 0, 3, 5].iter().enumerate() {
            db.insert_sample(Some(sid), base + i as f64, Some(*steps), Some(0), Some(60), Some(0), Some(0), Some(3)).unwrap();
        }
        let totals = db.day_totals(&today).unwrap();
        assert_eq!(totals["steps"].as_i64().unwrap(), 15, "monotonic accumulator should sum positive deltas + reset value");
    }

    #[test]
    fn deglitch_handles_spikes_resets_and_dips() {
        // Real-world shape from a crash/reconnect: a stale low frame (346) wedged
        // between 1800 and 1891. The old accumulator added 346 + (1891-346)=~1500
        // phantom steps; the de-glitcher drops the spike and keeps the real climb.
        assert_eq!(deglitch_total(&[1797, 1800, 346, 1891, 1896, 1901], 50, 10), 1901);
        // Genuine power-cycle reset to ~0 then a fresh climb: 0..10 then 0..5 = 15.
        assert_eq!(deglitch_total(&[0, 4, 10, 0, 3, 5], 50, 10), 15);
        // A one-off dip that reverts (not a reset) is dropped, never re-added.
        assert_eq!(deglitch_total(&[100, 103, 40, 106, 109], 50, 10), 109);
        // Real steps walked across a reconnect gap (no glitch) are kept.
        assert_eq!(deglitch_total(&[10, 20, 55, 60], 50, 10), 60);
        // Counter reset caught after it already climbed past reset_max (488 -> 42):
        // the drop is >half, so it's a reset and the post-reset climb (42->45) counts.
        assert_eq!(deglitch_total(&[400, 402, 42, 44, 45], 50, 10), 400 + 2 + 3);
    }

    #[test]
    fn day_totals_ignores_stale_reconnect_frame() {
        let db = mem();
        let today = local_date(now_ts());
        let sid = db.open_session(now_ts(), "km/h", Some(0), Some(0)).unwrap();
        let base = now_ts();
        // 1800 -> stale 346 -> 1891 -> 1901: only +101 of real climb after 1800.
        for (i, steps) in [1797u32, 1800, 346, 1891, 1896, 1901].iter().enumerate() {
            db.insert_sample(Some(sid), base + i as f64, Some(*steps), Some(0), Some(60), Some(0), Some(0), Some(3)).unwrap();
        }
        let totals = db.day_totals(&today).unwrap();
        assert_eq!(
            totals["steps"].as_i64().unwrap(),
            1901,
            "a stale reconnect frame must not inject phantom steps"
        );
    }

    #[test]
    fn hourly_steps_reconcile_with_day_total() {
        let db = mem();
        let today = local_date(now_ts());
        let sid = db.open_session(now_ts(), "km/h", Some(0), Some(0)).unwrap();
        let base = now_ts();
        for (i, steps) in [1797u32, 1800, 346, 1891, 1896, 1901].iter().enumerate() {
            db.insert_sample(Some(sid), base + i as f64, Some(*steps), Some(0), Some(60), Some(0), Some(0), Some(3)).unwrap();
        }
        let day = db.day_totals(&today).unwrap()["steps"].as_i64().unwrap();
        let sum: i64 = db
            .hourly_steps(&today)
            .unwrap()
            .iter()
            .map(|h| h["steps"].as_i64().unwrap())
            .sum();
        assert_eq!(day, 1901);
        assert_eq!(sum, day, "hourly buckets must sum to the de-glitched day total");
    }

    #[test]
    fn rollup_deglitches_stale_frame() {
        let db = mem();
        let sid = db.open_session(now_ts() - 600.0, "km/h", Some(0), Some(0)).unwrap();
        // Align to a minute boundary ~10 min ago so all samples share one bucket
        // and fall before the rollup cutoff (now - 60s).
        let base = (((now_ts() as i64 - 600) / 60) * 60) as f64 + 1.0;
        for (i, steps) in [1797u32, 1800, 346, 1891, 1896, 1901].iter().enumerate() {
            db.insert_sample(Some(sid), base + i as f64, Some(*steps), Some(0), Some(60), Some(0), Some(0), Some(3)).unwrap();
        }
        db.rollup_samples().unwrap();
        let delta: i64 = db
            .conn()
            .query_row(
                "SELECT COALESCE(SUM(steps_delta), 0) FROM sample_rollups_1m WHERE session_id = ?",
                params![sid],
                |r| r.get(0),
            )
            .unwrap();
        // Old MAX-MIN gave 1901-346 = 1555; de-glitched increments = 3+91+5+5.
        assert_eq!(delta, 104, "rollup writer must drop the stale 346 frame");
    }

    #[test]
    fn export_import_round_trips() {
        let db = mem();
        let sid = db.open_session(1000.0, "km/h", Some(0), Some(0)).unwrap();
        db.insert_sample(Some(sid), 1001.0, Some(5), Some(10), Some(60), Some(2), Some(1), Some(3)).unwrap();
        db.close_session(sid, 1002.0, Some(5), Some(10), Some(2), Some(1), Some(60), "stopped").unwrap();
        let dump = db.export_all().unwrap();

        let db2 = mem();
        let res = db2.import_dump(&dump, "merge").unwrap();
        assert_eq!(res["sessions"].as_i64().unwrap(), 1);
        assert_eq!(res["samples"].as_i64().unwrap(), 1);
        // Re-importing the same dump is idempotent (skips duplicates).
        let res2 = db2.import_dump(&dump, "merge").unwrap();
        assert_eq!(res2["skipped_sessions"].as_i64().unwrap(), 1);
        assert_eq!(db2.list_sessions(10).unwrap().len(), 1);
    }

    #[test]
    fn rejects_foreign_dump() {
        let db = mem();
        assert!(db.import_dump(&serde_json::json!({"format": "nope"}), "merge").is_err());
    }
}
