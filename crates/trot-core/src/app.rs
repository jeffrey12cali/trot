//! Shared application state: DB handle, the live broadcast hub, and the
//! worker's current view (connected / device / active session / last telemetry).
//! Mirrors the responsibilities split across backend/app.py + worker.py + hub.py.

use crate::db::{now_ts, Db};
use crate::protocol::{speed_kmh, speed_mph, Telemetry};
use chrono::{Local, TimeZone};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, Notify};

pub struct AppState {
    pub db: Arc<Db>,
    pub hub: broadcast::Sender<Value>,
    /// Displayed speed/distance unit ("km/h" or "mph"). Runtime-settable from the
    /// setup wizard / settings, so it's behind a lock.
    display_unit: Mutex<String>,
    /// Random per-launch token required on state-changing /api calls. The daemon
    /// publishes it (with the port) in the handshake file so the CLI / Nowhere can
    /// send it; it stops other local processes and cross-site requests from driving
    /// the loopback API. Enforced by the request `guard` in `api.rs`.
    pub token: String,
    pub device_id: Mutex<Option<String>>,
    pub connected: AtomicBool,
    pub active_session_id: Mutex<Option<i64>>,
    pub last_state: Mutex<Option<Telemetry>>,
    /// Ring buffer of the most recent (ts, opcode, raw response bytes) for
    /// protocol diagnostics — lets us verify each opcode's response is decoded
    /// from the right frame. Capped; dumped via /api/diag.
    pub frames: Mutex<std::collections::VecDeque<(f64, u8, Vec<u8>)>>,
    /// Set when device_id changes or shutdown is requested, to wake the worker.
    pub wake: Notify,
    pub stop: AtomicBool,
    /// Manually paused ("Disconnect" in the UI): the worker drops the BLE link
    /// and stays paired but idle — no reconnect — until resumed. Unlike `stop`
    /// this is not terminal: the engine keeps running so cloud sync still works
    /// while the treadmill is disconnected.
    pub paused: AtomicBool,
    /// Set after too many consecutive failed connect attempts (treadmill off, out
    /// of range, or already linked to another device). The worker stops retrying
    /// and waits for a manual reconnect instead of scanning forever.
    pub connect_failed: AtomicBool,
    /// Fired once by the BLE worker after its loop has exited — i.e. after the
    /// peripheral has been cleanly disconnected. `shutdown()` awaits this so the
    /// process doesn't die (taking the OS BLE handle with it) before the SC110
    /// gets a real GATT disconnect.
    pub ble_done: Notify,
    /// Memoised `today_payload()` as `(computed_at, local_date, payload)`.
    ///
    /// Recomputing it means re-reading and de-glitch-walking EVERY raw sample of
    /// the current day (see `Db::day_totals`), which is O(samples-so-far) — ~75 ms
    /// at 10k samples, ~410 ms at 50k. The BLE worker broadcasts on every poll
    /// (~10–15 Hz), so recomputing per poll made the engine DB-bound within ~10
    /// minutes of walking and, because it all runs under the single DB mutex,
    /// stalled every API read behind it. We cache for `TODAY_CACHE_TTL` and
    /// invalidate explicitly on session start/end, so live numbers stay correct
    /// to within one tick without the quadratic re-walk.
    today_cache: Mutex<Option<(f64, String, Value)>>,
}

/// Max raw frames retained for diagnostics (~a few minutes at 20 Hz).
const FRAME_RING_CAP: usize = 1200;

/// How long a computed `today_payload()` stays fresh. One second keeps the live
/// step counter visually real-time (the UI ticks at ~1 Hz anyway) while cutting
/// the aggregation from ~15×/s to ~1×/s.
const TODAY_CACHE_TTL: f64 = 1.0;

impl AppState {
    pub fn new(
        db: Arc<Db>,
        display_unit: String,
        device_id: Option<String>,
        token: String,
    ) -> Arc<Self> {
        let (hub, _rx) = broadcast::channel(256);
        Arc::new(AppState {
            db,
            hub,
            display_unit: Mutex::new(display_unit),
            token,
            device_id: Mutex::new(device_id),
            connected: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            connect_failed: AtomicBool::new(false),
            active_session_id: Mutex::new(None),
            last_state: Mutex::new(None),
            frames: Mutex::new(std::collections::VecDeque::with_capacity(FRAME_RING_CAP)),
            today_cache: Mutex::new(None),
            wake: Notify::new(),
            stop: AtomicBool::new(false),
            ble_done: Notify::new(),
        })
    }

