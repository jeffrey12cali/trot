//! KingSmith "props" driver — the obfuscated **text key/value protocol** of the
//! app-cipher WalkingPad generation (R2, X21, X23, G1, and the Xiaomi-branded
//! K12 Pro: `KS-X21*`, `KS-R1AC`, `KS-HC-R1A*`, `KS-HDSC/HDSY/NACH/NGCH-X21C`,
//! `KS-NACH-MXG`, `KS-NGCH-G1C`, `KS-ST-K12PRO`).
//!
//! ## Sources (see THIRD-PARTY-NOTICES.md)
//!
//! * **cagnulein/qdomyos-zwift** (GPL-3.0, same license as Trot) —
//!   `src/devices/kingsmithr2treadmill/kingsmithr2treadmill.{cpp,h}`: the
//!   primary source. The seven substitution-cipher tables, the transport
//!   pipeline (UTF-8 → base64 → per-character substitution → `0x0D`
//!   terminator → 16-byte GATT chunks, write-without-response), the three
//!   service/characteristic address spaces with their per-model routing and
//!   fallbacks, the init message sequence with the observed reply to each
//!   frame (its inline comments), the `props <key> <value>…` response
//!   grammar with its `Error`/`mcu_version`/`goal` special cases, the
//!   telemetry key list, and the advertised-name matcher
//!   (`src/devices/bluetooth.cpp`). Its belt-control paths — the
//!   `props CurrentSpeed/runState/ControlMode <value>` setter writes — are
//!   deliberately not ported (see the actuation boundary below).
//! * **LucasFrendorf/walkingpad-ble-footpod** (GPL-3.0) —
//!   `kingsmith_g1c.py`: an independent client for the KS-NGCH-G1C built on
//!   the same protocol facts, and the only source that *polls* for
//!   telemetry: its monitor loop re-sends `servers getProp …` at 1 Hz and
//!   reads the full state from the `props` replies, which is what
//!   establishes the poll-driven steady state this driver uses. It also
//!   confirms both address-space variants on real G1C hardware revisions,
//!   the 16-byte write-without-response chunking, and the G1C's v6 cipher
//!   table. Its belt-control writes are likewise not ported.
//!
//! One further public implementation exists (a Kotlin Android client) but
//! carries **no license**, so per this project's rules nothing was taken
//! from it — it is noted here only as evidence the protocol facts are
//! reproducible. No implementation of this protocol has published a raw
//! capture; every fixture in the tests below is synthetic, built from the
//! two licensed sources' shared facts, and says so.
//!
//! ## The transport (docs/drivers/README.md protocol shape 4)
//!
//! ```text
//! plaintext  "props CurrentSpeed 3.5"
//!    ↓ UTF-8 → base64 (standard alphabet, `=` padding)
//!    ↓ per-character substitution through a 65-entry cipher table
//!    ↓ append 0x0D terminator
//!    ↓ split into 16-byte GATT writes (write-without-response)
//! ```
//!
//! Inbound reverses it: notifications are chunks, buffered until `0x0D`
//! ([`util::FrameAssembler`]), inverse-substituted and base64-decoded
//! ([`util::TransportCodec`] — [`AppCipherCodec`]), then parsed as text.
//! Three GATT layouts carry the protocol ([`Transport`]): the plain 16-bit
//! block (service `0x1234`, write `0xFED7`, notify `0xFED8`) and two
//! "address-spaced" 128-bit variants (`0001xxxx` and `0002xxxx` of the same
//! three words). qdomyos routes models to spaces by name *with fallbacks
//! because the routing misses* (its X21C/G1C handlers retry the other
//! space); Trot probes the actual GATT table instead and requires the
//! characteristics to sit under their own variant's service with the right
//! roles.
//!
//! ## The seven cipher tables, and how the driver picks one
//!
//! Seven per-model tables exist (v1–v7), each a permutation of the base64
//! alphabet differing from its siblings by only 1–3 transposed characters,
//! with `=` a fixed point in all seven — so a frame decoded with the wrong
//! table is still structurally valid base64 and only the decoded *content*
//! can tell tables apart. qdomyos cannot auto-detect the table; it ships
//! the choice as a user setting. Trot has no user-settings mechanism for
//! driver internals and does not want one, so this driver **detects the
//! table from the traffic** ([`TableDetector`]), which the table structure
//! makes sound in practice:
//!
//! * A full telemetry line (`props CurrentSpeed … RunningSteps …`) decodes
//!   to printable ASCII under **exactly one** table — all six wrong decodes
//!   contain non-printable bytes (verified exhaustively in the tests for
//!   every true-table choice). One real telemetry frame therefore
//!   identifies the table outright.
//! * Short frames genuinely can be ambiguous: `shake 00` decodes under a
//!   wrong table to the printable `shake 0?`, and a `… goal 0` line
//!   encoded with v5 decodes under v1/v3/v6 to the equally plausible
//!   `… goal 8` — same grammar, different *value*. No content check can
//!   catch that, so the detector never trusts a single frame: it keeps all
//!   seven tables as candidates, eliminates a candidate only when a
//!   strongly-structured frame (a recognised message word) decodes strictly
//!   better under its rivals, and **uses a frame only when every surviving
//!   candidate decodes it identically** ([`Decoded::Agreed`]). A frame the
//!   survivors disagree on is skipped — absence over possibly-wrong values.
//! * The outbound side self-corrects: `servers getProp …` encodes
//!   **differently under every pair of tables** (pinned in the tests), and
//!   the device only answers a correctly-enciphered poll with `props` —
//!   wrong-table polls draw an error line or silence. The poll loop rotates
//!   through surviving candidates until `props` replies arrive, so even a
//!   device that never volunteers a discriminating frame converges. The
//!   init handshake needs no table at all: `""`, `shake`, `get_dn` and
//!   `get_pk` encipher identically under all seven tables (their base64
//!   avoids every transposed character — also pinned in tests).
//!
//! Model hints order the probing (qdomyos hard-codes v7 for `KS-NACH-MXG`
//! and defaults the G1 to v6, which the footpod client confirms on
//! hardware) but are never trusted as an answer.
//!
//! Residual risk, stated honestly: until a discriminating frame arrives,
//! surviving tables that disagree on a frame suppress it (data delayed, not
//! wrong), and a table pair whose disagreement only ever lands in numeric
//! digits could in principle survive an entire session undetected — the
//! agreement gate then reports nothing rather than guessing. In practice
//! every telemetry line contains long key names and locks the table on the
//! first frame.
//!
//! ## The query/actuation boundary
//!
//! This protocol makes the boundary unusually legible because it is text:
//! **the direction of `props` is the verb.** Inbound, `props <key> <value>…`
//! is the device *reporting* its properties. Outbound, the same shape is
//! the setter — qdomyos writes `props CurrentSpeed 3.5`,
//! `props runState 1`, `props ControlMode 1` to drive the belt. The reads
//! are the bare interrogatives (`shake`, `net`, `get_dn`, `get_pk`,
//! `version` — no operand, nothing to set) and the explicit property read
//! `servers getProp <ids…>`, which asks the device to *report* the listed
//! properties. Rule enforced here and pinned by the write-set test:
//! **this driver never writes a message starting with `props`**, and every
//! outbound message is in the fixed read vocabulary [`READ_MESSAGES`].
//!
//! Init frames, characterised one by one (qdomyos sends eight; the reply
//! comments are its own):
//!
//! * `""` (a bare `0x0D`) — **sent.** Carries zero bytes, so it provably
//!   cannot set anything. The device replies with a format-error line; the
//!   plausible function is flushing a half-buffered frame in the device's
//!   own terminator-scanning parser after a reconnect, and its reply is
//!   ciphertext this driver's table detection can use for free.
//! * `shake` — **sent.** A bare handshake interrogative (reply `shake 00`);
//!   no operand. Table-invariant on the wire.
//! * `net` — **sent.** Reads the connectivity mode (reply `net cloud`); no
//!   operand, value unused here.
//! * `get_dn` / `get_pk` — **sent.** Read device identifiers (replies
//!   `get_dn XXXX…` / `get_pk XXXX…`); no operand, values unused here.
//!   Table-invariant on the wire.
//! * `time_posix <unix>` — **never sent.** This one is a *write*: it
//!   carries a value and sets the device's clock. Both upstreams send it;
//!   neither shows the telemetry stream depending on it (the reply is a
//!   bare ack, and `servers getProp` is what produces telemetry). The same
//!   call five drivers have now made — WiLink's `B1`/`B3`, Sperax's `0x13`,
//!   PitPat's `D7`, FitShow's clock-carrying model query — an
//!   observe-only driver does not set a clock. If some model's stream
//!   provably won't start without it, reopen with a capture.
//! * `version` — **sent.** Reads the firmware version (reply
//!   `version 0014`); no operand.
//! * `servers getProp 1 2 7 12 23 24 31` — **sent**, at init and as the
//!   steady-state poll. An explicit read request for the listed property
//!   ids (the id→key map is unpublished; the reply is a `props` line with
//!   the telemetry keys). Byte-for-byte qdomyos' and the footpod client's
//!   frame.
//!
//! ## Payload keys, per-field provenance
//!
//! qdomyos' *code* consumes only `CurrentSpeed` (km/h, compared against
//! its 0–22 km/h request range), `spm`, `ControlMode` and `runState`
//! (enums below) — it fabricates distance by integrating speed, so the
//! counter-field semantics rest on its inline comments, which the footpod
//! client's field comments independently restate (weaker evidence than
//! code, and flagged as such):
//!
//! * `CurrentSpeed` — km/h, decimal string (code-level in both sources).
//! * `RunningDistance` — metres, "update each 10 m / 0.01 mile"
//!   (comment-level, both sources).
//! * `RunningSteps` — steps (comment-level, both sources).
//! * `BurnCalories` — kcal × 1000 (comment-level, both sources; the
//!   division is done here, so a wrong claim under-reports by 1000× rather
//!   than over-reporting — no capture exists to verify. If your console
//!   shows calories and Trot shows ~0, send a capture).
//! * `RunningTotalTime` — seconds (comment-level, both sources).
//! * `runState` — 0 stop, 1 start (code-level: qdomyos' enum) →
//!   [`BeltState`] via [`belt_state`], everything else passed through raw.
//! * Parsed but not surfaced: none. Unparsed on purpose: `spm` (cadence —
//!   no `Sample` field), `ControlMode` (0 auto / 1 manual / 2 standby —
//!   drives qdomyos' UI, not a belt state), `mcu_version`, `goal` (string
//!   params both upstreams skip), `Error` (terminates parsing — its quoted
//!   payload breaks the key/value pairing, per qdomyos), and any unknown
//!   key (skipped pairwise, never fatal).
//!
//! The device pushes `props` updates on its own after init on at least
//! some models (qdomyos never polls and still receives updates) and
//! answers `servers getProp` on request (the footpod client's 1 Hz
//! monitor); this driver takes both — subscribe-and-push with a 1 Hz
//! `getProp` nudge when the stream is quiet.

