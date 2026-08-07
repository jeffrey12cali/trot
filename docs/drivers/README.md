# Writing a treadmill driver

This is the guide for adding support for a treadmill Trot can't read yet. It
assumes you've never seen this codebase; it does not assume you've written
Bluetooth code before, though it helps.

The short version: a driver is **one file** in `crates/trot-core/src/drivers/`
plus **one line** in the registry. It recognises its device, speaks its
protocol, and emits neutral SI-unit samples. Everything else — connecting,
reconnecting, sessions, storage, the API — already exists and you must not
touch it.

## How Trot talks to a treadmill

```
treadmill ──BLE──> your driver ──Sample (SI units)──> engine ──> sessions, storage, /api + /ws
```

The engine (`ble.rs`) owns the connection lifecycle. It scans, connects,
discovers services, asks the registry which driver wants the device, and then
calls that driver's `run()`. When the user pauses, switches devices, or quits,
the engine *cancels* `run()` and tears the link down itself. When `run()`
returns an error, the engine reconnects with backoff, and gives up after
repeated failures. Your driver never handles any of that.

What your driver does: turn the device's Bluetooth traffic into `Sample`
values — belt speed in km/h, distance in meters, steps, seconds, kcal — and
push each one into the `emit` sink. That's the whole job.

## Step 1: identify your device

With the treadmill awake and no phone app connected (most treadmills allow one
central at a time — kill the vendor app):

```sh
trot scan --all
```

That lists every BLE device in range with its advertised name and service
UUIDs. Find your treadmill (turn it off and on again if unsure which row it
is). What you learn here feeds `matches()` later.

If the daemon can already *connect* to your device but reads it wrongly (for
example a device that advertises FTMS but sends garbage), grab a diagnostic
dump while it's connected:

```sh
curl -s "http://127.0.0.1:$(jq -r .port < "$TROT_DATA_DIR/runtime.json")/api/diag" > diag.json
```

The `recent_frames` array in there is a timestamped ring of the raw frames the
active driver captured — see "Capturing raw frames" below.

Even if you go no further: a `trot scan --all` listing plus an `/api/diag` dump
attached to an issue is a genuinely useful contribution on its own.

## Step 2: understand the protocol

Treadmill BLE protocols come in a handful of shapes. Identify yours before
writing code, because it decides what your `run()` loop looks like:

1. **Subscribe-and-push.** The device notifies a data record on its own (~1 Hz)
   once you subscribe. Standard FTMS works this way — see `drivers/ftms.rs`.
2. **Request/response polling.** The device answers one value per request: you
   write an opcode, it notifies the reply. LifeSpan works this way — see
   `drivers/lifespan.rs`, which rotates through its opcodes ~50 ms apart.
   Others in this family (KingSmith WiLink, FitShow) add checksums and enforce
   minimum spacing between commands (WiLink wants ≥690 ms) — your loop owns its
   own timing, so just `sleep` accordingly.
3. **Init handshake, then push.** The device is silent until you write one or
   more magic frames (Urevo, Sperax, and others need between 1 and ~11 ordered
   init writes, some with delays between them). Do the writes at the top of
   `run()`, then fall into a subscribe-and-push loop.
4. **Per-command pre-amble.** Some shared vendor BLE modules require a write to
   a *separate* unlock characteristic before each real command is accepted. Do
   the extra write in your loop. If the same module turns up in a second brand
   (this happens — several Chinese ODM modules are shared across brands), pull
   the helper up into a small shared module under `drivers/` rather than
   copy-pasting it.
5. **Obfuscated transport.** The nastiest real-world case: a text protocol,
   base64'd, run through a substitution cipher, split into 16-byte GATT chunks,
   terminated by a marker byte (some KingSmith generations). Your `run()` loop
   then has three layers: reassemble notifications into complete messages
   (buffer until the terminator), decode the transport, parse the payload. All
   of that still lives in your one driver file — the trait hands you raw
   notifications and doesn't care how many layers you stack on them.

Three hard-won warnings:

