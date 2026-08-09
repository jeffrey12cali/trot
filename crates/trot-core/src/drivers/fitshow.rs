//! FitShow driver — the white-label `02 … 03` treadmill protocol behind many
//! rebadged brands (`FS-…` walking pads and full-size treadmills, NoblePro,
//! Tunturi T80, WinFita, and a long tail of OEM units on the same FitShow
//! BLE module).
//!
//! ## Sources (see THIRD-PARTY-NOTICES.md)
//!
//! * **cagnulein/qdomyos-zwift** (GPL-3.0, same license as Trot) —
//!   `src/devices/fitshowtreadmill/fitshowtreadmill.{cpp,h}`: the primary
//!   source. Frame envelope and XOR trailer (`checkIncomingPacket` /
//!   `writePayload`), the status-response field map in both byte orders
//!   (standard and "anyrun" — see below), the status-code table, the three
//!   transport layouts, and the advertised-name matcher with its carve-outs
//!   (`src/devices/bluetooth.cpp`).
//! * **sstjohn/milltender** (AGPL-3.0) — `milltender.py` +
//!   `phase0/fitshow_probe.py`: the only implementation whose telemetry path
//!   is verified on real hardware (a "TX6 Glow-Up" walking pad on a FitShow
//!   FS-BT-D2 module, advertising `FS-3D6CD7`). Establishes on hardware: the
//!   FFF0 transport with **LifeSpan-shaped roles** (write FFF2, notify FFF1),
//!   that the status stream answers a bare `02 51 51 03` poll with **no
//!   prior login or init write of any kind** (its production poll loop and
//!   its explicitly read-only probe mode both write only that frame), the
//!   ≥12-byte status-payload floor, inbound XOR validation, and the
//!   imperial wire scales on that hardware (0.1 mph speed, 0.001 mile
//!   distance). Protocol knowledge only is ported — no code.
//! * **aradix85/fitshow-treadmill-accessible** (MIT) — `docs/PROTOCOL.md`:
//!   documents that newer FitShow modules (FS-BT-C1, advertising `FS-…`)
//!   speak plain standard FTMS, with the vendor `FFF0` service reduced to a
//!   notify-only side channel. That finding is why `supports()` verifies
//!   the vendor *write* role and lets FTMS-only FitShow hardware fall
//!   through to the FTMS driver.
//! * **FitShow's own OEM protocol document** (treadmill protocol v1.1,
//!   Chinese; publicly mirrored at `limdongkyu/fitshow-device-protocol`,
//!   no license) — used to *verify* the envelope, the little-endian rule,
//!   the field order and the status-code table against the implementations
//!   above. No text or code was taken from it; every ported byte comes from
//!   the licensed sources. It settles two things the implementations alone
//!   could not: the spec's example frames pin the XOR trailer on multi-byte
//!   payloads, and the spec declares little-endian as the protocol-wide
//!   rule (which is what makes the big-endian "anyrun" variant a
//!   non-conforming offshoot rather than an equally-supported dialect).
//!
//! ## The transport is not one thing
//!
//! Three characteristic pairs carry this one protocol; `supports()` probes
//! them in qdomyos' effective preference order ([`select_transport`]):
//!
//! | Variant | Service | Write | Notify | Verified |
//! |---|---|---|---|---|
//! | FFF0 | `FFF0` | **`FFF2`** | **`FFF1`** | hardware (milltender) |
//! | AE00 | `AE00` | `AE01` | `AE02` | qdomyos (NoblePro units) |
//! | FFE0 | `FFE0` | `FFE1` | `FFE4` | qdomyos source only |
//!
//! The FFF0 variant is the contested block **with exactly LifeSpan's role
//! arrangement** — notify FFF1, write FFF2, byte-identical to the
//! LifeSpan/Urevo/Sperax shape (unlike Deerrun, whose swapped roles the
//! property checks catch). Role verification therefore cannot keep a
//! FitShow off a LifeSpan console or vice versa; the advertised-name gate
//! is the whole adjudication, exactly as for Urevo and Sperax, and
//! `drivers/mod.rs` pins it: a recognised FitShow name claims the shape, a
//! LifeSpan name never reaches this driver, and a nameless FFF1/FFF2
//! device is left for the deliberate LifeSpan fallback. On the FFE0 pair
//! note the notify characteristic is `FFE4`, not the usual `FFE1` — the
//! role checks enforce that too.
//!
//! ## Names, and who keeps FTMS
//!
//! The matcher is ported from qdomyos' device router, carve-outs included:
//!
//! * `FS-` (minus `FS-YK-`, an FTMS exercise-bike family), `TR510-T` and
//!   `TUNTURI T80-` are claimed even when the device also exposes real
//!   FTMS — qdomyos hard-codes exactly these as the never-switch-to-FTMS
//!   set, and the native protocol reports steps where FTMS cannot.
//!   (`TUNTURI T60-`/`T90-` are plain FTMS and do not match.)
//! * `NOBLEPRO CONNECT`, `WINFITA`, `SW-BLE`, `BF70` and the `SW` rule
//!   below are claimed **only when the GATT table has no FTMS Treadmill
//!   Data** — for these qdomyos either routes to FTMS outright or
//!   force-switches to it once it sees service 0x1826, so FTMS is the
//!   protocol that demonstrably works on those units.
//! * The real `SW` rule is narrower than the prefix suggests: exactly 14
//!   characters, no `(` or `)`, and no FTMS (qdomyos'
//!   `startsWith("SW") && length()==14 && !contains('(')…`). Still broad —
//!   a 14-character `SW…` anything matches at scan time — but that is the
//!   deployed upstream rule, and `supports()` additionally requires a
//!   verified FitShow transport before a connection is claimed.
//!
//! ## Interaction model, and what we send
//!
//! **Request/response polling.** Write the status query `02 51 51 03`; the
//! device answers with a status frame on the notify characteristic. That
//! query is **the only frame this driver ever writes** — a one-byte
//! command, no payload, nothing to set.
//!
//! The verb boundary in this protocol is the **command family byte**:
//! `0x50` (info) / `0x51` (status) / `0x52` (data) are read families whose
//! requests carry no settable values; **`0x53` is the control family** —
//! ready/start, set-speed, set-incline, stop, pause AND the "user login"
//! all live under it — and no frame this driver builds may carry it (a
//! test pins the write set, frames and count).
//!
//! **The login frame is deliberately dropped.** qdomyos opens with
//! `53 00 <user-id…> <weight> <height>` — a control-family write of
//! settable user data (the device uses it for its own calorie estimate).
//! That is exactly the kind of uncharacterised value-carrying init write
//! four drivers before this one refused to port — and here the evidence is
//! better than usual: milltender's production loop polls real hardware
//! with the bare status query and no login whatsoever, and the OEM spec
//! files the user write under the pre-workout *control* flow with no stated
//! connection to status polling. qdomyos' remaining init writes are also
//! dropped: its `50 00 <current date+time>` "model query" smuggles a
//! clock-set payload the spec's own model query does not have, and its
//! range/date/odometer queries (`50 01..04`) feed values this driver never
//! uses. The login frame's bytes survive below only as checksum vectors.
//!
//! ## Status frame (little-endian; payload offsets after the 0x02 header)
//!
//! ```text
//! byte  0:     0x51 (echoes the command)
//! byte  1:     status code (see [`belt_state`])
//! -- when status ∈ {END, RUNNING, STOPPING, PAUSED} and payload ≥ 12: --
//! byte  2:     belt speed, 0.1 unit (km/h or mph — see "Units" below)
//! byte  3:     incline, signed (unparsed — no Sample field)
//! bytes 4..6:  elapsed time, seconds (u16 LE)
//! bytes 6..8:  distance (u16 LE; 0.001 mile on the imperial hardware —
//!              see "Units")
//! bytes 8..10: calories (u16 LE; **unparsed** — scale conflict, below)
//! bytes 10..12: steps (u16 LE)
//! byte  12:    heart rate (unparsed — no Sample field)
//! byte  13:    program segment (unparsed)
//! ```
//!
//! Other statuses carry state-specific bytes (countdown seconds under
//! START, an error code under ERROR, a safety/sleep code under DISABLE) —
//! all state-only here: counters come out absent, not zero.
//!
//! ## Units — the honest version
//!
//! The wire unit is device-dependent and no implementation reads a
//! discriminator: the OEM spec defines speed as 0.1 km/h and flags
//! imperial devices via a config bit nobody has ever validated, qdomyos
//! punts to a user setting ("the treadmill send the speed in miles
//! always", contradicted by its own metric default), and milltender's US
//! walking pad demonstrably wires 0.1 mph / 0.001 mile. Trot's analog of
//! qdomyos' user setting is `host.display_unit` — the unit the user's
//! console displays — so this driver scales speed by it (the LifeSpan
//! precedent: the wire unit follows the console's unit setting):
//! `"km/h"` → 0.1 km/h per unit; anything else → 0.1 mph per unit
//! (matching `telemetry.rs`'s same-string convention).
//!
//! Two fields are deliberately not reported:
//!
//! * **Distance on metric consoles.** The imperial scale (0.001 mi,
//!   ×1.609344 m) is hardware-verified by milltender; the metric scale is
//!   not verified anywhere — qdomyos decodes ÷10 (0.1 km) but the value is
//!   dead code (it integrates speed over wall-clock instead), and the
//!   symmetric guess would be 0.001 km, a 100× disagreement. A distance
//!   that might be 100× wrong would corrupt stored day totals, so on
//!   metric consoles `distance_m` stays `None` until someone sends a
//!   capture. (Imperial consoles get the verified scale.)
//! * **Calories, on every device.** qdomyos reads the field as whole kcal
//!   but never uses the value; milltender — the only implementation that
//!   consumes it — divides by 10 on its hardware. A 10× conflict between
//!   sources, resolvable only with a capture: absent, not wrong.
//!
//! ## The "anyrun" variant
//!
//! qdomyos supports a user-toggled `anyrun` mode in which the *same
//! offsets* read big-endian and the elapsed field is (minutes, seconds)
//! instead of a u16 of seconds — a non-conforming offshoot of the
//! little-endian OEM spec, presumably the AnyRun-app OEM family. There is
//! **no known on-wire discriminator and no known advertised name** for
//! these devices (upstream ships it purely as a settings toggle, off by
//! default), so this driver decodes the spec-conforming standard order
//! only; [`parse_status_with_order`] implements both orders so tests can
//! pin the difference in both directions, and a real anyrun capture is
//! what it would take to wire the variant up safely. Until then an anyrun
//! device would decode with byte-swapped counters — the checksum cannot
//! catch it (both orders are valid frames) — which is the known residual
//! risk this module accepts and documents rather than hides.
//!
//! No belt state is derived from speed: the status byte is authoritative,
//! so speed-unit ambiguity never leaks into session detection.