use super::util::{run_init_sequence, FrameAssembler, InitStep, TransportCodec};
use super::{Advertisement, BeltState, Driver, DriverHost, Emit, Sample};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use btleplug::api::{CharPropFlags, Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use futures::{FutureExt, StreamExt};
use std::collections::BTreeSet;
use std::time::Duration;
use uuid::Uuid;

// ---- UUIDs: the three address-space variants ---------------------------------

/// Full 128-bit form of a 32-bit word on the Bluetooth base UUID —
/// `0x0001FED7` → `0001fed7-0000-1000-8000-00805f9b34fb`. The 16-bit SIG
/// helper is the `0x0000xxxx` special case of this.
const fn base_uuid(word: u32) -> Uuid {
    Uuid::from_u128(((word as u128) << 96) | 0x0000_1000_8000_0080_5f9b_34fb)
}

/// Plain 16-bit block: service 0x1234, write FED7, notify FED8 (the default
/// layout — K12 Pro, R1AC, the plain X21, HDSC/HDSY models).
pub const STD_SERVICE_UUID: Uuid = super::sig_uuid(0x1234);
pub const STD_WRITE_UUID: Uuid = super::sig_uuid(0xfed7);
pub const STD_NOTIFY_UUID: Uuid = super::sig_uuid(0xfed8);

/// Address space 0001 (G1C revision 1, NACH-MXG, some NACH-X21C).
pub const SPACE1_SERVICE_UUID: Uuid = base_uuid(0x0001_1234);
pub const SPACE1_WRITE_UUID: Uuid = base_uuid(0x0001_fed7);
pub const SPACE1_NOTIFY_UUID: Uuid = base_uuid(0x0001_fed8);

/// Address space 0002 (G1C revision 2, some HDSY-X21C).
pub const SPACE2_SERVICE_UUID: Uuid = base_uuid(0x0002_1234);
pub const SPACE2_WRITE_UUID: Uuid = base_uuid(0x0002_fed7);
pub const SPACE2_NOTIFY_UUID: Uuid = base_uuid(0x0002_fed8);

// ---- Advertised names -------------------------------------------------------
//
// From qdomyos' device router (src/devices/bluetooth.cpp). These names are
// the whole adjudication against the KingSmith WiLink driver: every name
// here that collides with a WiLink prefix (`KS-H…`) appears in WiLink's
// ADV_NAME_EXCLUDE_PREFIXES, and a test below pins the two drivers'
// agreement in both directions — no device claimed twice, none orphaned.

/// Name prefixes of the app-cipher generation.
pub const ADV_NAME_PREFIXES: &[&str] = &[
    "KS-ST-K12PRO", // Xiaomi K12 Pro
    "KS-R1AC",      // WalkingPad R2
    "KS-HC-R1A",    // WalkingPad R2 (KS-HC-R1AA / KS-HC-R1AC)
    "KS-X21",       // WalkingPad X21
    "KS-HDSC-X21C",
    "KS-HDSY-X21C",
    "KS-NACH-X21C",
    "KS-NGCH-X21C",
    "KS-NACH-MXG", // X23
    "KS-NGCH-G1C", // G1
];

// ---- The cipher --------------------------------------------------------------

/// One frame's terminator, both directions. Never occurs inside a frame:
/// the payload is base64 text, so terminator-scanning reassembly is safe
/// (the `FrameAssembler` precondition).
pub const TERMINATOR: u8 = 0x0D;
/// Outbound frames are split into GATT writes of at most this many bytes.
pub const CHUNK_LEN: usize = 16;

/// The substitution alphabet: base64 plus the `=` pad, in base64 order.
pub const BASE64_ALPHABET: &[u8; 65] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";

pub const TABLE_COUNT: usize = 7;

/// The seven per-model cipher tables (qdomyos' `ENCRYPT_TABLE`[_v2..v7]),
/// each a permutation of [`BASE64_ALPHABET`] (verified in tests) applied
/// character-wise to the base64 text. Index 0 = v1 (the default), …,
/// index 6 = v7 (`KS-NACH-MXG`).
pub const CIPHER_TABLES: [&[u8; 65]; TABLE_COUNT] = [
    b"SaCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5078KZ3=",
    b"ZaCw4FGHIJqLhN+P9RMTU/WcY6ObDdefgEijklmnopQrsBuvVxXz1yA2t5078KS3=",
    b"0aCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5Z78KS3=",
    b"ZaCw4FGHIJqLhN9P+RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5078KS3=",
    b"iaCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEZjklmnopQrsBuvMxXz1yA2t5078KS3=",
    b"ZaCw4FGHIJqLhN+P8RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5079KS3=",
    b"baCw4FGHIJqLhN+P9RVTU/WcY6OZDdefgEijklmnopQrsBuvMxXz1yA2t5078KS3=",
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("byte 0x{0:02x} is not in the cipher alphabet")]
    BadCipherByte(u8),
    #[error("base64 length {0} is not a multiple of 4")]
    BadBase64Length(usize),
    #[error("byte 0x{0:02x} is not a base64 character")]
    BadBase64Byte(u8),
    #[error("misplaced base64 padding")]
    BadBase64Padding,
}

// A tiny standard-alphabet base64, written here rather than pulled in as a
// dependency: the whole need is canonical encode/decode of short ASCII
// strings.

/// Base64-encode (standard alphabet, canonical `=` padding).
pub fn base64_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let b = [
            group[0],
            *group.get(1).unwrap_or(&0),
            *group.get(2).unwrap_or(&0),
        ];
        let quad = [
            b[0] >> 2,
            ((b[0] & 0x03) << 4) | (b[1] >> 4),
            ((b[1] & 0x0F) << 2) | (b[2] >> 6),
            b[2] & 0x3F,
        ];
        let chars = 1 + group.len(); // 2, 3 or 4 significant output chars
        for (i, q) in quad.iter().enumerate() {
            out.push(if i < chars {
                BASE64_ALPHABET[*q as usize]
            } else {
                b'='
            });
        }
    }
    out
}

fn base64_value(c: u8) -> Result<u8, ProtocolError> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        other => Err(ProtocolError::BadBase64Byte(other)),
    }
}

