//! BLE engine: owns the connection to the one paired treadmill, picks the
//! driver for it from the registry, and does everything a driver must not —
//! scanning, connect/reconnect with backoff, give-up-after-N-failures,
//! cancellation (pause / device switch / shutdown), session detection,
//! throttled persistence, and the WebSocket broadcast.
//!
//! Drivers (see `drivers/`) only translate a device's Bluetooth traffic into
//! neutral `Sample`s; the conversion to the presentation `Telemetry` happens
//! once, here, at `Telemetry::from_sample`.

use crate::app::{state_dict, unix_now, AppState};
use crate::drivers::{self, Advertisement, DriverHost, Sample};
use crate::telemetry::Telemetry;
use anyhow::{anyhow, Result};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Consecutive failed connect attempts (never reaching the treadmill) before the
/// worker gives up and waits for a manual reconnect, instead of scanning forever.
/// Each attempt scans up to ~10s, so this is roughly a minute of trying.
const MAX_CONNECT_ATTEMPTS: u32 = 6;
const SESSION_DEBOUNCE: i32 = 1;

async fn first_adapter() -> Result<Adapter> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    adapters
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no Bluetooth adapter found"))
}

/// What a peripheral advertised, in the neutral shape drivers match against.
fn advertisement(name: &str, services: &[uuid::Uuid]) -> Advertisement {
    Advertisement {
        name: name.to_string(),
        services: services.to_vec(),
    }
}

/// Active scan returning treadmill-looking candidates (or everything if
/// all_devices). "Treadmill-looking" is decided by the driver registry, so a
/// newly added driver's devices show up here with no extra wiring.
pub async fn scan(seconds: f64, all_devices: bool) -> Result<serde_json::Value> {
    let seconds = seconds.clamp(1.0, 15.0);
    let adapter = first_adapter().await?;
    adapter.start_scan(ScanFilter::default()).await?;
    tokio::time::sleep(Duration::from_secs_f64(seconds)).await;
    let peripherals = adapter.peripherals().await?;
    let _ = adapter.stop_scan().await;

    let mut rows = Vec::new();
    for p in peripherals {
        let props = match p.properties().await? {
            Some(props) => props,
            None => continue,
        };
        let name = props.local_name.clone().unwrap_or_default();
        let is_match = drivers::any_match(&advertisement(&name, &props.services));
        if !all_devices && !is_match {
            continue;
        }
        rows.push(json!({
            "device_id": p.id().to_string(),
            "name": name,
            "rssi": props.rssi,
            "service_uuids": props.services.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            "match": is_match,
        }));
    }
    rows.sort_by(|a, b| {
        let ra = a["rssi"].as_i64().unwrap_or(-999);
        let rb = b["rssi"].as_i64().unwrap_or(-999);
        rb.cmp(&ra)
    });
    Ok(json!({"devices": rows, "scanned_s": seconds}))
}