use super::util::CommandSpacer;
use super::{Advertisement, BeltState, Driver, DriverHost, Emit, Sample};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use btleplug::api::{CharPropFlags, Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use futures::{FutureExt, StreamExt};
use std::collections::BTreeSet;
use std::time::Duration;
use uuid::Uuid;

// ---- UUIDs: the three transport variants ------------------------------------

/// FFF0 — the contested block, with exactly LifeSpan's role arrangement
/// (notify FFF1, write FFF2). Hardware-verified by milltender's FS-BT-D2.
pub const FFF0_NOTIFY_UUID: Uuid = super::sig_uuid(0xfff1);
pub const FFF0_WRITE_UUID: Uuid = super::sig_uuid(0xfff2);

/// AE00 — the service qdomyos labels "nobleproconnect"; write AE01, notify
/// AE02.
pub const AE00_WRITE_UUID: Uuid = super::sig_uuid(0xae01);
pub const AE00_NOTIFY_UUID: Uuid = super::sig_uuid(0xae02);

/// FFE0 — the generic UART-style block. Write FFE1, notify **FFE4** (not
/// FFE1 — qdomyos' characteristic pick is explicit about this).
pub const FFE0_WRITE_UUID: Uuid = super::sig_uuid(0xffe1);
pub const FFE0_NOTIFY_UUID: Uuid = super::sig_uuid(0xffe4);

/// FTMS Treadmill Data — presence decides the FTMS-preferred names' fate.
const FTMS_TREADMILL_DATA_UUID: Uuid = super::sig_uuid(0x2acd);

// ---- Advertised names -------------------------------------------------------
//
// All from qdomyos' device router (src/devices/bluetooth.cpp) and its
// fitshowtreadmill driver's own name fixes. Comparison is case-insensitive
// on the trimmed name (house convention; qdomyos compares some of these
// case-sensitively, but every observed real name is upper-case anyway).

/// Names claimed even when the device also exposes real FTMS — qdomyos'
/// never-switch-to-FTMS set (`fs_connected` / `tunturi_t80_connected`), and
/// the native protocol reports steps where FTMS cannot.
pub const ADV_NAME_PREFIXES_NATIVE: &[&str] = &["FS-", "TR510-T", "TUNTURI T80-"];

/// Names claimed only when the GATT table has no FTMS Treadmill Data: for
/// these qdomyos routes to FTMS when service 0x1826 is present (NoblePro,
/// the SW rule) or force-switches to it after connecting (WinFita, SW-BLE,
/// BF70), so FTMS is the protocol that demonstrably works on those units.
pub const ADV_NAME_PREFIXES_FTMS_PREFERRED: &[&str] =
    &["NOBLEPRO CONNECT", "WINFITA", "SW-BLE", "BF70"];

/// FitShow-named devices that are NOT treadmills: `FS-YK-` is an FTMS
/// exercise-bike family (qdomyos routes it to a bike driver). Trot reads
/// treadmills only.
pub const ADV_NAME_EXCLUDE_PREFIXES: &[&str] = &["FS-YK-"];

/// The `SW` name rule, exactly as qdomyos deploys it: starts with `SW`,
/// exactly this many characters, and no parentheses. (Belongs to the
/// FTMS-preferred class; `!deviceHasService(0x1826)` is part of the
/// upstream rule.)
pub const SW_NAME_LEN: usize = 14;

// ---- Wire constants ---------------------------------------------------------

/// Every frame, both directions: `02 <cmd> <data…> <xor> 03`, where the
/// trailer is the XOR of everything between header and trailer.
pub const FRAME_HEADER: u8 = 0x02;
pub const FRAME_FOOTER: u8 = 0x03;
/// Header + one command byte + trailer + footer.
pub const MIN_FRAME_LEN: usize = 4;

/// Read family: device info (model, speed/incline ranges, odometer).
pub const MSG_INFO: u8 = 0x50;
/// Read family: current status + live counters. The one family we poll.
pub const MSG_STATUS: u8 = 0x51;
/// Read family: sport-data / workout-info queries.
pub const MSG_DATA: u8 = 0x52;
/// **The control family — the actuation boundary.** Ready/start, set-speed,
/// set-incline, stop, pause and the user-data login all carry this byte.
/// No frame this driver builds may ever use it; the write-set test pins it.
pub const MSG_CONTROL: u8 = 0x53;

/// The only frame this driver ever writes: the bare status query
/// (`XOR of [0x51]` = `0x51`). Byte-identical to milltender's poll and
/// qdomyos' steady-state poll.
pub const STATUS_QUERY: [u8; 4] = [FRAME_HEADER, MSG_STATUS, MSG_STATUS, FRAME_FOOTER];

// Status codes (qdomyos' table, confirmed against the OEM spec; STUDY is
// qdomyos-only and the spec's READY is absent from qdomyos — both handled).
pub const STATUS_NORMAL: u8 = 0x00;
pub const STATUS_END: u8 = 0x01;
pub const STATUS_START: u8 = 0x02;
pub const STATUS_RUNNING: u8 = 0x03;
pub const STATUS_STOPPING: u8 = 0x04;
pub const STATUS_ERROR: u8 = 0x05;
pub const STATUS_DISABLED: u8 = 0x06;
pub const STATUS_STUDY: u8 = 0x07;
pub const STATUS_READY: u8 = 0x09;
pub const STATUS_PAUSED: u8 = 0x0A;

/// The statuses whose frames carry the counter block (qdomyos reads
/// counters for exactly these; the OEM spec's table agrees).
pub const COUNTER_STATUSES: [u8; 4] = [STATUS_END, STATUS_RUNNING, STATUS_STOPPING, STATUS_PAUSED];

/// Counters need payload offsets up to 11; milltender's hardware-verified
/// floor (its device usually appends heart-rate and segment bytes on top).
pub const COUNTER_PAYLOAD_MIN_LEN: usize = 12;

/// km/h per 0.1 km/h wire unit (metric consoles).
pub const KMH_PER_RAW_SPEED_METRIC: f64 = 0.1;
/// km/h per 0.1 mph wire unit (imperial consoles; milltender's hardware).
pub const KMH_PER_RAW_SPEED_IMPERIAL: f64 = 0.160_934_4;
/// Meters per 0.001 mile wire unit (imperial consoles; milltender's
/// hardware — the only verified distance scale in any source).
pub const METERS_PER_RAW_DISTANCE_IMPERIAL: f64 = 1.609_344;

/// Poll cadence. qdomyos polls at 200 ms, the vendor app at ~3 Hz,
/// milltender at 1 Hz on real hardware; half a second splits the
/// difference and nothing needs speed updates faster than that.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
/// Consecutive unanswered polls before the link is declared dead — same
/// rationale as the LifeSpan/WiLink/Sperax drivers: macOS can hold a stale
/// handle open with no disconnect event.
const MAX_DEAD_POLLS: u32 = 15;

// ---- Frame building ---------------------------------------------------------

/// `02 <payload…> <xor of payload> 03`.
pub fn build_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.push(FRAME_HEADER);
    frame.extend_from_slice(payload);
    frame.push(super::util::checksum_xor(payload));
    frame.push(FRAME_FOOTER);
    frame
}