/// Base64-decode (standard alphabet; `=` only as 1–2 trailing pad chars).
pub fn base64_decode(text: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if text.len() % 4 != 0 {
        return Err(ProtocolError::BadBase64Length(text.len()));
    }
    let pad = text.iter().rev().take_while(|&&c| c == b'=').count();
    if pad > 2 || text[..text.len() - pad].contains(&b'=') {
        return Err(ProtocolError::BadBase64Padding);
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for group in text.chunks(4) {
        let mut v = [0u8; 4];
        let significant = group.iter().take_while(|&&c| c != b'=').count();
        for (i, &c) in group.iter().enumerate().take(significant) {
            v[i] = base64_value(c)?;
        }
        out.push((v[0] << 2) | (v[1] >> 4));
        if significant > 2 {
            out.push((v[1] << 4) | (v[2] >> 2));
        }
        if significant > 3 {
            out.push((v[2] << 6) | v[3]);
        }
    }
    Ok(out)
}

/// Plaintext bytes → wire bytes (before terminator/chunking): UTF-8 →
/// base64 → substitute through table `table`.
pub fn cipher_encode(table: usize, plain: &[u8]) -> Vec<u8> {
    let enc = CIPHER_TABLES[table];
    base64_encode(plain)
        .iter()
        .map(|c| {
            // base64_encode only emits alphabet characters, so the lookup
            // cannot fail.
            let idx = BASE64_ALPHABET
                .iter()
                .position(|p| p == c)
                .expect("base64 output");
            enc[idx]
        })
        .collect()
}

/// Wire bytes (terminator already stripped) → plaintext bytes: inverse
/// substitution through table `table`, then base64 decode.
pub fn cipher_decode(table: usize, wire: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let enc = CIPHER_TABLES[table];
    let b64: Vec<u8> = wire
        .iter()
        .map(|c| {
            enc.iter()
                .position(|e| e == c)
                .map(|idx| BASE64_ALPHABET[idx])
                .ok_or(ProtocolError::BadCipherByte(*c))
        })
        .collect::<Result<_, _>>()?;
    base64_decode(&b64)
}

/// The [`TransportCodec`] for this driver: one locked cipher table behind
/// the seam, so reassembly and parsing never learn the cipher exists. The
/// multi-candidate detection phase uses [`cipher_decode`] across tables
/// directly; once a table wins, this is the steady-state codec.
pub struct AppCipherCodec {
    pub table: usize,
}

impl TransportCodec for AppCipherCodec {
    fn decode(&self, raw: &[u8]) -> Result<Vec<u8>> {
        Ok(cipher_decode(self.table, raw)?)
    }
    fn encode(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(cipher_encode(self.table, plain))
    }
}

// ---- Messages ----------------------------------------------------------------

/// The property read request — asks the device to report the listed
/// property ids (byte-for-byte qdomyos' and the footpod client's frame; the
/// id→key map is unpublished). Sent at init and as the steady-state poll.
pub const MSG_GET_PROPS: &str = "servers getProp 1 2 7 12 23 24 31";

/// The init messages, in upstream order, minus `time_posix` (a clock-SET —
/// see the module docs) — every entry a bare read.
pub const INIT_MESSAGES: &[&str] = &["", "shake", "net", "get_dn", "get_pk", "version"];

/// The complete outbound vocabulary of this driver. The write-set test
/// pins that nothing outside this list is ever built, and that no entry
/// starts with `props` (the setter form) or `time_posix` (the clock set).
pub const READ_MESSAGES: &[&str] = &[
    "",
    "shake",
    "net",
    "get_dn",
    "get_pk",
    "version",
    MSG_GET_PROPS,
];

/// A whole outbound message as GATT writes: encode through the codec,
/// append the terminator, split into 16-byte chunks (the cipher runs over
/// the message, not the chunks).
pub fn wire_chunks(codec: &dyn TransportCodec, msg: &str) -> Result<Vec<Vec<u8>>> {
    let mut wire = codec.encode(msg.as_bytes())?;
    wire.push(TERMINATOR);
    Ok(wire.chunks(CHUNK_LEN).map(<[u8]>::to_vec).collect())
}

/// Pause after each init message for its reply (qdomyos waits 300 ms; the
/// replies also feed table detection once the loop drains them).
pub const INIT_REPLY_GAP_MS: u64 = 300;

/// The init handshake as [`InitStep`]s: each message chunked, a reply gap
/// after each message's final chunk. `time_posix` is deliberately absent.
pub fn init_steps(transport: Transport, table: usize, with_response: bool) -> Vec<InitStep> {
    let codec = AppCipherCodec { table };
    let mut steps = Vec::new();
    for msg in INIT_MESSAGES {
        let chunks = wire_chunks(&codec, msg).expect("encoding text messages cannot fail");
        let last = chunks.len() - 1;
        for (i, chunk) in chunks.into_iter().enumerate() {
            let mut step = InitStep::write(transport.write_uuid(), chunk);
            if !with_response {
                step = step.without_response();
            }
            if i == last {
                step = step.then_wait_ms(INIT_REPLY_GAP_MS);
            }
            steps.push(step);
        }
    }
    steps
}

// ---- Table detection ---------------------------------------------------------

/// `props` response keys both sources establish (see the module docs).
/// Used only to *score* candidate decodes — unknown keys still parse.
pub const KNOWN_KEYS: &[&str] = &[
    "CurrentSpeed",
    "RunningDistance",
    "RunningSteps",
    "BurnCalories",
    "RunningTotalTime",
    "spm",
    "runState",
    "ControlMode",
    "mcu_version",
    "goal",
    "Error",
];

/// Message words the device is known to lead a line with (each init read's
/// reply echoes its word, per qdomyos' reply comments).
const KNOWN_WORDS: &[&str] = &["props", "shake", "net", "get_dn", "get_pk", "version"];

fn key_shaped(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn value_shaped(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// How plausible a candidate decode is as a line of this protocol.
/// 0 = impossible (non-printable — the protocol is printable ASCII text);
/// ≥[`STRONG_SCORE`] = structurally recognised (leads with a known word).
pub fn plaintext_score(plain: &[u8]) -> u32 {
    if !plain.iter().all(|&b| (0x20..=0x7E).contains(&b)) {
        return 0;
    }
    // Printable ASCII is always valid UTF-8.
    let text = std::str::from_utf8(plain).expect("printable ASCII");
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let Some(first) = tokens.first() else {
        return 1; // empty or all-whitespace: unobjectionable, uninformative
    };
    let mut score = 1;
    if KNOWN_WORDS.contains(first) {
        score += 4;
    }
    if *first == "props" {
        let mut i = 1;
        while i < tokens.len() {
            let key = tokens[i];
            score += if KNOWN_KEYS.contains(&key) {
                4
            } else if key_shaped(key) {
                1
            } else {
                0
            };
            if let Some(value) = tokens.get(i + 1) {
                if value_shaped(value) {
                    score += 1;
                }
            }
            i += 2;
        }
    } else {
        score += tokens[1..].iter().filter(|t| value_shaped(t)).count() as u32;
    }
    score
}

/// A decode is "strong" when it leads with a recognised message word —
/// random radio corruption essentially cannot reach this, which is what
/// makes it safe to *eliminate* rival tables on such frames.
pub const STRONG_SCORE: u32 = 5;

/// What one observed frame yielded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// Every surviving candidate table decodes this frame to the same
    /// plaintext — safe to use.
    Agreed(Vec<u8>),
    /// Surviving candidates disagree and none is provably right: the frame
    /// is skipped (absence over possibly-wrong values). A later frame with
    /// more discriminating content resolves the split.
    Ambiguous,
    /// No candidate produces printable text — radio noise or a corrupt
    /// frame; no candidate is eliminated by it.
    Garbage,
}

/// Cross-frame cipher-table detection: starts with all seven tables as
/// candidates, eliminates a candidate only when a strongly-structured frame
/// decodes strictly better under a rival, and releases a frame's plaintext
/// only on unanimous agreement among survivors. See the module docs for
/// why per-frame detection would be unsound.
#[derive(Debug)]
pub struct TableDetector {
    alive: [bool; TABLE_COUNT],
}

impl Default for TableDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TableDetector {
    pub fn new() -> Self {
        TableDetector {
            alive: [true; TABLE_COUNT],
        }
    }

    pub fn is_alive(&self, table: usize) -> bool {
        self.alive[table]
    }

    pub fn alive_count(&self) -> usize {
        self.alive.iter().filter(|a| **a).count()
    }

    /// The single surviving table, once detection has converged.
    pub fn locked(&self) -> Option<usize> {
        let mut it = (0..TABLE_COUNT).filter(|&t| self.alive[t]);
        match (it.next(), it.next()) {
            (Some(t), None) => Some(t),
            _ => None,
        }
    }

    /// Feed one reassembled (terminator-stripped) ciphertext frame.
    pub fn observe(&mut self, wire: &[u8]) -> Decoded {
        let decoded: Vec<(usize, u32, Vec<u8>)> = (0..TABLE_COUNT)
            .filter(|&t| self.alive[t])
            .map(|t| {
                let plain = cipher_decode(t, wire).unwrap_or_default();
                let score = if plain.is_empty() && !wire.is_empty() {
                    0 // failed decode (a non-empty frame never decodes empty)
                } else {
                    plaintext_score(&plain)
                };
                (t, score, plain)
            })
            .collect();
        let max = decoded.iter().map(|(_, s, _)| *s).max().unwrap_or(0);
        if max == 0 {
            return Decoded::Garbage;
        }
        if max >= STRONG_SCORE {
            // A structurally recognised decode exists: rivals that decode
            // this frame strictly worse cannot be the device's table.
            for (t, score, _) in &decoded {
                if *score < max {
                    self.alive[*t] = false;
                }
            }
        }
        let mut winners = decoded
            .iter()
            .filter(|(t, s, _)| self.alive[*t] && *s == max)
            .map(|(_, _, p)| p);
        let Some(first) = winners.next() else {
            return Decoded::Garbage; // unreachable: the max scorer survives
        };
        if winners.all(|p| p == first) {
            Decoded::Agreed(first.clone())
        } else {
            Decoded::Ambiguous
        }
    }

    /// Which table to encipher the next poll with: the hint first, then the
    /// other survivors round-robin as consecutive polls go unanswered — the
    /// behavioural probe (only the device's own table draws a `props`
    /// reply, because `servers getProp` enciphers differently under every
    /// pair of tables).
    pub fn poll_table(&self, hint: usize, unanswered_polls: usize) -> usize {
        let order: Vec<usize> = std::iter::once(hint)
            .chain((0..TABLE_COUNT).filter(|&t| t != hint))
            .filter(|&t| self.alive[t])
            .collect();
        if order.is_empty() {
            return hint; // unreachable: elimination never empties the set
        }
        order[unanswered_polls % order.len()]
    }
}

/// The table to probe first, by model: qdomyos hard-codes v7 for
/// `KS-NACH-MXG` and the G1 defaults to v6 (its setting name, confirmed by
/// the footpod client on hardware). A hint orders probing; it never skips
/// detection.
pub fn table_hint(name: &str) -> usize {
    let n = normalized(name);
    if n.starts_with("KS-NACH-MXG") {
        6 // v7
    } else if n.starts_with("KS-NGCH-G1C") {
        5 // v6
    } else {
        0 // v1, qdomyos' default
    }
}

// ---- props parsing -----------------------------------------------------------

/// Split a decoded line into `props` key/value pairs, or `None` when the
/// line is some other message (init replies, error lines). Unknown keys
/// come through as pairs — the caller skips what it doesn't consume — and
/// the two upstream special cases are honoured: `Error` terminates the
/// pairing (its quoted payload breaks the key/value rhythm) and a trailing
/// key with no value is dropped rather than fatal.
pub fn parse_props(line: &str) -> Option<Vec<(&str, &str)>> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.first() != Some(&"props") {
        return None;
    }
    let mut pairs = Vec::new();
    let mut i = 1;
    while i < tokens.len() {
        let key = tokens[i];
        if key == "Error" {
            break;
        }
        let Some(value) = tokens.get(i + 1) else {
            break;
        };
        pairs.push((key, *value));
        i += 2;
    }
    Some(pairs)
}

/// The wire's `runState` as a neutral [`BeltState`]: 0 stop, 1 start
/// (qdomyos' enum, code-level); anything else passes through raw.
pub(crate) fn belt_state(v: u8) -> BeltState {
    match v {
        0 => BeltState::Standby,
        1 => BeltState::Running,
        other => BeltState::Other(other),
    }
}

/// Cumulative telemetry state across `props` frames. The device may report
/// any subset of keys per line, so fields accumulate; a field the device
/// never reports stays `None` — absent, not zero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PadState {
    /// `CurrentSpeed`, km/h.
    pub speed_kmh: Option<f64>,
    /// `RunningDistance`, metres.
    pub distance_m: Option<f64>,
    /// `RunningSteps`.
    pub steps: Option<u32>,
    /// `BurnCalories` as the wire carries it (kcal × 1000 per both
    /// sources' comments — division happens in [`PadState::to_sample`]).
    pub calories_raw: Option<f64>,
    /// `RunningTotalTime`, seconds.
    pub duration_s: Option<u32>,
    /// `runState`, raw.
    pub run_state: Option<u8>,
}

