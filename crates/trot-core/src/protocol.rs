//! SC110 BLE protocol: constants, request builder, and the incremental Reader.
//! Ported faithfully from the Python `lifespan_sc110` package (parser.py + __init__.py).
//!
//! Protocol knowledge was bootstrapped from and cross-checked against
//! blak3r/treadspan (MIT, © 2025 Blake Robertson). See THIRD-PARTY-NOTICES.md.
//!
//! Frame format (confirmed on SC110):
//!   byte 0: 0xA1 (prefix)
//!   byte 1: 0xAA (ok) or 0xFF (unknown opcode)
//!   bytes 2..5: opcode-dependent payload
//!
//! The response does NOT echo the request opcode, so decoding requires knowing
//! which opcode we polled last — `Reader` tracks that.

use serde::Serialize;

// ---- UUIDs (service 0xFFF0) -------------------------------------------------
pub const SERVICE_UUID: &str = "0000fff0-0000-1000-8000-00805f9b34fb";
pub const NOTIFY_CHAR_UUID: &str = "0000fff1-0000-1000-8000-00805f9b34fb";
pub const WRITE_CHAR_UUID: &str = "0000fff2-0000-1000-8000-00805f9b34fb";

// Match any LifeSpan console (TR1200/TR5000/SC/DT… all share the Omni protocol),
// not just "LifeSpan-TM". "ESP32" is kept as a fallback for units whose BLE
// module advertises only its generic chipset name. See docs/lifespan-models.md.
pub const ADV_NAME_PREFIXES: &[&str] = &["LifeSpan", "ESP32"];

pub const REQ_PREFIX: u8 = 0xA1;
pub const RESP_OK: u8 = 0xAA;
pub const RESP_ERR: u8 = 0xFF;

pub const OPCODE_SPEED: u8 = 0x82;
pub const OPCODE_DISTANCE: u8 = 0x85;
pub const OPCODE_CALORIES: u8 = 0x87;
pub const OPCODE_STEPS: u8 = 0x88;
pub const OPCODE_DURATION: u8 = 0x89;
pub const OPCODE_STATUS: u8 = 0x91;

pub const STATUS_STANDBY: u8 = 0x01;
pub const STATUS_RUNNING: u8 = 0x03;
pub const STATUS_SUMMARY: u8 = 0x04;
pub const STATUS_PAUSED: u8 = 0x05;

pub const KMH_PER_MPH: f64 = 1.609344;

/// SPEED is interleaved 5x so the belt speed updates feel live.
pub const DEFAULT_POLL_ROTATION: &[u8] = &[
    OPCODE_SPEED,
    OPCODE_STATUS,
    OPCODE_SPEED,
    OPCODE_STEPS,
    OPCODE_SPEED,
    OPCODE_DISTANCE,
    OPCODE_SPEED,
    OPCODE_DURATION,
    OPCODE_SPEED,
    OPCODE_CALORIES,
];

pub fn status_name(value: u8) -> String {
    match value {
        STATUS_STANDBY => "STANDBY".to_string(),
        STATUS_RUNNING => "RUNNING".to_string(),
        STATUS_SUMMARY => "SUMMARY_SCREEN".to_string(),
        STATUS_PAUSED => "PAUSED".to_string(),
        v => format!("UNKNOWN_0x{v:02x}"),
    }
}

/// 6-byte `A1 OPCODE 00 00 00 00` request frame.
pub fn build_request(opcode: u8) -> [u8; 6] {
    [REQ_PREFIX, opcode, 0, 0, 0, 0]
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("expected 6 bytes, got {0}")]
    BadLength(usize),
    #[error("bad prefix")]
    BadPrefix,
    #[error("device reported unknown-opcode (A1 FF ...)")]
    UnknownOpcode,
    #[error("unexpected response flag 0x{0:02x}")]
    UnexpectedFlag(u8),
    #[error("out-of-range duration bytes")]
    BadDuration,
}

fn validate(frame: &[u8]) -> Result<(), ProtocolError> {
    if frame.len() != 6 {
        return Err(ProtocolError::BadLength(frame.len()));
    }
    if frame[0] != REQ_PREFIX {
        return Err(ProtocolError::BadPrefix);
    }
    if frame[1] == RESP_ERR {
        return Err(ProtocolError::UnknownOpcode);
    }
    if frame[1] != RESP_OK {
        return Err(ProtocolError::UnexpectedFlag(frame[1]));
    }
    Ok(())
}

fn u16_be(frame: &[u8]) -> u32 {
    ((frame[2] as u32) << 8) | frame[3] as u32
}

pub fn decode_steps(frame: &[u8]) -> Result<u32, ProtocolError> {
    validate(frame)?;
    Ok(u16_be(frame))
}

