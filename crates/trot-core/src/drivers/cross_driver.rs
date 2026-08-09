//! Cross-driver invariants at the [`Sample`] boundary.
//!
//! Every driver promises: a field the device did not report is `None` —
//! **absent, never `Some(0)`** — because a fabricated zero flows into the
//! stored day totals and corrupts them (`db.rs` treats reported zeros as
//! real odometer readings). Each driver's own suite touches this in
//! passing; this module asserts the invariant for the whole layer in one
//! table, so a new driver (or a refactor of an old one) that starts
//! zero-filling shows up as a failure here with the driver named.

use super::{BeltState, Sample};

/// Fields a [`Sample`] can carry, for the assertion messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Speed,
    Distance,
    Steps,
    Duration,
    Calories,
    State,
}

fn assert_absent(driver: &str, sample: &Sample, absent: &[Field]) {
    for f in absent {
        let is_none = match f {
            Field::Speed => sample.speed_kmh.is_none(),
            Field::Distance => sample.distance_m.is_none(),
            Field::Steps => sample.steps.is_none(),
            Field::Duration => sample.duration_s.is_none(),
            Field::Calories => sample.calories.is_none(),
            Field::State => sample.state.is_none(),
        };
        assert!(
            is_none,
            "{driver}: {f:?} must be absent (None), not fabricated — sample {sample:?}"
        );
    }
    // The system-level half: what is absent in the Sample stays absent in
    // the Telemetry the API serves (never re-materialised as zero).
    let t = crate::telemetry::Telemetry::from_sample(sample, "km/h");
    for f in absent {
        let stayed_none = match f {
            Field::Speed => t.speed_raw.is_none() && t.speed_kmh.is_none(),
            Field::Distance => t.distance_raw.is_none() && t.distance_m.is_none(),
            Field::Steps => t.steps.is_none(),
            Field::Duration => t.duration_s.is_none(),
            Field::Calories => t.calories.is_none(),
            Field::State => t.status.is_none() && t.status_name.is_none(),
        };
        assert!(
            stayed_none,
            "{driver}: {f:?} was absent in the Sample but present in Telemetry"
        );
    }
}