fn finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A finite, non-negative value no larger than `u32::MAX` — the same range
/// discipline [`counter`] applies to the integer counters, for the f64
/// fields of the same frame family. The wire is decimal text, so nothing
/// stops a garbled or hostile line from carrying `1e30`; unbounded, a
/// `RunningDistance` like that saturates the presentation encoding
/// (`distance_raw`) to `u32::MAX` and writes garbage into permanent
/// history. (`telemetry::distance_meters` saturates as well — both layers
/// guard, because that value would be stored.)
fn bounded(value: &str) -> Option<f64> {
    finite(value).filter(|v| (0.0..=u32::MAX as f64).contains(v))
}

fn counter(value: &str) -> Option<u32> {
    finite(value)
        .filter(|v| (0.0..=u32::MAX as f64).contains(v))
        .map(|v| v as u32)
}

impl PadState {
    /// Apply one key/value pair; returns whether the key was consumed.
    /// Non-numeric values for numeric keys are ignored (state keeps its
    /// last good value), matching both upstreams' tolerant parsing.
    pub fn apply(&mut self, key: &str, value: &str) -> bool {
        match key {
            "CurrentSpeed" => {
                if let Some(v) = bounded(value) {
                    self.speed_kmh = Some(v);
                }
            }
            "RunningDistance" => {
                if let Some(v) = bounded(value) {
                    self.distance_m = Some(v);
                }
            }
            "RunningSteps" => {
                if let Some(v) = counter(value) {
                    self.steps = Some(v);
                }
            }
            "BurnCalories" => {
                if let Some(v) = finite(value) {
                    self.calories_raw = Some(v);
                }
            }
            "RunningTotalTime" => {
                if let Some(v) = counter(value) {
                    self.duration_s = Some(v);
                }
            }
            "runState" => {
                if let Some(v) = finite(value) {
                    self.run_state = Some(v.clamp(0.0, 255.0) as u8);
                }
            }
            _ => return false, // spm / ControlMode / strings / unknown keys
        }
        true
    }

    /// The accumulated state as a neutral SI sample. The wire is already
    /// SI (km/h, metres, seconds); calories divide by 1000 (kcal × 1000 on
    /// the wire — comment-level provenance, see the module docs).
    pub fn to_sample(&self) -> Sample {
        Sample {
            speed_kmh: self.speed_kmh,
            distance_m: self.distance_m,
            steps: self.steps,
            duration_s: self.duration_s,
            calories: self
                .calories_raw
                .filter(|v| (0.0..=u32::MAX as f64 * 1000.0).contains(v))
                .map(|v| (v / 1000.0) as u32),
            state: self.run_state.map(belt_state),
        }
    }
}

// ---- Transport probing ------------------------------------------------------

/// One of the three GATT address spaces this protocol ships behind. All
/// three use the same word values (service x1234, write xFED7, notify
/// xFED8) in different 128-bit prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// The plain 16-bit block. qdomyos' default for most models.
    Std,
    /// The `0001xxxx` space (G1C rev 1, NACH-MXG — footpod-verified).
    Space1,
    /// The `0002xxxx` space (G1C rev 2, some HDSY-X21C).
    Space2,
}

impl Transport {
    /// Probe order: the widely-deployed default first, then the two
    /// address-spaced variants. A real device exposes exactly one.
    pub const ALL: [Transport; 3] = [Transport::Std, Transport::Space1, Transport::Space2];

    pub fn service_uuid(self) -> Uuid {
        match self {
            Transport::Std => STD_SERVICE_UUID,
            Transport::Space1 => SPACE1_SERVICE_UUID,
            Transport::Space2 => SPACE2_SERVICE_UUID,
        }
    }

    pub fn write_uuid(self) -> Uuid {
        match self {
            Transport::Std => STD_WRITE_UUID,
            Transport::Space1 => SPACE1_WRITE_UUID,
            Transport::Space2 => SPACE2_WRITE_UUID,
        }
    }

    pub fn notify_uuid(self) -> Uuid {
        match self {
            Transport::Std => STD_NOTIFY_UUID,
            Transport::Space1 => SPACE1_NOTIFY_UUID,
            Transport::Space2 => SPACE2_NOTIFY_UUID,
        }
    }
}

/// The transport this GATT table carries, if any. Stricter than the
/// house's usual role check: the notify and write characteristics must
/// carry the right roles AND sit under their own variant's service —
/// `0x1234` is a placeholder-flavoured UUID other vendors could squat, so
/// the full service+characteristics+roles triple is the claim.
pub fn select_transport(gatt: &BTreeSet<Characteristic>) -> Option<Transport> {
    Transport::ALL.into_iter().find(|t| {
        let under = |uuid: Uuid, role: CharPropFlags| {
            gatt.iter().any(|c| {
                c.uuid == uuid
                    && c.service_uuid == t.service_uuid()
                    && c.properties.intersects(role)
            })
        };
        under(t.notify_uuid(), CharPropFlags::NOTIFY)
            && under(
                t.write_uuid(),
                CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE,
            )
    })
}

// ---- The driver -------------------------------------------------------------

