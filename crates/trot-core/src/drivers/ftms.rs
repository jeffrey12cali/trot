//! Bluetooth SIG Fitness Machine Service (FTMS) driver — Treadmill Data.
//!
//! Interaction model: **subscribe-and-push.** The treadmill notifies a
//! Treadmill Data record (~1 Hz while the belt moves); we subscribe once and
//! decode each push. FTMS has no step counter, so `steps` stays `None`.
//!
//! The parser half is a clean-room implementation of the Treadmill Data characteristic (0x2ACD)
//! byte layout, written from the official FTMS v1.0.1 service specification and
//! the Bluetooth SIG Assigned Numbers / GATT Specification Supplement field
//! definitions. No third-party source was consulted; the byte unpacking below
//! is implemented directly from the published field order, sizes, signedness,
//! and resolutions.
//!
//! Frame format (little-endian throughout):
//!   bytes 0..2: Flags (uint16). Each bit signals presence of an optional field.
//!   then, in strict spec order, the present fields follow.
//!
//! Flags bit map (Table 4.5, FTMS v1.0.1):
//!   bit 0  More Data            — when 0, Instantaneous Speed IS present
//!   bit 1  Average Speed
//!   bit 2  Total Distance
//!   bit 3  Inclination + Ramp Angle Setting (pair)
//!   bit 4  Elevation Gain (positive + negative pair)
//!   bit 5  Instantaneous Pace
//!   bit 6  Average Pace
//!   bit 7  Expended Energy (total + per-hour + per-minute triple)
//!   bit 8  Heart Rate
//!   bit 9  Metabolic Equivalent
//!   bit 10 Elapsed Time
//!   bit 11 Remaining Time
//!   bit 12 Force on Belt + Power Output (pair)
//!
//! Field sizes / resolutions (GATT Specification Supplement, Treadmill Data):
//!   Instantaneous Speed   uint16  0.01 km/h
//!   Average Speed         uint16  0.01 km/h
//!   Total Distance        uint24  1 m
//!   Inclination           sint16  0.1 %
//!   Ramp Angle Setting    sint16  0.1 deg
//!   Positive Elev. Gain   uint16  0.1 m
//!   Negative Elev. Gain   uint16  0.1 m
//!   Instantaneous Pace    uint8   0.1 km/min
//!   Average Pace          uint8   0.1 km/min
//!   Total Energy          uint16  1 kcal
//!   Energy per Hour       uint16  1 kcal
//!   Energy per Minute     uint8   1 kcal
//!   Heart Rate            uint8   1 bpm
//!   Metabolic Equivalent  uint8   0.1
//!   Elapsed Time          uint16  1 s
//!   Remaining Time        uint16  1 s
//!   Force on Belt         sint16  1 N
//!   Power Output          sint16  1 W

use super::{Advertisement, BeltState, Driver, DriverHost, Emit, Sample};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use btleplug::api::{Characteristic, Peripheral as _};
use btleplug::platform::Peripheral;
use futures::StreamExt;
use serde::Serialize;
use std::collections::BTreeSet;
use std::time::Duration;

// ---- UUIDs ------------------------------------------------------------------

/// Build the full 128-bit string form of a 16-bit Bluetooth SIG assigned UUID.
/// e.g. `0x1826` -> `00001826-0000-1000-8000-00805f9b34fb`.
#[allow(dead_code)] // helper kept alongside the UUID set
pub fn uuid16(short: u16) -> String {
    format!("0000{short:04x}-0000-1000-8000-00805f9b34fb")
}

/// Fitness Machine Service.
pub const FITNESS_MACHINE_SERVICE_UUID: &str = "00001826-0000-1000-8000-00805f9b34fb";
/// Treadmill Data characteristic (notify).
pub const TREADMILL_DATA_UUID: &str = "00002acd-0000-1000-8000-00805f9b34fb";
// The following are part of the FTMS UUID set, reserved for the next phase
// (reading machine features and driving the control point to start/pause the
// belt). Kept here so the set lives in one place.
/// Fitness Machine Feature characteristic (read).
#[allow(dead_code)]
pub const FITNESS_MACHINE_FEATURE_UUID: &str = "00002acc-0000-1000-8000-00805f9b34fb";
/// Fitness Machine Control Point characteristic (write/indicate).
#[allow(dead_code)]
pub const FITNESS_MACHINE_CONTROL_POINT_UUID: &str = "00002ad9-0000-1000-8000-00805f9b34fb";
/// Training Status characteristic (read/notify).
#[allow(dead_code)]
pub const TRAINING_STATUS_UUID: &str = "00002ad3-0000-1000-8000-00805f9b34fb";

