# Contributing to Trot

Trot is a small project with a narrow purpose: read an under-desk treadmill over
Bluetooth, keep the record locally, and expose it as a plain API. Contributions
that make it do that better are very welcome.

## The most useful thing you can contribute

**An adapter for a treadmill we can't read yet.** Trot currently ships eight
drivers: native adapters for LifeSpan/Omni, KingSmith WalkingPad (both the
WiLink and app-cipher generations), Urevo, Sperax, PitPat/Deerrun/SupeRun and
the FitShow OEM family, plus generic FTMS for everything else. Only LifeSpan
is tested by us on real hardware — the rest are ports of open-source reverse
engineering pinned against published captures, so hardware reports (even "it
works") are valuable on their own.

If your treadmill doesn't work, a bug report with a `trot scan --all` listing and
a `GET /api/diag` dump is genuinely valuable even if you never write a line of
code. It's the part nobody else can do for us.

Want to write the adapter yourself? A driver is one self-contained file plus
one registration line, and you don't need to touch the engine, storage, or the
API to add one. **[docs/drivers/README.md](docs/drivers/README.md)** walks
through the whole thing — identifying your device, capturing raw frames, the
driver trait, testing without hardware, and a complete worked example.

## Getting set up

```sh
git clone https://github.com/marcuspuchalla/trot
cd trot
cargo build
cargo test --workspace
```

You need **Rust 1.85 or newer**. On Linux you also need `libdbus-1-dev` and
`pkg-config` — btleplug talks to BlueZ over D-Bus.

You do **not** need a treadmill to work on most of the codebase: the protocol
decoders, storage, de-glitching, and the HTTP API are all covered by tests that
run without hardware.

## Before you open a pull request

CI runs these on Linux, macOS and Windows, and a release cannot be built unless
they pass — so it's worth running them locally first:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets   # CI treats warnings as errors
cargo test --workspace
```

If you changed the CLI's commands or flags, regenerate the completion scripts —
CI diffs them and fails if they've drifted:

```sh
cargo run --bin trot -- completions bash       > completions/trot.bash
cargo run --bin trot -- completions zsh        > completions/_trot
cargo run --bin trot -- completions fish       > completions/trot.fish
cargo run --bin trot -- completions powershell > completions/_trot.ps1
cargo run --bin trot -- completions elvish     > completions/trot.elv
```

## Things worth knowing about the codebase

- **Trot observes treadmills; it never controls them.** No code in this tree
  starts, stops, or changes the speed of a belt, and none ever will — that's
  a design commitment, not a missing feature. It's what lets someone run a
  daemon with Bluetooth access to the machine under their feet and know it
  cannot move that machine, whatever else goes wrong. A PR that adds belt
  control (speed, start/stop, incline, mode) will be declined, however well
  built. Writes that merely *ask* the device for data — poll frames, init
  handshakes — are fine and often required; the driver guide
  ([docs/drivers/README.md](docs/drivers/README.md)) spells out the
  distinction, which matters because most reference implementations you'd
  port from do both.
- **`/api` + `/ws` is a public contract.** Other things are built on it. Adding a
  route is easy; changing or removing one is a breaking change and needs to be
  treated as such.
- **Output is data.** CLI subcommands print their result and nothing else — no
  banners, no taglines. The one flourish lives in `--help`, deliberately.
- **The device lies.** Treadmill odometers emit stale frames, reset mid-session,
  and wrap. `db.rs` has a de-glitching accumulator with tests pinning real-world
  failure shapes. If you touch it, add the case that broke.
- **Local-first is not decoration.** No accounts, no cloud, no telemetry, and
  nothing on the landing page fetched from a third party. Changes that quietly
  add a network dependency will be declined.

## Style

Match the code around you. Comments should explain *why* something is the way it
is — especially where the reason is a hardware quirk or a platform constraint
that isn't obvious from the code.

## Licensing

Trot is **GPLv3**. By contributing you agree your contribution ships under it.

The name "Trot" and the runner mark are reserved and are *not* covered by the
GPL (see the Trademarks section of the README). Fork the code freely — just give
your fork its own name.