fn normalized(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

/// Does the advertised name identify an app-cipher KingSmith?
fn matches_name(name: &str) -> bool {
    let n = normalized(name);
    ADV_NAME_PREFIXES.iter().any(|pfx| n.starts_with(pfx))
}

/// Frame tags in the `/api/diag` ring: raw ciphertext chunks and, once a
/// frame decodes unanimously, its plaintext — the pair is what lets an
/// issue reporter (or us) check the cipher work offline.
const TAG_RAW: u8 = 0x00;
const TAG_TEXT: u8 = 0x01;

/// Steady-state cadence: quiet this long → send the `getProp` read (the
/// footpod client polls at this rate; push-happy firmware never lets it
/// fire).
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Bound on a single GATT write — a stale link can block forever with no
/// disconnect event (the LifeSpan/WiLink rationale).
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// Consecutive quiet intervals with unanswered polls before the link is
/// declared dead. Comfortable room for the table rotation (7 candidates ×
/// 2 rounds) on a device that answers nothing.
const MAX_DEAD_POLLS: u32 = 15;

pub struct KingSmithProps;

impl KingSmithProps {
    /// Decode-and-consume one notification chunk: reassemble, feed table
    /// detection, parse unanimous `props` lines into the cumulative state,
    /// emit. Returns whether a `props` line was consumed (the poll-answer
    /// signal that stops table rotation).
    fn handle_chunk(
        chunk: &[u8],
        assembler: &mut FrameAssembler,
        detector: &mut TableDetector,
        state: &mut PadState,
        host: &DriverHost<'_>,
        emit: &mut (dyn FnMut(Sample) + Send),
    ) -> bool {
        host.record_frame(TAG_RAW, chunk);
        let mut got_props = false;
        for frame in assembler.push(chunk) {
            match detector.observe(&frame) {
                Decoded::Agreed(plain) => {
                    host.record_frame(TAG_TEXT, &plain);
                    // Agreed plaintexts are printable ASCII by construction.
                    let text = String::from_utf8_lossy(&plain);
                    match parse_props(&text) {
                        Some(pairs) => {
                            got_props = true;
                            for (key, value) in pairs {
                                state.apply(key, value);
                            }
                            emit(state.to_sample());
                        }
                        None => tracing::debug!("kingsmith-props non-props line: {text}"),
                    }
                }
                Decoded::Ambiguous => tracing::debug!(
                    "cipher-table candidates disagree on a frame; skipped \
                     ({} candidates alive)",
                    detector.alive_count()
                ),
                Decoded::Garbage => tracing::debug!("undecodable frame skipped"),
            }
        }
        got_props
    }
}

#[async_trait]
impl Driver for KingSmithProps {
    fn id(&self) -> &'static str {
        "kingsmith-props"
    }

    fn matches(&self, adv: &Advertisement) -> bool {
        // A recognised name, or any of the three service variants. Scan
        // stays permissive; supports() verifies the real table.
        matches_name(&adv.name)
            || Transport::ALL
                .iter()
                .any(|t| adv.services.contains(&t.service_uuid()))
    }

    fn supports(&self, adv: &Advertisement, gatt: &BTreeSet<Characteristic>) -> bool {
        // A verified transport plus a recognised or absent name. The
        // service+characteristics+roles triple appears in no other known
        // protocol, so a nameless device with the exact layout is accepted
        // (the WiLink don't-strand-a-paired-pad precedent); any *other*
        // name — a WiLink pad, a LifeSpan, an unknown — is refused and
        // falls through.
        let Some(_transport) = select_transport(gatt) else {
            return false;
        };
        matches_name(&adv.name) || normalized(&adv.name).is_empty()
    }

    async fn run(&self, link: &Peripheral, host: &DriverHost<'_>, emit: Emit<'_>) -> Result<()> {
        let chars: BTreeSet<Characteristic> = link.characteristics();
        let transport = select_transport(&chars)
            .ok_or_else(|| anyhow!("no KingSmith props transport in the GATT table"))?;
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
        // Both upstreams write without response; fall back to acknowledged
        // writes on a table that doesn't offer WWR.
        let wwr = write_char
            .properties
            .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE);
        let write_type = if wwr {
            WriteType::WithoutResponse
        } else {
            WriteType::WithResponse
        };

        // The model decides which cipher table to probe first (run() gets
        // no advertisement; read the name back, empty when the platform
        // won't say — the FTMS-driver pattern).
        let name = link
            .properties()
            .await
            .ok()
            .flatten()
            .and_then(|p| p.local_name)
            .unwrap_or_default();
        let hint = table_hint(&name);

        // Subscribe first — init replies start immediately and feed table
        // detection; none may be missed.
        link.subscribe(&notify_char).await?;
        let mut notifications = link.notifications().await?;

        // The init reads. Their wire bytes are table-invariant except one
        // character of `net`/`version` under v7, so the hint table is safe
        // regardless of what detection later concludes; replies queue in
        // the notification stream and are drained below.
        run_init_sequence(link, &init_steps(transport, hint, !wwr)).await?;

        let mut assembler = FrameAssembler::new(TERMINATOR);
        let mut detector = TableDetector::new();
        let mut state = PadState::default();
        let mut dead_polls: u32 = 0;
        // Consecutive polls that drew no `props` line: rotates the poll's
        // cipher table (the behavioural probe — see TableDetector).
        let mut unanswered_polls: usize = 0;
        let mut poll_pending = true; // send the first getProp immediately

        loop {
            // Drain everything already buffered (init replies on the first
            // pass, coalesced pushes later).
            while let Some(n) = notifications.next().now_or_never().flatten() {
                if Self::handle_chunk(
                    &n.value,
                    &mut assembler,
                    &mut detector,
                    &mut state,
                    host,
                    &mut *emit,
                ) {
                    unanswered_polls = 0;
                }
            }

            if poll_pending {
                poll_pending = false;
                let table = detector.poll_table(hint, unanswered_polls);
                unanswered_polls += 1;
                let codec = AppCipherCodec { table };
                for chunk in wire_chunks(&codec, MSG_GET_PROPS)? {
                    match tokio::time::timeout(
                        WRITE_TIMEOUT,
                        link.write(&write_char, &chunk, write_type),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => return Err(e.into()), // real BLE error → reconnect
                        Err(_) => return Err(anyhow!("write stalled; forcing reconnect")),
                    }
                }
            }

            match tokio::time::timeout(POLL_INTERVAL, notifications.next()).await {
                Ok(Some(n)) => {
                    dead_polls = 0;
                    if Self::handle_chunk(
                        &n.value,
                        &mut assembler,
                        &mut detector,
                        &mut state,
                        host,
                        &mut *emit,
                    ) {
                        unanswered_polls = 0;
                    }
                }
                Ok(None) => return Err(anyhow!("notification stream ended")),
                Err(_) => {
                    // A quiet second: push firmware never goes silent while
                    // running, poll firmware wants the read repeated —
                    // still the only steady-state frame this driver writes.
                    if !link.is_connected().await.unwrap_or(false) {
                        return Err(anyhow!("link dropped; reconnecting"));
                    }
                    dead_polls += 1;
                    if dead_polls >= MAX_DEAD_POLLS {
                        return Err(anyhow!("link unresponsive; forcing reconnect"));
                    }
                    poll_pending = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::util::GattIo;
    use super::*;
    use crate::telemetry::Telemetry;
    use std::sync::Mutex;
    use tokio::time::Instant;

    // A realistic full telemetry line built from the two sources' shared
    // key facts (no implementation has published a raw capture, so this is
    // synthetic and labelled as such — the values exercise every parsed
    // key).
    const TELEMETRY_LINE: &str = "props CurrentSpeed 3.5 RunningDistance 1234 \
         RunningSteps 2211 BurnCalories 87432 RunningTotalTime 1805 spm 108 \
         runState 1 ControlMode 1";

    // ---- The cipher tables ---------------------------------------------------

    /// Every table must be a permutation of the base64+pad alphabet with
    /// `=` a fixed point — the structural facts the whole transport (and
    /// the detection design) rests on.
    #[test]
    fn tables_are_permutations_with_a_fixed_pad() {
        for (i, table) in CIPHER_TABLES.iter().enumerate() {
            let mut sorted: Vec<u8> = table.to_vec();
            sorted.sort_unstable();
            let mut plain: Vec<u8> = BASE64_ALPHABET.to_vec();
            plain.sort_unstable();
            assert_eq!(sorted, plain, "v{} must permute the alphabet", i + 1);
            assert_eq!(table[64], b'=', "v{} must keep '=' fixed", i + 1);
        }
    }

    /// Round-trip in both directions for every table: encode∘decode and
    /// decode∘encode are identities (base64 canonical form makes the wire
    /// direction exact too).
    #[test]
    fn cipher_round_trips_both_directions_for_every_table() {
        for table in 0..TABLE_COUNT {
            for msg in [
                "",
                "shake",
                "servers getProp 1 2 7 12 23 24 31",
                TELEMETRY_LINE,
                "props mcu_version V1.0.19 goal 0",
            ] {
                let wire = cipher_encode(table, msg.as_bytes());
                assert_eq!(
                    cipher_decode(table, &wire).unwrap(),
                    msg.as_bytes(),
                    "v{} {msg:?}",
                    table + 1
                );
                // Wire → plain → wire is exact as well.
                assert_eq!(
                    cipher_encode(table, &cipher_decode(table, &wire).unwrap()),
                    wire,
                    "v{}",
                    table + 1
                );
            }
        }
    }

    /// A byte outside the cipher alphabet must fail decoding cleanly — one
    /// corrupt notification must not take the daemon down.
    #[test]
    fn bytes_outside_the_alphabet_are_rejected() {
        assert_eq!(
            cipher_decode(0, &[0xFF, 0x41, 0x41, 0x41]),
            Err(ProtocolError::BadCipherByte(0xFF))
        );
        assert_eq!(
            cipher_decode(0, b"DAEE\x00AU="),
            Err(ProtocolError::BadCipherByte(0x00))
        );
    }

    // ---- base64 --------------------------------------------------------------

    /// The RFC 4648 vectors, both directions, plus the padding edge cases.
    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (plain, encoded) in [
            (&b""[..], &b""[..]),
            (b"f", b"Zg=="),
            (b"fo", b"Zm8="),
            (b"foo", b"Zm9v"),
            (b"foob", b"Zm9vYg=="),
            (b"fooba", b"Zm9vYmE="),
            (b"foobar", b"Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(plain), encoded, "{plain:?}");
            assert_eq!(base64_decode(encoded).unwrap(), plain, "{encoded:?}");
        }
    }

    #[test]
    fn base64_rejects_malformed_input() {
        assert_eq!(
            base64_decode(b"Zg="),
            Err(ProtocolError::BadBase64Length(3))
        );
        assert_eq!(base64_decode(b"Z==="), Err(ProtocolError::BadBase64Padding));
        assert_eq!(base64_decode(b"=Zg="), Err(ProtocolError::BadBase64Padding));
        assert_eq!(base64_decode(b"Zm=v"), Err(ProtocolError::BadBase64Padding));
        assert_eq!(
            base64_decode(b"Zm9!"),
            Err(ProtocolError::BadBase64Byte(b'!'))
        );
        // All-pad and bare-pad forms are padding errors, not panics.
        assert_eq!(base64_decode(b"===="), Err(ProtocolError::BadBase64Padding));
    }

    // ---- The write set -------------------------------------------------------

    fn hx(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Every byte this driver writes must be a read. The outbound
    /// vocabulary is pinned — frames and count — and the actuation
    /// boundary of this protocol is textual: outbound `props …` is the
    /// setter form and `time_posix …` sets the clock, so no message may
    /// begin with either. (qdomyos' `props CurrentSpeed/runState/
    /// ControlMode <value>` writes and its `time_posix <unix>` init are
    /// exactly what this test exists to keep out.)
    #[test]
    fn the_driver_only_ever_writes_reads() {
        // The complete vocabulary, pinned.
        assert_eq!(
            READ_MESSAGES,
            &[
                "",
                "shake",
                "net",
                "get_dn",
                "get_pk",
                "version",
                MSG_GET_PROPS
            ]
        );
        assert_eq!(INIT_MESSAGES.len() + 1, READ_MESSAGES.len());
        for msg in READ_MESSAGES {
            assert!(
                !msg.starts_with("props"),
                "outbound `props` is the SETTER form: {msg:?}"
            );
            assert!(
                !msg.starts_with("time_posix"),
                "time_posix sets the device clock: {msg:?}"
            );
            // Reads are the bare interrogatives and the explicit getProp
            // request; none carries a settable key.
            for key in KNOWN_KEYS {
                assert!(!msg.contains(key), "{msg:?} must not carry key {key}");
            }
        }

        // The init write set under the default table, byte-exact with the
        // terminator appended, one chunk per message (all fit in 16
        // bytes). Hand-computed from the tables.
        let payloads: Vec<Vec<u8>> = init_steps(Transport::Std, 0, false)
            .iter()
            .map(|s| s.payload.clone())
            .collect();
        assert_eq!(
            payloads,
            vec![
                hx("0d"),                                     // ""
                hx("44 41 45 45 4f 41 55 3d 0d"),             // "shake"  → DAEEOAU=
                hx("62 6d 2f 31 0d"),                         // "net"    → bm/1
                hx("36 41 2f 31 63 41 52 75 0d"),             // "get_dn" → 6A/1cARu
                hx("36 41 2f 31 63 32 61 72 0d"),             // "get_pk" → 6A/1c2ar
                hx("64 6d 2f 58 44 41 6c 76 62 67 3d 3d 0d"), // "version" → dm/XDAlvbg==
            ],
            "init is the six reads, nothing else — time_posix is dropped"
        );

        // The steady-state poll under v1: 45 wire bytes split 16/16/13 —
        // the 16-byte outbound chunking, byte-exact.
        let codec = AppCipherCodec { table: 0 };
        assert_eq!(
            wire_chunks(&codec, MSG_GET_PROPS).unwrap(),
            vec![
                hx("44 41 2f 58 64 6d 2f 58 44 58 61 6e 36 63 52 39"),
                hx("44 6d 4b 4d 49 77 34 67 68 69 53 32 49 77 34 58"),
                hx("49 77 49 7a 49 77 49 31 49 77 68 78 0d"),
            ]
        );
    }

    /// The init sequence on the virtual clock: each message one write to
    /// the transport's write characteristic, 300 ms reply gap after each,
    /// six writes total. Pinning the count is what makes a smuggled extra
    /// frame (a ported time_posix, say) a failing test rather than silent
    /// new traffic.
    #[tokio::test(start_paused = true)]
    async fn init_writes_exactly_six_reads_with_reply_gaps() {
        type RecordedWrite = (Uuid, Vec<u8>, bool, Instant);
        #[derive(Default)]
        struct MockLink {
            writes: Mutex<Vec<RecordedWrite>>,
        }
        #[async_trait]
        impl GattIo for MockLink {
            async fn write_uuid(&self, c: Uuid, p: &[u8], wr: bool) -> Result<()> {
                self.writes
                    .lock()
                    .unwrap()
                    .push((c, p.to_vec(), wr, Instant::now()));
                Ok(())
            }
            async fn subscribe_uuid(&self, _c: Uuid) -> Result<()> {
                Ok(())
            }
        }

        for transport in Transport::ALL {
            let link = MockLink::default();
            let start = Instant::now();
            run_init_sequence(&link, &init_steps(transport, 0, false))
                .await
                .unwrap();
            let writes = link.writes.lock().unwrap().clone();
            assert_eq!(writes.len(), INIT_MESSAGES.len(), "{transport:?}");
            for (i, (uuid, payload, with_response, at)) in writes.iter().enumerate() {
                assert_eq!(*uuid, transport.write_uuid(), "{transport:?} frame {i}");
                assert!(
                    !*with_response,
                    "{transport:?} frame {i}: both upstreams use WWR"
                );
                assert_eq!(
                    *payload.last().unwrap(),
                    TERMINATOR,
                    "{transport:?} frame {i}"
                );
                assert_eq!(
                    *at - start,
                    Duration::from_millis(INIT_REPLY_GAP_MS * i as u64),
                    "{transport:?} frame {i} must wait out the reply gap"
                );
                // Every init frame decodes back to its read message.
                let plain = cipher_decode(0, &payload[..payload.len() - 1]).unwrap();
                assert_eq!(plain, INIT_MESSAGES[i].as_bytes(), "{transport:?}");
            }
        }
    }

    // ---- Table invariance and discrimination ---------------------------------

    /// The handshake probes must be sendable before the table is known:
    /// `""`, `shake`, `get_dn` and `get_pk` encipher identically under all
    /// seven tables (their base64 avoids every transposed character), while
    /// the `getProp` poll enciphers differently under EVERY pair — which is
    /// what makes poll rotation a complete behavioural probe.
    #[test]
    fn probe_invariance_and_poll_discrimination() {
        for msg in ["", "shake", "get_dn", "get_pk"] {
            let reference = cipher_encode(0, msg.as_bytes());
            for table in 1..TABLE_COUNT {
                assert_eq!(
                    cipher_encode(table, msg.as_bytes()),
                    reference,
                    "{msg:?} must be table-invariant (v{})",
                    table + 1
                );
            }
        }
        // `net`/`version` differ only under v7 (their base64 contains 'b').
        for msg in ["net", "version"] {
            for table in 1..6 {
                assert_eq!(
                    cipher_encode(table, msg.as_bytes()),
                    cipher_encode(0, msg.as_bytes()),
                    "{msg:?} v{}",
                    table + 1
                );
            }
            assert_ne!(
                cipher_encode(6, msg.as_bytes()),
                cipher_encode(0, msg.as_bytes()),
                "{msg:?} differs under v7"
            );
        }
        let polls: Vec<Vec<u8>> = (0..TABLE_COUNT)
            .map(|t| cipher_encode(t, MSG_GET_PROPS.as_bytes()))
            .collect();
        for a in 0..TABLE_COUNT {
            for b in a + 1..TABLE_COUNT {
                assert_ne!(
                    polls[a],
                    polls[b],
                    "getProp must differ between v{} and v{}",
                    a + 1,
                    b + 1
                );
            }
        }
    }

    // ---- Table auto-detection ------------------------------------------------

    /// The soundness core: a full telemetry line decodes printable under
    /// EXACTLY its own table — for every choice of true table, all six
    /// wrong decodes contain non-printable bytes. One real telemetry frame
    /// therefore locks detection, and (the negative half) a wrong table can
    /// never false-positive on such a frame.
    #[test]
    fn a_telemetry_line_identifies_its_table_uniquely() {
        for true_table in 0..TABLE_COUNT {
            let wire = cipher_encode(true_table, TELEMETRY_LINE.as_bytes());
            for candidate in 0..TABLE_COUNT {
                let plain = cipher_decode(candidate, &wire).unwrap();
                if candidate == true_table {
                    assert_eq!(plain, TELEMETRY_LINE.as_bytes());
                } else {
                    assert!(
                        plain.iter().any(|&b| !(0x20..=0x7E).contains(&b)),
                        "v{} must NOT decode a v{} frame to printable text",
                        candidate + 1,
                        true_table + 1
                    );
                }
            }

            // And the detector agrees: one observation eliminates all six
            // rivals and releases the plaintext.
            let mut detector = TableDetector::new();
            assert_eq!(
                detector.observe(&wire),
                Decoded::Agreed(TELEMETRY_LINE.as_bytes().to_vec())
            );
            assert_eq!(
                detector.locked(),
                Some(true_table),
                "true v{}",
                true_table + 1
            );
        }
    }

    /// The documented ambiguity, pinned: `props mcu_version V1.0.19 goal 0`
    /// enciphered with v5 decodes printable-and-plausible under v1/v3/v6 as
    /// `… goal 8` (same grammar, different VALUE — no content check can
    /// tell). The detector must NOT release either reading — the frame is
    /// skipped — and a later discriminating frame must resolve the split to
    /// the true table.
    #[test]
    fn plausible_wrong_decodes_are_held_back_not_guessed() {
        let ambiguous = cipher_encode(4, b"props mcu_version V1.0.19 goal 0"); // v5
        assert_eq!(
            cipher_decode(0, &ambiguous).unwrap(),
            b"props mcu_version V1.0.19 goal 8".to_vec(),
            "the wrong-table decode really is plausible — that is the point"
        );

        let mut detector = TableDetector::new();
        assert_eq!(detector.observe(&ambiguous), Decoded::Ambiguous);
        // v2 and v7 decode non-printable and are eliminated; v1/v3/v4/v5/v6
        // survive in disagreement.
        assert_eq!(detector.alive_count(), 5);
        assert_eq!(detector.locked(), None);

        // A discriminating frame resolves the split.
        let telemetry = cipher_encode(4, TELEMETRY_LINE.as_bytes());
        assert_eq!(
            detector.observe(&telemetry),
            Decoded::Agreed(TELEMETRY_LINE.as_bytes().to_vec())
        );
        assert_eq!(detector.locked(), Some(4));
        // …and the ambiguous frame now reads unambiguously.
        assert_eq!(
            detector.observe(&ambiguous),
            Decoded::Agreed(b"props mcu_version V1.0.19 goal 0".to_vec())
        );
    }

    /// A shake reply carries just enough structure to discriminate: the
    /// true decode `shake 00` outranks the rivals' `shake 0?` (a `?` is
    /// not a plausible value token), so even the handshake locks the table
    /// on most models.
    #[test]
    fn a_shake_reply_locks_the_table() {
        let wire = cipher_encode(0, b"shake 00");
        assert_eq!(hx("44 41 45 45 4f 41 55 67 68 77 53 3d"), wire);
        let mut detector = TableDetector::new();
        assert_eq!(
            detector.observe(&wire),
            Decoded::Agreed(b"shake 00".to_vec())
        );
        assert_eq!(detector.locked(), Some(0));
    }

    /// Radio noise must eliminate nobody: frames no candidate can decode to
    /// printable text are Garbage, and detection state is untouched. This
    /// is the guard against a corrupted frame knocking out the TRUE table
    /// (a wrong table can accidentally decode corruption to printable
    /// junk while the true table cannot).
    #[test]
    fn garbage_frames_do_not_eliminate_candidates() {
        let mut detector = TableDetector::new();
        // Bytes outside the cipher alphabet entirely.
        assert_eq!(detector.observe(&[0xFF, 0x00, 0x0A]), Decoded::Garbage);
        // Alphabet bytes whose decode is non-printable under every table
        // ('h' maps to plain 'M' in all seven; "MMMM" decodes to 0x30 0xC3
        // 0x0C).
        assert_eq!(detector.observe(b"hhhh"), Decoded::Garbage);
        assert_eq!(detector.alive_count(), TABLE_COUNT);
    }

    /// Poll rotation: hint first, then the other survivors round-robin;
    /// once detection converges the rotation collapses to the winner.
    #[test]
    fn poll_rotation_walks_the_survivors_hint_first() {
        let detector = TableDetector::new();
        assert_eq!(detector.poll_table(5, 0), 5, "hint (v6) first");
        assert_eq!(detector.poll_table(5, 1), 0);
        assert_eq!(detector.poll_table(5, 2), 1);
        assert_eq!(detector.poll_table(5, 7), 5, "wraps around");

        let mut locked = TableDetector::new();
        locked.observe(&cipher_encode(3, TELEMETRY_LINE.as_bytes()));
        assert_eq!(locked.locked(), Some(3));
        for unanswered in 0..10 {
            assert_eq!(locked.poll_table(0, unanswered), 3);
        }
    }

    /// Model hints order probing (qdomyos hard-codes v7 for the MXG; the
    /// G1 defaults to v6 on hardware) but never skip detection.
    #[test]
    fn model_hints_pick_the_documented_tables() {
        assert_eq!(table_hint("KS-NACH-MXG"), 6, "X23 → v7");
        assert_eq!(table_hint("ks-nach-mxg1"), 6);
        assert_eq!(table_hint("KS-NGCH-G1C"), 5, "G1 → v6");
        assert_eq!(table_hint("KS-X21"), 0);
        assert_eq!(table_hint(""), 0);
    }

    // ---- Frame reassembly through the codec seam -----------------------------

    /// The full inbound stack the way run() wires it: chunks →
    /// FrameAssembler → detector/codec → parse. A frame split across
    /// chunk boundaries AND two frames sharing one chunk must both come
    /// through — the exact case the assembler was built for.
    #[test]
    fn reassembly_and_decode_across_chunk_boundaries() {
        let frame_a = cipher_encode(0, TELEMETRY_LINE.as_bytes());
        let frame_b = cipher_encode(0, b"props RunningSteps 2213");
        // Wire: frame A split mid-way, its tail sharing a chunk with the
        // whole of frame B.
        let mut wire = frame_a.clone();
        wire.push(TERMINATOR);
        let (head, tail) = wire.split_at(20);
        let mut second = tail.to_vec();
        second.extend_from_slice(&frame_b);
        second.push(TERMINATOR);

        let mut assembler = FrameAssembler::new(TERMINATOR);
        let mut detector = TableDetector::new();
        let mut decoded = Vec::new();
        for chunk in [head.to_vec(), second] {
            for frame in assembler.push(&chunk) {
                if let Decoded::Agreed(plain) = detector.observe(&frame) {
                    decoded.push(plain);
                }
            }
        }
        assert_eq!(
            decoded,
            vec![
                TELEMETRY_LINE.as_bytes().to_vec(),
                b"props RunningSteps 2213".to_vec()
            ]
        );

        // And the steady-state seam (locked codec) decodes the same frame.
        let codec = AppCipherCodec { table: 0 };
        assert_eq!(codec.decode(&frame_b).unwrap(), b"props RunningSteps 2213");
        assert_eq!(codec.encode(b"props RunningSteps 2213").unwrap(), frame_b);
    }

    // ---- props parsing -------------------------------------------------------

    #[test]
    fn parses_props_lines_including_unknown_keys() {
        assert_eq!(
            parse_props("props CurrentSpeed 3.5 spm 108"),
            Some(vec![("CurrentSpeed", "3.5"), ("spm", "108")])
        );
        // Unknown keys come through as pairs; the consumer skips them.
        assert_eq!(
            parse_props("props FutureKey 7 CurrentSpeed 2.0"),
            Some(vec![("FutureKey", "7"), ("CurrentSpeed", "2.0")])
        );
        // Non-props lines are identified, not mangled.
        assert_eq!(parse_props("shake 00"), None);
        assert_eq!(parse_props("format error"), None);
        assert_eq!(parse_props(""), None);
        // Bare "props" is a valid, empty report.
        assert_eq!(parse_props("props"), Some(vec![]));
    }

    /// The two upstream special cases: `Error` terminates pairing (its
    /// quoted payload breaks the key/value rhythm — qdomyos bails exactly
    /// there), and a trailing key with no value is dropped, not fatal.
    #[test]
    fn error_terminates_and_odd_tails_are_dropped() {
        assert_eq!(
            parse_props("props CurrentSpeed 2.0 Error \"ErrorCode\" -5000"),
            Some(vec![("CurrentSpeed", "2.0")])
        );
        assert_eq!(parse_props("props Error \"ErrorCode\" -5000"), Some(vec![]));
        assert_eq!(
            parse_props("props CurrentSpeed 2.0 RunningSteps"),
            Some(vec![("CurrentSpeed", "2.0")])
        );
    }

    /// Malformed values must not panic and must not clobber good state.
    #[test]
    fn malformed_values_are_ignored_without_panicking() {
        let mut state = PadState::default();
        state.apply("CurrentSpeed", "3.5");
        state.apply("RunningSteps", "2211");
        // Garbage values leave the last good value in place.
        state.apply("CurrentSpeed", "fast");
        state.apply("RunningSteps", "-3");
        state.apply("RunningSteps", "NaN");
        state.apply("RunningSteps", "99999999999999999999");
        state.apply("BurnCalories", "inf");
        assert_eq!(state.speed_kmh, Some(3.5));
        assert_eq!(state.steps, Some(2211));
        assert_eq!(state.calories_raw, None);
        // Unknown and unparsed-on-purpose keys are not consumed.
        assert!(!state.apply("spm", "108"));
        assert!(!state.apply("ControlMode", "1"));
        assert!(!state.apply("mcu_version", "V1.0.19"));
        assert!(!state.apply("NoSuchKey", "1"));
    }

    /// Range discipline is uniform across the frame family: the f64 fields
    /// (`CurrentSpeed`, `RunningDistance`) reject out-of-range values exactly
    /// like the integer counters do. The wire is decimal text — a garbled or
    /// hostile `RunningDistance 1e30` used to sail through as a finite f64,
    /// saturate `distance_raw` to `u32::MAX` at the presentation boundary,
    /// and (pre-fix) overflow `telemetry::distance_meters` into a panic.
    #[test]
    fn absurd_speed_and_distance_values_are_dropped_like_absurd_counters() {
        let mut state = PadState::default();
        state.apply("CurrentSpeed", "3.5");
        state.apply("RunningDistance", "1234");
        for hostile in ["1e30", "-1", "4294967296", "inf", "NaN"] {
            state.apply("CurrentSpeed", hostile);
            state.apply("RunningDistance", hostile);
        }
        assert_eq!(
            state.speed_kmh,
            Some(3.5),
            "CurrentSpeed must apply the same 0..=u32::MAX range filter as \
             the integer counters — see `bounded` in kingsmith_props.rs"
        );
        assert_eq!(
            state.distance_m,
            Some(1234.0),
            "RunningDistance must apply the same 0..=u32::MAX range filter \
             as the integer counters (an unbounded value here becomes a \
             permanently stored distance) — see `bounded` in kingsmith_props.rs"
        );
    }

    /// Keys accumulate across partial updates — the device may report any
    /// subset per line — and fields never reported stay absent.
    #[test]
    fn partial_updates_accumulate_and_absent_fields_stay_absent() {
        let mut state = PadState::default();
        for (k, v) in parse_props("props CurrentSpeed 2.0 spm 96").unwrap() {
            state.apply(k, v);
        }
        let s = state.to_sample();
        assert_eq!(s.speed_kmh, Some(2.0));
        assert_eq!(s.steps, None, "never reported — absent, not zero");
        assert_eq!(s.distance_m, None);
        assert_eq!(s.calories, None);
        assert_eq!(s.duration_s, None);
        assert_eq!(s.state, None);

        for (k, v) in parse_props("props RunningSteps 2213 runState 1").unwrap() {
            state.apply(k, v);
        }
        let s = state.to_sample();
        assert_eq!(s.speed_kmh, Some(2.0), "earlier keys persist");
        assert_eq!(s.steps, Some(2213));
        assert_eq!(s.state, Some(BeltState::Running));
    }

    // ---- Belt state ----------------------------------------------------------

    /// runState 0/1 are the code-level facts (qdomyos' enum); everything
    /// else passes through raw.
    #[test]
    fn belt_states_map_and_unknowns_pass_through() {
        assert_eq!(belt_state(0), BeltState::Standby);
        assert_eq!(belt_state(1), BeltState::Running);
        for v in [2u8, 3, 5, 9, 0x7F, 0xFF] {
            assert_eq!(belt_state(v), BeltState::Other(v), "byte {v}");
        }
    }

    // ---- Sample / Telemetry golden pins --------------------------------------

    /// Fixture line → PadState → Sample → Telemetry, end to end: the wire
    /// is already SI except calories (kcal × 1000 → kcal).
    #[test]
    fn golden_fixture_to_telemetry() {
        let approx = |a: f64, b: f64| (a - b).abs() < 1e-12;
        let mut state = PadState::default();
        for (k, v) in parse_props(TELEMETRY_LINE).unwrap() {
            state.apply(k, v);
        }
        let sample = state.to_sample();
        assert!(approx(sample.speed_kmh.unwrap(), 3.5));
        assert!(approx(sample.distance_m.unwrap(), 1234.0));
        assert_eq!(sample.steps, Some(2211));
        assert_eq!(sample.duration_s, Some(1805));
        assert_eq!(sample.calories, Some(87), "87432 wire units → 87 kcal");
        assert_eq!(sample.state, Some(BeltState::Running));

        let t = Telemetry::from_sample(&sample, "km/h");
        assert_eq!(t.speed_raw, Some(350), "3.50 km/h in centi-units");
        assert_eq!(t.distance_raw, Some(123), "1234 m → 123 decameters");
        assert_eq!(t.distance_m, Some(1230));
        assert_eq!(t.steps, Some(2211));
        assert_eq!(t.duration_s, Some(1805));
        assert_eq!(t.calories, Some(87));
        assert_eq!(t.status_name.as_deref(), Some("RUNNING"));
        assert!(t.is_running);
    }

    /// A stopped pad presents as the contract's STANDBY code; an unknown
    /// runState passes through raw.
    #[test]
    fn stopped_and_unknown_states_present_as_contract_codes() {
        let mut state = PadState::default();
        for (k, v) in parse_props("props CurrentSpeed 0.0 runState 0").unwrap() {
            state.apply(k, v);
        }
        let t = Telemetry::from_sample(&state.to_sample(), "km/h");
        assert_eq!(t.status, Some(0x01));
        assert_eq!(t.status_name.as_deref(), Some("STANDBY"));
        assert!(!t.is_running);

        state.apply("runState", "7");
        let t = Telemetry::from_sample(&state.to_sample(), "km/h");
        assert_eq!(t.status, Some(0x07), "raw passthrough");
    }

    // ---- Transport selection -------------------------------------------------

    use btleplug::api::CharPropFlags;

    const N: CharPropFlags = CharPropFlags::NOTIFY;
    const W: CharPropFlags = CharPropFlags::WRITE;
    const WWR: CharPropFlags = CharPropFlags::WRITE_WITHOUT_RESPONSE;

    fn gatt(chars: &[(Uuid, Uuid, CharPropFlags)]) -> BTreeSet<Characteristic> {
        chars
            .iter()
            .map(|(service_uuid, uuid, properties)| Characteristic {
                uuid: *uuid,
                service_uuid: *service_uuid,
                properties: *properties,
                descriptors: BTreeSet::new(),
            })
            .collect()
    }

    fn shape(t: Transport) -> Vec<(Uuid, Uuid, CharPropFlags)> {
        vec![
            (t.service_uuid(), t.notify_uuid(), N),
            (t.service_uuid(), t.write_uuid(), WWR),
        ]
    }

    /// All three address spaces select their transport; roles and the
    /// parent service are both verified, not just UUIDs.
    #[test]
    fn each_address_space_selects_its_transport() {
        assert_eq!(
            select_transport(&gatt(&shape(Transport::Std))),
            Some(Transport::Std)
        );
        assert_eq!(
            select_transport(&gatt(&shape(Transport::Space1))),
            Some(Transport::Space1)
        );
        assert_eq!(
            select_transport(&gatt(&shape(Transport::Space2))),
            Some(Transport::Space2)
        );
        assert_eq!(select_transport(&gatt(&[])), None);

        // Acknowledged write also satisfies the write role.
        assert_eq!(
            select_transport(&gatt(&[
                (STD_SERVICE_UUID, STD_NOTIFY_UUID, N),
                (STD_SERVICE_UUID, STD_WRITE_UUID, W),
            ])),
            Some(Transport::Std)
        );
        // Roles swapped: refused.
        assert_eq!(
            select_transport(&gatt(&[
                (STD_SERVICE_UUID, STD_NOTIFY_UUID, WWR),
                (STD_SERVICE_UUID, STD_WRITE_UUID, N),
            ])),
            None
        );
        // Half a table: refused.
        assert_eq!(
            select_transport(&gatt(&[(STD_SERVICE_UUID, STD_NOTIFY_UUID, N)])),
            None
        );
        // Right characteristics under a FOREIGN service: refused — the
        // parent service is part of the claim on this placeholder-ish
        // block.
        assert_eq!(
            select_transport(&gatt(&[
                (super::super::sig_uuid(0xfff0), STD_NOTIFY_UUID, N),
                (super::super::sig_uuid(0xfff0), STD_WRITE_UUID, WWR),
            ])),
            None
        );
        // Mixed spaces (0001 notify, 0002 write): refused — a real device
        // exposes one space whole.
        assert_eq!(
            select_transport(&gatt(&[
                (SPACE1_SERVICE_UUID, SPACE1_NOTIFY_UUID, N),
                (SPACE2_SERVICE_UUID, SPACE2_WRITE_UUID, WWR),
            ])),
            None
        );
    }

    // ---- Name matching and the WiLink boundary -------------------------------

    fn adv(name: &str) -> Advertisement {
        Advertisement {
            name: name.into(),
            services: vec![],
        }
    }

    #[test]
    fn known_app_cipher_names_match_case_insensitively() {
        for name in [
            "KS-ST-K12PRO", // Xiaomi K12 Pro
            "KS-R1AC-1234",
            "KS-HC-R1AA",
            "KS-HC-R1AC",
            "ks-x21",
            "KS-X21C-5678",
            "KS-HDSC-X21C",
            "KS-HDSY-X21C",
            "KS-NACH-X21C",
            "KS-NGCH-X21C",
            "KS-NACH-MXG", // X23
            "KS-NGCH-G1C", // G1
        ] {
            assert!(KingSmithProps.matches(&adv(name)), "{name}");
        }
        for name in [
            "",
            "KS-HD-Z1D", // the FTMS WalkingPad Z1 — neither ours nor WiLink's
            "KS-ST-A1P", // WiLink A1 Pro — theirs
            "WalkingPad A1",
            "KINGSMITH",
            "KS-MC21",
            "LifeSpan-TM",
        ] {
            assert!(!matches_name(name), "{name}");
        }
        // The three service variants also surface the device in scans.
        for t in Transport::ALL {
            assert!(KingSmithProps.matches(&Advertisement {
                name: String::new(),
                services: vec![t.service_uuid()],
            }));
        }
    }

    /// The boundary with the WiLink driver, pinned in both directions so
    /// no KingSmith device is claimed twice and none is orphaned:
    /// every name this driver claims is refused by WiLink (its exclusion
    /// list carries exactly the colliding ones), every WiLink name is
    /// refused here, and the FTMS Z1 is refused by both.
    #[test]
    fn the_wilink_boundary_is_exact() {
        use super::super::kingsmith_wilink;

        // Our full model list: ours, and never WiLink's.
        for name in [
            "KS-ST-K12PRO",
            "KS-R1AC",
            "KS-HC-R1AA",
            "KS-HC-R1AC",
            "KS-X21",
            "KS-HDSC-X21C",
            "KS-HDSY-X21C",
            "KS-NACH-X21C",
            "KS-NGCH-X21C",
            "KS-NACH-MXG",
            "KS-NGCH-G1C",
        ] {
            assert!(matches_name(name), "{name} must be ours");
            assert!(
                !kingsmith_wilink::KingSmithWiLink.matches(&adv(name)),
                "{name} must not be claimed by WiLink"
            );
        }

        // Every app-cipher name that collides with a WiLink prefix must
        // appear in WiLink's exclusion list — that list and this driver's
        // prefix list are two views of one boundary.
        for pfx in ADV_NAME_PREFIXES {
            let collides = kingsmith_wilink::ADV_NAME_PREFIXES
                .iter()
                .any(|w| pfx.starts_with(w));
            let excluded = kingsmith_wilink::ADV_NAME_EXCLUDE_PREFIXES
                .iter()
                .any(|e| pfx.starts_with(e));
            assert_eq!(
                collides, excluded,
                "{pfx}: WiLink collision and exclusion must agree"
            );
        }

        // WiLink's exclusions, from the other side: the app-cipher ones
        // are ours; the FTMS Z1 belongs to neither native driver.
        for name in ["KS-HC-R1AA", "KS-HDSC-X21C", "KS-HDSY-X21C"] {
            assert!(matches_name(name), "{name} excluded by WiLink must be ours");
        }
        assert!(!matches_name("KS-HD-Z1D"));
        assert!(!kingsmith_wilink::KingSmithWiLink.matches(&adv("KS-HD-Z1D")));

        // And WiLink's own names never reach this driver.
        for name in ["WalkingPad A1", "KS-ST-A1P", "KS-BLC2", "KS-H101", "R1 PRO"] {
            assert!(!matches_name(name), "{name} is WiLink's");
        }
    }

    // ---- supports() ----------------------------------------------------------

    #[test]
    fn supports_needs_a_transport_and_a_recognised_or_absent_name() {
        for t in Transport::ALL {
            // A recognised name claims every address space.
            assert!(
                KingSmithProps.supports(&adv("KS-X21"), &gatt(&shape(t))),
                "{t:?}"
            );
            // Nameless + the exact layout: accepted (platforms lose names
            // at connect time; the layout appears in no other protocol).
            assert!(KingSmithProps.supports(&adv(""), &gatt(&shape(t))), "{t:?}");
            // A foreign or WiLink name: refused even with the layout.
            for name in ["WalkingPad A1", "KS-HD-Z1D", "LifeSpan-TM", "Mystery Pad"] {
                assert!(
                    !KingSmithProps.supports(&adv(name), &gatt(&shape(t))),
                    "{name} on {t:?}"
                );
            }
        }
        // The right name with no transport: refused.
        assert!(!KingSmithProps.supports(&adv("KS-X21"), &gatt(&[])));
        // …or with a WiLink-shaped table: refused (that pad is not this
        // protocol, whatever the name claims).
        assert!(!KingSmithProps.supports(
            &adv("KS-X21"),
            &gatt(&[
                (
                    super::super::sig_uuid(0xfe00),
                    super::super::sig_uuid(0xfe01),
                    N
                ),
                (
                    super::super::sig_uuid(0xfe00),
                    super::super::sig_uuid(0xfe02),
                    W
                ),
            ])
        ));
    }
}