- **A service UUID proves nothing.** `0xFFF0` with `FFF1`/`FFF2` is a generic
  vendor-module layout used by at least five mutually incompatible treadmill
  protocols — and at least one of them swaps the notify/write roles relative to
  the others. Match on the advertised **name prefix plus** the service, and in
  `supports()` verify the characteristic **properties** (notify where you'll
  subscribe, write where you'll write) rather than trusting UUIDs.
- **Firmware is fragile.** Cheap treadmill firmware silently drops notification
  subscriptions that arrive within a few tens of milliseconds of each other. If
  you subscribe to more than one characteristic and one mysteriously never
  fires, space the subscriptions out (the vendor apps use 100–300 ms).
- **The device lies.** Counters emit stale frames, reset mid-session, and wrap.
  Report what the device says; the storage layer de-glitches. Never smooth,
  clamp, or invent values in the driver.

### Where to look things up

If your treadmill works with the vendor's app, someone may have decoded it
already. Check licenses before you take more than knowledge:

- **qdomyos-zwift** is GPL-3.0 — license-compatible with Trot, so its dozens of
  treadmill implementations are directly reusable (port the logic, keep the
  attribution, note it in `THIRD-PARTY-NOTICES.md`).
- **peteh/pacekeeper** (GPL-3.0) — PitPat/Deerrun, including the step decode.
- **ph4-walkingpad** and **blak3r/treadspan** (both MIT) — clean references
  with raw captures for KingSmith and LifeSpan respectively.
- Anything **without a license file is not usable** — don't copy from it, even
  a little. (This is why Trot's FTMS parser is a clean-room implementation from
  the Bluetooth SIG spec.)

When you reuse decoded protocol knowledge, say so in your driver's module
comment the way `lifespan.rs` credits treadspan.

### Capturing raw frames

Call `host.record_frame(tag, &frame)` for every frame your driver receives.
The engine keeps the last ~1200 in a ring buffer and dumps them, hex-encoded
and timestamped, as `recent_frames` in `/api/diag`. The `tag` is a byte of your
choosing — LifeSpan uses the request opcode so each response can be checked
against the request that caused it; a push-style driver can use `0x00`, or
distinct tags for raw-vs-decoded layers.

This is deliberately low-tech: walk on the belt, change speed, stop, then pull
one diag dump and read the story offline. It beats attaching a debugger to a
moving treadmill, and it gives issue reporters a way to send you captures from
hardware you don't own.

## Step 3: write the driver

Create `crates/trot-core/src/drivers/yourdevice.rs`. The trait
(`drivers/mod.rs`) is three questions and a loop:

```rust
#[async_trait]
pub trait Driver: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches(&self, adv: &Advertisement) -> bool;
    fn supports(&self, adv: &Advertisement, gatt: &BTreeSet<Characteristic>) -> bool;
    async fn run(&self, link: &Peripheral, host: &DriverHost<'_>, emit: Emit<'_>) -> Result<()>;
}
```

- **`id`** — a short stable name (`"lifespan"`, `"ftms"`). Logs only.
- **`matches`** — scan-time: does this *advertisement* (name + advertised
  service UUIDs) look like your device? This is what makes it show up in
  `trot scan`. Be permissive; you get a better look later. Keep your name
  prefixes in a `const` table, not buried in code — the real-world list grows.
- **`supports`** — connect-time: the engine has connected and discovered
  services, and asks each registered driver in order whether the actual GATT
  table checks out. Verify the characteristics you will use exist *and have the
  right properties*. Returning `false` lets the device fall through to the next
  driver — that's how a mis-advertising device still ends up on the right
  protocol.
- **`run`** — speak the protocol forever. Handshake if needed, subscribe or
  poll, decode, and call `emit(sample)` with the **full latest state** (not a
  delta) on every update. Return `Err` only when the link is dead or useless;
  the engine then reconnects with backoff. Don't watch for shutdown, don't
  disconnect, don't sleep-retry a dead link yourself — cancellation and
  reconnection are the engine's job.

### The `Sample`

```rust
pub struct Sample {
    pub speed_kmh: Option<f64>,   // belt speed, km/h
    pub distance_m: Option<f64>,  // cumulative distance, meters
    pub steps: Option<u32>,       // cumulative steps, as the console counts them
    pub duration_s: Option<u32>,  // elapsed workout time, seconds
    pub calories: Option<u32>,    // cumulative energy, kcal
    pub state: Option<BeltState>, // Standby / Running / Summary / Paused / Other(u8)
}
```

SI units, always — km/h, meters, seconds — no matter what the wire speaks.
The engine converts to the API's presentation shape once, at the boundary
(`telemetry::Telemetry::from_sample`); a driver never learns that encoding
exists.

Every field is an `Option`, and `None` means **"this device cannot report
that"** — which the rest of the system treats as absent, not zero. FTMS has no
step counter, so the FTMS driver leaves `steps` as `None` and step totals
simply don't accrue from those treadmills. Do the same: report what your
device actually measures, leave out the rest, and never derive a number to
fill a hole (a fabricated step count would silently corrupt day totals that
real hardware reports correctly).

`state` drives session detection: sustained `Running` opens a session,
sustained not-`Running` closes it. If your device has no explicit status,
derive it from belt speed the way the FTMS driver does (moving ⇒ `Running`).

`host.display_unit` is the unit the user's console displays (`"km/h"` or
`"mph"`). Ignore it unless your wire format itself depends on the console's
display setting — LifeSpan is the only known case.