/// Worker entry point — runs forever, reconnecting with backoff.
pub async fn run(state: Arc<AppState>) {
    if let Ok(closed) = state.db.close_stale_active("backend_restart") {
        if closed > 0 {
            tracing::warn!("closed {closed} stale active session(s) at startup");
        }
    }

    let mut fails: u32 = 0;
    while !state.stop.load(Ordering::Relaxed) {
        // Register interest in `wake` BEFORE reading any of the state it guards.
        //
        // `Notify::notify_waiters()` (what set_paused / set_device_id call) only
        // wakes waiters that are ALREADY registered — unlike `notify_one()` it
        // stores no permit. Checking a flag and only then awaiting would drop a
        // wake that lands in between, parking the worker forever while
        // `/api/connect` cheerfully returned {"ok":true}. `enable()` registers us
        // up front, so such a wake is delivered to this future and the await
        // returns immediately. The same future is reused for the reconnect
        // backoff below, so no wake can be swallowed mid-iteration either.
        let wake = state.wake.notified();
        tokio::pin!(wake);
        wake.as_mut().enable();

        let device_id = state.device_id();
        if device_id.is_none() {
            state.connected.store(false, Ordering::Relaxed);
            state.broadcast(json!({"type": "status", "connected": false, "paired": false}));
            tracing::info!("no device paired; waiting for /api/pair");
            wake.as_mut().await;
            continue;
        }
        let device_id = device_id.unwrap();

        // Idle: manually disconnected, or gave up after repeated failures. Either
        // way stop trying and wait for a manual reconnect — the engine (and cloud
        // sync) keep running throughout.
        if state.is_paused() || state.is_connect_failed() {
            state.connected.store(false, Ordering::Relaxed);
            state.broadcast(json!({
                "type": "status", "connected": false, "paired": true,
                "paused": state.is_paused(), "connect_failed": state.is_connect_failed()
            }));
            wake.as_mut().await;
            continue;
        }

        // A drop after a successful connect is a normal reconnect (resets the
        // counter); never reaching the treadmill counts toward giving up.
        let was_connected = match connect_and_poll(&state, &device_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("BLE error: {e:#}");
                false
            }
        };
        if was_connected {
            fails = 0;
        } else {
            fails += 1;
            tracing::warn!("connect attempt {fails}/{MAX_CONNECT_ATTEMPTS} failed for {device_id}");
            if fails >= MAX_CONNECT_ATTEMPTS {
                fails = 0;
                state.set_connect_failed(true);
                tracing::warn!("giving up auto-connect; waiting for manual reconnect");
                continue; // → idle branch broadcasts the failure and waits
            }
        }

        state.connected.store(false, Ordering::Relaxed);
        state.broadcast(json!({
            "type": "status", "connected": false, "paired": state.device_id().is_some(),
            "paused": state.is_paused(), "connect_failed": state.is_connect_failed()
        }));

        // Close any open session on link loss.
        let active = state.active_session();
        if let Some(sid) = active {
            let last = state.last_state();
            persist_close(&state, sid, last.as_ref(), "ble_disconnect");
            state.invalidate_today();
            state.broadcast(json!({"type": "session_end", "id": sid}));
            state.set_active_session(None);
        }

        tokio::select! {
            _ = tokio::time::sleep(RECONNECT_DELAY) => {}
            _ = wake.as_mut() => {}
        }
    }

    // Loop exited (stop requested). The peripheral was disconnected on the way
    // out of connect_and_poll; tell shutdown() we're done so it can stop waiting.
    tracing::info!("BLE worker stopped");
    state.ble_done.notify_one();
}

async fn find_peripheral(adapter: &Adapter, device_id: &str) -> Result<Peripheral> {
    adapter.start_scan(ScanFilter::default()).await?;
    // Poll for up to 10s for a peripheral whose id matches the saved one.
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        for p in adapter.peripherals().await? {
            if p.id().to_string() == device_id {
                let _ = adapter.stop_scan().await;
                return Ok(p);
            }
        }
    }
    let _ = adapter.stop_scan().await;
    Err(anyhow!("device {device_id} not found in scan"))
}

/// Resolves when the driver must be torn down: shutdown, a manual disconnect
/// (pause), or the paired device changing under us. Registered-before-check on
/// `wake`, for the same lost-wake reason as the worker loop above.
async fn cancelled(state: &Arc<AppState>, device_id: &str) {
    loop {
        let wake = state.wake.notified();
        tokio::pin!(wake);
        wake.as_mut().enable();
        if state.stop.load(Ordering::Relaxed) {
            return;
        }
        if state.is_paused() {
            tracing::info!("manual disconnect requested; dropping link");
            return;
        }
        if state.device_id().as_deref() != Some(device_id) {
            tracing::info!("device_id changed; dropping connection");
            return;
        }
        wake.as_mut().await;
    }
}

