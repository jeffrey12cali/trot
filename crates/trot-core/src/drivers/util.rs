//! Shared driver plumbing — the fiddly, protocol-independent parts every
//! non-trivial treadmill driver would otherwise hand-roll.
//!
//! Real treadmill firmware is fragile in a handful of recurring ways, and the
//! helpers here each encode one hard-won lesson:
//!
//! * Some devices are silent until they receive **ordered init frames**, some
//!   with mandatory pauses between them ([`InitStep`], [`run_init_sequence`]).
//! * Some devices drop or garble commands that arrive too close together —
//!   KingSmith WiLink needs ≥690 ms between writes ([`CommandSpacer`]).
//! * Cheap firmware silently ignores notification-enable writes that land
//!   within a few tens of milliseconds of each other; the vendor apps space
//!   them 100–300 ms apart ([`subscribe_staggered`]).
//! * Message-oriented protocols arrive as arbitrary GATT chunks and must be
//!   reassembled on a terminator byte ([`FrameAssembler`]).
//! * A few protocols obfuscate the transport (base64 + substitution cipher);
//!   [`TransportCodec`] is the seam where that transform plugs in without the
//!   reassembly or parsing layers knowing about it.
//!
//! None of this is speculative — every helper corresponds to at least one
//! real device family documented in `docs/drivers/README.md`. Use what your
//! protocol needs and ignore the rest; a driver that doesn't opt in pays
//! nothing (LifeSpan, notably, has **no** known write-spacing requirement, so
//! no spacing is ever imposed by default).
//!
//! One category of plumbing is deliberately absent: anything that would help
//! a driver *command* the machine. Trot observes treadmills — it never
//! actuates them — so every write these helpers perform exists to ask for
//! data or to wake the stream, never to move the belt. Several shared ODM BLE
//! modules gate their command characteristic behind a per-command "unlock"
//! write; Trot sends no commands, so it needs no unlock, and no helper for
//! one lives here. See `docs/drivers/README.md` for the policy and the
//! query-vs-actuation distinction.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use btleplug::api::{CharPropFlags, Peripheral as _, WriteType};
use std::time::Duration;
use uuid::Uuid;

// ---- GATT access by UUID ----------------------------------------------------

/// The few GATT operations the helpers need, addressed by characteristic UUID.
///
/// Implemented for the platform [`Peripheral`](btleplug::platform::Peripheral)
/// a driver's `run()` receives, so a driver just calls the helpers with its
/// `link`. Tests implement it with an in-memory recorder — which is the reason
/// this trait exists at all: the helpers' ordering and timing guarantees are
/// unit-tested without a radio.
#[async_trait]
pub trait GattIo: Send + Sync {
    /// Write `payload` to the characteristic with this UUID.
    async fn write_uuid(&self, char_uuid: Uuid, payload: &[u8], with_response: bool) -> Result<()>;

    /// Enable notifications on the characteristic with this UUID.
    async fn subscribe_uuid(&self, char_uuid: Uuid) -> Result<()>;
}

#[async_trait]
impl GattIo for btleplug::platform::Peripheral {
    async fn write_uuid(&self, char_uuid: Uuid, payload: &[u8], with_response: bool) -> Result<()> {
        let c = self
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == char_uuid)
            .ok_or_else(|| anyhow!("characteristic {char_uuid} missing"))?;
        let write_type = if with_response {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        };
        self.write(&c, payload, write_type).await?;
        Ok(())
    }

    async fn subscribe_uuid(&self, char_uuid: Uuid) -> Result<()> {
        let c = self
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == char_uuid)
            .ok_or_else(|| anyhow!("characteristic {char_uuid} missing"))?;
        self.subscribe(&c).await?;
        Ok(())
    }
}

// ---- Ordered init sequences (shape: init handshake, then push) ---------------