// ---- Flag bits --------------------------------------------------------------

const FLAG_MORE_DATA: u16 = 1 << 0;
const FLAG_AVG_SPEED: u16 = 1 << 1;
const FLAG_TOTAL_DISTANCE: u16 = 1 << 2;
const FLAG_INCLINATION: u16 = 1 << 3;
const FLAG_ELEVATION_GAIN: u16 = 1 << 4;
const FLAG_INST_PACE: u16 = 1 << 5;
const FLAG_AVG_PACE: u16 = 1 << 6;
const FLAG_EXPENDED_ENERGY: u16 = 1 << 7;
const FLAG_HEART_RATE: u16 = 1 << 8;
const FLAG_METABOLIC_EQUIV: u16 = 1 << 9;
const FLAG_ELAPSED_TIME: u16 = 1 << 10;
const FLAG_REMAINING_TIME: u16 = 1 << 11;
const FLAG_FORCE_POWER: u16 = 1 << 12;

// ---- Error ------------------------------------------------------------------

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FtmsError {
    /// The buffer ran out before a required field could be read.
    #[error("buffer too short: needed {expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
}

// ---- Decoded data -----------------------------------------------------------

/// Fully decoded Treadmill Data notification, in real-world units.
///
/// Every optional field is `None` unless its flag bit was set. `instantaneous_speed`
/// is always present per the spec (when the More Data bit is 0, which is the only
/// case a single self-contained notification appears in).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FtmsTreadmillData {
    /// Instantaneous belt speed, km/h.
    pub instantaneous_speed: Option<f64>,
    /// Average speed since session start, km/h.
    pub average_speed: Option<f64>,
    /// Total distance since session start, meters.
    pub total_distance_m: Option<u32>,
    /// Current inclination, percent (may be negative).
    pub inclination_pct: Option<f64>,
    /// Current ramp angle setting, degrees (may be negative).
    pub ramp_angle_deg: Option<f64>,
    /// Positive elevation gain since session start, meters.
    pub positive_elevation_gain_m: Option<f64>,
    /// Negative elevation gain since session start, meters.
    pub negative_elevation_gain_m: Option<f64>,
    /// Instantaneous pace, km/min.
    pub instantaneous_pace: Option<f64>,
    /// Average pace, km/min.
    pub average_pace: Option<f64>,
    /// Total expended energy, kcal.
    pub total_energy_kcal: Option<u32>,
    /// Energy per hour, kcal.
    pub energy_per_hour_kcal: Option<u32>,
    /// Energy per minute, kcal.
    pub energy_per_minute_kcal: Option<u32>,
    /// Current heart rate, bpm.
    pub heart_rate_bpm: Option<u8>,
    /// Metabolic equivalent (METs).
    pub metabolic_equivalent: Option<f64>,
    /// Elapsed time, seconds.
    pub elapsed_time_s: Option<u32>,
    /// Remaining time, seconds.
    pub remaining_time_s: Option<u32>,
    /// Force on the belt, Newtons (may be negative).
    pub force_on_belt_n: Option<i32>,
    /// Power output, Watts (may be negative).
    pub power_output_w: Option<i32>,
}

// ---- Little-endian cursor ---------------------------------------------------

