//! Persisted config in the OS app-data dir: the saved list of treadmills (with
//! one marked active) + display unit. Stored as devices.json; migrates the old
//! single `device_id` file automatically.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static DEVICE_ID_FILE: OnceLock<PathBuf> = OnceLock::new();
static DEVICES_FILE: OnceLock<PathBuf> = OnceLock::new();
static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static SNAPSHOT_PATH: OnceLock<PathBuf> = OnceLock::new();
static SETTINGS_FILE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub last_seen: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevicesConfig {
    pub active: Option<String>,
    pub devices: Vec<Device>,
}

/// Write `data` to `path` atomically: write a sibling temp file, then `rename`
/// it over the target (atomic on the same filesystem). A crash/power-loss can no
/// longer leave a half-written JSON file that a reader would then treat as
/// corrupt and silently reset to defaults. Used for every config/snapshot/handshake
/// write across the engine and daemon.
pub fn atomic_write(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

pub fn init_paths(data_dir: &std::path::Path) {
    let _ = DEVICE_ID_FILE.set(data_dir.join("device_id"));
    let _ = DEVICES_FILE.set(data_dir.join("devices.json"));
    let _ = DB_PATH.set(data_dir.join("lifespan.db"));
    let _ = SNAPSHOT_PATH.set(data_dir.join("snapshot.json"));
    let _ = SETTINGS_FILE.set(data_dir.join("settings.json"));
}

// ---- app settings (locale / unit / first-run flag) -------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_unit")]
    pub display_unit: String,
    #[serde(default)]
    pub setup_complete: bool,
}

fn default_locale() -> String {
    "en".into()
}
fn default_unit() -> String {
    "km/h".into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: default_locale(),
            display_unit: default_unit(),
            setup_complete: false,
        }
    }
}

pub fn load_settings() -> AppSettings {
    if let Some(path) = SETTINGS_FILE.get() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(s) = serde_json::from_str::<AppSettings>(&text) {
                return s;
            }
        }
    }
    // No settings file yet: seed sensible defaults and treat *existing* users
    // (who already have a paired device) as already set up, so the wizard only
    // greets genuinely fresh installs.
    let mut s = AppSettings::default();
    if let Ok(u) = std::env::var("SC110_DISPLAY_UNIT") {
        if u.to_lowercase() == "mph" {
            s.display_unit = "mph".into();
        }
    }
    if load_devices().active.is_some() {
        s.setup_complete = true;
    }
    s
}

pub fn save_settings(s: &AppSettings) {
    if let Some(path) = SETTINGS_FILE.get() {
        if let Ok(text) = serde_json::to_string_pretty(s) {
            let _ = atomic_write(path, text.as_bytes());
        }
    }
}

/// Wipe the paired-device store (used by the "reset to empty" testing flow so
/// the first-run wizard reappears).
pub fn clear_devices() {
    save_devices(&DevicesConfig::default());
}

pub fn db_path() -> PathBuf {
    DB_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("lifespan.db"))
}

/// Where the reset/restore snapshot of all data is parked. Persists across app
/// restarts and reinstalls (same data dir), so a "reset to empty for testing"
/// can be undone later.
pub fn snapshot_path() -> PathBuf {
    SNAPSHOT_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("snapshot.json"))
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ---- devices store ---------------------------------------------------------

pub fn load_devices() -> DevicesConfig {
    if let Some(path) = DEVICES_FILE.get() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<DevicesConfig>(&text) {
                return cfg;
            }
        }
    }
    // Migrate the legacy single device_id file, if present.
    if let Some(old) = DEVICE_ID_FILE.get() {
        if let Ok(text) = std::fs::read_to_string(old) {
            let id = text.trim().to_string();
            if !id.is_empty() {
                let cfg = DevicesConfig {
                    active: Some(id.clone()),
                    devices: vec![Device { id, name: "Treadmill".into(), last_seen: now() }],
                };
                save_devices(&cfg);
                return cfg;
            }
        }
    }
    DevicesConfig::default()
}

pub fn save_devices(cfg: &DevicesConfig) {
    if let Some(path) = DEVICES_FILE.get() {
        if let Ok(text) = serde_json::to_string_pretty(cfg) {
            let _ = atomic_write(path, text.as_bytes());
        }
    }
}

/// Add (or rename) a device and make it the active one. Returns the new config.
pub fn add_and_activate(id: &str, name: Option<&str>) -> DevicesConfig {
    let mut cfg = load_devices();
    match cfg.devices.iter_mut().find(|d| d.id == id) {
        Some(d) => {
            d.last_seen = now();
            if let Some(n) = name {
                if !n.is_empty() {
                    d.name = n.to_string();
                }
            }
        }
        None => cfg.devices.push(Device {
            id: id.to_string(),
            name: name.filter(|n| !n.is_empty()).unwrap_or("Treadmill").to_string(),
            last_seen: now(),
        }),
    }
    cfg.active = Some(id.to_string());
    save_devices(&cfg);
    cfg
}

/// Switch the active device (only if it's a known device). Returns true on success.
pub fn set_active(id: &str) -> bool {
    let mut cfg = load_devices();
    if cfg.devices.iter().any(|d| d.id == id) {
        cfg.active = Some(id.to_string());
        save_devices(&cfg);
        true
    } else {
        false
    }
}

/// Forget a device. If it was active, the active slot is cleared (or moved to
/// the first remaining device). Returns the new active id, if any.
pub fn forget(id: &str) -> Option<String> {
    let mut cfg = load_devices();
    cfg.devices.retain(|d| d.id != id);
    if cfg.active.as_deref() == Some(id) {
        cfg.active = cfg.devices.first().map(|d| d.id.clone());
    }
    save_devices(&cfg);
    cfg.active
}

pub fn active_device_id() -> Option<String> {
    if let Ok(env) = std::env::var("SC110_DEVICE_ID") {
        let t = env.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    load_devices().active
}

pub fn display_unit() -> String {
    // Env override wins (legacy/testing); otherwise the persisted setting.
    if let Ok(u) = std::env::var("SC110_DISPLAY_UNIT") {
        return if u.to_lowercase() == "mph" { "mph".into() } else { "km/h".into() };
    }
    let u = load_settings().display_unit.to_lowercase();
    if u == "mph" {
        "mph".into()
    } else {
        "km/h".into()
    }
}