/// One step of an init handshake: a write, then an optional settle delay.
///
/// Devices like the Urevo E1L need a single magic frame; Sperax, PitPat and
/// Zipro need 3–11 of them **in order, with pauses between** — send them too
/// fast and the device never starts streaming, with no error to tell you why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitStep {
    pub char_uuid: Uuid,
    pub payload: Vec<u8>,
    /// Pause after this write before the next step runs. Zero = none.
    pub delay_after: Duration,
    /// Write with response (the default — acknowledged writes are ordered and
    /// surfacing errors beats speed during a handshake).
    pub with_response: bool,
}

impl InitStep {
    /// An acknowledged write with no delay after it.
    pub fn write(char_uuid: Uuid, payload: impl Into<Vec<u8>>) -> Self {
        InitStep {
            char_uuid,
            payload: payload.into(),
            delay_after: Duration::ZERO,
            with_response: true,
        }
    }

    /// Pause this long after the write before the next step.
    pub fn then_wait_ms(mut self, ms: u64) -> Self {
        self.delay_after = Duration::from_millis(ms);
        self
    }

    /// Use an unacknowledged write (for characteristics that only accept
    /// write-without-response).
    pub fn without_response(mut self) -> Self {
        self.with_response = false;
        self
    }
}

/// Execute an init handshake: each write in order, honouring each step's
/// delay. Call once at the top of `run()`, after subscribing to whatever the
/// device will stream on (subscribe first — some devices answer the final
/// init frame immediately and you must not miss it).
pub async fn run_init_sequence<L: GattIo + ?Sized>(link: &L, steps: &[InitStep]) -> Result<()> {
    for step in steps {
        link.write_uuid(step.char_uuid, &step.payload, step.with_response)
            .await?;
        if step.delay_after > Duration::ZERO {
            tokio::time::sleep(step.delay_after).await;
        }
    }
    Ok(())
}

// ---- Minimum inter-command spacing -------------------------------------------

/// Enforces a minimum gap between commands, for devices that drop or garble
/// writes arriving too fast (KingSmith WiLink needs ≥690 ms).
///
/// `await pace()` before each write: the first call never waits, later calls
/// wait out whatever remains of the gap since the previous one. Time already
/// spent doing other work counts toward the gap, so a slow poll loop doesn't
/// pay twice.
///
/// This is strictly opt-in. LifeSpan has no known spacing requirement and gets
/// none — do not add a spacer to a driver without evidence the device needs
/// it, because every gap is added latency on live speed readings.
#[derive(Debug)]
pub struct CommandSpacer {
    min_gap: Duration,
    last: Option<tokio::time::Instant>,
}

impl CommandSpacer {
    pub fn new(min_gap: Duration) -> Self {
        CommandSpacer {
            min_gap,
            last: None,
        }
    }

    /// Wait until at least `min_gap` has passed since the previous `pace()`
    /// returned, then mark now as the new reference point.
    pub async fn pace(&mut self) {
        if let Some(last) = self.last {
            tokio::time::sleep_until(last + self.min_gap).await;
        }
        self.last = Some(tokio::time::Instant::now());
    }
}

// ---- Staggered CCCD subscription ---------------------------------------------

/// Subscribe to several characteristics with a settle delay after each one.
///
/// Treadmill firmware silently drops notification-enable writes that arrive
/// within ~30 ms of each other — one characteristic then just never fires,
/// with nothing in any log to say why. The vendor apps space subscriptions
/// 100–300 ms apart; do the same. The delay after the *last* subscription
/// also runs, which doubles as settle time before your first write.
pub async fn subscribe_staggered<L: GattIo + ?Sized>(
    link: &L,
    subscriptions: &[(Uuid, Duration)],
) -> Result<()> {
    for (char_uuid, delay_after) in subscriptions {
        link.subscribe_uuid(*char_uuid).await?;
        if *delay_after > Duration::ZERO {
            tokio::time::sleep(*delay_after).await;
        }
    }
    Ok(())
}

// ---- Frame reassembly --------------------------------------------------------

/// Sanity cap on a single reassembled frame. Real treadmill messages are tens
/// of bytes; a kilobyte of buffered data without a terminator means we joined
/// a stream mid-frame or the terminator byte is wrong, and buffering more
/// would just grow memory forever.
const DEFAULT_MAX_FRAME_LEN: usize = 1024;