/// Bounds-checked little-endian reader. Never index-panics: every read returns
/// `Err(TooShort{..})` when fewer bytes remain than the field requires.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Cursor { buf, pos: 0 }
    }

    fn need(&self, n: usize) -> Result<(), FtmsError> {
        if self.pos + n > self.buf.len() {
            Err(FtmsError::TooShort {
                expected: self.pos + n,
                got: self.buf.len(),
            })
        } else {
            Ok(())
        }
    }

    fn u8(&mut self) -> Result<u8, FtmsError> {
        self.need(1)?;
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, FtmsError> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn i16(&mut self) -> Result<i16, FtmsError> {
        Ok(self.u16()? as i16)
    }

    /// 24-bit little-endian unsigned integer.
    fn u24(&mut self) -> Result<u32, FtmsError> {
        self.need(3)?;
        let v = (self.buf[self.pos] as u32)
            | ((self.buf[self.pos + 1] as u32) << 8)
            | ((self.buf[self.pos + 2] as u32) << 16);
        self.pos += 3;
        Ok(v)
    }
}

// ---- Parser -----------------------------------------------------------------

/// Parse a Treadmill Data (0x2ACD) characteristic value into real-world units.
///
/// Reads the u16 LE flags first, then conditionally reads each optional field in
/// the exact order defined by the spec. Returns `FtmsError::TooShort` rather than
/// panicking when the buffer is truncated.
pub fn parse_treadmill_data(buf: &[u8]) -> Result<FtmsTreadmillData, FtmsError> {
    let mut cur = Cursor::new(buf);
    let flags = cur.u16()?;
    let mut out = FtmsTreadmillData::default();

    // Bit 0 (More Data): when 0, Instantaneous Speed is present in THIS frame.
    // When 1, the speed lives in a separate notification of the same record.
    if flags & FLAG_MORE_DATA == 0 {
        out.instantaneous_speed = Some(cur.u16()? as f64 * 0.01);
    }
    if flags & FLAG_AVG_SPEED != 0 {
        out.average_speed = Some(cur.u16()? as f64 * 0.01);
    }
    if flags & FLAG_TOTAL_DISTANCE != 0 {
        out.total_distance_m = Some(cur.u24()?);
    }
    if flags & FLAG_INCLINATION != 0 {
        out.inclination_pct = Some(cur.i16()? as f64 * 0.1);
        out.ramp_angle_deg = Some(cur.i16()? as f64 * 0.1);
    }
    if flags & FLAG_ELEVATION_GAIN != 0 {
        out.positive_elevation_gain_m = Some(cur.u16()? as f64 * 0.1);
        out.negative_elevation_gain_m = Some(cur.u16()? as f64 * 0.1);
    }
    if flags & FLAG_INST_PACE != 0 {
        out.instantaneous_pace = Some(cur.u8()? as f64 * 0.1);
    }
    if flags & FLAG_AVG_PACE != 0 {
        out.average_pace = Some(cur.u8()? as f64 * 0.1);
    }
    if flags & FLAG_EXPENDED_ENERGY != 0 {
        out.total_energy_kcal = Some(cur.u16()? as u32);
        out.energy_per_hour_kcal = Some(cur.u16()? as u32);
        out.energy_per_minute_kcal = Some(cur.u8()? as u32);
    }
    if flags & FLAG_HEART_RATE != 0 {
        out.heart_rate_bpm = Some(cur.u8()?);
    }
    if flags & FLAG_METABOLIC_EQUIV != 0 {
        out.metabolic_equivalent = Some(cur.u8()? as f64 * 0.1);
    }
    if flags & FLAG_ELAPSED_TIME != 0 {
        out.elapsed_time_s = Some(cur.u16()? as u32);
    }
    if flags & FLAG_REMAINING_TIME != 0 {
        out.remaining_time_s = Some(cur.u16()? as u32);
    }
    if flags & FLAG_FORCE_POWER != 0 {
        out.force_on_belt_n = Some(cur.i16()? as i32);
        out.power_output_w = Some(cur.i16()? as i32);
    }

    Ok(out)
}

// ---- The driver -------------------------------------------------------------

/// FTMS treadmills push Treadmill Data ~1 Hz; tolerate a quiet belt before
/// treating the link as dead.
const FTMS_IDLE_TIMEOUT: Duration = Duration::from_secs(15);