pub fn decode_calories(frame: &[u8]) -> Result<u32, ProtocolError> {
    validate(frame)?;
    Ok(u16_be(frame))
}

/// Raw u16 big-endian in units of 10 meters (decameters). `displayed_m = raw * 10`.
pub fn decode_distance_raw(frame: &[u8]) -> Result<u32, ProtocolError> {
    validate(frame)?;
    Ok(u16_be(frame))
}

/// Returns hundredths of the displayed speed unit: byte 2 = whole units,
/// byte 3 = hundredths (0..99). Callers divide by 100 for displayed value.
pub fn decode_speed_raw(frame: &[u8]) -> Result<u32, ProtocolError> {
    validate(frame)?;
    Ok(frame[2] as u32 * 100 + frame[3] as u32)
}

pub fn decode_duration_seconds(frame: &[u8]) -> Result<u32, ProtocolError> {
    validate(frame)?;
    let (h, m, s) = (frame[2] as u32, frame[3] as u32, frame[4] as u32);
    if m >= 60 || s >= 60 {
        return Err(ProtocolError::BadDuration);
    }
    Ok(h * 3600 + m * 60 + s)
}

pub fn decode_status(frame: &[u8]) -> Result<(u8, String), ProtocolError> {
    validate(frame)?;
    let v = frame[2];
    Ok((v, status_name(v)))
}

pub fn distance_meters(raw: u32) -> u32 {
    raw * 10
}

pub fn speed_kmh(raw: u32, display_unit: &str) -> f64 {
    let displayed = raw as f64 / 100.0;
    if display_unit == "km/h" {
        displayed
    } else {
        displayed * KMH_PER_MPH
    }
}

pub fn speed_mph(raw: u32, display_unit: &str) -> f64 {
    let displayed = raw as f64 / 100.0;
    if display_unit == "mph" {
        displayed
    } else {
        displayed / KMH_PER_MPH
    }
}

/// Latest known state, assembled across multiple polls. Mirrors Python `Telemetry`.
#[derive(Debug, Clone, Serialize)]
pub struct Telemetry {
    pub steps: Option<u32>,
    pub duration_s: Option<u32>,
    pub distance_raw: Option<u32>,
    pub calories: Option<u32>,
    pub speed_raw: Option<u32>,
    pub status: Option<u8>,
    pub status_name: Option<String>,
    pub display_unit: String,
    // Derived fields (serialized for the UI, matching the Python _state_dict).
    pub speed_kmh: Option<f64>,
    pub speed_mph: Option<f64>,
    pub distance_m: Option<u32>,
    pub distance_km: Option<f64>,
    pub distance_mi: Option<f64>,
    pub is_running: bool,
}

impl Telemetry {
    pub fn new(display_unit: &str) -> Self {
        Telemetry {
            steps: None,
            duration_s: None,
            distance_raw: None,
            calories: None,
            speed_raw: None,
            status: None,
            status_name: None,
            display_unit: display_unit.to_string(),
            speed_kmh: None,
            speed_mph: None,
            distance_m: None,
            distance_km: None,
            distance_mi: None,
            is_running: false,
        }
    }

    /// Recompute the derived fields from the raw fields + display unit.
    fn refresh_derived(&mut self) {
        self.speed_kmh = self.speed_raw.map(|r| speed_kmh(r, &self.display_unit));
        self.speed_mph = self.speed_raw.map(|r| speed_mph(r, &self.display_unit));
        self.distance_m = self.distance_raw.map(distance_meters);
        self.distance_km = self
            .distance_raw
            .map(|r| distance_meters(r) as f64 / 1000.0);
        self.distance_mi = self
            .distance_raw
            .map(|r| distance_meters(r) as f64 / 1000.0 / KMH_PER_MPH);
        self.is_running = self.status == Some(STATUS_RUNNING);
    }
}

/// Incremental decoder: feed (opcode_of_last_poll, response_frame) pairs.
pub struct Reader {
    state: Telemetry,
}

impl Reader {
    pub fn new(display_unit: &str) -> Self {
        Reader {
            state: Telemetry::new(display_unit),
        }
    }

    #[allow(dead_code)]
    pub fn state(&self) -> &Telemetry {
        &self.state
    }