// ---- Frame parsing ----------------------------------------------------------

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("expected at least {MIN_FRAME_LEN} bytes, got {0}")]
    BadLength(usize),
    #[error("bad prefix 0x{0:02x}")]
    BadPrefix(u8),
    #[error("missing 0x03 terminator")]
    BadTerminator,
    #[error("checksum mismatch: computed 0x{computed:02x}, frame carries 0x{found:02x}")]
    BadChecksum { computed: u8, found: u8 },
    /// A well-formed frame of another read family (an info or data reply).
    /// Expected traffic on some firmware — skip it, don't warn.
    #[error("not a status frame (command family 0x{0:02x})")]
    NotStatus(u8),
}

/// Validate the envelope (header, footer, XOR trailer) and return the
/// payload between them. Both qdomyos and milltender validate inbound
/// frames exactly like this, so strictness here is upstream parity, not
/// extra caution.
pub fn frame_payload(frame: &[u8]) -> Result<&[u8], ProtocolError> {
    if frame.len() < MIN_FRAME_LEN {
        return Err(ProtocolError::BadLength(frame.len()));
    }
    if frame[0] != FRAME_HEADER {
        return Err(ProtocolError::BadPrefix(frame[0]));
    }
    if frame[frame.len() - 1] != FRAME_FOOTER {
        return Err(ProtocolError::BadTerminator);
    }
    let payload = &frame[1..frame.len() - 2];
    let computed = super::util::checksum_xor(payload);
    let found = frame[frame.len() - 2];
    if computed != found {
        return Err(ProtocolError::BadChecksum { computed, found });
    }
    Ok(payload)
}

/// The two byte orders this protocol ships in. The driver decodes
/// [`Standard`](ByteOrder::Standard) only — see the module docs for why the
/// anyrun variant stays un-wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// Little-endian, elapsed time a u16 of seconds — the OEM spec's rule
    /// and qdomyos' default.
    Standard,
    /// Big-endian counters and a (minutes, seconds) elapsed pair — the
    /// qdomyos `fitshow_anyrun` toggle. Implemented for tests and for the
    /// day a discriminator is found; never selected by the driver.
    AnyRun,
}

/// The counter block carried by END/RUNNING/STOPPING/PAUSED frames, as the
/// wire reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counters {
    /// Belt speed in 0.1 display units (km/h or mph — see the module docs).
    pub speed_raw: u8,
    /// Elapsed time, seconds.
    pub duration_s: u32,
    /// Distance in wire units (0.001 mile on the verified imperial
    /// hardware; metric scale unverified — see the module docs).
    pub distance_raw: u32,
    /// Calories in wire units. **Not surfaced** — the 1-vs-0.1 kcal scale
    /// conflict is unresolved (module docs).
    pub calories_raw: u32,
    /// Cumulative steps (unit-free — the one counter safe on any device).
    pub steps: u32,
}

/// One decoded status frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    /// Status code (see [`belt_state`]).
    pub state: u8,
    /// Present on counter-status frames; `None` on NORMAL/START/ERROR/
    /// DISABLE/READY frames (and on short frames), never zero-filled.
    pub counters: Option<Counters>,
}

fn u16_le(payload: &[u8], at: usize) -> u32 {
    (payload[at] as u32) | ((payload[at + 1] as u32) << 8)
}

fn u16_be(payload: &[u8], at: usize) -> u32 {
    ((payload[at] as u32) << 8) | payload[at + 1] as u32
}

/// Parse a notification in an explicit byte order. Pure function of the
/// bytes; never panics on malformed input.
pub fn parse_status_with_order(frame: &[u8], order: ByteOrder) -> Result<Status, ProtocolError> {
    let payload = frame_payload(frame)?;
    if payload[0] != MSG_STATUS {
        return Err(ProtocolError::NotStatus(payload[0]));
    }
    if payload.len() < 2 {
        return Err(ProtocolError::BadLength(frame.len()));
    }
    let state = payload[1];
    let counters = (COUNTER_STATUSES.contains(&state) && payload.len() >= COUNTER_PAYLOAD_MIN_LEN)
        .then(|| {
            let word = match order {
                ByteOrder::Standard => u16_le,
                ByteOrder::AnyRun => u16_be,
            };
            Counters {
                speed_raw: payload[2],
                duration_s: match order {
                    ByteOrder::Standard => u16_le(payload, 4),
                    // The anyrun elapsed pair is (minutes, seconds).
                    ByteOrder::AnyRun => payload[4] as u32 * 60 + payload[5] as u32,
                },
                distance_raw: word(payload, 6),
                calories_raw: word(payload, 8),
                steps: word(payload, 10),
            }
        });
    Ok(Status { state, counters })
}

/// Parse a notification in the spec-conforming standard order — the only
/// order the driver uses.
pub fn parse_status(frame: &[u8]) -> Result<Status, ProtocolError> {
    parse_status_with_order(frame, ByteOrder::Standard)
}

