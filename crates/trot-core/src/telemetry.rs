//! The presentation layer: the `Telemetry` shape the API serves, the shared
//! unit helpers, and the one place a device-neutral [`Sample`] is converted
//! into that shape.
//!
//! `Telemetry` is part of the public `/api` + `/ws` contract — its fields are
//! what Nowhere and every third-party client read. Its raw fields are
//! LifeSpan-shaped for historical reasons (`speed_raw` is hundredths of the
//! *displayed* unit, `distance_raw` is decameters, `status` carries the
//! LifeSpan console's status codes): the first supported treadmill defined the
//! wire format, and the contract froze it. Drivers never deal with any of
//! that — they emit SI-unit [`Sample`]s and the conversion happens here, once,
//! at the boundary (`Telemetry::from_sample`).

use crate::drivers::{BeltState, Sample};
use serde::Serialize;

// ---- Status codes -----------------------------------------------------------
//
// These numeric values are the LifeSpan console's wire codes. They leaked into
// the public API's `status` field before other drivers existed, so they are now
// frozen presentation vocabulary: every driver's `BeltState` maps onto them.

pub const STATUS_STANDBY: u8 = 0x01;
pub const STATUS_RUNNING: u8 = 0x03;
pub const STATUS_SUMMARY: u8 = 0x04;
pub const STATUS_PAUSED: u8 = 0x05;

pub fn status_name(value: u8) -> String {
    match value {
        STATUS_STANDBY => "STANDBY".to_string(),
        STATUS_RUNNING => "RUNNING".to_string(),
        STATUS_SUMMARY => "SUMMARY_SCREEN".to_string(),
        STATUS_PAUSED => "PAUSED".to_string(),
        v => format!("UNKNOWN_0x{v:02x}"),
    }
}

/// The `status` byte a [`BeltState`] serializes as. Inverse of the LifeSpan
/// driver's byte→state mapping — `drivers::lifespan` pins the round trip.
pub(crate) fn status_code(state: BeltState) -> u8 {
    match state {
        BeltState::Standby => STATUS_STANDBY,
        BeltState::Running => STATUS_RUNNING,
        BeltState::Summary => STATUS_SUMMARY,
        BeltState::Paused => STATUS_PAUSED,
        BeltState::Other(v) => v,
    }
}

// ---- Unit helpers -----------------------------------------------------------

pub const KMH_PER_MPH: f64 = 1.609344;

/// Meters represented by a raw distance value (stored in decameters).
pub fn distance_meters(raw: u32) -> u32 {
    raw * 10
}

/// km/h from a raw speed (hundredths of the displayed unit).
pub fn speed_kmh(raw: u32, display_unit: &str) -> f64 {
    let displayed = raw as f64 / 100.0;
    if display_unit == "km/h" {
        displayed
    } else {
        displayed * KMH_PER_MPH
    }
}

/// mph from a raw speed (hundredths of the displayed unit).
pub fn speed_mph(raw: u32, display_unit: &str) -> f64 {
    let displayed = raw as f64 / 100.0;
    if display_unit == "mph" {
        displayed
    } else {
        displayed / KMH_PER_MPH
    }
}

// ---- Telemetry --------------------------------------------------------------

