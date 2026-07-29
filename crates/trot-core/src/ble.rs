//! BLE worker: owns the connection to the one SC110, polls the opcode rotation,
//! decodes via the protocol Reader, detects session boundaries, persists samples,
//! and broadcasts every update to the hub. Ported from backend/worker.py using
//! btleplug instead of bleak.

use crate::app::{state_dict, unix_now, AppState};
use crate::ftms;
use crate::protocol::{
    self, build_request, Reader, Telemetry, ADV_NAME_PREFIXES, DEFAULT_POLL_ROTATION,
    NOTIFY_CHAR_UUID, STATUS_RUNNING, WRITE_CHAR_UUID,
};
use anyhow::{anyhow, Result};
use btleplug::api::{
    Central, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::{FutureExt, StreamExt};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// Consecutive failed connect attempts (never reaching the treadmill) before the
/// worker gives up and waits for a manual reconnect, instead of scanning forever.
/// Each attempt scans up to ~10s, so this is roughly a minute of trying.
const MAX_CONNECT_ATTEMPTS: u32 = 6;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Consecutive unanswered polls (~2s each) before we treat a seemingly-"connected"
/// link as dead and force a full reconnect. macOS can leave a peripheral handle
/// open with no disconnect event after the treadmill sleeps/powers off; without
/// this the worker would poll a stale link forever and never re-scan when the
/// belt comes back.
const MAX_DEAD_POLLS: u32 = 15;
const SESSION_DEBOUNCE: i32 = 1;
/// FTMS treadmills push Treadmill Data ~1 Hz; tolerate a quiet belt before
/// treating the link as dead.
const FTMS_IDLE_TIMEOUT: Duration = Duration::from_secs(15);

fn notify_uuid() -> Uuid {
    Uuid::parse_str(NOTIFY_CHAR_UUID).unwrap()
}
fn write_uuid() -> Uuid {
    Uuid::parse_str(WRITE_CHAR_UUID).unwrap()
}
fn ftms_data_uuid() -> Uuid {
    Uuid::parse_str(ftms::TREADMILL_DATA_UUID).unwrap()
}

async fn first_adapter() -> Result<Adapter> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    adapters
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no Bluetooth adapter found"))
}

/// Active scan returning SC110-looking candidates (or everything if all_devices).
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
        let advertises = |uuid: &str| {
            props
                .services
                .iter()
                .any(|u| u.to_string().eq_ignore_ascii_case(uuid))
        };
        // Match SC110 (service 0xFFF0), any FTMS treadmill (0x1826), or a known
        // LifeSpan/ESP32 advertised name.
        let is_match = ADV_NAME_PREFIXES.iter().any(|pfx| name.starts_with(pfx))
            || advertises(protocol::SERVICE_UUID)
            || advertises(ftms::FITNESS_MACHINE_SERVICE_UUID);
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
        let device_id = state.device_id();
        if device_id.is_none() {
            state.connected.store(false, Ordering::Relaxed);
            state.broadcast(json!({"type": "status", "connected": false, "paired": false}));
            tracing::info!("no device paired; waiting for /api/pair");
            state.wake.notified().await;
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
            state.wake.notified().await;
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
            let last = state.last_state.lock().unwrap().clone();
            persist_close(&state, sid, last.as_ref(), "ble_disconnect");
            state.invalidate_today();
            state.broadcast(json!({"type": "session_end", "id": sid}));
            *state.active_session_id.lock().unwrap() = None;
        }

        tokio::select! {
            _ = tokio::time::sleep(RECONNECT_DELAY) => {}
            _ = state.wake.notified() => {}
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

    let chars = peripheral.characteristics();
    let notify_char = chars.iter().find(|c| c.uuid == notify_uuid()).cloned();
    let write_char = chars.iter().find(|c| c.uuid == write_uuid()).cloned();

    // Prefer the native SC110 protocol (FFF1/FFF2). Fall back to the standard
    // Bluetooth FTMS Treadmill Data characteristic (0x2ACD) so other LifeSpan
    // models and third-party FTMS treadmills work too.
    let poll = if let (Some(notify_char), Some(write_char)) = (notify_char, write_char) {
        poll_sc110(state, &peripheral, device_id, notify_char, write_char).await
    } else if let Some(data_char) = chars.iter().find(|c| c.uuid == ftms_data_uuid()).cloned() {
        stream_ftms(state, &peripheral, device_id, data_char).await
    } else {
        let _ = peripheral.disconnect().await;
        return Err(anyhow!(
            "device exposes neither SC110 (FFF1/FFF2) nor FTMS (2ACD) characteristics"
        ));
    };
    if let Err(e) = poll {
        tracing::warn!("BLE session ended: {e:#}");
    }
    Ok(true) // we did connect; a drop here is a normal reconnect
}