fn hx(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Every driver, one minimal "device did not report X" frame each, asserted
/// through its real `to_sample` conversion.
#[test]
fn no_driver_turns_an_unreported_field_into_some_zero() {
    use Field::*;

    // LifeSpan: the reader accumulates one field per poll; after a single
    // steps response everything else is still unknown.
    {
        let mut r = super::lifespan::Reader::new();
        r.feed(super::lifespan::OPCODE_STEPS, &hx("a1 aa 00 23 00 00"))
            .unwrap();
        let s = super::lifespan::to_sample(r.state(), "km/h");
        assert_eq!(s.steps, Some(35));
        assert_absent(
            "lifespan",
            &s,
            &[Speed, Distance, Duration, Calories, State],
        );
    }

    // FTMS: a speed-only frame reports nothing but speed (and the
    // speed-derived state). Standard FTMS has no step counter at all.
    {
        let d = super::ftms::parse_treadmill_data(&[0x00, 0x00, 0xDC, 0x05]).unwrap();
        let s = super::ftms::to_sample(&d);
        assert_eq!(s.speed_kmh, Some(15.0));
        assert_absent("ftms", &s, &[Distance, Steps, Duration, Calories]);

        // A distance-only continuation frame (More Data set): even the
        // state must stay absent — no speed, no derived state.
        let mut buf = vec![0x05, 0x00]; // More Data | Total Distance
        buf.extend_from_slice(&[0xE8, 0x03, 0x00]); // 1000 m
        let d = super::ftms::parse_treadmill_data(&buf).unwrap();
        let s = super::ftms::to_sample(&d);
        assert_eq!(s.distance_m, Some(1000.0));
        assert_absent("ftms", &s, &[Speed, Steps, Duration, Calories, State]);
    }

    // KingSmith WiLink: the status frame has no energy field — calories are
    // absent on EVERY frame, however complete the rest is.
    {
        let f = hx("f8 a2 01 3c 01 00 02 2a 00 00 4f 00 03 d1 b4 00 00 00 e3 fd");
        let s =
            super::kingsmith_wilink::to_sample(&super::kingsmith_wilink::parse_status(&f).unwrap());
        assert_eq!(s.steps, Some(977));
        assert_absent("kingsmith-wilink", &s, &[Calories]);
    }

    // Urevo: the 6-byte deep-standby frame carries only the state.
    {
        let s =
            super::urevo::to_sample(&super::urevo::parse_status(&hx("02 51 00 00 09 03")).unwrap());
        assert_eq!(s.state, Some(BeltState::Standby));
        assert_absent("urevo", &s, &[Speed, Distance, Steps, Duration, Calories]);
    }

    // Sperax: the packet verifiably carries steps and speed and NOTHING
    // else — distance in particular must never be integrated from speed.
    {
        let mut f = vec![0u8; 24];
        f[0] = 0xF5;
        f[1] = 24;
        f[15] = 0x02;
        f[16] = 0x01;
        f[17] = 27;
        f[23] = 0xFA;
        let s = super::sperax::to_sample(&super::sperax::parse_status(&f).unwrap());
        assert_eq!(s.steps, Some(513));
        assert_absent("sperax", &s, &[Distance, Duration, Calories]);
    }

    // FitShow: state-only statuses (NORMAL here) carry no counters; and on
    // a metric console even a counter frame reports no distance (scale
    // unverified) and never calories (scale conflict).
    {
        let normal = super::fitshow::build_frame(&[super::fitshow::MSG_STATUS, 0x00]);
        let s = super::fitshow::to_sample(
            &super::fitshow::parse_status(&normal).unwrap(),
            super::fitshow::WireUnit::Metric,
        );
        assert_eq!(s.state, Some(BeltState::Standby));
        assert_absent(
            "fitshow (state-only)",
            &s,
            &[Speed, Distance, Steps, Duration, Calories],
        );

        let running = hx("02 51 03 0e 00 45 01 70 04 fd 00 6b 01 00 00 fb 03");
        let s = super::fitshow::to_sample(
            &super::fitshow::parse_status(&running).unwrap(),
            super::fitshow::WireUnit::Metric,
        );
        assert_eq!(s.steps, Some(363));
        assert_absent("fitshow (metric counters)", &s, &[Distance, Calories]);

        // Imperial consoles report no distance either: the wire scale is
        // inferred from the USER'S display preference, and a preference must
        // not determine a stored cumulative counter (fitshow.rs module docs,
        // Units section). Only a capture pinning the scale to an advertised
        // name may wire this up.
        let s = super::fitshow::to_sample(
            &super::fitshow::parse_status(&running).unwrap(),
            super::fitshow::WireUnit::Imperial,
        );
        assert_absent("fitshow (imperial counters)", &s, &[Distance, Calories]);
    }

    // KingSmith props: the pad reports any subset of keys per line; keys
    // never seen stay absent.
    {
        let mut state = super::kingsmith_props::PadState::default();
        for (k, v) in super::kingsmith_props::parse_props("props CurrentSpeed 2.0 spm 96").unwrap()
        {
            state.apply(k, v);
        }
        let s = state.to_sample();
        assert_eq!(s.speed_kmh, Some(2.0));
        assert_absent(
            "kingsmith-props",
            &s,
            &[Distance, Steps, Duration, Calories, State],
        );
    }

    // PitPat is the one driver with no absent fields: its ≥31-byte status
    // frame genuinely reports every counter, so zeros there are REPORTED
    // values (the real idle capture reads all-zero counters on a pad that
    // has done nothing). Assert that reported shape stays fully populated —
    // the inverse boundary of this invariant.
    {
        let idle = hx(
            "68 34 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
             00 00 2a 1b 00 17 70 00 05 00 74 6c 4b 61 31 39 31 66 55 70 54 73 \
             73 36 30 33 0e 00 40 43",
        );
        let s = super::pitpat::to_sample(&super::pitpat::parse_status(&idle).unwrap());
        assert_eq!(s.steps, Some(0), "a reported zero is data, not absence");
        assert_eq!(s.speed_kmh, Some(0.0));
        assert_eq!(s.state, Some(BeltState::Standby));
    }
}