/// Reassembles terminator-delimited messages from arbitrary GATT chunks.
///
/// Notifications carry transport chunks, not messages: a logical frame can be
/// split across several notifications AND several frames can share one
/// notification, in the same session. Feed every chunk to [`push`] and it
/// yields each complete frame exactly once, terminator stripped.
///
/// Empty frames (a terminator with nothing buffered before it) are discarded:
/// every known protocol in this family sends non-empty messages, so an empty
/// one is line noise or a doubled terminator, not data.
///
/// If a frame exceeds the length cap the buffered bytes are dropped and
/// reassembly resyncs at the next terminator — one corrupt frame must not
/// poison everything after it.
///
/// [`push`]: FrameAssembler::push
#[derive(Debug)]
pub struct FrameAssembler {
    terminator: u8,
    buf: Vec<u8>,
    max_frame_len: usize,
}

impl FrameAssembler {
    pub fn new(terminator: u8) -> Self {
        FrameAssembler {
            terminator,
            buf: Vec::new(),
            max_frame_len: DEFAULT_MAX_FRAME_LEN,
        }
    }

    /// Override the frame length cap (rarely needed).
    pub fn with_max_frame_len(mut self, max_frame_len: usize) -> Self {
        self.max_frame_len = max_frame_len;
        self
    }

    /// Feed one notification chunk; get back every frame it completed
    /// (zero or more), in order, without the terminator byte.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        for &byte in chunk {
            if byte == self.terminator {
                if !self.buf.is_empty() {
                    frames.push(std::mem::take(&mut self.buf));
                }
            } else {
                if self.buf.len() >= self.max_frame_len {
                    tracing::warn!(
                        "frame exceeded {} bytes without a terminator; dropping buffer to resync",
                        self.max_frame_len
                    );
                    self.buf.clear();
                }
                self.buf.push(byte);
            }
        }
        frames
    }

    /// Bytes buffered toward an incomplete frame. Diagnostics only.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}

// ---- Transport codec ---------------------------------------------------------

/// The seam between raw GATT bytes and protocol parsing.
///
/// Most protocols need nothing here ([`IdentityCodec`]). The exception is the
/// obfuscated-transport family (KingSmith R2/X21: UTF-8 → base64 →
/// per-character substitution cipher, model-dependent tables), where the
/// transform must sit *between* frame reassembly and payload parsing. This
/// trait is deliberately just that seam — the cipher tables themselves belong
/// in the driver that owns them, not here.
///
/// Inbound: reassemble first, then `decode` each complete frame. Outbound:
/// `encode` the whole message first, then split into GATT-MTU-sized writes
/// (`payload.chunks(16)`) — the cipher runs over the message, not the chunks.
pub trait TransportCodec: Send + Sync {
    /// Reassembled frame bytes → protocol payload bytes.
    fn decode(&self, raw: &[u8]) -> Result<Vec<u8>>;
    /// Protocol payload bytes → wire bytes (before chunking/terminator).
    fn encode(&self, plain: &[u8]) -> Result<Vec<u8>>;
}

/// The no-op codec, for the (majority of) protocols whose wire bytes are the
/// payload.
pub struct IdentityCodec;

impl TransportCodec for IdentityCodec {
    fn decode(&self, raw: &[u8]) -> Result<Vec<u8>> {
        Ok(raw.to_vec())
    }
    fn encode(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(plain.to_vec())
    }
}

// ---- Checksums ---------------------------------------------------------------

/// Additive checksum: `sum(bytes) mod 256`. One of the two trailer bytes
/// that occur in the wild (the KingSmith request/response family among its
/// users).
pub fn checksum_sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

/// XOR checksum: `bytes[0] ^ bytes[1] ^ …`. The other trailer byte that
/// actually occurs in the wild — FitShow's `02 … 03` frames and the PitPat
/// family both use it. (An earlier comment here claimed FitShow's trailer
/// was additive; it is not — every FitShow source, its own OEM spec
/// included, XORs.)
pub fn checksum_xor(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc ^ b)
}