/// Native SC110 path: poll the opcode rotation and decode via `Reader`.
async fn poll_sc110(
    state: &Arc<AppState>,
    peripheral: &Peripheral,
    device_id: &str,
    notify_char: btleplug::api::Characteristic,
    write_char: btleplug::api::Characteristic,
) -> Result<()> {
    peripheral.subscribe(&notify_char).await?;
    let mut notifications = peripheral.notifications().await?;

    announce_connected(state, device_id, "SC110");

    let mut reader = Reader::new(&state.display_unit());
    let mut last_status: Option<u8> = None;
    let mut status_streak: i32 = 0;
    let mut idx = 0usize;
    let mut dead_polls: u32 = 0;

    while !state.stop.load(Ordering::Relaxed) {
        if state.is_paused() {
            tracing::info!("manual disconnect requested; dropping link");
            break;
        }
        if state.device_id().as_deref() != Some(device_id) {
            tracing::info!("device_id changed; dropping connection");
            break;
        }
        let opcode = DEFAULT_POLL_ROTATION[idx % DEFAULT_POLL_ROTATION.len()];
        idx += 1;

        // Drain any stale buffered notifications so the response we read below is
        // the one for THIS request. Responses don't echo their opcode, so a single
        // buffered/lagging frame would otherwise mis-assign every field (speed
        // reading as steps, etc.).
        while notifications.next().now_or_never().flatten().is_some() {}

        // Bound the write: a stale link can block the write forever with no
        // disconnect event, which would wedge the worker.
        match tokio::time::timeout(
            RESPONSE_TIMEOUT,
            peripheral.write(&write_char, &build_request(opcode), WriteType::WithResponse),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e.into()), // real BLE error → reconnect
            Err(_) => {
                dead_polls += 1;
                tracing::warn!("timeout writing opcode 0x{opcode:02x} ({dead_polls}/{MAX_DEAD_POLLS})");
                if dead_polls >= MAX_DEAD_POLLS {
                    return Err(anyhow!("link unresponsive; forcing reconnect"));
                }
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        }

        // Await the next notification with a timeout (responses don't echo opcode).
        let frame = match tokio::time::timeout(RESPONSE_TIMEOUT, notifications.next()).await {
            Ok(Some(n)) => n.value,
            Ok(None) => return Err(anyhow!("notification stream ended")),
            Err(_) => {
                dead_polls += 1;
                tracing::warn!(
                    "timeout waiting for response to opcode 0x{opcode:02x} ({dead_polls}/{MAX_DEAD_POLLS})"
                );
                if dead_polls >= MAX_DEAD_POLLS {
                    return Err(anyhow!("link unresponsive; forcing reconnect"));
                }
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };
        dead_polls = 0; // a frame arrived → the link is alive
        state.record_frame(opcode, &frame); // raw capture for protocol diagnostics

        let telem = match reader.feed(opcode, &frame) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("decode error opcode 0x{opcode:02x}: {e}");
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };

        ingest_sample(state, &telem, &mut last_status, &mut status_streak);
        broadcast_state(state, &telem);
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let _ = peripheral.disconnect().await;
    Ok(())
}

/// Standard FTMS path: subscribe to Treadmill Data and decode each push.
async fn stream_ftms(
    state: &Arc<AppState>,
    peripheral: &Peripheral,
    device_id: &str,
    data_char: btleplug::api::Characteristic,
) -> Result<()> {
    peripheral.subscribe(&data_char).await?;
    let mut notifications = peripheral.notifications().await?;

    announce_connected(state, device_id, "FTMS");

    let mut last_status: Option<u8> = None;
    let mut status_streak: i32 = 0;

    while !state.stop.load(Ordering::Relaxed) {
        if state.is_paused() {
            tracing::info!("manual disconnect requested; dropping link");
            break;
        }
        if state.device_id().as_deref() != Some(device_id) {
            tracing::info!("device_id changed; dropping connection");
            break;
        }
        let frame = match tokio::time::timeout(FTMS_IDLE_TIMEOUT, notifications.next()).await {
            Ok(Some(n)) => n.value,
            Ok(None) => return Err(anyhow!("notification stream ended")),
            Err(_) => {
                // A quiet belt is normal for FTMS (no push when stopped). Only
                // treat the link as dead if the OS no longer considers us
                // connected — recovers a stale handle without churning on
                // legitimate pauses.
                if !peripheral.is_connected().await.unwrap_or(false) {
                    return Err(anyhow!("FTMS link dropped; reconnecting"));
                }
                continue;
            }
        };
        let data = match ftms::parse_treadmill_data(&frame) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("FTMS decode error: {e}");
                continue;
            }
        };
        let telem = ftms_to_telemetry(&data, &state.display_unit());
        ingest_sample(state, &telem, &mut last_status, &mut status_streak);
        broadcast_state(state, &telem);
    }
    let _ = peripheral.disconnect().await;
    Ok(())
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