### Register it

Two edits in `drivers/mod.rs`:

```rust
pub mod yourdevice;

pub static DRIVERS: &[&dyn Driver] = &[&lifespan::LifeSpan, &ftms::Ftms, &yourdevice::YourDevice];
```

Order matters: the first driver whose `supports()` accepts a device wins. Put
a native protocol *before* FTMS if your device exposes both (native protocols
usually report more — steps, most importantly). That registration makes the
device discoverable in `trot scan` and connectable by the daemon; there is no
second place to edit.

## Step 4: test it without the hardware

You don't need a treadmill to test the part that matters most: the decoding.
Keep your parser as pure functions from bytes to values (like
`lifespan.rs`'s `decode_*` family and `ftms.rs`'s `parse_treadmill_data`), and
pin them with fixture frames — real bytes from your capture, as hex strings,
with the values the console displayed when you captured them:

```rust
#[test]
fn decodes_a_real_frame() {
    // Captured 2026-08-07 via /api/diag; console showed 3.5 km/h, 1234 steps.
    let frame = hx("f7 a2 01 5e 04 d2 ... fd");
    let d = parse_frame(&frame).unwrap();
    assert_eq!(d.speed_kmh, 3.5);
    assert_eq!(d.steps, 1234);
}
```

Also test the ugly cases — truncated frames, bad checksums, an unknown status
byte — they must return `Err` (or pass the raw value through), never panic:
one malformed frame from a sleepy treadmill must not take the daemon down.

Then run what CI runs:

```sh
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets   # warnings are errors in CI
```

The `run()` loop itself is the one part that genuinely wants hardware; keep it
thin (transport only, all decoding in the pure functions) and the untestable
surface stays small.

## Step 5: the pull request

Include:

- The driver file, registered, with tests on fixture frames.
- Your `trot scan --all` listing and, if you can, a short `/api/diag` capture —
  they let us review the protocol claims against reality.
- A line in `README.md`'s *Supported treadmills* section. If you name a new
  brand, add it to the *Trademarks* section too.
- Attribution in `THIRD-PARTY-NOTICES.md` if you ported protocol knowledge
  from another project.
- What hardware you tested on, and which fields you verified against the
  console (speed? steps? distance?).

Don't bump versions or edit `CHANGELOG.md` — releases are handled separately.

## A complete worked example

A fictional minimal driver for the "Acme Tread" — an init-then-push device:
it advertises the name `AcmeTread` and service `0xFF10`, stays silent until it
receives `A5 01` on `0xFF12`, then notifies 8-byte frames on `0xFF11`:

```
byte 0:    0xA5           prefix
byte 1..2: speed, cm/s    (u16 big-endian; 100 = 1 m/s)
byte 3..5: distance, m    (u24 big-endian)
byte 6..7: steps          (u16 big-endian)
```

`crates/trot-core/src/drivers/acme.rs`:

