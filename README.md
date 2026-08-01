<p align="center">
  <img src="./docs/brand/readme-header.svg" alt="Trot — the open-source engine under the desk" width="100%">
</p>

<p align="center">
  <a href="https://github.com/marcuspuchalla/trot/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/marcuspuchalla/trot?style=flat-square&color=87E939&labelColor=111A17"></a>
  <a href="./LICENSE"><img alt="GPLv3" src="https://img.shields.io/badge/license-GPLv3-87E939?style=flat-square&labelColor=111A17"></a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-EAF4EC?style=flat-square&logo=rust&logoColor=EAF4EC&labelColor=111A17">
  <img alt="Local first" src="https://img.shields.io/badge/data-local--first-87E939?style=flat-square&labelColor=111A17">
</p>

> **TROT's Really Only Treadmilling.** An honest little tracker for treadmill
> walking — a CLI + background daemon that talks to your under-desk treadmill,
> records sessions, and exposes clean data. No opinions about presentation; it
> does the modest thing and says so on the tin.

**Bluetooth in. Local API out. Nothing leaves your machine.**

- **Website:** [trot.fmp.dev](https://trot.fmp.dev)
- **License:** GPLv3.
- **Status:** early days — `v0.2` builds a working `trot` CLI + daemon for
  LifeSpan (native) and generic FTMS treadmills. Interfaces may still shift.

## What it does
- Device connectivity for under-desk treadmills (LifeSpan/Omni + generic FTMS
  first; architected for more adapters).
- Session lifecycle (start/stop/pause/resume), streaks, local-first history.
- Metrics: distance, time, cadence, speed, derived calories.
- A stable local **HTTP + WebSocket API** — the contract anything on top consumes.
- A real terminal surface:
  ```
  trot daemon           # run the engine (serves the API)
  trot scan             # scan + interactively pick a treadmill to pair
  trot pair <id>        # pair a specific device id (non-interactive)
  trot devices          # list paired treadmills (* = active)
  trot unpair           # forget the active treadmill
  trot status           # is the daemon up? treadmill connected?
  trot today            # what you did today
  trot log --week       # the week's ledger
  ```

## Install

**Prebuilt binaries** for every release live on the
[Releases page](https://github.com/marcuspuchalla/trot/releases/latest). One-liners:

```sh
# macOS & Linux
curl -LsSf https://github.com/marcuspuchalla/trot/releases/latest/download/trot-installer.sh | sh
```
```powershell
# Windows (PowerShell)
irm https://github.com/marcuspuchalla/trot/releases/latest/download/trot-installer.ps1 | iex
```

| Platform | Archive |
|---|---|
| macOS · Apple Silicon | `trot-aarch64-apple-darwin.tar.xz` |
| macOS · Intel | `trot-x86_64-apple-darwin.tar.xz` |
| Linux · x86_64 | `trot-x86_64-unknown-linux-gnu.tar.xz` |
| Linux · arm64 (Raspberry Pi, ARM servers) | `trot-aarch64-unknown-linux-gnu.tar.xz` |
| Windows · x64 | `trot-x86_64-pc-windows-msvc.zip` |

Every archive carries the binary plus `LICENSE`, `README.md`, `CHANGELOG.md`,
`THIRD-PARTY-NOTICES.md` and a `completions/` directory, and each has a `.sha256`
alongside it.

> **macOS:** the binaries are not signed or notarized (Trot is a one-person
> project and an Apple Developer account is a yearly fee). The `curl … | sh`
> installer is unaffected, but if you download an archive **in a browser** macOS
> quarantines it. Clear that with:
> ```sh
> xattr -dr com.apple.quarantine ./trot
> ```

**Linux** needs BlueZ at runtime (`libdbus`/`bluez` — present on any desktop
distro). **Windows** needs Bluetooth LE support, which is standard on Windows 10+.

### Shell completions
`trot` can complete its own subcommands and flags — `trot da<Tab>` becomes
`trot daemon`.

```sh
trot completions --install        # guesses your shell from $SHELL
trot completions zsh --install    # or name it
```

That writes the script where your shell looks for it (`~/.zfunc/_trot`,
`~/.local/share/bash-completion/completions/trot`,
`~/.config/fish/completions/trot.fish`) and prints anything you still need to add
to your rc file. Restart the shell afterwards.

Prefer to place it yourself, or packaging Trot? The script goes to stdout without
`--install`:

```sh
trot completions bash > /usr/local/etc/bash_completion.d/trot
```

PowerShell and Elvish have no drop-in directory, so append the output to your
profile instead. Pre-generated scripts for every shell also ship in each release
archive under `completions/`.

### Build from source
Needs a [Rust toolchain](https://rustup.rs) — **1.85 or newer** — and on Linux
also `libdbus-1-dev` and `pkg-config`.

```sh
git clone https://github.com/marcuspuchalla/trot
cd trot && cargo build --release   # binary at target/release/trot
```

## The local API
The daemon serves JSON over HTTP plus a live WebSocket stream on an **ephemeral
loopback port**. On start it writes `runtime.json` (`{port, token}`) into the data
directory; that's how the CLI and any UI find it.

| Route | Method | What it gives you |
|---|---|---|
| `/api/health` | GET | Is the daemon up, is a treadmill connected, and which engine version |
| `/api/state` | GET | Full snapshot: connection, live telemetry, today's totals |
| `/api/today` | GET | Today's totals plus a 24-hour step breakdown |
| `/api/sessions`, `/api/sessions/:id` | GET | Recorded sessions |
| `/api/analytics` | GET | Bucketed timeseries (`metric`, `resolution`, `range_days`) |
| `/api/timeofday` | GET | Cumulative steps up to a point in a given day |
| `/api/steps/by-device` | GET | Daily steps split by the device that recorded them |
| `/api/devices` | GET | Paired treadmills |
| `/api/scan` | GET | Scan for nearby treadmills |
| `/api/export`, `/api/diag` | GET | Full data export / support dump |
| `/api/pair`, `/api/unpair` | POST | Pair / forget the active treadmill |
| `/api/connect`, `/api/disconnect` | POST | Reversibly drop or resume the BLE link |
| `/api/devices/active`, `/api/devices/forget` | POST | Switch / forget a device |
| `/api/settings` | GET/POST | Unit, locale, device label, first-run flag |
| `/api/import`, `/api/data/reset`, `/api/data/restore` | POST | Backup & restore |
| `/api/rollup/status`, `/api/rollup/run` | GET/POST | Retention internals |
| `/api/mark/speed` | POST | Record the speed you set on the console |
| `/ws` | WS | Live telemetry, session and status events |

**Writes need the token.** Send it as `x-sc110-token`, read from `runtime.json`.

## Security & privacy
Trot is local-first: your data lives in a SQLite file on your machine, there is no
account, no cloud, and no telemetry. The API is bound to `127.0.0.1` only.

What's enforced:
- The data directory is created `0700` and `runtime.json` (which carries the API
  token) `0600`, so **other users on the machine can't read your data or drive the
  API**.
- Every state-changing call requires the per-launch token.
- Requests must carry a loopback `Host` (defeats DNS rebinding), and any browser
  `Origin` must be on a small allow-list — this covers the `/ws` upgrade, which
  CORS does not.

What's deliberately *not* enforced: **read-only endpoints don't require the
token.** A process running as you, that discovers the port, can read your activity
data — but such a process could equally well just open the SQLite file, so the
token would be theatre. Trot assumes your user account is your trust boundary.

## Supported treadmills
Trot reads treadmills over **Bluetooth Low Energy** — under-desk walking pads and
full-size treadmills alike. Two ways in:

- **LifeSpan\*** — LifeSpan consoles use a proprietary protocol, so Trot ships a
  **native adapter** for them (developed and tested against a LifeSpan walking pad).
- **Standard FTMS\*** — any treadmill that broadcasts the standard Bluetooth
  **Fitness Machine Service** (FTMS, `0x1826`). Models *documented* to broadcast
  FTMS include Horizon\* AT-series, Technogym MyRun\*, BowFlex\* T9, 3G Cardio\*,
  Matrix\* (XER-02+) and newer WalkingPad\* / KingSmith pads — it's per-model, so
  verify yours. (Only LifeSpan is tested by us on real hardware.)

FTMS is per-model: if your treadmill has an "FTMS" or "broadcast to Zwift/Kinomap"
mode, `trot scan` will find it. Closed ecosystems — **iFit** (NordicTrack\* /
ProForm\*), **Peloton\*** and **Echelon\*** — don't broadcast their data, so no
third-party tool can read them.

<sub>\* Trademarks of their respective owners — see [Trademarks](#trademarks).</sub>

## Pairing a treadmill
Sessions are tracked against the **active** paired treadmill, which is remembered
across restarts. First-time setup:

```
trot scan       # scans, then arrow-key pick a treadmill to pair
trot daemon     # start tracking; auto-connects to the paired treadmill
```

`trot scan` shows an interactive picker on a terminal (use `--list` for plain
output, or `trot pair <id>` to pair a specific device id in scripts). Pairing works
**with or without** the daemon running: if it's up it owns the Bluetooth adapter,
so the pick goes through its API and it connects immediately — no restart; if it's
down, your choice is saved and the next `trot daemon` picks it up.

The daemon **connects to the paired treadmill on start and disconnects it cleanly
on stop** (Ctrl-C or SIGTERM), so the belt's Bluetooth link isn't left open.

## Structure

<p align="center">
  <img src="./docs/brand/architecture.svg" alt="Treadmill over BLE into the trot daemon (Rust engine + local SQLite), out over localhost as HTTP + WebSocket to CLI, app and script clients" width="100%">
</p>

```
Cargo.toml               # workspace
crates/
  trot-core/             # engine library: ble · protocol · ftms · db · state · api · config
  trot-daemon/           # the `trot` binary: `daemon` serves /api + /ws; the other
                         # subcommands (scan/pair/today/…) drive it over that API
```

## Development & testing

```sh
cargo test --workspace          # the whole suite
cargo clippy --workspace --all-targets   # lints (CI runs these with -D warnings)
cargo fmt --all --check         # formatting
```

**CI runs the same gate on every push and pull request** — formatting, Clippy and
the test suite on Linux, macOS *and* Windows, plus `cargo audit` for dependency
advisories. That gate lives in one reusable workflow
([`test.yml`](.github/workflows/test.yml)) which the release pipeline also calls
via dist's `plan-jobs`, so **a release cannot be built unless the tests pass** —
every build job depends on it, and publishing depends on those.

Coverage runs on each CI build (`cargo llvm-cov`): the per-file table is printed
into the run summary, and an LCOV + browsable HTML report is attached to the run
as the `coverage` artifact.

Roughly where it stands — the protocol and storage layers are the parts worth
trusting, and they're the well-covered ones:

| Area | Lines |
|---|---|
| `protocol.rs` — LifeSpan frame decoding | ~99% |
| `ftms.rs` — Bluetooth FTMS parsing | ~96% |
| `db.rs` — storage, de-glitching, rollups | ~82% |
| `app.rs` — shared state, today-cache | ~70% |
| `api.rs` — routes + security guard | ~33% |
| `ble.rs` — the device worker | ~21% |
| `main.rs` — CLI | 0% |
| **total** | **~58%** |

`ble.rs` and the CLI are low because they want a real treadmill and a real
terminal; the security-critical half of `api.rs` (the request guard, the bounds
checks) is covered by [`tests/api_guard.rs`](crates/trot-core/tests/api_guard.rs),
which drives the actual router over HTTP.

## Design rules
- Presentation-agnostic. Local-first. Privacy-respecting (no cloud required).
- The API is the product's public surface — keep it stable and documented.
- Voice: honest, modest, self-aware. You're not "training," you're trotting along
  at 3 km/h answering email, and the tool says so.

## Trademarks
Trot is an independent, unofficial interoperability tool. It is **not affiliated
with, endorsed by, or sponsored by** any treadmill manufacturer, and it only reads
data your treadmill already broadcasts over Bluetooth.

LifeSpan, Horizon, BowFlex, Technogym, Matrix, 3G Cardio, WalkingPad / KingSmith,
NordicTrack, ProForm, Peloton and Echelon are trademarks or registered trademarks
of their respective owners. The **Bluetooth®** word mark and logos are registered
trademarks owned by Bluetooth SIG, Inc. All other product and company names are the
property of their respective holders; their use here is for identification and
compatibility purposes only.

Trot's own name and mark are reserved and are **not** covered by the GPLv3 that
covers the code — see [`docs/brand/`](docs/brand/README.md). Fork the code freely;
just give your fork its own name.

## Acknowledgements
Trot's LifeSpan / Omni protocol support was bootstrapped from and cross-checked
against [**blak3r/treadspan**](https://github.com/blak3r/treadspan) (MIT,
© 2025 Blake Robertson), which reverse-engineered the LifeSpan Omni console
protocol. See [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