/// Map a decoded FTMS Treadmill Data record onto the app's `Telemetry` shape so
/// the existing UI, session detection, and storage work unchanged. FTMS does not
/// report step counts, so `steps` stays `None` for these treadmills.
fn ftms_to_telemetry(d: &ftms::FtmsTreadmillData, unit: &str) -> Telemetry {
    let mut t = Telemetry::new(unit);
    if let Some(kmh) = d.instantaneous_speed {
        // Telemetry.speed_raw is hundredths of the *displayed* unit.
        let displayed = if unit == "mph" {
            kmh / protocol::KMH_PER_MPH
        } else {
            kmh
        };
        let raw = (displayed * 100.0).round().max(0.0) as u32;
        t.speed_raw = Some(raw);
        t.speed_kmh = Some(protocol::speed_kmh(raw, unit));
        t.speed_mph = Some(protocol::speed_mph(raw, unit));
        let running = kmh > 0.05;
        let status = if running {
            protocol::STATUS_RUNNING
        } else {
            protocol::STATUS_STANDBY
        };
        t.status = Some(status);
        t.status_name = Some(protocol::status_name(status));
        t.is_running = running;
    }
    if let Some(m) = d.total_distance_m {
        // Telemetry.distance_raw is decameters (×10 = meters).
        let raw = (m as f64 / 10.0).round() as u32;
        t.distance_raw = Some(raw);
        t.distance_m = Some(protocol::distance_meters(raw));
        t.distance_km = Some(protocol::distance_meters(raw) as f64 / 1000.0);
        t.distance_mi = Some(protocol::distance_meters(raw) as f64 / 1000.0 / protocol::KMH_PER_MPH);
    }
    if let Some(s) = d.elapsed_time_s {
        t.duration_s = Some(s);
    }
    if let Some(c) = d.total_energy_kcal {
        t.calories = Some(c);
    }
    t
}

fn ingest_sample(
    state: &Arc<AppState>,
    telem: &Telemetry,
    last_status: &mut Option<u8>,
    status_streak: &mut i32,
) {
    let now = unix_now();
    *state.last_state.lock().unwrap() = Some(telem.clone());

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
            *state.active_session_id.lock().unwrap() = Some(sid);
            state.invalidate_today();
            state.broadcast(json!({"type": "session_start", "id": sid}));
            tracing::info!("session {sid} started (start_steps={:?})", telem.steps);
        }
    } else if confirmed && !telem.is_running && active.is_some() {
        let sid = active.unwrap();
        let reason = telem.status_name.clone().unwrap_or_else(|| "stopped".into());
        persist_close(state, sid, Some(telem), &reason);
        state.invalidate_today();
        tracing::info!("session {sid} closed");
        state.broadcast(json!({"type": "session_end", "id": sid}));
        *state.active_session_id.lock().unwrap() = None;
    }

    if let Some(sid) = state.active_session() {
        let _ = state.db.update_active_session(
            sid,
            telem.steps,
            telem.duration_s,
            telem.distance_raw,
            telem.calories,
            telem.speed_raw,
        );
    }

    let _ = state.db.insert_sample(
        state.active_session(),
        now,
        telem.steps,
        telem.duration_s,
        telem.speed_raw,
        telem.distance_raw,
        telem.calories,
        telem.status,
    );
    let _ = STATUS_RUNNING; // referenced via is_running
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

use serde_json::Value;
