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
    /// Random per-launch token injected into the served index.html and required
    /// on state-changing /api calls. Stops other local processes / cross-site
    /// requests from driving the loopback API (see server.rs guard).
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
}

/// Max raw frames retained for diagnostics (~a few minutes at 20 Hz).
const FRAME_RING_CAP: usize = 1200;

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
            active_session_id: Mutex::new(None),
            last_state: Mutex::new(None),
            frames: Mutex::new(std::collections::VecDeque::with_capacity(FRAME_RING_CAP)),
            wake: Notify::new(),
            stop: AtomicBool::new(false),
        })
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
        self.wake.notify_waiters();
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
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
    pub fn today_payload(&self) -> Value {
        let today = Self::today_str();
        let mut day = self.db.day_totals(&today).unwrap_or_else(|_| json!({}));
        if let Value::Object(ref mut m) = day {
            let steps = m.get("steps").cloned().unwrap_or(json!(0));
            m.insert("total_steps_live".into(), steps);
            if let Value::Object(avg) = self.avg_speed_payload(&today) {
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