/// A belt reporting less than this is stopped: FTMS has no explicit status in
/// Treadmill Data, so the running flag is derived from the speed itself.
const RUNNING_THRESHOLD_KMH: f64 = 0.05;

/// A decoded record as a neutral SI sample. FTMS already speaks SI, so this is
/// mostly a field mapping; only the belt state is synthesised (from speed).
fn to_sample(d: &FtmsTreadmillData) -> Sample {
    Sample {
        speed_kmh: d.instantaneous_speed,
        distance_m: d.total_distance_m.map(|m| m as f64),
        steps: None, // FTMS has no step counter — absent, not zero
        duration_s: d.elapsed_time_s,
        calories: d.total_energy_kcal,
        state: d.instantaneous_speed.map(|kmh| {
            if kmh > RUNNING_THRESHOLD_KMH {
                BeltState::Running
            } else {
                BeltState::Standby
            }
        }),
    }
}

pub struct Ftms;

#[async_trait]
impl Driver for Ftms {
    fn id(&self) -> &'static str {
        "ftms"
    }

    fn matches(&self, adv: &Advertisement) -> bool {
        adv.services.contains(&super::sig_uuid(0x1826))
    }

    fn supports(&self, _adv: &Advertisement, gatt: &BTreeSet<Characteristic>) -> bool {
        gatt.iter().any(|c| c.uuid == super::sig_uuid(0x2acd))
    }

    async fn run(&self, link: &Peripheral, _host: &DriverHost<'_>, emit: Emit<'_>) -> Result<()> {
        let chars = link.characteristics();
        let data_char = chars
            .iter()
            .find(|c| c.uuid == super::sig_uuid(0x2acd))
            .cloned()
            .ok_or_else(|| anyhow!("Treadmill Data characteristic (2ACD) missing"))?;
        link.subscribe(&data_char).await?;
        let mut notifications = link.notifications().await?;

        loop {
            let frame = match tokio::time::timeout(FTMS_IDLE_TIMEOUT, notifications.next()).await {
                Ok(Some(n)) => n.value,
                Ok(None) => return Err(anyhow!("notification stream ended")),
                Err(_) => {
                    // A quiet belt is normal for FTMS (no push when stopped). Only
                    // treat the link as dead if the OS no longer considers us
                    // connected — recovers a stale handle without churning on
                    // legitimate pauses.
                    if !link.is_connected().await.unwrap_or(false) {
                        return Err(anyhow!("FTMS link dropped; reconnecting"));
                    }
                    continue;
                }
            };
            let data = match parse_treadmill_data(&frame) {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("FTMS decode error: {e}");
                    continue;
                }
            };
            emit(to_sample(&data));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// uuid16 helper builds the canonical 128-bit base form.
    #[test]
    fn uuid16_builds_base_form() {
        assert_eq!(uuid16(0x1826), FITNESS_MACHINE_SERVICE_UUID);
        assert_eq!(uuid16(0x2acd), TREADMILL_DATA_UUID);
        assert_eq!(uuid16(0x2acc), FITNESS_MACHINE_FEATURE_UUID);
    }

    /// Flags = 0x0000: More Data bit clear, no optional bits.
    /// Exercises only the always-present Instantaneous Speed field.
    #[test]
    fn speed_only_frame() {
        // flags=0x0000, speed=0x05DC -> 1500 * 0.01 = 15.00 km/h
        let buf = [0x00, 0x00, 0xDC, 0x05];
        let d = parse_treadmill_data(&buf).unwrap();
        assert!(approx(d.instantaneous_speed.unwrap(), 15.0));
        assert_eq!(d.average_speed, None);
        assert_eq!(d.total_distance_m, None);
    }

    /// Flags bits: 0 (More Data clear -> speed present), 2 (Total Distance),
    /// 7 (Expended Energy), 10 (Elapsed Time).
    /// flags = 0b0100_1000_0100 = 0x0484.
    #[test]
    fn speed_distance_energy_time_frame() {
        let flags: u16 = FLAG_TOTAL_DISTANCE | FLAG_EXPENDED_ENERGY | FLAG_ELAPSED_TIME;
        assert_eq!(flags, 0x0484);
        let mut buf = Vec::new();
        buf.extend_from_slice(&flags.to_le_bytes());
        // instantaneous speed: 0x0258 = 600 -> 6.00 km/h
        buf.extend_from_slice(&600u16.to_le_bytes());
        // total distance u24: 12345 m -> bytes 0x39 0x30 0x00
        buf.extend_from_slice(&[0x39, 0x30, 0x00]);
        // total energy 250 kcal
        buf.extend_from_slice(&250u16.to_le_bytes());
        // energy per hour 480 kcal
        buf.extend_from_slice(&480u16.to_le_bytes());
        // energy per minute 8 kcal
        buf.push(8u8);
        // elapsed time 3661 s
        buf.extend_from_slice(&3661u16.to_le_bytes());

        let d = parse_treadmill_data(&buf).unwrap();
        assert!(approx(d.instantaneous_speed.unwrap(), 6.0));
        assert_eq!(d.total_distance_m, Some(12345));
        assert_eq!(d.total_energy_kcal, Some(250));
        assert_eq!(d.energy_per_hour_kcal, Some(480));
        assert_eq!(d.energy_per_minute_kcal, Some(8));
        assert_eq!(d.elapsed_time_s, Some(3661));
        // Untouched fields stay None.
        assert_eq!(d.average_speed, None);
        assert_eq!(d.heart_rate_bpm, None);
    }

    /// Flags bits: 0 (More Data clear -> speed present), 1 (Average Speed),
    /// 3 (Inclination + Ramp Angle, both signed). Tests a NEGATIVE inclination.
    /// flags = 0b1010 = 0x000A.
    #[test]
    fn avg_speed_and_negative_inclination_frame() {
        let flags: u16 = FLAG_AVG_SPEED | FLAG_INCLINATION;
        assert_eq!(flags, 0x000A);
        let mut buf = Vec::new();
        buf.extend_from_slice(&flags.to_le_bytes());
        // inst speed 5.55 km/h -> 555
        buf.extend_from_slice(&555u16.to_le_bytes());
        // avg speed 5.00 km/h -> 500
        buf.extend_from_slice(&500u16.to_le_bytes());
        // inclination -3.5% -> raw -35 (sint16)
        buf.extend_from_slice(&(-35i16).to_le_bytes());
        // ramp angle +2.0 deg -> raw 20
        buf.extend_from_slice(&20i16.to_le_bytes());

        let d = parse_treadmill_data(&buf).unwrap();
        assert!(approx(d.instantaneous_speed.unwrap(), 5.55));
        assert!(approx(d.average_speed.unwrap(), 5.0));
        assert!(approx(d.inclination_pct.unwrap(), -3.5));
        assert!(approx(d.ramp_angle_deg.unwrap(), 2.0));
    }

    /// Flags bit 12 (Force on Belt + Power Output, both signed). Verifies signed
    /// decoding of a negative force/power and that the More Data bit being SET
    /// (bit 0) correctly suppresses the speed field.
    #[test]
    fn force_and_power_with_more_data_set() {
        let flags: u16 = FLAG_MORE_DATA | FLAG_FORCE_POWER;
        let mut buf = Vec::new();
        buf.extend_from_slice(&flags.to_le_bytes());
        // force -120 N, power -45 W
        buf.extend_from_slice(&(-120i16).to_le_bytes());
        buf.extend_from_slice(&(-45i16).to_le_bytes());

        let d = parse_treadmill_data(&buf).unwrap();
        // More Data set -> speed NOT present in this frame.
        assert_eq!(d.instantaneous_speed, None);
        assert_eq!(d.force_on_belt_n, Some(-120));
        assert_eq!(d.power_output_w, Some(-45));
    }

    /// Truncated buffer: flags claim Total Distance (u24) present but only 1
    /// distance byte follows. Must return Err, never panic.
    #[test]
    fn truncated_buffer_returns_err() {
        // flags with speed + total distance, but cut off mid-distance.
        let flags: u16 = FLAG_TOTAL_DISTANCE;
        let mut buf = Vec::new();
        buf.extend_from_slice(&flags.to_le_bytes());
        buf.extend_from_slice(&600u16.to_le_bytes()); // speed
        buf.push(0x39); // only 1 of 3 distance bytes
        let err = parse_treadmill_data(&buf).unwrap_err();
        assert!(matches!(err, FtmsError::TooShort { .. }));
    }

    /// Empty buffer cannot even read the mandatory flags field.
    #[test]
    fn empty_buffer_returns_err() {
        assert_eq!(
            parse_treadmill_data(&[]).unwrap_err(),
            FtmsError::TooShort {
                expected: 2,
                got: 0
            }
        );
    }

    // ---- Golden pins: decoded record → Sample → Telemetry ------------------
    //
    // These values were captured against the PRE-refactor `ftms_to_telemetry`
    // (the old direct FTMS→Telemetry mapping in ble.rs), so they pin that the
    // Sample detour puts byte-identical raw fields on the wire. The raw ints
    // are asserted exactly; the floats are derived from them by the shared
    // unit helpers and follow automatically.

    use crate::telemetry::Telemetry;

    fn telem(d: &FtmsTreadmillData, unit: &str) -> Telemetry {
        Telemetry::from_sample(&to_sample(d), unit)
    }

    fn approx12(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn golden_full_frame_kmh() {
        let d = FtmsTreadmillData {
            instantaneous_speed: Some(6.0),
            total_distance_m: Some(12345),
            elapsed_time_s: Some(3661),
            total_energy_kcal: Some(250),
            ..Default::default()
        };
        let t = telem(&d, "km/h");
        assert_eq!(t.speed_raw, Some(600));
        assert_eq!(t.status, Some(3));
        assert_eq!(t.status_name.as_deref(), Some("RUNNING"));
        assert!(t.is_running);
        assert_eq!(t.distance_raw, Some(1235));
        assert_eq!(t.distance_m, Some(12350));
        assert_eq!(t.duration_s, Some(3661));
        assert_eq!(t.calories, Some(250));
        assert_eq!(t.steps, None);
        assert!(approx12(t.speed_kmh.unwrap(), 6.0));
        assert!(approx12(t.speed_mph.unwrap(), 6.0 / 1.609344));
        assert!(approx12(t.distance_km.unwrap(), 12.35));
        assert!(approx12(t.distance_mi.unwrap(), 12.35 / 1.609344));
    }

    #[test]
    fn golden_speed_mph_console() {
        let d = FtmsTreadmillData {
            instantaneous_speed: Some(3.0),
            ..Default::default()
        };
        let t = telem(&d, "mph");
        assert_eq!(t.speed_raw, Some(186));
        assert_eq!(t.status, Some(3));
        assert!(approx12(t.speed_kmh.unwrap(), 2.99337984));
        assert!(approx12(t.speed_mph.unwrap(), 1.86));
    }

    #[test]
    fn golden_stopped_belt() {
        let d = FtmsTreadmillData {
            instantaneous_speed: Some(0.0),
            ..Default::default()
        };
        let t = telem(&d, "km/h");
        assert_eq!(t.speed_raw, Some(0));
        assert_eq!(t.status, Some(1));
        assert_eq!(t.status_name.as_deref(), Some("STANDBY"));
        assert!(!t.is_running);
    }

    #[test]
    fn golden_distance_only_frame() {
        let d = FtmsTreadmillData {
            total_distance_m: Some(100),
            ..Default::default()
        };
        let t = telem(&d, "km/h");
        assert_eq!(t.speed_raw, None);
        assert_eq!(t.status, None);
        assert_eq!(t.status_name, None);
        assert!(!t.is_running);
        assert_eq!(t.distance_raw, Some(10));
        assert_eq!(t.distance_m, Some(100));
    }

    /// The driver UUIDs and the documented string set must agree.
    #[test]
    fn driver_uuids_match_the_documented_set() {
        assert_eq!(
            crate::drivers::sig_uuid(0x1826).to_string(),
            FITNESS_MACHINE_SERVICE_UUID
        );
        assert_eq!(
            crate::drivers::sig_uuid(0x2acd).to_string(),
            TREADMILL_DATA_UUID
        );
    }
}
