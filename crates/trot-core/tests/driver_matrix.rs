//! Cross-driver dispatch adjudication — the system-level matrix.
//!
//! Every driver was written and tested in isolation; this file tests the
//! *registry as a whole*. For each representative device (advertisement +
//! GATT table) it asserts not just which driver wins `for_device`, but the
//! **exact set** of drivers whose `supports()` accepts the device — so a
//! future driver that starts shadowing another one (or widening its claim
//! onto the contested `0xFFF0` block) fails here, loudly, with the full
//! supporter list in the message.
//!
//! `0xFFF0` with `FFF1`/`FFF2` is squatted by six mutually incompatible
//! protocols (LifeSpan, Urevo, Sperax, FitShow, Deerrun-via-PitPat, and the
//! deliberate fallback); the advertised name is the whole adjudication for
//! four of them and the swapped roles for the fifth. Rows below cover every
//! claimant on both role arrangements.

use btleplug::api::{CharPropFlags, Characteristic};
use std::collections::BTreeSet;
use trot_core::drivers::{self, sig_uuid, Advertisement, DRIVERS};
use uuid::Uuid;

const N: CharPropFlags = CharPropFlags::NOTIFY;
const W: CharPropFlags = CharPropFlags::WRITE;
const WWR: CharPropFlags = CharPropFlags::WRITE_WITHOUT_RESPONSE;

fn adv(name: &str) -> Advertisement {
    Advertisement {
        name: name.into(),
        services: vec![],
    }
}

fn chr(service: Uuid, uuid: Uuid, properties: CharPropFlags) -> Characteristic {
    Characteristic {
        uuid,
        service_uuid: service,
        properties,
        descriptors: BTreeSet::new(),
    }
}

fn gatt(chars: &[(u16, u16, CharPropFlags)]) -> BTreeSet<Characteristic> {
    chars
        .iter()
        .map(|(svc, u, p)| chr(sig_uuid(*svc), sig_uuid(*u), *p))
        .collect()
}

/// Every driver whose `supports()` accepts the device, in registry order.
/// `for_device` picks the first of these — asserting the whole set is what
/// catches shadowing.
fn supporters(a: &Advertisement, g: &BTreeSet<Characteristic>) -> Vec<&'static str> {
    DRIVERS
        .iter()
        .filter(|d| d.supports(a, g))
        .map(|d| d.id())
        .collect()
}

// ---- Canonical GATT tables ---------------------------------------------------

/// The LifeSpan/Urevo/Sperax/FitShow arrangement on the contested block:
/// notify FFF1, write FFF2.
fn lifespan_shape() -> BTreeSet<Characteristic> {
    gatt(&[(0xfff0, 0xfff1, N), (0xfff0, 0xfff2, W)])
}

/// The Deerrun arrangement: same UUIDs, roles swapped.
fn deerrun_shape() -> BTreeSet<Characteristic> {
    gatt(&[(0xfff0, 0xfff1, WWR), (0xfff0, 0xfff2, N)])
}

fn with_ftms(mut g: BTreeSet<Characteristic>) -> BTreeSet<Characteristic> {
    g.insert(chr(sig_uuid(0x1826), sig_uuid(0x2acd), N));
    g
}

fn ftms_only() -> BTreeSet<Characteristic> {
    gatt(&[(0x1826, 0x2acd, N), (0x1826, 0x2ada, N)])
}

fn wilink_shape() -> BTreeSet<Characteristic> {
    gatt(&[(0xfe00, 0xfe01, N), (0xfe00, 0xfe02, W)])
}

fn props_shape() -> BTreeSet<Characteristic> {
    gatt(&[(0x1234, 0xfed8, N), (0x1234, 0xfed7, WWR)])
}

fn pitpat_fba0() -> BTreeSet<Characteristic> {
    gatt(&[(0xfba0, 0xfba1, W), (0xfba0, 0xfba2, N)])
}