    /// Signal the BLE worker to stop and wait until it has cleanly disconnected
    /// the treadmill (bounded by `timeout`, so a wedged link can't hang forever).
    /// Call this before tearing the process down — a bare kill leaves the SC110
    /// holding the link until it's power-cycled, because the OS closes the socket
    /// without a GATT disconnect.
    pub async fn shutdown(&self, timeout: std::time::Duration) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake.notify_waiters();
        // The worker calls `ble_done.notify_one()` once its loop has exited.
        // notify_one stores a permit if we aren't awaiting yet, so this can't
        // miss the signal even if the worker exits first.
        let _ = tokio::time::timeout(timeout, self.ble_done.notified()).await;
    }

    /// Current displayed unit ("km/h" or "mph").
    pub fn display_unit(&self) -> String {
        self.display_unit.lock().unwrap().clone()
    }

    /// Change the displayed unit at runtime (also persist via config elsewhere).
    pub fn set_display_unit(&self, unit: &str) {
        *self.display_unit.lock().unwrap() = unit.to_string();
    }

    /// Record a raw protocol frame for diagnostics (drops the oldest when full).
    pub fn record_frame(&self, opcode: u8, frame: &[u8]) {
        let mut buf = self.frames.lock().unwrap();
        if buf.len() >= FRAME_RING_CAP {
            buf.pop_front();
        }
        buf.push_back((now_ts(), opcode, frame.to_vec()));
    }

    /// Recent raw frames as JSON `{ts, iso, opcode, hex}`, oldest first.
    pub fn frames_snapshot(&self) -> Vec<Value> {
        let buf = self.frames.lock().unwrap();
        buf.iter()
            .map(|(ts, opcode, bytes)| {
                let hex = bytes
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                json!({
                    "ts": ts,
                    "iso": Local
                        .timestamp_opt(*ts as i64, 0)
                        .single()
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default(),
                    "opcode": format!("0x{opcode:02x}"),
                    "hex": hex,
                })
            })
            .collect()
    }

    pub fn broadcast(&self, msg: Value) {
        // Err only means no subscribers; that's fine.
        let _ = self.hub.send(msg);
    }

    pub fn device_id(&self) -> Option<String> {
        self.device_id.lock().unwrap().clone()
    }

    pub fn set_device_id(&self, id: Option<String>) {
        *self.device_id.lock().unwrap() = id;
        // A (re)paired or switched device should connect, even if we were paused
        // by a manual disconnect or had given up.
        self.paused.store(false, Ordering::Relaxed);
        self.connect_failed.store(false, Ordering::Relaxed);
        self.wake.notify_waiters();
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn is_connect_failed(&self) -> bool {
        self.connect_failed.load(Ordering::Relaxed)
    }

    /// Manually disconnect (pause) the BLE worker without stopping the engine:
    /// it drops the link, stays paired, and idles until resumed. Pass `false` to
    /// resume (reconnect) — which also clears a prior give-up. Sync and history
    /// keep running throughout.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
        if !paused {
            self.connect_failed.store(false, Ordering::Relaxed);
        }
        self.wake.notify_waiters();
    }

    /// Record whether the worker has given up auto-connecting.
    pub fn set_connect_failed(&self, failed: bool) {
        self.connect_failed.store(failed, Ordering::Relaxed);
    }

    pub fn active_session(&self) -> Option<i64> {
        *self.active_session_id.lock().unwrap()
    }

    fn today_str() -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }

    fn avg_speed_payload(&self, local_date: &str) -> Value {
        match self.db.day_avg_speed_raw(local_date).ok().flatten() {
            None => json!({"avg_speed_raw": null, "avg_speed_kmh": null, "avg_speed_mph": null}),
            Some(avg) => {
                let avg_int = avg.round() as u32;
                let unit = self.display_unit();
                json!({
                    "avg_speed_raw": avg,
                    "avg_speed_kmh": speed_kmh(avg_int, &unit),
                    "avg_speed_mph": speed_mph(avg_int, &unit),
                })
            }
        }
    }

    /// `today` object combining day_totals + total_steps_live + avg speed.
    ///
    /// Served from a ~1 s cache (see `today_cache`): the underlying aggregation
    /// walks every raw sample of the day, and the BLE worker asks for this on
    /// every poll. Cheap correctness guards: the cache is keyed by local date (so
    /// it can't serve yesterday's totals past midnight) and is dropped outright on
    /// session start/end via `invalidate_today`.
    pub fn today_payload(&self) -> Value {
        let today = Self::today_str();
        let now = now_ts();
        if let Some((at, ref date, ref payload)) = *self.today_cache.lock().unwrap() {
            if date == &today && now - at < TODAY_CACHE_TTL {
                return payload.clone();
            }
        }
        let payload = self.compute_today_payload(&today);
        *self.today_cache.lock().unwrap() = Some((now, today, payload.clone()));
        payload
    }

    /// Drop the memoised `today` payload so the next read recomputes. Called when
    /// something happens that must be reflected immediately rather than within the
    /// cache TTL (a session opening or closing, or the data being wiped/restored).
    pub fn invalidate_today(&self) {
        *self.today_cache.lock().unwrap() = None;
    }

    /// The uncached aggregation. Only `today_payload` should call this.
    fn compute_today_payload(&self, today: &str) -> Value {
        let mut day = self.db.day_totals(today).unwrap_or_else(|_| json!({}));
        if let Value::Object(ref mut m) = day {
            let steps = m.get("steps").cloned().unwrap_or(json!(0));
            m.insert("total_steps_live".into(), steps);
            if let Value::Object(avg) = self.avg_speed_payload(today) {
                for (k, v) in avg {
                    m.insert(k, v);
                }
            }
        }
        day
    }

    /// Full snapshot sent on WS connect and exposed at /api/state.
    pub fn snapshot(&self) -> Value {
        json!({
            "connected": self.is_connected(),
            "paused": self.is_paused(),
            "connect_failed": self.is_connect_failed(),
            "display_unit": self.display_unit(),
            "device_id": self.device_id(),
            "state": self.last_state.lock().unwrap().clone(),
            "active_session_id": self.active_session(),
            "today": self.today_payload(),
        })
    }

    /// /api/today response shape.
    pub fn today_response(&self) -> Value {
        let today = Self::today_str();
        let hourly = self.db.hourly_steps(&today).unwrap_or_default();
        json!({
            "date": today,
            "totals": self.today_payload(),
            "hourly": hourly,
            "display_unit": self.display_unit(),
            "device_id": self.device_id(),
            "active_session_id": self.active_session(),
            "connected": self.is_connected(),
            "paused": self.is_paused(),
            "connect_failed": self.is_connect_failed(),
        })
    }
}