/// The wire's status code as a neutral [`BeltState`].
///
/// Per-value provenance (qdomyos' handling + the OEM spec's state table;
/// milltender's session logic agrees on 0/1/3/4/10):
///
/// * `0x00` NORMAL (standby) → `Standby`.
/// * `0x01` END — stopped, console not yet back to standby, final counters
///   still on screen: the post-workout summary → `Summary`.
/// * `0x02` START — the 3-2-1 countdown; the workout is starting →
///   `Running` (the PitPat/Urevo countdown call).
/// * `0x03` RUNNING → `Running`.
/// * `0x04` STOPPING — decelerating toward a stop or a pause; the belt is
///   factually still moving and the counters still advance → `Running`
///   (the terminal state arrives seconds later and closes the session).
/// * `0x05` ERROR — machine fault, belt stopped → `Standby`, deliberately
///   NOT `Other(5)`: the raw passthrough would collide with the API
///   contract's PAUSED code and present a faulted machine as paused (the
///   WiLink state-5 precedent).
/// * `0x06` DISABLE — safety key removed or console asleep → `Standby`.
/// * `0x09` READY (spec only) — armed to start, belt stopped → `Standby`.
/// * `0x0A` PAUSED → `Paused`.
///
/// `0x07` STUDY (qdomyos only, uncharacterised — possibly belt
/// calibration) and everything else pass through as [`BeltState::Other`].
pub(crate) fn belt_state(state: u8) -> BeltState {
    match state {
        STATUS_NORMAL => BeltState::Standby,
        STATUS_END => BeltState::Summary,
        STATUS_START | STATUS_RUNNING | STATUS_STOPPING => BeltState::Running,
        STATUS_ERROR => BeltState::Standby,
        STATUS_DISABLED | STATUS_READY => BeltState::Standby,
        STATUS_PAUSED => BeltState::Paused,
        other => BeltState::Other(other),
    }
}

/// The wire unit this console speaks, resolved from `host.display_unit`
/// with the same string convention as `telemetry.rs` (`"km/h"` exact →
/// metric, anything else → mph). See the module docs for why the console's
/// display unit is the best available discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireUnit {
    Metric,
    Imperial,
}

pub fn wire_unit(display_unit: &str) -> WireUnit {
    if display_unit == "km/h" {
        WireUnit::Metric
    } else {
        WireUnit::Imperial
    }
}

/// A [`Status`] as a neutral SI sample. Speed scales by the console unit;
/// distance is reported only where its scale is verified (imperial);
/// calories are never reported (scale conflict) — all per the module docs.
/// The state comes from the status byte alone, never from speed.
pub(crate) fn to_sample(s: &Status, unit: WireUnit) -> Sample {
    match &s.counters {
        Some(c) => Sample {
            speed_kmh: Some(match unit {
                WireUnit::Metric => c.speed_raw as f64 * KMH_PER_RAW_SPEED_METRIC,
                WireUnit::Imperial => c.speed_raw as f64 * KMH_PER_RAW_SPEED_IMPERIAL,
            }),
            distance_m: match unit {
                WireUnit::Metric => None, // unverified scale — absent, not wrong
                WireUnit::Imperial => {
                    Some(c.distance_raw as f64 * METERS_PER_RAW_DISTANCE_IMPERIAL)
                }
            },
            steps: Some(c.steps),
            duration_s: Some(c.duration_s),
            calories: None, // 1-vs-0.1 kcal source conflict — absent, not wrong
            state: Some(belt_state(s.state)),
        },
        None => Sample {
            // NORMAL/START/ERROR/DISABLE/READY frames carry no counters:
            // everything is absent, not zero.
            state: Some(belt_state(s.state)),
            ..Sample::default()
        },
    }
}

// ---- Transport probing ------------------------------------------------------

/// One of the three characteristic pairs this protocol ships behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Write FFF2, notify FFF1 — LifeSpan's exact role arrangement on the
    /// contested block. Hardware-verified (milltender's FS-BT-D2).
    Fff0,
    /// Write AE01, notify AE02 — qdomyos' "nobleproconnect" service.
    Ae00,
    /// Write FFE1, notify FFE4 — from qdomyos' characteristic pick only.
    Ffe0,
}

impl Transport {
    /// Probe order mirrors qdomyos' effective service pick: its handler
    /// takes FFF0 unconditionally, over an already-chosen AE00 or FFE0, so
    /// a device exposing several lands where the deployed implementation
    /// puts it.
    pub const ALL: [Transport; 3] = [Transport::Fff0, Transport::Ae00, Transport::Ffe0];

    pub fn write_uuid(self) -> Uuid {
        match self {
            Transport::Fff0 => FFF0_WRITE_UUID,
            Transport::Ae00 => AE00_WRITE_UUID,
            Transport::Ffe0 => FFE0_WRITE_UUID,
        }
    }

    pub fn notify_uuid(self) -> Uuid {
        match self {
            Transport::Fff0 => FFF0_NOTIFY_UUID,
            Transport::Ae00 => AE00_NOTIFY_UUID,
            Transport::Ffe0 => FFE0_NOTIFY_UUID,
        }
    }
}

/// The transport this GATT table carries, if any — notify and write roles
/// both verified, not just UUIDs. Note what this does and does not decide
/// on 0xFFF0: the swapped (Deerrun) arrangement is refused here, but the
/// LifeSpan arrangement *matches* because FitShow genuinely uses it — the
/// advertised-name gate in `supports()` is what keeps this driver off
/// LifeSpan consoles.
pub fn select_transport(gatt: &BTreeSet<Characteristic>) -> Option<Transport> {
    Transport::ALL.into_iter().find(|t| {
        super::util::has_notify(gatt, t.notify_uuid())
            && super::util::has_write(gatt, t.write_uuid())
    })
}

// ---- Name matching ----------------------------------------------------------

fn normalized(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

/// How a name relates to this driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameClass {
    /// Not a FitShow treadmill name (or a carved-out non-treadmill).
    NotOurs,
    /// Claimed even when the device also exposes real FTMS.
    Native,
    /// Claimed only when the GATT table has no FTMS Treadmill Data.
    FtmsPreferred,
}

pub fn classify_name(name: &str) -> NameClass {
    let n = normalized(name);
    if n.is_empty()
        || ADV_NAME_EXCLUDE_PREFIXES
            .iter()
            .any(|pfx| n.starts_with(pfx))
    {
        return NameClass::NotOurs;
    }
    if ADV_NAME_PREFIXES_NATIVE
        .iter()
        .any(|pfx| n.starts_with(pfx))
    {
        return NameClass::Native;
    }
    if ADV_NAME_PREFIXES_FTMS_PREFERRED
        .iter()
        .any(|pfx| n.starts_with(pfx))
    {
        return NameClass::FtmsPreferred;
    }
    // The deployed qdomyos `SW` rule: exactly 14 characters, no parens.
    if n.starts_with("SW")
        && n.chars().count() == SW_NAME_LEN
        && !n.contains('(')
        && !n.contains(')')
    {
        return NameClass::FtmsPreferred;
    }
    NameClass::NotOurs
}

// ---- The driver -------------------------------------------------------------

pub struct FitShow;