fn pitpat_ffff() -> BTreeSet<Characteristic> {
    gatt(&[(0xffff, 0xff01, W), (0xffff, 0xff02, N)])
}

fn pitpat_1910() -> BTreeSet<Characteristic> {
    gatt(&[(0x1910, 0x2b11, W), (0x1910, 0x2b10, N)])
}

fn fitshow_ae00() -> BTreeSet<Characteristic> {
    gatt(&[(0xae00, 0xae01, W), (0xae00, 0xae02, N)])
}

fn fitshow_ffe0() -> BTreeSet<Characteristic> {
    gatt(&[(0xffe0, 0xffe1, WWR), (0xffe0, 0xffe4, N)])
}

// ---- The matrix --------------------------------------------------------------

/// One row: device profile → the exact supporter set (winner = first entry).
struct Row {
    label: &'static str,
    name: &'static str,
    table: BTreeSet<Characteristic>,
    expect: &'static [&'static str],
}

fn matrix() -> Vec<Row> {
    let row = |label, name, table, expect| Row {
        label,
        name,
        table,
        expect,
    };
    vec![
        // -- The contested 0xFFF0 block, LifeSpan role arrangement, all
        //    claimants by name. The fallback ALWAYS also accepts this shape;
        //    the named driver must sit in front of it, and only it.
        row(
            "named LifeSpan console",
            "LifeSpan-TM",
            lifespan_shape(),
            &["lifespan", "lifespan-fallback"],
        ),
        row(
            "ESP32-named LifeSpan module",
            "ESP32-treadmill",
            lifespan_shape(),
            &["lifespan", "lifespan-fallback"],
        ),
        row(
            "nameless FFF1/FFF2 device",
            "",
            lifespan_shape(),
            &["lifespan-fallback"],
        ),
        row(
            "unrecognised-name FFF1/FFF2 device",
            "Mystery Pad 3000",
            lifespan_shape(),
            &["lifespan-fallback"],
        ),
        row(
            "Urevo E1L",
            "URTM041",
            lifespan_shape(),
            &["urevo", "lifespan-fallback"],
        ),
        row(
            "Urevo E1L also exposing FTMS",
            "URTM041",
            with_ftms(lifespan_shape()),
            &["urevo", "ftms", "lifespan-fallback"],
        ),
        row(
            "Urevo Spacewalk 3S (plain FTMS despite the URTM name)",
            "URTM024",
            with_ftms(lifespan_shape()),
            &["ftms", "lifespan-fallback"],
        ),
        row(
            "Sperax RM01 (hyphen-less, proprietary)",
            "SPERAX_RM01_74FE70",
            lifespan_shape(),
            &["sperax", "lifespan-fallback"],
        ),
        row(
            "Sperax RM-02 (proprietary)",
            "SPERAX_RM-02_AB12",
            lifespan_shape(),
            &["sperax", "lifespan-fallback"],
        ),
        row(
            "Sperax RM-01 (hyphenated, FTMS hardware)",
            "SPERAX_RM-01_74FE70",
            with_ftms(lifespan_shape()),
            &["ftms", "lifespan-fallback"],
        ),
        row(
            "FitShow FS module on FFF0",
            "FS-3D6CD7",
            lifespan_shape(),
            &["fitshow", "lifespan-fallback"],
        ),
        row(
            "FitShow FS module on FFF0 alongside FTMS (steps beat FTMS)",
            "FS-3D6CD7",
            with_ftms(lifespan_shape()),
            &["fitshow", "ftms", "lifespan-fallback"],
        ),
        row(
            "Tunturi T80 alongside FTMS",
            "TUNTURI T80-1",
            with_ftms(lifespan_shape()),
            &["fitshow", "ftms", "lifespan-fallback"],
        ),
        row(
            "PitPat name on LifeSpan roles (unknown table → benign fallback)",
            "PitPat-T01",
            lifespan_shape(),
            &["lifespan-fallback"],
        ),
        // -- The contested block, Deerrun (swapped) arrangement.
        row(
            "named Deerrun pad (swapped roles)",
            "PitPat-T01",
            deerrun_shape(),
            &["pitpat"],
        ),
        row(
            "nameless swapped-roles device stays unclaimed",
            "",
            deerrun_shape(),
            &[],
        ),
        row(
            "LifeSpan name cannot claim swapped roles",
            "LifeSpan-TM",
            deerrun_shape(),
            &[],
        ),
        row(
            "Urevo name cannot claim swapped roles",
            "URTM041",
            deerrun_shape(),
            &[],
        ),
        row(
            "FitShow name cannot claim swapped roles",
            "FS-3D6CD7",
            deerrun_shape(),
            &[],
        ),
        // -- PitPat's other transports.
        row(
            "PitPat on its native FBA0",
            "PitPat-T01",
            pitpat_fba0(),
            &["pitpat"],
        ),
        row(
            "nameless FBA0 (distinctive)",
            "",
            pitpat_fba0(),
            &["pitpat"],
        ),
        row(
            "PitPat on the SupeRun FFFF layout",
            "PITPAT-T02",
            pitpat_ffff(),
            &["pitpat"],
        ),
        row(
            "PitPat on the 1910 layout",
            "PITPAT-T02",
            pitpat_1910(),
            &["pitpat"],
        ),
        row("nameless FFFF stays unclaimed", "", pitpat_ffff(), &[]),
        row("nameless 1910 stays unclaimed", "", pitpat_1910(), &[]),
        row(
            "the PitPat BIKE gets no driver",
            "PITPAT-S1",
            pitpat_fba0(),
            &[],
        ),
        // -- KingSmith, all three generations.
        row(
            "WiLink WalkingPad",
            "WalkingPad A1",
            wilink_shape(),
            &["kingsmith-wilink"],
        ),
        row(
            "WiLink pad also exposing FTMS (native reports steps)",
            "WalkingPad A1",
            with_ftms(wilink_shape()),
            &["kingsmith-wilink", "ftms"],
        ),
        row(
            "nameless WiLink shape",
            "",
            wilink_shape(),
            &["kingsmith-wilink"],
        ),
        row(
            "app-cipher pad on the props layout",
            "KS-NGCH-G1C",
            props_shape(),
            &["kingsmith-props"],
        ),
        row(
            "nameless props layout (distinctive)",
            "",
            props_shape(),
            &["kingsmith-props"],
        ),
        row(
            "app-cipher name on a WiLink table: carved out of both",
            "KS-HDSY-X21C",
            wilink_shape(),
            &[],
        ),
        row(
            "WiLink name on the props layout: refused",
            "WalkingPad A1",
            props_shape(),
            &[],
        ),
        row(
            "the FTMS WalkingPad Z1 goes to FTMS",
            "KS-HD-Z1D",
            ftms_only(),
            &["ftms"],
        ),
        // -- FitShow's other transports and its FTMS split.
        row("FitShow on AE00", "FS-3D6CD7", fitshow_ae00(), &["fitshow"]),
        row(
            "FitShow on FFE0 (notify FFE4)",
            "FS-3D6CD7",
            fitshow_ffe0(),
            &["fitshow"],
        ),
        row(
            "NoblePro without FTMS keeps the native protocol",
            "NOBLEPRO CONNECT 1",
            fitshow_ae00(),
            &["fitshow"],
        ),
        row(
            "NoblePro with FTMS yields to FTMS",
            "NOBLEPRO CONNECT 1",
            with_ftms(fitshow_ae00()),
            &["ftms"],
        ),
        row(
            "modern FS-BT-C1: FTMS + notify-only vendor FFF1",
            "FS-AB12CD",
            {
                let mut g = gatt(&[(0xfff0, 0xfff1, N)]);
                g.insert(chr(sig_uuid(0x1826), sig_uuid(0x2acd), N));
                g
            },
            &["ftms"],
        ),
        // -- Non-treadmills and pathological tables match nothing.
        row("plain FTMS treadmill", "", ftms_only(), &["ftms"]),
        row(
            "heart-rate monitor",
            "Polar H10",
            gatt(&[(0x180d, 0x2a37, N)]),
            &[],
        ),
        row("empty GATT table", "", BTreeSet::new(), &[]),
        row(
            "empty GATT table with a treadmill name",
            "LifeSpan-TM",
            BTreeSet::new(),
            &[],
        ),
        row(
            "notify-only half of the LifeSpan shape",
            "",
            gatt(&[(0xfff0, 0xfff1, N)]),
            &[],
        ),
        row(
            "UUIDs present but zero properties",
            "LifeSpan-TM",
            gatt(&[
                (0xfff0, 0xfff1, CharPropFlags::empty()),
                (0xfff0, 0xfff2, CharPropFlags::empty()),
            ]),
            &[],
        ),
        // -- Foreign names on foreign tables never cross-claim.
        row(
            "Urevo name on a WiLink table",
            "URTM041",
            wilink_shape(),
            &[],
        ),
        row(
            "Sperax name on a WiLink table",
            "SPERAX_RM01",
            wilink_shape(),
            &[],
        ),
        row(
            "LifeSpan name on a PitPat table",
            "LifeSpan-TM",
            pitpat_fba0(),
            &[],
        ),
    ]
}