// ---- Verifying characteristic properties -------------------------------------

/// Does this GATT table have a notifiable characteristic with this UUID?
///
/// Property checks are how a driver avoids claiming a lookalike: vendor
/// UUID blocks like `0xFFF0`/`FFF1`/`FFF2` are shared across mutually
/// incompatible protocols, and at least one (Deerrun) swaps the notify/write
/// roles relative to the others. A UUID proves nothing; a UUID **with the
/// role you need** is evidence.
pub fn has_notify(
    gatt: &std::collections::BTreeSet<btleplug::api::Characteristic>,
    char_uuid: Uuid,
) -> bool {
    gatt.iter()
        .any(|c| c.uuid == char_uuid && c.properties.contains(CharPropFlags::NOTIFY))
}

/// Does this GATT table have a writable (with or without response)
/// characteristic with this UUID? See [`has_notify`] for why properties
/// matter.
pub fn has_write(
    gatt: &std::collections::BTreeSet<btleplug::api::Characteristic>,
    char_uuid: Uuid,
) -> bool {
    gatt.iter().any(|c| {
        c.uuid == char_uuid
            && c.properties
                .intersects(CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::sig_uuid;
    use std::sync::Mutex;
    use tokio::time::Instant;

    // ---- Recording mock link ------------------------------------------------

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Op {
        Write {
            char_uuid: Uuid,
            payload: Vec<u8>,
            with_response: bool,
        },
        Subscribe {
            char_uuid: Uuid,
        },
    }

    /// Records every operation with the (virtual) instant it happened, so the
    /// ordering AND timing guarantees are assertable without a radio.
    #[derive(Default)]
    struct MockLink {
        ops: Mutex<Vec<(Op, Instant)>>,
        fail_writes: bool,
    }

    impl MockLink {
        fn ops(&self) -> Vec<(Op, Instant)> {
            self.ops.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl GattIo for MockLink {
        async fn write_uuid(
            &self,
            char_uuid: Uuid,
            payload: &[u8],
            with_response: bool,
        ) -> Result<()> {
            if self.fail_writes {
                return Err(anyhow!("write refused"));
            }
            self.ops.lock().unwrap().push((
                Op::Write {
                    char_uuid,
                    payload: payload.to_vec(),
                    with_response,
                },
                Instant::now(),
            ));
            Ok(())
        }

        async fn subscribe_uuid(&self, char_uuid: Uuid) -> Result<()> {
            self.ops
                .lock()
                .unwrap()
                .push((Op::Subscribe { char_uuid }, Instant::now()));
            Ok(())
        }
    }

    // ---- Init sequences -----------------------------------------------------

    /// The Urevo E1L wake write, then a Sperax-style multi-frame handshake:
    /// every write must land in order and each delay must be honoured before
    /// the next write goes out. Virtual clock (start_paused) makes the timing
    /// assertions exact.
    #[tokio::test(start_paused = true)]
    async fn init_sequence_runs_in_order_with_delays() {
        let link = MockLink::default();
        let fff2 = sig_uuid(0xfff2);
        let steps = [
            InitStep::write(fff2, [0x02, 0x51, 0x0B, 0x03]).then_wait_ms(100),
            InitStep::write(fff2, [0x0A, 0x0B]).then_wait_ms(250),
            InitStep::write(fff2, [0x0C]).without_response(),
        ];
        let start = Instant::now();
        run_init_sequence(&link, &steps).await.unwrap();

        let ops = link.ops();
        assert_eq!(ops.len(), 3);
        assert_eq!(
            ops[0].0,
            Op::Write {
                char_uuid: fff2,
                payload: vec![0x02, 0x51, 0x0B, 0x03],
                with_response: true,
            }
        );
        assert_eq!(ops[0].1 - start, Duration::ZERO);
        assert_eq!(ops[1].1 - start, Duration::from_millis(100));
        assert_eq!(ops[2].1 - start, Duration::from_millis(350));
        assert_eq!(
            ops[2].0,
            Op::Write {
                char_uuid: fff2,
                payload: vec![0x0C],
                with_response: false,
            }
        );
    }

    /// A failed init write must abort the handshake — continuing after a
    /// refused frame would leave the device in an undefined half-initialised
    /// state.
    #[tokio::test(start_paused = true)]
    async fn init_sequence_stops_on_first_error() {
        let link = MockLink {
            fail_writes: true,
            ..Default::default()
        };
        let steps = [
            InitStep::write(sig_uuid(0xfff2), [0x01]),
            InitStep::write(sig_uuid(0xfff2), [0x02]),
        ];
        assert!(run_init_sequence(&link, &steps).await.is_err());
        assert!(link.ops().is_empty());
    }

    // ---- Command spacing ----------------------------------------------------

    /// The WiLink constraint: back-to-back pace() calls must be at least the
    /// gap apart, but the first call must not wait at all (the gap is between
    /// commands, not before the first one).
    #[tokio::test(start_paused = true)]
    async fn spacer_enforces_the_minimum_gap() {
        let mut spacer = CommandSpacer::new(Duration::from_millis(690));
        let start = Instant::now();

        spacer.pace().await;
        assert_eq!(Instant::now() - start, Duration::ZERO, "first call is free");

        spacer.pace().await;
        assert_eq!(Instant::now() - start, Duration::from_millis(690));

        spacer.pace().await;
        assert_eq!(Instant::now() - start, Duration::from_millis(1380));
    }

    /// Time spent doing real work between commands counts toward the gap — a
    /// driver that took 700 ms decoding must not wait another 690 ms on top.
    #[tokio::test(start_paused = true)]
    async fn spacer_credits_time_already_spent() {
        let mut spacer = CommandSpacer::new(Duration::from_millis(690));
        spacer.pace().await;
        tokio::time::sleep(Duration::from_millis(700)).await; // simulated work
        let before = Instant::now();
        spacer.pace().await;
        assert_eq!(Instant::now() - before, Duration::ZERO);

        // Partial credit: 400 ms of work leaves 290 ms to wait.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let before = Instant::now();
        spacer.pace().await;
        assert_eq!(Instant::now() - before, Duration::from_millis(290));
    }

    /// A zero gap is a no-op spacer: no waiting, ever. This is the "LifeSpan
    /// default" — no spacing unless a driver opts in with a real number.
    #[tokio::test(start_paused = true)]
    async fn zero_gap_spacer_never_waits() {
        let mut spacer = CommandSpacer::new(Duration::ZERO);
        let start = Instant::now();
        for _ in 0..10 {
            spacer.pace().await;
        }
        assert_eq!(Instant::now() - start, Duration::ZERO);
    }

    // ---- Staggered subscription ---------------------------------------------

    /// The firmware-drops-fast-CCCD-writes workaround: each subscription must
    /// go out only after the previous one's settle delay has fully elapsed
    /// (the vendor apps use 100/200/300 ms).
    #[tokio::test(start_paused = true)]
    async fn staggered_subscribe_spaces_the_enables() {
        let link = MockLink::default();
        let (a, b, c) = (sig_uuid(0xfff1), sig_uuid(0xfff2), sig_uuid(0xfff4));
        let start = Instant::now();
        subscribe_staggered(
            &link,
            &[
                (a, Duration::from_millis(100)),
                (b, Duration::from_millis(200)),
                (c, Duration::from_millis(300)),
            ],
        )
        .await
        .unwrap();

        let ops = link.ops();
        assert_eq!(
            ops.iter().map(|(op, _)| op.clone()).collect::<Vec<_>>(),
            vec![
                Op::Subscribe { char_uuid: a },
                Op::Subscribe { char_uuid: b },
                Op::Subscribe { char_uuid: c },
            ]
        );
        assert_eq!(ops[0].1 - start, Duration::ZERO);
        assert_eq!(ops[1].1 - start, Duration::from_millis(100));
        assert_eq!(ops[2].1 - start, Duration::from_millis(300));
        // The trailing delay also runs — settle time before the first write.
        assert_eq!(Instant::now() - start, Duration::from_millis(600));
    }

    // ---- Frame reassembly ---------------------------------------------------

    #[test]
    fn one_chunk_one_frame() {
        let mut asm = FrameAssembler::new(0x0D);
        assert_eq!(asm.push(b"props ok\x0D"), vec![b"props ok".to_vec()]);
        assert_eq!(asm.pending(), 0);
    }

    /// A frame split across three notifications must come out once, whole.
    #[test]
    fn frame_split_across_chunks() {
        let mut asm = FrameAssembler::new(0x0D);
        assert!(asm.push(b"props Cur").is_empty());
        assert!(asm.push(b"rentSpeed ").is_empty());
        assert_eq!(asm.pending(), 19);
        assert_eq!(
            asm.push(b"3.5\x0D"),
            vec![b"props CurrentSpeed 3.5".to_vec()]
        );
    }

    /// Two frames coalesced into one notification must come out as two, plus
    /// the tail buffered toward the next.
    #[test]
    fn multiple_frames_in_one_chunk() {
        let mut asm = FrameAssembler::new(0x0D);
        assert_eq!(
            asm.push(b"one\x0Dtwo\x0Dthr"),
            vec![b"one".to_vec(), b"two".to_vec()]
        );
        assert_eq!(asm.pending(), 3);
        assert_eq!(asm.push(b"ee\x0D"), vec![b"three".to_vec()]);
    }

    /// Empty chunks happen (some stacks deliver zero-length notifications);
    /// they must be a no-op, not a frame boundary.
    #[test]
    fn empty_chunks_are_ignored() {
        let mut asm = FrameAssembler::new(0x0D);
        assert!(asm.push(&[]).is_empty());
        assert!(asm.push(b"abc").is_empty());
        assert!(asm.push(&[]).is_empty());
        assert_eq!(asm.push(b"\x0D"), vec![b"abc".to_vec()]);
    }

    /// A terminator as the very first byte (or doubled terminators) closes an
    /// empty frame — which is noise, not data, and must be discarded.
    #[test]
    fn terminator_first_and_doubled_yield_no_empty_frames() {
        let mut asm = FrameAssembler::new(0x0D);
        assert!(asm.push(b"\x0D").is_empty());
        assert_eq!(asm.push(b"\x0D\x0Dok\x0D\x0D"), vec![b"ok".to_vec()]);
        assert_eq!(asm.pending(), 0);
    }

    /// A terminator as the last byte of a chunk closes the frame exactly
    /// there, leaving nothing pending.
    #[test]
    fn terminator_as_last_byte_closes_cleanly() {
        let mut asm = FrameAssembler::new(0x0D);
        assert_eq!(asm.push(b"tail\x0D"), vec![b"tail".to_vec()]);
        assert_eq!(asm.pending(), 0);
    }

    /// Terminator-free garbage must not grow the buffer forever: past the cap
    /// the buffer is dropped and reassembly resyncs at the next terminator.
    #[test]
    fn runaway_frame_is_dropped_and_resyncs() {
        let mut asm = FrameAssembler::new(0x0D).with_max_frame_len(8);
        assert!(asm.push(&[0xAA; 100]).is_empty());
        assert!(asm.pending() <= 8);
        // The truncated garbage frame is closed and discarded along with the
        // resync; the next real frame comes through intact.
        let frames = asm.push(b"\x0Dgood\x0D");
        assert_eq!(frames.len(), 2); // residual garbage + the good frame
        assert_eq!(frames[1], b"good".to_vec());
    }

    // ---- Codec seam ----------------------------------------------------------

    /// Identity in, identity out — and the seam composes with reassembly the
    /// way a real driver stacks them: chunks → frames → decode → parse.
    #[test]
    fn identity_codec_round_trips_through_reassembly() {
        let codec = IdentityCodec;
        let mut asm = FrameAssembler::new(0x0D);
        let mut decoded = Vec::new();
        for chunk in [&b"props Cur"[..], b"rentSpeed 3.5\x0Dprops mode 1\x0D"] {
            for frame in asm.push(chunk) {
                decoded.push(codec.decode(&frame).unwrap());
            }
        }
        assert_eq!(
            decoded,
            vec![b"props CurrentSpeed 3.5".to_vec(), b"props mode 1".to_vec()]
        );
        assert_eq!(codec.encode(b"abc").unwrap(), b"abc".to_vec());
    }

    /// The seam accepts a non-trivial transform (here ROT13-ish byte add) —
    /// the shape the KingSmith cipher-table codec takes for real
    /// (`kingsmith_props::AppCipherCodec`).
    #[test]
    fn a_custom_codec_plugs_into_the_seam() {
        struct AddOne;
        impl TransportCodec for AddOne {
            fn decode(&self, raw: &[u8]) -> Result<Vec<u8>> {
                Ok(raw.iter().map(|b| b.wrapping_sub(1)).collect())
            }
            fn encode(&self, plain: &[u8]) -> Result<Vec<u8>> {
                Ok(plain.iter().map(|b| b.wrapping_add(1)).collect())
            }
        }
        let codec: &dyn TransportCodec = &AddOne;
        let wire = codec.encode(b"speed").unwrap();
        assert_ne!(wire, b"speed".to_vec());
        assert_eq!(codec.decode(&wire).unwrap(), b"speed".to_vec());
    }

    // ---- Checksums -----------------------------------------------------------

    #[test]
    fn additive_checksum() {
        assert_eq!(checksum_sum(&[]), 0);
        assert_eq!(checksum_sum(&[0x01, 0x02, 0x03]), 0x06);
        assert_eq!(checksum_sum(&[0xFF, 0x01]), 0x00, "wraps mod 256");
        assert_eq!(checksum_sum(&[0xFF, 0xFF, 0x02]), 0x00);
    }

    #[test]
    fn xor_checksum() {
        assert_eq!(checksum_xor(&[]), 0);
        assert_eq!(checksum_xor(&[0x01, 0x02, 0x03]), 0x00);
        assert_eq!(checksum_xor(&[0xF0, 0x0F]), 0xFF);
        assert_eq!(checksum_xor(&[0xAA, 0xAA]), 0x00);
    }

    // ---- Property checks ------------------------------------------------------

    #[test]
    fn property_checks_verify_roles_not_just_uuids() {
        use btleplug::api::Characteristic;
        use std::collections::BTreeSet;

        let chr = |short: u16, props: CharPropFlags| Characteristic {
            uuid: sig_uuid(short),
            service_uuid: sig_uuid(0xfff0),
            properties: props,
            descriptors: BTreeSet::new(),
        };
        // LifeSpan-shaped table: notify on FFF1, write on FFF2.
        let lifespan_shaped: BTreeSet<_> = [
            chr(0xfff1, CharPropFlags::NOTIFY),
            chr(0xfff2, CharPropFlags::WRITE),
        ]
        .into();
        assert!(has_notify(&lifespan_shaped, sig_uuid(0xfff1)));
        assert!(has_write(&lifespan_shaped, sig_uuid(0xfff2)));
        assert!(!has_write(&lifespan_shaped, sig_uuid(0xfff1)));
        assert!(!has_notify(&lifespan_shaped, sig_uuid(0xfff2)));

        // Deerrun-shaped table: same UUIDs, roles swapped. The UUIDs alone
        // would pass; the property checks must not.
        let deerrun_shaped: BTreeSet<_> = [
            chr(0xfff1, CharPropFlags::WRITE_WITHOUT_RESPONSE),
            chr(0xfff2, CharPropFlags::NOTIFY),
        ]
        .into();
        assert!(!has_notify(&deerrun_shaped, sig_uuid(0xfff1)));
        assert!(!has_write(&deerrun_shaped, sig_uuid(0xfff2)));
        assert!(has_write(&deerrun_shaped, sig_uuid(0xfff1)));
    }
}