/// Returns `Ok(true)` once a link was established (a later mid-session drop is
/// still `true` — a normal reconnect, not a failure), `Ok(false)` if we never
/// reached the treadmill (counts toward the give-up limit).
async fn connect_and_poll(state: &Arc<AppState>, device_id: &str) -> Result<bool> {
    tracing::info!("connecting to {device_id}...");
    let adapter = first_adapter().await?;
    let peripheral = match find_peripheral(&adapter, device_id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::info!("{device_id} not found in scan: {e:#}");
            return Ok(false);
        }
    };
    if let Err(e) = peripheral.connect().await {
        tracing::warn!("connect to {device_id} failed: {e:#}");
        return Ok(false);
    }
    if let Err(e) = peripheral.discover_services().await {
        tracing::warn!("service discovery failed: {e:#}");
        let _ = peripheral.disconnect().await;
        return Ok(false);
    }

    // Pick the driver: first registry entry whose `supports()` accepts this
    // device's GATT table (and advertisement). Registry order is the tiebreak —
    // see `drivers::DRIVERS` for why LifeSpan outranks FTMS.
    let adv = match peripheral.properties().await {
        Ok(Some(props)) => advertisement(&props.local_name.unwrap_or_default(), &props.services),
        _ => advertisement("", &[]),
    };
    let gatt = peripheral.characteristics();
    let driver = match drivers::for_device(&adv, &gatt) {
        Some(d) => d,
        None => {
            let _ = peripheral.disconnect().await;
            return Err(anyhow!(
                "no driver supports this device's characteristics (drivers: {})",
                drivers::ids().join(", ")
            ));
        }
    };

    announce_connected(state, device_id, driver.id());

    // The display unit is captured once per connection: the driver interprets
    // its wire format with it and `from_sample` re-encodes with it, and the two
    // MUST agree or raw values would drift on a mid-session unit change.
    let unit = state.display_unit();
    let recorder = |tag: u8, frame: &[u8]| state.record_frame(tag, frame);
    let host = DriverHost::new(unit.clone(), &recorder);

    let mut last_status: Option<u8> = None;
    let mut status_streak: i32 = 0;
    let mut last_persist: f64 = 0.0;
    let mut emit = |sample: Sample| {
        let telem = Telemetry::from_sample(&sample, &unit);
        ingest_sample(
            state,
            &telem,
            &mut last_status,
            &mut status_streak,
            &mut last_persist,
        );
        broadcast_state(state, &telem);
    };

    // The driver runs until the link errors; we cancel it on shutdown, pause,
    // or device switch. Either way the disconnect below is ours, not the
    // driver's — a driver never manages the link's lifecycle.
    let outcome = tokio::select! {
        r = driver.run(&peripheral, &host, &mut emit) => r,
        _ = cancelled(state, device_id) => Ok(()),
    };
    let _ = peripheral.disconnect().await;
    if let Err(e) = outcome {
        tracing::warn!("BLE session ended: {e:#}");
    }
    Ok(true) // we did connect; a drop here is a normal reconnect
}

fn announce_connected(state: &Arc<AppState>, device_id: &str, kind: &str) {
    state.connected.store(true, Ordering::Relaxed);
    state.broadcast(json!({
        "type": "status", "connected": true, "paired": true,
        "display_unit": state.display_unit(), "device_id": device_id,
    }));
    tracing::info!("connected ({kind})");
}

fn broadcast_state(state: &Arc<AppState>, telem: &Telemetry) {
    let mut msg = json!({"type": "state", "state": state_dict(telem)});
    if let Value::Object(ref mut m) = msg {
        m.insert("today".into(), state.today_payload());
        m.insert("active_session_id".into(), json!(state.active_session()));
    }
    state.broadcast(msg);
}

/// Minimum spacing between PERSISTED raw samples. We poll far faster than this
/// (~50 ms plus the radio round trip, so 10–15 telemetry updates a second) but
/// storing every one of them wrote ~1M rows per day of walking — bloating the
/// database and making every day-total aggregation proportionally slower — for no
/// extra fidelity: the rollups are per-minute and the UI ticks about once a
/// second. Mirrors `db::SAMPLE_INTERVAL_S`, which converts a count of running
/// samples back into seconds; the two MUST stay in step.
const SAMPLE_MIN_INTERVAL_S: f64 = crate::db::SAMPLE_INTERVAL_S;