/// The matrix itself: winner AND full supporter set, per row.
#[test]
fn every_representative_device_lands_on_exactly_the_intended_driver() {
    for r in matrix() {
        let a = adv(r.name);
        let got = supporters(&a, &r.table);
        assert_eq!(
            got, r.expect,
            "{}: supporter set mismatch (name={:?})",
            r.label, r.name
        );
        let winner = drivers::for_device(&a, &r.table).map(|d| d.id());
        assert_eq!(
            winner,
            r.expect.first().copied(),
            "{}: for_device disagrees with the supporter set",
            r.label
        );
    }
}

/// Registry ordering invariants: unique ids, the permissive fallback dead
/// last, and — via the matrix above — no driver shadowed for a device it
/// should own (the intended driver is always the FIRST supporter).
#[test]
fn registry_ids_are_unique_and_ordered() {
    let ids = drivers::ids();
    let unique: BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "duplicate driver id in {ids:?}");
    assert_eq!(ids.last(), Some(&"lifespan-fallback"));
    // The fallback accepts anything LifeSpan-shaped, so nothing may sit
    // behind it: assert no OTHER driver would accept the plain fallback
    // shape without a name (it could then never win).
    let anon = adv("");
    for d in &DRIVERS[..DRIVERS.len() - 1] {
        assert!(
            !d.supports(&anon, &lifespan_shape()) || d.id() == "lifespan",
            "{} would contest the nameless FFF1/FFF2 shape the fallback owns",
            d.id()
        );
    }
}