/// Convert a Telemetry to the `state` object the UI expects (raw + derived).
pub fn state_dict(t: &Telemetry) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

pub fn unix_now() -> f64 {
    now_ts()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> Arc<AppState> {
        let db = Arc::new(Db::open(":memory:").unwrap());
        AppState::new(db, "km/h".into(), None, "tok".into())
    }

    /// The cache must not hide new walking beyond its TTL, and must be dropped
    /// immediately when a session boundary invalidates it.
    #[test]
    fn today_cache_serves_then_invalidates() {
        let s = state();
        let today = AppState::today_str();
        let sid = s
            .db
            .open_session(now_ts(), "km/h", Some(0), Some(0), None)
            .unwrap();
        s.db.insert_sample(Some(sid), now_ts(), Some(10), Some(1), Some(60), Some(0), Some(0), Some(3))
            .unwrap();

        let first = s.today_payload();
        assert_eq!(first["steps"].as_i64().unwrap(), 10);

        // More steps land, but within the TTL the cached value is still served.
        s.db.insert_sample(Some(sid), now_ts(), Some(40), Some(2), Some(60), Some(0), Some(0), Some(3))
            .unwrap();
        assert_eq!(
            s.today_payload()["steps"].as_i64().unwrap(),
            10,
            "within TTL the memoised payload is reused"
        );

        // An explicit invalidation (what session start/end does) recomputes now.
        s.invalidate_today();
        assert_eq!(
            s.today_payload()["steps"].as_i64().unwrap(),
            40,
            "invalidate_today must force a recompute"
        );

        // Cache is keyed by local date, so a stale entry from another day is
        // never served for today.
        *s.today_cache.lock().unwrap() =
            Some((now_ts(), "1999-12-31".to_string(), json!({"steps": 999})));
        assert_eq!(
            s.today_payload()["steps"].as_i64().unwrap(),
            40,
            "a cache entry for a different local date must be ignored"
        );
        assert_eq!(today, AppState::today_str());
    }
}
