//! The driver system: one self-contained module per treadmill protocol, a
//! neutral [`Sample`] they all emit, and the registry the engine consults.
//!
//! The split of responsibilities is deliberate:
//!
//! * A **driver** knows how to recognise its device and turn its Bluetooth
//!   traffic into [`Sample`]s — SI units, nothing vendor-shaped. That is all.
//! * The **engine** (`ble.rs`) owns everything else: scanning, connecting,
//!   reconnect/backoff, give-up-after-N-failures, cancellation (pause,
//!   device switch, shutdown), session detection, persistence throttling and
//!   the WebSocket broadcast. A driver never touches any of it, which is what
//!   keeps a new driver to one file plus one registration line.
//!
//! Adding a driver: write `drivers/yourdevice.rs`, add it to [`DRIVERS`], done.
//! The scan path and the connect path both consult the registry, so the one
//! line makes the device discoverable *and* connectable. The full guide for
//! contributors lives in `docs/drivers/README.md`.

pub mod ftms;
pub mod kingsmith_wilink;
pub mod lifespan;
pub mod util;

use anyhow::Result;
use async_trait::async_trait;
use btleplug::api::Characteristic;
use btleplug::platform::Peripheral;
use std::collections::BTreeSet;
use uuid::Uuid;

/// Every driver Trot ships, in priority order — when a device satisfies more
/// than one driver, the first match wins. **Order is load-bearing: strict
/// drivers come before permissive ones.**
///
/// * `LifeSpan` matches strictly (advertised-name prefix AND the exact
///   notify/write characteristic roles) and outranks FTMS because LifeSpan
///   consoles expose their native service alongside whatever else they
///   advertise, and the native protocol reports steps where FTMS cannot.
/// * `KingSmithWiLink` matches strictly too (recognised or absent name AND
///   the FE01-notify/FE02-write roles, with the known FTMS/app-cipher
///   KingSmith models carved out by name) and outranks FTMS for the same
///   reason: the native protocol reports steps.
/// * `Ftms` requires the standard Treadmill Data characteristic.
/// * `LifeSpanFallback` is the deliberate last resort: a device nobody else
///   claimed, whose `FFF1`/`FFF2` roles are exactly LifeSpan-shaped, is
///   driven as LifeSpan even under an unrecognised name — that is what keeps
///   an already-paired console working when its advertised name isn't in our
///   prefix list. Every future driver goes **before** it.
///
/// **This is the registration point.** One line here is the only edit outside
/// your driver file.
pub static DRIVERS: &[&dyn Driver] = &[
    &lifespan::LifeSpan,
    &kingsmith_wilink::KingSmithWiLink,
    &ftms::Ftms,
    &lifespan::LifeSpanFallback,
];

/// A treadmill protocol driver. In-tree, compiled in, reviewed — there is no
/// dynamic loading, deliberately.
#[async_trait]
pub trait Driver: Send + Sync {
    /// Short stable identifier ("lifespan", "ftms"). Shows up in logs.
    fn id(&self) -> &'static str;

    /// Does this advertisement look like a device you can drive? Called during
    /// `trot scan` — before any connection exists — so all you have is the
    /// advertised name and service UUIDs. Be permissive here; [`Self::supports`]
    /// gets the real service table later.
    fn matches(&self, adv: &Advertisement) -> bool;

    /// Does this connected device look like yours up close? Called after
    /// connect + service discovery to pick the driver, with the full GATT
    /// characteristic table (UUIDs *and* properties) plus the advertisement.
    ///
    /// Match on what you will actually subscribe to or write — and be aware
    /// that a service UUID alone proves nothing: 0xFFF0 alone hosts at least
    /// five mutually incompatible vendor protocols, some with the notify/write
    /// roles swapped. When your protocol shares a service with others, check
    /// characteristic properties and the advertised name, not just UUIDs. A
    /// device whose advertisement looked like yours but whose table doesn't
    /// check out falls through to the next driver in [`DRIVERS`].
    fn supports(&self, adv: &Advertisement, gatt: &BTreeSet<Characteristic>) -> bool;