pub(crate) fn ingest_sample(
    state: &Arc<AppState>,
    telem: &Telemetry,
    last_status: &mut Option<u8>,
    status_streak: &mut i32,
    last_persist: &mut f64,
) {
    let now = unix_now();
    state.set_last_state(Some(telem.clone()));

    // Remember the status we came in with: the streak bookkeeping below overwrites
    // `last_status`, but the persistence throttle needs to know whether this
    // telemetry represents a transition.
    let status_changed = telem.status.is_some() && telem.status != *last_status;

    if let Some(st) = telem.status {
        if Some(st) == *last_status {
            *status_streak += 1;
        } else {
            *status_streak = 0;
            *last_status = Some(st);
        }
    }
    let confirmed = *status_streak >= SESSION_DEBOUNCE;
    let active = state.active_session();

    if confirmed && telem.is_running && active.is_none() {
        // Attribute the session to this install so the multi-device breakdown can
        // split steps by device. Empty name → NULL (surfaced as "Unknown").
        let source = crate::config::device_name();
        let source = (!source.is_empty()).then_some(source);
        if let Ok(sid) = state.db.open_session(
            now,
            &state.display_unit(),
            telem.steps,
            telem.duration_s,
            source.as_deref(),
        ) {
            state.set_active_session(Some(sid));
            state.invalidate_today();
            state.broadcast(json!({"type": "session_start", "id": sid}));
            tracing::info!("session {sid} started (start_steps={:?})", telem.steps);
        }
    } else if confirmed && !telem.is_running && active.is_some() {
        let sid = active.unwrap();
        let reason = telem
            .status_name
            .clone()
            .unwrap_or_else(|| "stopped".into());
        persist_close(state, sid, Some(telem), &reason);
        state.invalidate_today();
        tracing::info!("session {sid} closed");
        state.broadcast(json!({"type": "session_end", "id": sid}));
        state.set_active_session(None);
    }

    // Persist at most one row per SAMPLE_MIN_INTERVAL_S. A status change is always
    // written through, so a start/stop transition is never quietly dropped by the
    // throttle. Session detection above runs off live telemetry, not stored rows,
    // so throttling cannot affect it.
    if !status_changed && now - *last_persist < SAMPLE_MIN_INTERVAL_S {
        return;
    }
    *last_persist = now;

    if let Some(sid) = state.active_session() {
        if let Err(e) = state.db.update_active_session(
            sid,
            telem.steps,
            telem.duration_s,
            telem.distance_raw,
            telem.calories,
            telem.speed_raw,
        ) {
            tracing::warn!("could not update session {sid}: {e}");
        }
    }

    // Persist the raw sample. Never silently swallow the error: a dropped write
    // is lost walking, and when the cause is contention (a second daemon on the
    // same database) the log line is the only way to notice.
    if let Err(e) = state.db.insert_sample(
        state.active_session(),
        now,
        telem.steps,
        telem.duration_s,
        telem.speed_raw,
        telem.distance_raw,
        telem.calories,
        telem.status,
    ) {
        tracing::warn!("could not persist sample: {e}");
    }
}

fn persist_close(state: &Arc<AppState>, sid: i64, telem: Option<&Telemetry>, reason: &str) {
    let _ = state.db.close_session(
        sid,
        unix_now(),
        telem.and_then(|t| t.steps),
        telem.and_then(|t| t.duration_s),
        telem.and_then(|t| t.distance_raw),
        telem.and_then(|t| t.calories),
        telem.and_then(|t| t.speed_raw),
        reason,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::telemetry::{STATUS_RUNNING, STATUS_STANDBY};

    fn running(steps: u32) -> Telemetry {
        let mut t = Telemetry::new("km/h");
        t.status = Some(STATUS_RUNNING);
        t.status_name = Some("RUNNING".into());
        t.is_running = true;
        t.steps = Some(steps);
        t
    }

    fn raw_count(db: &Db) -> i64 {
        db.rollup_status().unwrap()["raw_samples"].as_i64().unwrap()
    }

    /// The worker produces 10-15 telemetry updates a second; storing every one of
    /// them wrote ~1M rows per day of walking. At most one row per interval should
    /// land — but a status transition must always be written through, so a
    /// start/stop edge is never lost to the throttle.
    #[test]
    fn throttles_raw_writes_but_never_drops_a_transition() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        let state = AppState::new(db.clone(), "km/h".into(), None, "tok".into());
        let (mut last_status, mut streak, mut last_persist) = (None, 0i32, 0.0f64);

        // A burst of same-status telemetry, all well inside one interval.
        for i in 0..50 {
            ingest_sample(
                &state,
                &running(i),
                &mut last_status,
                &mut streak,
                &mut last_persist,
            );
        }
        assert_eq!(
            raw_count(&db),
            1,
            "a burst within one interval must collapse to a single stored sample"
        );

        // Stopping is a transition: it must be persisted immediately, even though
        // we are still inside the throttle window.
        let mut stopped = running(50);
        stopped.status = Some(STATUS_STANDBY);
        stopped.status_name = Some("STANDBY".into());
        stopped.is_running = false;
        ingest_sample(
            &state,
            &stopped,
            &mut last_status,
            &mut streak,
            &mut last_persist,
        );
        assert_eq!(
            raw_count(&db),
            2,
            "a status change must be written through the throttle"
        );
    }
}