    /// Decode one response in the context of the opcode last polled.
    /// Returns a clone of the updated state, or the error if decoding failed
    /// (in which case state is left unchanged, mirroring the Python client).
    pub fn feed(&mut self, last_opcode: u8, response: &[u8]) -> Result<Telemetry, ProtocolError> {
        match last_opcode {
            OPCODE_STEPS => self.state.steps = Some(decode_steps(response)?),
            OPCODE_DURATION => self.state.duration_s = Some(decode_duration_seconds(response)?),
            OPCODE_DISTANCE => self.state.distance_raw = Some(decode_distance_raw(response)?),
            OPCODE_CALORIES => self.state.calories = Some(decode_calories(response)?),
            OPCODE_SPEED => self.state.speed_raw = Some(decode_speed_raw(response)?),
            OPCODE_STATUS => {
                let (v, name) = decode_status(response)?;
                self.state.status = Some(v);
                self.state.status_name = Some(name);
            }
            _ => {}
        }
        self.state.refresh_derived();
        Ok(self.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hx(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn build_request_frames() {
        assert_eq!(build_request(OPCODE_STEPS), hx("a1 88 00 00 00 00")[..]);
        assert_eq!(build_request(OPCODE_STATUS), hx("a1 91 00 00 00 00")[..]);
    }

    #[test]
    fn validation_errors() {
        assert!(matches!(
            decode_steps(&hx("a1 aa 00 06")),
            Err(ProtocolError::BadLength(4))
        ));
        assert!(matches!(
            decode_steps(&hx("a2 aa 00 06 00 00")),
            Err(ProtocolError::BadPrefix)
        ));
        assert!(matches!(
            decode_steps(&hx("a1 ff 00 00 00 00")),
            Err(ProtocolError::UnknownOpcode)
        ));
    }

    #[test]
    fn decoders() {
        assert_eq!(decode_steps(&hx("a1 aa 00 23 00 00")).unwrap(), 35);
        assert_eq!(decode_steps(&hx("a1 aa 01 00 00 00")).unwrap(), 256);
        assert_eq!(decode_steps(&hx("a1 aa ff ff 00 00")).unwrap(), 65535);
        assert_eq!(decode_calories(&hx("a1 aa 00 02 00 00")).unwrap(), 2);
        assert_eq!(decode_distance_raw(&hx("a1 aa 01 1f 00 00")).unwrap(), 287);
        assert_eq!(decode_speed_raw(&hx("a1 aa 00 3c 00 00")).unwrap(), 60);
        assert_eq!(
            decode_duration_seconds(&hx("a1 aa 00 01 0b 00")).unwrap(),
            71
        );
        assert_eq!(
            decode_duration_seconds(&hx("a1 aa 00 00 0e 00")).unwrap(),
            14
        );
        assert_eq!(
            decode_duration_seconds(&hx("a1 aa 00 3b 3b 00")).unwrap(),
            59 * 60 + 59
        );
        assert_eq!(
            decode_duration_seconds(&hx("a1 aa 01 00 00 00")).unwrap(),
            3600
        );
    }

    #[test]
    fn duration_out_of_range() {
        assert!(decode_duration_seconds(&hx("a1 aa 00 3c 00 00")).is_err());
        assert!(decode_duration_seconds(&hx("a1 aa 00 00 3c 00")).is_err());
    }

    #[test]
    fn status_decode() {
        assert_eq!(
            decode_status(&hx("a1 aa 03 00 00 00")).unwrap(),
            (STATUS_RUNNING, "RUNNING".to_string())
        );
        assert_eq!(
            decode_status(&hx("a1 aa 01 00 00 00")).unwrap(),
            (STATUS_STANDBY, "STANDBY".to_string())
        );
    }

    #[test]
    fn speed_units() {
        assert!((speed_kmh(60, "km/h") - 0.6).abs() < 1e-9);
        assert!((speed_mph(60, "km/h") - 0.6 / KMH_PER_MPH).abs() < 1e-4);
        assert!((speed_mph(60, "mph") - 0.6).abs() < 1e-9);
        assert!((speed_kmh(60, "mph") - 0.6 * KMH_PER_MPH).abs() < 1e-4);
    }

    #[test]
    fn reader_aggregates_across_opcodes() {
        let mut r = Reader::new("km/h");
        r.feed(OPCODE_STEPS, &hx("a1 aa 00 23 00 00")).unwrap();
        r.feed(OPCODE_STATUS, &hx("a1 aa 03 00 00 00")).unwrap();
        r.feed(OPCODE_DURATION, &hx("a1 aa 00 01 0b 00")).unwrap();
        r.feed(OPCODE_SPEED, &hx("a1 aa 00 3c 00 00")).unwrap();
        r.feed(OPCODE_CALORIES, &hx("a1 aa 00 02 00 00")).unwrap();
        r.feed(OPCODE_DISTANCE, &hx("a1 aa 00 00 00 00")).unwrap();
        let s = r.state();
        assert_eq!(s.steps, Some(35));
        assert_eq!(s.duration_s, Some(71));
        assert_eq!(s.calories, Some(2));
        assert_eq!(s.speed_raw, Some(60));
        assert!((s.speed_kmh.unwrap() - 0.6).abs() < 1e-9);
        assert_eq!(s.status_name.as_deref(), Some("RUNNING"));
        assert!(s.is_running);
    }
}