    /// Drive the device: subscribe/poll/handshake as the protocol requires and
    /// call `emit` with a cumulative [`Sample`] on every update.
    ///
    /// Run forever. Do not watch for shutdown or pause and do not disconnect —
    /// the engine cancels this future and tears the link down itself. Return
    /// `Err` only when the link is dead or unusable; that triggers the
    /// engine's reconnect-with-backoff path.
    async fn run(&self, link: &Peripheral, host: &DriverHost<'_>, emit: Emit<'_>) -> Result<()>;
}

/// The sink a driver feeds. Call it with the full latest state (not a delta)
/// each time anything changes; the engine handles throttling and sessions.
pub type Emit<'a> = &'a mut (dyn FnMut(Sample) + Send);

/// What a driver may see of a device while it is only advertising (scan and
/// pairing, before any connection).
#[derive(Debug, Clone)]
pub struct Advertisement {
    /// Advertised local name; empty when the device doesn't broadcast one.
    pub name: String,
    /// Advertised service UUIDs. Often a subset of the real GATT table.
    pub services: Vec<Uuid>,
}

/// One reading from the belt, in SI units. This is the only currency a driver
/// deals in — no vendor encodings, no display units.
///
/// Every field is `Option` because "this device cannot report that" is
/// meaningful: FTMS treadmills have no step counter, so their driver leaves
/// `steps` as `None` and the rest of the engine treats it as absent rather
/// than zero. Leave out what your device doesn't know; never invent values.
///
/// Counters (`distance_m`, `steps`, `duration_s`, `calories`) are cumulative
/// since the session started on the console, exactly as the device reports
/// them — the engine's storage layer de-glitches resets and stale frames, so
/// don't try to smooth them in the driver.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    /// Belt speed, km/h.
    pub speed_kmh: Option<f64>,
    /// Distance, meters.
    pub distance_m: Option<f64>,
    /// Step count, as the console reports it.
    pub steps: Option<u32>,
    /// Elapsed workout time, seconds.
    pub duration_s: Option<u32>,
    /// Energy, kcal.
    pub calories: Option<u32>,
    /// What the belt is doing, if the device reports it.
    pub state: Option<BeltState>,
}

/// What the belt is doing. `Running` is what opens and (its absence) closes
/// sessions; the other states exist because some consoles distinguish them and
/// clients display them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltState {
    /// Powered on, belt stopped.
    Standby,
    /// Belt moving.
    Running,
    /// Console showing the post-workout summary screen.
    Summary,
    /// Workout paused.
    Paused,
    /// A state Trot doesn't know; the raw device value is passed through.
    Other(u8),
}

/// What the engine provides to a running driver.
pub struct DriverHost<'a> {
    /// The unit the user's console displays ("km/h" or "mph"). Only relevant
    /// to drivers whose wire format depends on the console's display setting
    /// (LifeSpan encodes speed in hundredths of the *displayed* unit). Drivers
    /// that report SI natively should ignore it.
    pub display_unit: String,
    recorder: &'a (dyn Fn(u8, &[u8]) + Send + Sync),
}

impl<'a> DriverHost<'a> {
    pub fn new(display_unit: String, recorder: &'a (dyn Fn(u8, &[u8]) + Send + Sync)) -> Self {
        DriverHost {
            display_unit,
            recorder,
        }
    }

    /// Record a raw frame into the diagnostics ring buffer (dumped by
    /// `/api/diag` as `recent_frames`). Call this for every frame you receive,
    /// with a tag of your choosing (LifeSpan uses the request opcode) — it is
    /// the tool a contributor uses to reverse-engineer and debug a protocol
    /// without attaching a debugger to a moving treadmill.
    pub fn record_frame(&self, tag: u8, frame: &[u8]) {
        (self.recorder)(tag, frame)
    }
}