#[async_trait]
impl Driver for FitShow {
    fn id(&self) -> &'static str {
        "fitshow"
    }

    fn matches(&self, adv: &Advertisement) -> bool {
        // Name only. None of the three service blocks proves anything at
        // scan time: 0xFFF0 is the contested block LifeSpan already lists,
        // 0xFFE0 is the generic UART block, and 0xAE00 is squatted by
        // plenty of non-treadmill vendor modules.
        classify_name(&adv.name) != NameClass::NotOurs
    }

    fn supports(&self, adv: &Advertisement, gatt: &BTreeSet<Characteristic>) -> bool {
        // A recognised name AND a role-verified transport. Nameless devices
        // are always refused: every block here is generic or contested, and
        // a nameless FFF1/FFF2 device belongs to the LifeSpan fallback.
        let Some(_transport) = select_transport(gatt) else {
            return false;
        };
        match classify_name(&adv.name) {
            NameClass::NotOurs => false,
            NameClass::Native => true,
            // Mirror qdomyos' adjudication for these names with better
            // data than it has (the real GATT table, not the adverts):
            // when the device carries real FTMS, fall through to FTMS.
            NameClass::FtmsPreferred => !gatt.iter().any(|c| c.uuid == FTMS_TREADMILL_DATA_UUID),
        }
    }

    async fn run(&self, link: &Peripheral, host: &DriverHost<'_>, emit: Emit<'_>) -> Result<()> {
        let chars = link.characteristics();
        let transport = select_transport(&chars)
            .ok_or_else(|| anyhow!("no FitShow transport variant in the GATT table"))?;
        let notify_char = chars
            .iter()
            .find(|c| c.uuid == transport.notify_uuid())
            .cloned()
            .ok_or_else(|| anyhow!("notify characteristic missing"))?;
        let write_char = chars
            .iter()
            .find(|c| c.uuid == transport.write_uuid())
            .cloned()
            .ok_or_else(|| anyhow!("write characteristic missing"))?;
        // qdomyos' write-type rule: unacknowledged when the characteristic
        // offers it (the FFE1-style UART chars are often
        // write-without-response only), acknowledged otherwise.
        let write_type = if write_char
            .properties
            .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
        {
            WriteType::WithoutResponse
        } else {
            WriteType::WithResponse
        };

        let unit = wire_unit(&host.display_unit);

        // Subscribe first, then poll — the device answers the first query
        // immediately and the reply must not be missed. No init handshake:
        // the status query is the whole conversation (see the module docs
        // for the dropped login).
        link.subscribe(&notify_char).await?;
        let mut notifications = link.notifications().await?;

        let mut spacer = CommandSpacer::new(POLL_INTERVAL);
        let mut dead_polls: u32 = 0;

        loop {
            spacer.pace().await;

            // Drain stale buffered notifications so the read below answers
            // THIS query.
            while notifications.next().now_or_never().flatten().is_some() {}

            // Bound the write: a stale link can block forever with no
            // disconnect event.
            match tokio::time::timeout(
                RESPONSE_TIMEOUT,
                link.write(&write_char, &STATUS_QUERY, write_type),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e.into()), // real BLE error → reconnect
                Err(_) => {
                    dead_polls += 1;
                    tracing::warn!("timeout writing status query ({dead_polls}/{MAX_DEAD_POLLS})");
                    if dead_polls >= MAX_DEAD_POLLS {
                        return Err(anyhow!("link unresponsive; forcing reconnect"));
                    }
                    continue;
                }
            }

            let frame = match tokio::time::timeout(RESPONSE_TIMEOUT, notifications.next()).await {
                Ok(Some(n)) => n.value,
                Ok(None) => return Err(anyhow!("notification stream ended")),
                Err(_) => {
                    dead_polls += 1;
                    tracing::warn!(
                        "timeout waiting for status response ({dead_polls}/{MAX_DEAD_POLLS})"
                    );
                    if dead_polls >= MAX_DEAD_POLLS {
                        return Err(anyhow!("link unresponsive; forcing reconnect"));
                    }
                    continue;
                }
            };
            dead_polls = 0; // a frame arrived → the link is alive
            host.record_frame(MSG_STATUS, &frame); // raw capture for /api/diag

            match parse_status(&frame) {
                Ok(status) => emit(to_sample(&status, unit)),
                Err(ProtocolError::NotStatus(family)) => {
                    tracing::debug!("ignoring non-status frame family 0x{family:02x}");
                }
                Err(e) => tracing::debug!("fitshow frame skipped: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::Telemetry;

    fn hx(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // ---- The write set -------------------------------------------------------

    /// Every byte this driver writes must be a read. The whole write set is
    /// ONE frame — the bare status query, byte-identical to milltender's
    /// hardware-verified poll — and the count is part of the assertion:
    /// qdomyos opens with a login and a clock-carrying model query under
    /// other family bytes, and none of those may ever appear here. The
    /// command family byte is the verb boundary in this protocol: 0x50/
    /// 0x51/0x52 read, 0x53 (ready/start/speed/stop/pause/login) actuates.
    #[test]
    fn the_driver_only_ever_writes_the_status_query() {
        // The full write vocabulary, frames and count.
        let write_set: Vec<Vec<u8>> = vec![STATUS_QUERY.to_vec()];
        assert_eq!(write_set, vec![hx("02 51 51 03")]);
        assert_eq!(write_set.len(), 1, "one frame — no login, no init");

        // The builder reproduces it, and the verb byte is the read family.
        assert_eq!(build_frame(&[MSG_STATUS]), STATUS_QUERY.to_vec());
        for frame in &write_set {
            assert_eq!(frame[1], MSG_STATUS, "read family in {frame:02x?}");
            assert_ne!(frame[1], MSG_CONTROL, "control family in {frame:02x?}");
        }
    }

    // ---- Checksum / framing vectors ------------------------------------------

    /// The XOR trailer must reproduce every published frame of this
    /// protocol: the query we send, the OEM spec's own worked examples
    /// (the only multi-byte vectors with a published trailer), and — as
    /// decode-direction vectors only, the Sperax/PitPat precedent — the
    /// control-family frames upstream builds, which are NEVER sent.
    #[test]
    fn checksum_reproduces_every_published_trailer() {
        for (raw, why) in [
            ("02 51 51 03", "the status query — the frame we send"),
            (
                "02 7f 01 02 7c 03",
                "OEM spec worked example (unknown command)",
            ),
            ("02 7f 7f 03", "OEM spec worked example (bare echo)"),
            (
                "02 53 03 50 03",
                "milltender's stop command — NEVER SENT, vector only",
            ),
            (
                "02 53 00 01 00 6e 28 50 aa ee 03",
                "the user-data login — NEVER SENT, vector only",
            ),
        ] {
            let frame = hx(raw);
            assert_eq!(
                super::super::util::checksum_xor(&frame[1..frame.len() - 2]),
                frame[frame.len() - 2],
                "{why}"
            );
            // And the envelope parser accepts each one whole.
            assert!(frame_payload(&frame).is_ok(), "{why}");
        }
    }

    // ---- Status fixtures ------------------------------------------------------
    //
    // No public real capture of an inbound status frame exists (milltender
    // logs frames at runtime but publishes none), so these are synthetic:
    // built to the field map that qdomyos, milltender and the OEM spec
    // agree on, to pin OUR reader against THAT map. Labelled accordingly.

    /// Encode a status frame in the standard (little-endian) order —
    /// milltender's test-fixture layout: 14-byte payload through the
    /// heart-rate and segment bytes.
    fn build_status_frame(state: u8, c: &Counters) -> Vec<u8> {
        build_frame(&[
            MSG_STATUS,
            state,
            c.speed_raw,
            0x00, // incline — unparsed
            (c.duration_s & 0xFF) as u8,
            (c.duration_s >> 8) as u8,
            (c.distance_raw & 0xFF) as u8,
            (c.distance_raw >> 8) as u8,
            (c.calories_raw & 0xFF) as u8,
            (c.calories_raw >> 8) as u8,
            (c.steps & 0xFF) as u8,
            (c.steps >> 8) as u8,
            0x00, // heart rate — unparsed
            0x00, // program segment — unparsed
        ])
    }

    fn running_counters() -> Counters {
        Counters {
            speed_raw: 14, // 1.4 units
            duration_s: 325,
            distance_raw: 1136,
            calories_raw: 253,
            steps: 363,
        }
    }

    /// A hand-computed hex vector freezes the builder itself (a builder bug
    /// would otherwise hide a parser bug of the same shape).
    #[test]
    fn the_synthetic_builder_matches_the_hand_computed_frame() {
        assert_eq!(
            build_status_frame(STATUS_RUNNING, &running_counters()),
            hx("02 51 03 0e 00 45 01 70 04 fd 00 6b 01 00 00 fb 03")
        );
    }

    #[test]
    fn decodes_a_running_frame() {
        let s = parse_status(&build_status_frame(STATUS_RUNNING, &running_counters())).unwrap();
        assert_eq!(s.state, STATUS_RUNNING);
        assert_eq!(s.counters, Some(running_counters()));
    }

    /// END, STOPPING and PAUSED frames carry counters (qdomyos reads them;
    /// the spec's table spans them); every other status is state-only and
    /// its counters come out absent — not zero — even when trailing bytes
    /// are present.
    #[test]
    fn counter_statuses_carry_counters_and_the_rest_are_state_only() {
        let c = running_counters();
        for state in COUNTER_STATUSES {
            let s = parse_status(&build_status_frame(state, &c)).unwrap();
            assert_eq!(s.counters, Some(c.clone()), "status 0x{state:02x}");
        }
        for state in [
            STATUS_NORMAL,
            STATUS_START,
            STATUS_ERROR,
            STATUS_DISABLED,
            STATUS_STUDY,
            STATUS_READY,
        ] {
            let s = parse_status(&build_status_frame(state, &c)).unwrap();
            assert_eq!(s.counters, None, "status 0x{state:02x} is state-only");
        }

        // The short state-specific frames decode too: a NORMAL frame with
        // no data, a START frame carrying only the countdown byte, an
        // ERROR frame carrying only the error code.
        let normal = build_frame(&[MSG_STATUS, STATUS_NORMAL]);
        assert_eq!(
            parse_status(&normal).unwrap(),
            Status {
                state: STATUS_NORMAL,
                counters: None
            }
        );
        let start = build_frame(&[MSG_STATUS, STATUS_START, 3]); // 3-second countdown
        assert_eq!(parse_status(&start).unwrap().counters, None);
        let error = build_frame(&[MSG_STATUS, STATUS_ERROR, 0x0E]);
        assert_eq!(parse_status(&error).unwrap().state, STATUS_ERROR);

        // A counter status whose payload is one byte short of the counter
        // block degrades to state-only rather than misreading.
        let full = build_status_frame(STATUS_RUNNING, &c);
        let payload = &full[1..full.len() - 2];
        let truncated = build_frame(&payload[..COUNTER_PAYLOAD_MIN_LEN - 1]);
        assert_eq!(parse_status(&truncated).unwrap().counters, None);
    }

    /// Counters exercise the full u16 little-endian width — a wrong
    /// endianness or width would silently corrupt step data.
    #[test]
    fn wide_counters_are_little_endian_across_the_full_width() {
        let wide = Counters {
            speed_raw: 0xFF,
            duration_s: 0xABCD,
            distance_raw: 0x1234,
            calories_raw: 0x5678,
            steps: 0xFEDC,
        };
        let s = parse_status(&build_status_frame(STATUS_RUNNING, &wide)).unwrap();
        assert_eq!(s.counters, Some(wide));
    }

    // ---- The anyrun byte order, both directions -------------------------------

    /// The same bytes under the two orders — the disambiguation this driver
    /// deliberately does NOT attempt on the wire (no discriminator exists;
    /// module docs). Both directions are pinned: a standard frame misread
    /// as anyrun and an anyrun frame misread as standard both produce
    /// wrong-but-plausible counters, which is exactly why the driver stays
    /// pinned to the spec-conforming standard order rather than guessing.
    #[test]
    fn the_anyrun_order_reads_the_same_bytes_differently_both_directions() {
        // Standard-encoded: steps 513 = 01 02 LE, elapsed 7685 s = 05 1e LE.
        let standard = build_frame(&[
            MSG_STATUS,
            STATUS_RUNNING,
            20,
            0,
            0x05,
            0x1E, // 0x1E05 = 7685 s
            0,
            0,
            0,
            0,
            0x01,
            0x02, // 0x0201 = 513 steps
        ]);
        let s = parse_status_with_order(&standard, ByteOrder::Standard).unwrap();
        let c = s.counters.unwrap();
        assert_eq!(c.duration_s, 7685);
        assert_eq!(c.steps, 513);

        // The SAME bytes under the anyrun reading: elapsed becomes
        // 5 min 30 s and the steps byte-swap to 258.
        let a = parse_status_with_order(&standard, ByteOrder::AnyRun)
            .unwrap()
            .counters
            .unwrap();
        assert_eq!(a.duration_s, 5 * 60 + 0x1E);
        assert_eq!(a.steps, 0x0102);
        assert_eq!(a.steps, 258);

        // And the reverse direction: a frame an anyrun device would send
        // for 330 s / 513 steps decodes correctly as anyrun and wrongly as
        // standard.
        let anyrun_wire = build_frame(&[
            MSG_STATUS,
            STATUS_RUNNING,
            20,
            0,
            5,
            30, // 5 min 30 s
            0,
            0,
            0,
            0,
            0x02,
            0x01, // 513 big-endian
        ]);
        let a = parse_status_with_order(&anyrun_wire, ByteOrder::AnyRun)
            .unwrap()
            .counters
            .unwrap();
        assert_eq!(a.duration_s, 330);
        assert_eq!(a.steps, 513);
        let s = parse_status_with_order(&anyrun_wire, ByteOrder::Standard)
            .unwrap()
            .counters
            .unwrap();
        assert_eq!(s.duration_s, 30 << 8 | 5, "misread, as documented");
        assert_eq!(s.steps, 0x0102, "misread, as documented");

        // The driver's decode path is pinned to Standard.
        assert_eq!(
            parse_status(&standard),
            parse_status_with_order(&standard, ByteOrder::Standard)
        );
    }

    // ---- Malformed input ------------------------------------------------------

    #[test]
    fn malformed_frames_error_without_panicking() {
        assert_eq!(parse_status(&[]), Err(ProtocolError::BadLength(0)));
        assert_eq!(
            parse_status(&hx("02 51 03")),
            Err(ProtocolError::BadLength(3))
        );
        // A LifeSpan-style response — wrong prefix entirely. This is the
        // frame a mis-adjudicated LifeSpan console would answer with; it
        // must fail cleanly, not decode.
        assert_eq!(
            parse_status(&hx("a1 aa 00 23 00 00")),
            Err(ProtocolError::BadPrefix(0xA1))
        );
        // Right envelope, missing terminator.
        let mut no_term = build_status_frame(STATUS_RUNNING, &running_counters());
        let n = no_term.len();
        no_term[n - 1] = 0x00;
        assert_eq!(parse_status(&no_term), Err(ProtocolError::BadTerminator));
        // Truncated mid-frame: the footer lands elsewhere.
        assert!(
            parse_status(&build_status_frame(STATUS_RUNNING, &running_counters())[..9]).is_err()
        );
        // A valid envelope whose status payload lacks the state byte.
        assert_eq!(
            parse_status(&hx("02 51 51 03")),
            Err(ProtocolError::BadLength(4)),
            "our own query frame is not a status frame"
        );
    }

    /// A single corrupted counter byte must be rejected, not parsed into
    /// someone's step history — and fixing the trailer must make the same
    /// bytes parse again (proof the rejection was the checksum's).
    #[test]
    fn corruption_is_rejected_and_a_fixed_trailer_parses_again() {
        let mut corrupt = build_status_frame(STATUS_RUNNING, &running_counters());
        corrupt[11] ^= 0x01; // flip one steps byte: 363 → 362
        assert!(matches!(
            parse_status(&corrupt),
            Err(ProtocolError::BadChecksum { .. })
        ));
        let n = corrupt.len();
        corrupt[n - 2] = super::super::util::checksum_xor(&corrupt[1..n - 2]);
        assert_eq!(parse_status(&corrupt).unwrap().counters.unwrap().steps, 362);
    }

    /// Info and data replies (families 0x50/0x52) and control acks (0x53)
    /// are well-formed expected traffic on some firmware; they must be
    /// identified, not mangled or warned about as corruption.
    #[test]
    fn non_status_families_are_identified_not_mangled() {
        // An INFO_SPEED reply: max 60, min 8.
        let info = build_frame(&[MSG_INFO, 0x02, 60, 8]);
        assert_eq!(parse_status(&info), Err(ProtocolError::NotStatus(0x50)));
        // A sport-data reply.
        let data = build_frame(&[MSG_DATA, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(parse_status(&data), Err(ProtocolError::NotStatus(0x52)));
        // A control ack — traffic we never solicit, still identified.
        assert_eq!(
            parse_status(&hx("02 53 03 50 03")),
            Err(ProtocolError::NotStatus(0x53))
        );
    }

    // ---- Belt state -----------------------------------------------------------

    #[test]
    fn belt_states_map_and_unknowns_pass_through() {
        assert_eq!(belt_state(STATUS_NORMAL), BeltState::Standby);
        assert_eq!(belt_state(STATUS_END), BeltState::Summary, "post-workout");
        assert_eq!(belt_state(STATUS_START), BeltState::Running, "countdown");
        assert_eq!(belt_state(STATUS_RUNNING), BeltState::Running);
        assert_eq!(
            belt_state(STATUS_STOPPING),
            BeltState::Running,
            "decelerating — the belt is factually still moving"
        );
        assert_eq!(
            belt_state(STATUS_ERROR),
            BeltState::Standby,
            "a fault must not present as the contract's PAUSED code"
        );
        assert_eq!(belt_state(STATUS_DISABLED), BeltState::Standby);
        assert_eq!(belt_state(STATUS_READY), BeltState::Standby);
        assert_eq!(belt_state(STATUS_PAUSED), BeltState::Paused);
        for v in [STATUS_STUDY, 0x08, 0x0B, 0x7F, 0xFF] {
            assert_eq!(belt_state(v), BeltState::Other(v), "byte 0x{v:02x}");
        }
    }

    // ---- Units ---------------------------------------------------------------

    /// The wire unit follows the console's display unit (the LifeSpan
    /// precedent — see the module docs): metric consoles scale 0.1 km/h
    /// and report no distance (unverified scale); imperial consoles scale
    /// 0.1 mph and 0.001 mile. Steps and seconds are unit-free and
    /// identical on both. Calories are absent on both — never scaled,
    /// never guessed.
    #[test]
    fn unit_scaling_follows_the_console_display_unit() {
        let approx = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let s = parse_status(&build_status_frame(STATUS_RUNNING, &running_counters())).unwrap();

        assert_eq!(wire_unit("km/h"), WireUnit::Metric);
        assert_eq!(wire_unit("mph"), WireUnit::Imperial);
        // telemetry.rs treats any non-"km/h" string as mph; so do we.
        assert_eq!(wire_unit("miles"), WireUnit::Imperial);

        let metric = to_sample(&s, WireUnit::Metric);
        assert!(approx(metric.speed_kmh.unwrap(), 1.4));
        assert_eq!(metric.distance_m, None, "metric scale unverified — absent");
        assert_eq!(metric.steps, Some(363));
        assert_eq!(metric.duration_s, Some(325));
        assert_eq!(metric.calories, None, "kcal scale conflict — absent");

        let imperial = to_sample(&s, WireUnit::Imperial);
        assert!(approx(imperial.speed_kmh.unwrap(), 1.4 * 1.609344));
        assert!(approx(imperial.distance_m.unwrap(), 1136.0 * 1.609344));
        assert_eq!(imperial.steps, Some(363));
        assert_eq!(imperial.duration_s, Some(325));
        assert_eq!(imperial.calories, None);
    }

    // ---- Sample / Telemetry golden pins --------------------------------------

    /// Fixture frame → Sample → Telemetry on an imperial console: the
    /// mph wire scaling and the presentation re-encoding, pinned end to
    /// end. The raw speed must round-trip exactly (1.4 wire units → 140
    /// centi-mph), which is what keeps stored accounting stable.
    #[test]
    fn golden_fixture_to_telemetry() {
        let s = parse_status(&build_status_frame(STATUS_RUNNING, &running_counters())).unwrap();

        let t = Telemetry::from_sample(&to_sample(&s, WireUnit::Imperial), "mph");
        assert_eq!(t.speed_raw, Some(140), "1.40 mph in centi-units");
        assert_eq!(t.distance_raw, Some(183), "1828.2 m → 183 decameters");
        assert_eq!(t.steps, Some(363));
        assert_eq!(t.duration_s, Some(325));
        assert_eq!(t.calories, None);
        assert_eq!(t.status_name.as_deref(), Some("RUNNING"));
        assert!(t.is_running);

        let t = Telemetry::from_sample(&to_sample(&s, WireUnit::Metric), "km/h");
        assert_eq!(t.speed_raw, Some(140), "1.40 km/h in centi-units");
        assert_eq!(t.distance_raw, None);
        assert_eq!(t.distance_m, None);
        assert_eq!(t.steps, Some(363));
    }

    /// A paused belt presents as the contract's PAUSED code, END as the
    /// summary screen, NORMAL as standby — and a faulted machine (wire
    /// 0x05) presents as STANDBY, never as PAUSED (the collision the
    /// explicit ERROR mapping exists to prevent).
    #[test]
    fn statuses_present_as_the_contract_codes() {
        let telem = |state: u8| {
            let s = parse_status(&build_status_frame(state, &running_counters())).unwrap();
            Telemetry::from_sample(&to_sample(&s, WireUnit::Metric), "km/h")
        };

        let t = telem(STATUS_PAUSED);
        assert_eq!(t.status, Some(0x05));
        assert_eq!(t.status_name.as_deref(), Some("PAUSED"));
        assert!(!t.is_running);

        let t = telem(STATUS_END);
        assert_eq!(t.status, Some(0x04));
        assert_eq!(t.status_name.as_deref(), Some("SUMMARY_SCREEN"));

        let t = telem(STATUS_ERROR);
        assert_eq!(t.status, Some(0x01), "ERROR presents as STANDBY");
        assert_ne!(t.status, Some(0x05), "…and NEVER as PAUSED");

        let t = telem(STATUS_NORMAL);
        assert_eq!(t.status_name.as_deref(), Some("STANDBY"));
        assert!(!t.is_running);

        // A state-only standby frame reports no counters at all.
        let s = parse_status(&build_frame(&[MSG_STATUS, STATUS_NORMAL])).unwrap();
        let t = Telemetry::from_sample(&to_sample(&s, WireUnit::Metric), "km/h");
        assert_eq!(t.steps, None);
        assert_eq!(t.speed_raw, None);
        assert_eq!(t.duration_s, None);
    }

    // ---- Name matching --------------------------------------------------------

    fn adv(name: &str) -> Advertisement {
        Advertisement {
            name: name.into(),
            services: vec![],
        }
    }

    #[test]
    fn native_names_match_and_carveouts_do_not() {
        for name in [
            "FS-3D6CD7", // milltender's real advertised name
            "fs-1234",
            " FS-AB ",
            "TR510-T",
            "TUNTURI T80-123",
            "tunturi t80-1",
        ] {
            assert_eq!(classify_name(name), NameClass::Native, "{name}");
            assert!(FitShow.matches(&adv(name)), "{name}");
        }
        for name in [
            "FS-YK-100",     // FTMS exercise bike — carved out
            "TUNTURI T60-1", // plain FTMS treadmill
            "TUNTURI T90-1", // plain FTMS treadmill
            "TUNTURI",       // underspecified
            "FS",            // no hyphen — not the module's name shape
            "LifeSpan-TM",
            "URTM041",
            "SPERAX_RM01",
            "PitPat-T01",
            "WalkingPad A1",
            "",
        ] {
            assert_eq!(classify_name(name), NameClass::NotOurs, "{name}");
            assert!(!FitShow.matches(&adv(name)), "{name}");
        }
    }

    #[test]
    fn ftms_preferred_names_match_as_their_class() {
        for name in [
            "NOBLEPRO CONNECT 123",
            "WINFITA-01",
            "SW-BLE-1234",
            "BF70-XYZ",
        ] {
            assert_eq!(classify_name(name), NameClass::FtmsPreferred, "{name}");
            assert!(FitShow.matches(&adv(name)), "{name}");
        }
    }

    /// The real `SW` rule from qdomyos' matcher: exactly 14 characters, no
    /// parentheses (and, at connect time, no FTMS — pinned in the supports
    /// tests). A bare `SW` prefix alone claims nothing.
    #[test]
    fn the_sw_rule_is_fourteen_chars_and_no_parens() {
        assert_eq!(classify_name("SW123456789012"), NameClass::FtmsPreferred);
        assert_eq!(classify_name("sw123456789012"), NameClass::FtmsPreferred);
        // Trimming happens before the length check.
        assert_eq!(classify_name(" SW123456789012 "), NameClass::FtmsPreferred);
        for name in [
            "SW",              // 2 chars
            "SW12345678901",   // 13
            "SW1234567890123", // 15
            "SW12345678(01)",  // 14 but parenthesised
            "S-W-1234567890",  // 14 but not the SW prefix
        ] {
            assert_eq!(classify_name(name), NameClass::NotOurs, "{name}");
        }
        // SW-BLE matches at ANY length via its own prefix rule.
        assert_eq!(classify_name("SW-BLE"), NameClass::FtmsPreferred);
        assert_eq!(
            classify_name("SW-BLE-LONG-NAME-1"),
            NameClass::FtmsPreferred
        );
    }

    // ---- Transport selection --------------------------------------------------

    use btleplug::api::CharPropFlags;

    const N: CharPropFlags = CharPropFlags::NOTIFY;
    const W: CharPropFlags = CharPropFlags::WRITE;
    const WWR: CharPropFlags = CharPropFlags::WRITE_WITHOUT_RESPONSE;

    fn gatt(chars: &[(Uuid, CharPropFlags)]) -> BTreeSet<Characteristic> {
        chars
            .iter()
            .map(|(uuid, properties)| Characteristic {
                uuid: *uuid,
                service_uuid: super::super::sig_uuid(0x0000),
                properties: *properties,
                descriptors: BTreeSet::new(),
            })
            .collect()
    }

    fn fff0_shape() -> Vec<(Uuid, CharPropFlags)> {
        vec![(FFF0_NOTIFY_UUID, N), (FFF0_WRITE_UUID, W)]
    }
    fn ae00_shape() -> Vec<(Uuid, CharPropFlags)> {
        vec![(AE00_NOTIFY_UUID, N), (AE00_WRITE_UUID, W)]
    }
    fn ffe0_shape() -> Vec<(Uuid, CharPropFlags)> {
        // FFE1-style UART chars are typically write-without-response only.
        vec![(FFE0_NOTIFY_UUID, N), (FFE0_WRITE_UUID, WWR)]
    }

    #[test]
    fn each_layout_selects_its_transport() {
        assert_eq!(
            select_transport(&gatt(&fff0_shape())),
            Some(Transport::Fff0)
        );
        assert_eq!(
            select_transport(&gatt(&ae00_shape())),
            Some(Transport::Ae00)
        );
        assert_eq!(
            select_transport(&gatt(&ffe0_shape())),
            Some(Transport::Ffe0)
        );
        assert_eq!(select_transport(&gatt(&[])), None);

        // qdomyos' effective preference: FFF0 wins when several appear.
        let both: Vec<_> = ae00_shape().into_iter().chain(fff0_shape()).collect();
        assert_eq!(select_transport(&gatt(&both)), Some(Transport::Fff0));
    }

    /// Roles are verified, not just UUIDs. The FFF0 arrangement here IS the
    /// LifeSpan arrangement — that is the point: role checks cannot
    /// adjudicate this block (the name gate does, pinned in mod.rs) — but
    /// the swapped Deerrun arrangement, half-tables and property-less
    /// tables are all refused, and FFE0 must notify on FFE4, not FFE1.
    #[test]
    fn role_verification_refuses_the_wrong_arrangements() {
        // Deerrun-swapped FFF0: refused.
        assert_eq!(
            select_transport(&gatt(&[(FFF0_NOTIFY_UUID, WWR), (FFF0_WRITE_UUID, N)])),
            None
        );
        // Half a table: refused.
        assert_eq!(select_transport(&gatt(&[(FFF0_NOTIFY_UUID, N)])), None);
        // UUIDs present but no properties: refused.
        assert_eq!(
            select_transport(&gatt(&[
                (AE00_WRITE_UUID, CharPropFlags::default()),
                (AE00_NOTIFY_UUID, CharPropFlags::default()),
            ])),
            None
        );
        // A notify-only FFE1 with no FFE4 (the common HM-10 UART single
        // characteristic) is NOT this transport.
        assert_eq!(select_transport(&gatt(&[(FFE0_WRITE_UUID, N | WWR)])), None);
        // The aradix FS-BT-C1 shape: vendor FFF1 notify-only, no FFF2
        // write — refused here, so the device falls through to FTMS.
        assert_eq!(select_transport(&gatt(&[(FFF0_NOTIFY_UUID, N)])), None);
    }

    // ---- supports(): names × transports × FTMS --------------------------------

    #[test]
    fn supports_needs_a_recognised_name_and_a_verified_transport() {
        // A native name claims every transport variant.
        for shape in [fff0_shape(), ae00_shape(), ffe0_shape()] {
            assert!(FitShow.supports(&adv("FS-3D6CD7"), &gatt(&shape)));
        }
        // Nameless: refused on every shape — 0xFFF0 belongs to the
        // LifeSpan fallback, and the other blocks are generic.
        for shape in [fff0_shape(), ae00_shape(), ffe0_shape()] {
            assert!(!FitShow.supports(&adv(""), &gatt(&shape)));
        }
        // A foreign or carved-out name: refused whatever the table.
        for name in ["LifeSpan-TM", "FS-YK-100", "Mystery Pad 3000"] {
            assert!(
                !FitShow.supports(&adv(name), &gatt(&fff0_shape())),
                "{name}"
            );
        }
        // The right name with no recognisable transport: refused.
        assert!(!FitShow.supports(&adv("FS-3D6CD7"), &gatt(&[])));
        // The right name with the Deerrun-swapped FFF0 roles: refused —
        // that table is not this protocol, whatever the name says.
        assert!(!FitShow.supports(
            &adv("FS-3D6CD7"),
            &gatt(&[(FFF0_NOTIFY_UUID, W), (FFF0_WRITE_UUID, N)])
        ));
    }

    /// The FTMS adjudication, both directions: the never-switch names keep
    /// the native protocol even alongside real FTMS (steps beat FTMS
    /// there, per qdomyos' hard-coded carve-outs), while the FTMS-preferred
    /// names yield to FTMS the moment Treadmill Data is present.
    #[test]
    fn the_ftms_adjudication_follows_qdomyos_both_directions() {
        let with_ftms = |shape: Vec<(Uuid, CharPropFlags)>| {
            let mut v = shape;
            v.push((FTMS_TREADMILL_DATA_UUID, N));
            v
        };

        // Native names: claimed with and without FTMS.
        for name in ["FS-3D6CD7", "TR510-T", "TUNTURI T80-1"] {
            assert!(FitShow.supports(&adv(name), &gatt(&fff0_shape())), "{name}");
            assert!(
                FitShow.supports(&adv(name), &gatt(&with_ftms(fff0_shape()))),
                "{name} keeps the native protocol alongside FTMS"
            );
        }

        // FTMS-preferred names: claimed only without FTMS.
        for name in [
            "NOBLEPRO CONNECT 1",
            "WINFITA-01",
            "SW-BLE-1",
            "BF70-X",
            "SW123456789012",
        ] {
            assert!(FitShow.supports(&adv(name), &gatt(&ae00_shape())), "{name}");
            assert!(
                !FitShow.supports(&adv(name), &gatt(&with_ftms(ae00_shape()))),
                "{name} yields to FTMS"
            );
        }
    }
}