```rust
//! Acme Tread driver — init-then-push on service 0xFF10.
//!
//! Interaction model: silent until it receives the wake frame `A5 01` on
//! 0xFF12, then pushes an 8-byte record on 0xFF11 about once a second.

use super::{Advertisement, BeltState, Driver, DriverHost, Emit, Sample};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use futures::StreamExt;
use std::collections::BTreeSet;
use std::time::Duration;
use uuid::Uuid;

const SERVICE: Uuid = super::sig_uuid(0xff10);
const DATA: Uuid = super::sig_uuid(0xff11);
const CMD: Uuid = super::sig_uuid(0xff12);
const WAKE_FRAME: [u8; 2] = [0xA5, 0x01];
/// The belt stops pushing when idle; only a dead link stays quiet longer.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);
const NAME_PREFIXES: &[&str] = &["AcmeTread"];

/// Decoded record. Pure function of the bytes — this is the tested part.
fn parse_frame(frame: &[u8]) -> Result<(f64, u32, u32)> {
    if frame.len() != 8 || frame[0] != 0xA5 {
        return Err(anyhow!("bad frame: {frame:02x?}"));
    }
    let cm_s = u16::from_be_bytes([frame[1], frame[2]]) as f64;
    let dist_m = u32::from_be_bytes([0, frame[3], frame[4], frame[5]]);
    let steps = u16::from_be_bytes([frame[6], frame[7]]) as u32;
    Ok((cm_s * 0.036, dist_m, steps)) // cm/s → km/h
}

fn to_sample(speed_kmh: f64, distance_m: u32, steps: u32) -> Sample {
    Sample {
        speed_kmh: Some(speed_kmh),
        distance_m: Some(distance_m as f64),
        steps: Some(steps),
        duration_s: None, // Acme doesn't report elapsed time: absent, not zero
        calories: None,
        state: Some(if speed_kmh > 0.05 {
            BeltState::Running
        } else {
            BeltState::Standby
        }),
    }
}

pub struct Acme;

#[async_trait]
impl Driver for Acme {
    fn id(&self) -> &'static str {
        "acme"
    }

    fn matches(&self, adv: &Advertisement) -> bool {
        NAME_PREFIXES.iter().any(|p| adv.name.starts_with(p))
            || adv.services.contains(&SERVICE)
    }

    fn supports(&self, _adv: &Advertisement, gatt: &BTreeSet<Characteristic>) -> bool {
        use btleplug::api::CharPropFlags;
        // Verify roles, not just UUIDs — FFxx blocks are shared across vendors.
        gatt.iter()
            .any(|c| c.uuid == DATA && c.properties.contains(CharPropFlags::NOTIFY))
            && gatt.iter().any(|c| {
                c.uuid == CMD
                    && c.properties
                        .intersects(CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE)
            })
    }

    async fn run(&self, link: &Peripheral, host: &DriverHost<'_>, emit: Emit<'_>) -> Result<()> {
        let chars = link.characteristics();
        let data = chars.iter().find(|c| c.uuid == DATA).cloned()
            .ok_or_else(|| anyhow!("data characteristic missing"))?;
        let cmd = chars.iter().find(|c| c.uuid == CMD).cloned()
            .ok_or_else(|| anyhow!("command characteristic missing"))?;

        // Subscribe first, then wake — Acme sends its first frame immediately
        // after the wake write, and we must not miss it.
        link.subscribe(&data).await?;
        let mut notifications = link.notifications().await?;
        link.write(&cmd, &WAKE_FRAME, WriteType::WithResponse).await?;

        loop {
            let frame = match tokio::time::timeout(IDLE_TIMEOUT, notifications.next()).await {
                Ok(Some(n)) => n.value,
                Ok(None) => return Err(anyhow!("notification stream ended")),
                Err(_) => {
                    if !link.is_connected().await.unwrap_or(false) {
                        return Err(anyhow!("link dropped; reconnecting"));
                    }
                    continue; // idle belt — normal
                }
            };
            host.record_frame(0x00, &frame); // raw capture for /api/diag
            match parse_frame(&frame) {
                Ok((kmh, m, steps)) => emit(to_sample(kmh, m, steps)),
                Err(e) => tracing::warn!("acme decode error: {e}"), // skip, don't die
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_running_frame() {
        // 150 cm/s = 5.4 km/h, 250 m, 421 steps.
        let f = [0xA5, 0x00, 0x96, 0x00, 0x00, 0xFA, 0x01, 0xA5];
        let (kmh, m, steps) = parse_frame(&f).unwrap();
        assert!((kmh - 5.4).abs() < 1e-9);
        assert_eq!(m, 250);
        assert_eq!(steps, 421);
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        assert!(parse_frame(&[]).is_err());
        assert!(parse_frame(&[0xA5, 0x00]).is_err());
        assert!(parse_frame(&[0xFF; 8]).is_err());
    }
}
```

Registered in `drivers/mod.rs`:

```rust
pub mod acme;
pub static DRIVERS: &[&dyn Driver] = &[&lifespan::LifeSpan, &acme::Acme, &ftms::Ftms];
```

That's the whole surface. If your treadmill needs more ceremony — ordered init
frames with delays, a pre-amble write before each command, reassembling chunked
notifications into messages, or decoding an obfuscated transport — it all goes
inside your `run()` and your pure decode functions, in the same one file.