/// The driver claiming a connected device, if any. First match in [`DRIVERS`]
/// order wins.
pub fn for_device(
    adv: &Advertisement,
    gatt: &BTreeSet<Characteristic>,
) -> Option<&'static dyn Driver> {
    DRIVERS.iter().copied().find(|d| d.supports(adv, gatt))
}

/// Would any driver want this advertisement? The scan path uses this, so a
/// newly registered driver is discoverable with no further wiring.
pub fn any_match(adv: &Advertisement) -> bool {
    DRIVERS.iter().any(|d| d.matches(adv))
}

/// Registered driver ids, for error messages.
pub fn ids() -> Vec<&'static str> {
    DRIVERS.iter().map(|d| d.id()).collect()
}

/// Full 128-bit form of a 16-bit Bluetooth SIG assigned UUID,
/// e.g. `0x1826` → `00001826-0000-1000-8000-00805f9b34fb`.
pub const fn sig_uuid(short: u16) -> Uuid {
    Uuid::from_u128(((short as u128) << 96) | 0x0000_1000_8000_0080_5f9b_34fb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use btleplug::api::CharPropFlags;

    fn adv(name: &str, services: &[u16]) -> Advertisement {
        Advertisement {
            name: name.into(),
            services: services.iter().map(|s| sig_uuid(*s)).collect(),
        }
    }

    fn chr(short: u16, properties: CharPropFlags) -> Characteristic {
        Characteristic {
            uuid: sig_uuid(short),
            service_uuid: sig_uuid(0x0000),
            properties,
            descriptors: BTreeSet::new(),
        }
    }

    fn gatt(chars: &[(u16, CharPropFlags)]) -> BTreeSet<Characteristic> {
        chars.iter().map(|(u, p)| chr(*u, *p)).collect()
    }

    const N: CharPropFlags = CharPropFlags::NOTIFY;
    const W: CharPropFlags = CharPropFlags::WRITE;

    #[test]
    fn sig_uuid_builds_the_base_form() {
        assert_eq!(
            sig_uuid(0x1826).to_string(),
            "00001826-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(
            sig_uuid(0xfff0).to_string(),
            "0000fff0-0000-1000-8000-00805f9b34fb"
        );
    }

    /// The union of driver `matches()` must cover every supported device
    /// family — LifeSpan/ESP32 names and service 0xFFF0, the KingSmith WiLink
    /// names and service 0xFE00, FTMS 0x1826 — and nothing else.
    #[test]
    fn scan_matching_covers_the_known_devices() {
        assert!(any_match(&adv("LifeSpan-TM", &[])));
        assert!(any_match(&adv("ESP32-treadmill", &[])));
        assert!(any_match(&adv("", &[0xfff0])));
        assert!(any_match(&adv("", &[0x1826])));
        assert!(any_match(&adv("WalkingPad A1", &[])));
        assert!(any_match(&adv("", &[0xfe00])));
        assert!(!any_match(&adv("Some Headphones", &[0x180f])));
        assert!(!any_match(&adv("", &[])));
    }

    /// Connect-time dispatch: a named LifeSpan console with the right
    /// characteristic roles wins outright — including over FTMS, because the
    /// native protocol reports steps where FTMS cannot.
    #[test]
    fn a_named_lifespan_console_takes_the_strict_driver() {
        let named = adv("LifeSpan-TM", &[]);
        assert_eq!(
            for_device(&named, &gatt(&[(0xfff1, N), (0xfff2, W), (0x2acd, N)])).map(|d| d.id()),
            Some("lifespan")
        );
        assert_eq!(
            for_device(&named, &gatt(&[(0xfff1, N), (0xfff2, W)])).map(|d| d.id()),
            Some("lifespan")
        );
    }

    /// A nameless device with LifeSpan-shaped roles and no other claim lands
    /// on the fallback — this is what keeps an already-paired console working
    /// when its advertised name isn't in the prefix list (or the platform
    /// doesn't surface a name at connect time).
    #[test]
    fn an_unnamed_lifespan_shaped_device_falls_back_to_lifespan() {
        let anon = adv("", &[]);
        assert_eq!(
            for_device(&anon, &gatt(&[(0xfff1, N), (0xfff2, W)])).map(|d| d.id()),
            Some("lifespan-fallback")
        );
    }

    /// A device that ALSO speaks real FTMS is claimed by FTMS before the
    /// LifeSpan fallback: an unrecognised name plus a generic FFFx vendor
    /// block plus standard FTMS is the KingSmith/ODM shape, where FTMS is the
    /// protocol that actually works and LifeSpan opcodes are garbage.
    #[test]
    fn ftms_outranks_the_lifespan_fallback() {
        let anon = adv("", &[]);
        assert_eq!(
            for_device(&anon, &gatt(&[(0xfff1, N), (0xfff2, W), (0x2acd, N)])).map(|d| d.id()),
            Some("ftms")
        );
        assert_eq!(
            for_device(&anon, &gatt(&[(0x2acd, N)])).map(|d| d.id()),
            Some("ftms")
        );
    }

    /// Role-swapped FFF1/FFF2 (the Deerrun shape: write on FFF1, notify on
    /// FFF2) must not be claimed by ANY LifeSpan entry — writing LifeSpan
    /// opcodes at it would mis-drive a different protocol. Likewise partial
    /// tables and non-treadmills get no driver.
    #[test]
    fn wrong_roles_or_partial_tables_get_no_driver() {
        let anon = adv("", &[]);
        assert!(for_device(&anon, &gatt(&[(0xfff1, W), (0xfff2, N)])).is_none());
        assert!(
            for_device(&adv("LifeSpan-TM", &[]), &gatt(&[(0xfff1, W), (0xfff2, N)])).is_none(),
            "even a LifeSpan name cannot override role verification"
        );
        assert!(for_device(&anon, &gatt(&[(0xfff1, N)])).is_none());
        assert_eq!(
            for_device(&anon, &gatt(&[(0xfff1, N), (0x2acd, N)])).map(|d| d.id()),
            Some("ftms"),
            "FFF1 without a writable FFF2 must not claim the device for LifeSpan"
        );
        assert!(for_device(&anon, &gatt(&[(0x2a37, N)])).is_none());
    }

    /// A named WalkingPad with the WiLink notify/write roles takes the native
    /// driver — including over FTMS (some newer pads expose both, and only
    /// the native protocol reports steps). The carved-out FTMS model with the
    /// KingSmith name falls through to FTMS.
    #[test]
    fn a_named_walkingpad_takes_the_wilink_driver() {
        let named = adv("WalkingPad A1", &[]);
        assert_eq!(
            for_device(&named, &gatt(&[(0xfe01, N), (0xfe02, W)])).map(|d| d.id()),
            Some("kingsmith-wilink")
        );
        assert_eq!(
            for_device(&named, &gatt(&[(0xfe01, N), (0xfe02, W), (0x2acd, N)])).map(|d| d.id()),
            Some("kingsmith-wilink")
        );
        assert_eq!(
            for_device(&adv("KS-HD-Z1D", &[]), &gatt(&[(0x2acd, N)])).map(|d| d.id()),
            Some("ftms"),
            "the FTMS WalkingPad Z1 must reach the FTMS driver, not WiLink"
        );
    }

    /// Registry order is load-bearing: strict drivers first, the permissive
    /// LifeSpan fallback dead last. If this test fails because you added a
    /// driver, add it BEFORE "lifespan-fallback" — anything after the
    /// fallback can never claim an FFF1/FFF2-shaped device.
    #[test]
    fn registry_ids_are_unique_and_the_fallback_stays_last() {
        let ids = ids();
        assert_eq!(
            ids,
            vec!["lifespan", "kingsmith-wilink", "ftms", "lifespan-fallback"]
        );
        assert_eq!(
            ids.last(),
            Some(&"lifespan-fallback"),
            "the permissive fallback must remain the last registry entry"
        );
    }
}