// ---- Name-list disjointness --------------------------------------------------

/// Instantiate a prefix as a realistic advertised name (prefix + suffix),
/// except the handful of names that only match exactly.
fn instantiations(prefix: &str) -> Vec<String> {
    vec![prefix.to_string(), format!("{prefix}1A2B")]
}

/// Which drivers' scan-time `matches()` claim this advertised name (no
/// services advertised — the name alone).
fn name_claimants(name: &str) -> Vec<&'static str> {
    DRIVERS
        .iter()
        .filter(|d| d.matches(&adv(name)))
        .map(|d| d.id())
        .collect()
}

/// No advertised name is claimed by two drivers at scan time, with one
/// documented exception: `URTM041…` matches both Urevo (its verified native
/// name) and FTMS (whose real-world list carries the broader `URTM` prefix);
/// the registry order resolves that pair deliberately (native outranks FTMS).
#[test]
fn name_lists_are_disjoint_across_drivers() {
    use trot_core::drivers::{fitshow, ftms, kingsmith_props, kingsmith_wilink, lifespan};
    use trot_core::drivers::{pitpat, sperax, urevo};

    // (owner id, names instantiated from that driver's own pub name lists)
    let mut claims: Vec<(&str, Vec<String>)> = Vec::new();

    let expand = |prefixes: &[&str]| -> Vec<String> {
        prefixes.iter().flat_map(|p| instantiations(p)).collect()
    };

    claims.push(("lifespan", expand(lifespan::ADV_NAME_PREFIXES)));
    let mut wilink = expand(kingsmith_wilink::ADV_NAME_PREFIXES);
    wilink.extend(
        kingsmith_wilink::ADV_NAME_EXACT
            .iter()
            .map(|s| s.to_string()),
    );
    claims.push(("kingsmith-wilink", wilink));
    claims.push((
        "kingsmith-props",
        expand(kingsmith_props::ADV_NAME_PREFIXES),
    ));
    claims.push(("urevo", expand(urevo::ADV_NAME_PREFIXES)));
    claims.push(("sperax", expand(sperax::ADV_NAME_PREFIXES)));
    claims.push(("pitpat", expand(pitpat::ADV_NAME_PREFIXES)));
    let mut fs = expand(fitshow::ADV_NAME_PREFIXES_NATIVE);
    fs.extend(expand(fitshow::ADV_NAME_PREFIXES_FTMS_PREFERRED));
    claims.push(("fitshow", fs));
    claims.push(("ftms", expand(ftms::ADV_NAME_PREFIXES)));

    for (owner, names) in &claims {
        for name in names {
            let got = name_claimants(name);
            let expected: Vec<&str> = if name.to_ascii_uppercase().starts_with("URTM041") {
                vec!["urevo", "ftms"] // the documented deliberate overlap
            } else {
                vec![owner]
            };
            assert_eq!(
                got, expected,
                "advertised name {name:?} (from {owner}'s list) is claimed by {got:?}"
            );
        }
    }
}

