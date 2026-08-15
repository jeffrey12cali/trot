//! No driver panics on hostile input — the system-level fuzz table.
//!
//! Every inbound parser in the driver layer is fed the same corpus: empty
//! input, single bytes, all-zeros and all-0xFF at many lengths (including
//! far beyond any real frame), deterministic pseudo-random noise, every
//! truncation of every driver's *valid* fixture frame, every single-byte
//! corruption of those fixtures, and every fixture with trailing junk. A
//! malfunctioning or malicious peripheral is a legitimate threat model
//! (SECURITY.md): one bad notification must never take the daemon down.
//!
//! Nothing here asserts *values* — the per-driver suites do that. The only
//! assertion is that every call returns (Ok or Err, never a panic), which
//! the test harness enforces by completing.

use trot_core::drivers::{ftms, kingsmith_wilink, lifespan};
use trot_core::drivers::{pitpat, sperax, urevo};

fn hx(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// One valid frame per driver/characteristic, from the drivers' own test
/// fixtures — the seeds for the truncation/corruption corpus.
fn valid_fixtures() -> Vec<Vec<u8>> {
    let mut v = vec![
        // LifeSpan: steps, status, duration responses.
        hx("a1 aa 00 23 00 00"),
        hx("a1 aa 03 00 00 00"),
        hx("a1 aa 00 01 0b 00"),
        // WiLink: a real ph4-captured status frame.
        hx("f8 a2 01 3c 01 00 02 2a 00 00 4f 00 03 d1 b4 00 00 00 e3 fd"),
        // Urevo: a real treadspan-captured running frame + the standby frame.
        hx("02 51 03 0e 00 45 01 0b 00 80 00 6b 01 00 00 00 00 fb 03"),
        hx("02 51 00 00 09 03"),
        // PitPat: the real azmke idle capture (52 bytes).
        hx(
            "68 34 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
            00 00 2a 1b 00 17 70 00 05 00 74 6c 4b 61 31 39 31 66 55 70 54 73 \
            73 36 30 33 0e 00 40 43",
        ),
        // Sperax: a synthetic 24-byte packet in upstream's field map.
        {
            let mut f = vec![0u8; 24];
            f[0] = 0xF5;
            f[1] = 24;
            f[15] = 0x02;
            f[16] = 0x01;
            f[17] = 27;
            f[23] = 0xFA;
            f
        },
        // FTMS machine status: target speed change.
        hx("05 90 01"),
    ];

    // An FTMS Treadmill Data frame with the KingSmith extension (0x2484).
    let mut ks = Vec::new();
    ks.extend_from_slice(&0x2484u16.to_le_bytes());
    ks.extend_from_slice(&400u16.to_le_bytes());
    ks.extend_from_slice(&[0x10, 0x27, 0x00]);
    ks.extend_from_slice(&100u16.to_le_bytes());
    ks.extend_from_slice(&200u16.to_le_bytes());
    ks.push(4);
    ks.extend_from_slice(&2400u16.to_le_bytes());
    ks.extend_from_slice(&4321u16.to_le_bytes());
    ks.push(0);
    v.push(ks);
    v
}

/// The hostile corpus: noise of many shapes and sizes plus every truncation,
/// single-byte corruption and extension of every valid fixture.
fn corpus() -> Vec<Vec<u8>> {
    let mut c: Vec<Vec<u8>> = vec![vec![], vec![0x00], vec![0xFF], vec![0xA1], vec![0x02]];

    for len in [
        2usize, 3, 4, 5, 6, 7, 8, 12, 19, 20, 23, 24, 30, 31, 52, 64, 255, 1024, 4096,
    ] {
        c.push(vec![0x00; len]);
        c.push(vec![0xFF; len]);
        c.push((0..len).map(|i| (i * 37 % 251) as u8).collect());
    }

    // Deterministic pseudo-random frames (splitmix64).
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as u8
    };
    for _ in 0..200 {
        let len = next() as usize % 70;
        c.push((0..len).map(|_| next()).collect());
    }

    for fixture in valid_fixtures() {
        // Every truncation (including the empty prefix).
        for cut in 0..fixture.len() {
            c.push(fixture[..cut].to_vec());
        }
        // Every single-byte corruption, three flavours per position.
        for i in 0..fixture.len() {
            for mask in [0x01u8, 0x80, 0xFF] {
                let mut m = fixture.clone();
                m[i] ^= mask;
                c.push(m);
            }
        }
        // The valid frame with trailing junk.
        let mut long = fixture.clone();
        long.extend_from_slice(&[0xAA; 40]);
        c.push(long);
        c.push(fixture);
    }
    c
}