/// Latest known state, assembled across device updates. This is the `state`
/// object on `/ws` and in `/api/state` — field names and shapes are contract.
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

    /// Convert a driver's SI-unit [`Sample`] into the presentation shape.
    ///
    /// This is the single place device-neutral values are re-encoded into the
    /// contract's legacy raw fields. The encodings are lossy in general (raw
    /// speed has centi-unit resolution, raw distance decameter resolution), but
    /// they are exact round trips for values that originated as raw fields —
    /// `raw → SI → raw` returns the original for the whole u16 wire range,
    /// which is what keeps the refactored drivers byte-identical on the wire.
    /// The round trip is pinned by tests below; if you change any rounding
    /// here, run them.
    pub fn from_sample(sample: &Sample, display_unit: &str) -> Self {
        let mut t = Telemetry::new(display_unit);
        t.steps = sample.steps;
        t.duration_s = sample.duration_s;
        t.calories = sample.calories;
        if let Some(kmh) = sample.speed_kmh {
            // speed_raw is hundredths of the *displayed* unit.
            let displayed = if display_unit == "mph" {
                kmh / KMH_PER_MPH
            } else {
                kmh
            };
            t.speed_raw = Some((displayed * 100.0).round().max(0.0) as u32);
        }
        if let Some(m) = sample.distance_m {
            // distance_raw is decameters (×10 = meters).
            t.distance_raw = Some((m / 10.0).round().max(0.0) as u32);
        }
        if let Some(state) = sample.state {
            let code = status_code(state);
            t.status = Some(code);
            t.status_name = Some(status_name(code));
        }
        t.refresh_derived();
        t
    }

    /// Recompute the derived fields from the raw fields + display unit.
    pub(crate) fn refresh_derived(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_units() {
        assert!((speed_kmh(60, "km/h") - 0.6).abs() < 1e-9);
        assert!((speed_mph(60, "km/h") - 0.6 / KMH_PER_MPH).abs() < 1e-4);
        assert!((speed_mph(60, "mph") - 0.6).abs() < 1e-9);
        assert!((speed_kmh(60, "mph") - 0.6 * KMH_PER_MPH).abs() < 1e-4);
    }

    /// The de-glitch accumulator in db.rs and every historic row depend on raw
    /// values surviving the refactor unchanged. A driver decodes `raw`, reports
    /// SI, and `from_sample` re-encodes — this pins that the re-encoding
    /// recovers the exact original raw for the entire u16 wire range, in both
    /// display units. If this breaks, stored step/distance/speed accounting
    /// silently drifts.
    #[test]
    fn speed_raw_round_trips_exactly_for_the_whole_wire_range() {
        for unit in ["km/h", "mph"] {
            for raw in 0..=65535u32 {
                let sample = Sample {
                    speed_kmh: Some(speed_kmh(raw, unit)),
                    ..Default::default()
                };
                let t = Telemetry::from_sample(&sample, unit);
                assert_eq!(t.speed_raw, Some(raw), "unit={unit} raw={raw}");
            }
        }
    }

    #[test]
    fn distance_raw_round_trips_exactly_for_the_whole_wire_range() {
        for raw in 0..=65535u32 {
            let sample = Sample {
                distance_m: Some(distance_meters(raw) as f64),
                ..Default::default()
            };
            let t = Telemetry::from_sample(&sample, "km/h");
            assert_eq!(t.distance_raw, Some(raw), "raw={raw}");
        }
    }

    /// Fields a device cannot report stay absent — `None` must not turn into 0
    /// anywhere on the way to the wire (FTMS has no step counter, and a zero
    /// there would corrupt day totals).
    #[test]
    fn absent_sample_fields_stay_absent() {
        let t = Telemetry::from_sample(&Sample::default(), "km/h");
        assert_eq!(t.steps, None);
        assert_eq!(t.speed_raw, None);
        assert_eq!(t.speed_kmh, None);
        assert_eq!(t.distance_raw, None);
        assert_eq!(t.distance_m, None);
        assert_eq!(t.duration_s, None);
        assert_eq!(t.calories, None);
        assert_eq!(t.status, None);
        assert_eq!(t.status_name, None);
        assert!(!t.is_running);
    }

    /// A hostile/buggy driver emitting negative SI values must clamp to 0, not
    /// wrap or panic (this mirrors the old FTMS path's `.max(0.0)`).
    #[test]
    fn negative_si_values_clamp_to_zero() {
        let sample = Sample {
            speed_kmh: Some(-3.0),
            distance_m: Some(-100.0),
            ..Default::default()
        };
        let t = Telemetry::from_sample(&sample, "km/h");
        assert_eq!(t.speed_raw, Some(0));
        assert_eq!(t.distance_raw, Some(0));
    }

    #[test]
    fn belt_states_serialize_as_the_contract_codes() {
        for (state, code, name) in [
            (BeltState::Standby, 0x01, "STANDBY"),
            (BeltState::Running, 0x03, "RUNNING"),
            (BeltState::Summary, 0x04, "SUMMARY_SCREEN"),
            (BeltState::Paused, 0x05, "PAUSED"),
            (BeltState::Other(0x7f), 0x7f, "UNKNOWN_0x7f"),
        ] {
            let sample = Sample {
                state: Some(state),
                ..Default::default()
            };
            let t = Telemetry::from_sample(&sample, "km/h");
            assert_eq!(t.status, Some(code));
            assert_eq!(t.status_name.as_deref(), Some(name));
            assert_eq!(t.is_running, state == BeltState::Running);
        }
    }
}