/// The KingSmith three-way split (WiLink / app-cipher props / FTMS) orphans
/// nothing: every KingSmith-family name in any driver's list is claimed by
/// exactly one of the three, and the FTMS Z1 carve-out reaches FTMS only.
#[test]
fn the_kingsmith_split_covers_every_name_exactly_once() {
    use trot_core::drivers::{ftms, kingsmith_props, kingsmith_wilink};

    let mut ks_names: Vec<String> = Vec::new();
    for p in kingsmith_wilink::ADV_NAME_PREFIXES {
        ks_names.extend(instantiations(p));
    }
    for p in kingsmith_props::ADV_NAME_PREFIXES {
        ks_names.extend(instantiations(p));
    }
    // The KingSmith-named FTMS models from the FTMS list.
    for p in ftms::ADV_NAME_PREFIXES
        .iter()
        .filter(|p| p.starts_with("KS-"))
    {
        ks_names.extend(instantiations(p));
    }

    for name in &ks_names {
        let claimants = name_claimants(name);
        assert_eq!(
            claimants.len(),
            1,
            "KingSmith name {name:?} claimed by {claimants:?} — the generation \
             split must assign every name to exactly one driver"
        );
    }
    // The carve-out lists agree in both directions (also pinned inside the
    // props driver; restated here as the system-level statement).
    for excl in kingsmith_wilink::ADV_NAME_EXCLUDE_PREFIXES {
        let claimants = name_claimants(excl);
        assert!(
            !claimants.contains(&"kingsmith-wilink"),
            "WiLink still claims its own carve-out {excl:?}"
        );
        assert!(
            claimants.len() <= 1,
            "carved-out name {excl:?} claimed by {claimants:?}"
        );
    }
}