/// Every inbound parser in the tree, run over the whole corpus. The results
/// are deliberately discarded — completing without a panic IS the test.
#[test]
fn no_inbound_parser_panics_on_hostile_input() {
    let inputs = corpus();

    // LifeSpan: the incremental reader, across every opcode the driver polls
    // plus never-polled opcodes (the response does not echo the opcode, so a
    // confused link can pair any frame with any opcode).
    let opcodes = [
        lifespan::OPCODE_SPEED,
        lifespan::OPCODE_DISTANCE,
        lifespan::OPCODE_CALORIES,
        lifespan::OPCODE_STEPS,
        lifespan::OPCODE_DURATION,
        lifespan::OPCODE_STATUS,
        0x00,
        0xFF,
    ];
    let mut reader = lifespan::Reader::new();
    for input in &inputs {
        for op in opcodes {
            let _ = reader.feed(op, input);
        }
    }

    // FTMS: Treadmill Data with and without the KingSmith gate, and the
    // Fitness Machine Status characteristic.
    for input in &inputs {
        let _ = ftms::parse_treadmill_data(input);
        let _ = ftms::parse_treadmill_data_ext(input, true);
        let _ = ftms::parse_machine_status(input);
    }

    // KingSmith WiLink.
    for input in &inputs {
        let _ = kingsmith_wilink::parse_status(input);
    }

    // Urevo.
    for input in &inputs {
        let _ = urevo::parse_status(input);
    }

    // Sperax (plus its unescape helper, which sees wire bytes too).
    for input in &inputs {
        let _ = sperax::parse_status(input);
        let _ = sperax::unescape_frame(input);
    }

    // PitPat: bare parse plus both envelope interpretations.
    for input in &inputs {
        let _ = pitpat::parse_status(input);
        let _ = pitpat::decode_notification(input, true);
        let _ = pitpat::decode_notification(input, false);
    }
}

/// A valid frame with exactly one byte flipped must never decode as if it
/// were intact on the checksum-carrying protocols — this is the property
/// that keeps a corrupted counter out of someone's step history. (Flips in
/// framing bytes are rejected for framing reasons; flips in the payload are
/// rejected by the trailer; a flip in the checksum byte itself is rejected
/// because the payload no longer matches it.)
#[test]
fn single_byte_corruption_never_yields_the_original_values() {
    // WiLink: 20-byte frame, checksum over bytes 1..18 at byte 18.
    let wilink = hx("f8 a2 01 3c 01 00 02 2a 00 00 4f 00 03 d1 b4 00 00 00 e3 fd");
    let wl_ok = kingsmith_wilink::parse_status(&wilink).unwrap();
    for i in 0..wilink.len() {
        let mut m = wilink.clone();
        m[i] ^= 0x01;
        if let Ok(s) = kingsmith_wilink::parse_status(&m) {
            panic!("wilink accepted a corrupted frame (flip at {i}): {s:?} vs {wl_ok:?}");
        }
    }

    // Urevo: sum-xor-0x5A trailer.
    let urevo_f = hx("02 51 03 0e 00 45 01 0b 00 80 00 6b 01 00 00 00 00 fb 03");
    for i in 0..urevo_f.len() {
        let mut m = urevo_f.clone();
        m[i] ^= 0x01;
        assert!(
            urevo::parse_status(&m).is_err(),
            "urevo accepted a corrupted frame (flip at {i})"
        );
    }

    // PitPat: XOR trailer over bytes 1..=len-3.
    let idle = hx(
        "68 34 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
                   00 00 2a 1b 00 17 70 00 05 00 74 6c 4b 61 31 39 31 66 55 70 54 73 \
                   73 36 30 33 0e 00 40 43",
    );
    for i in 1..idle.len() {
        // byte 0 is deliberately unvalidated (no upstream checks the inbound
        // prefix), so start at 1: every OTHER flip must be rejected.
        let mut m = idle.clone();
        m[i] ^= 0x01;
        assert!(
            pitpat::parse_status(&m).is_err(),
            "pitpat accepted a corrupted frame (flip at {i})"
        );
    }
}
