# Changelog

All notable changes to `trot` are documented here.

## 0.3.1

### Fixed
- **Step counts were silently under-reported, and the gap grew all day.** The
  rollup cutoff was `now - 60s`, which lands in the middle of a minute. That
  minute was written from only the samples seen so far, then `last_rolled`
  advanced past its end so the remainder was never rolled — and because the
  upsert replaces `steps_delta` rather than adding to it, the truncated value
  was permanent. One minute was gutted per rollup run, forever. Measured on real
  data: 2688 steps of raw samples stored as 2251 (-16%), with affected buckets
  retaining ~28% of their samples. The cutoff is now aligned to a bucket
  boundary, so only complete minutes are rolled.
- A sample landing exactly on a rollup boundary was counted by neither the run
  that ended there (`ts < start`) nor the one that began there (`ts > start`).
  With one sample per second that is a guaranteed loss of a sample per boundary,
  which also under-reported running time. The lower bound is now inclusive.

## 0.3.0

Shell completions and a signed macOS build.

### Changed
- **Minimum supported Rust is now 1.85** (was 1.77), so `clap_complete` can track
  its 4.6 line rather than being pinned back to 4.5 to keep the old floor true.

### Added
- `trot completions <shell>` — shell completion for the subcommands and flags,
  so `trot da<Tab>` becomes `trot daemon`. `--install` writes the script where
  the shell will find it and guesses the shell from `$SHELL`; bash, zsh, fish,
  PowerShell and Elvish are supported. Pre-generated scripts also ship in every
  release archive under `completions/` for packagers. CI regenerates them and
  fails if they've drifted from the command tree.
- `trot --help` draws the Trot mark as ASCII art, in the logo's own colours.
  Terminal only — piped output stays clean and greppable — and it honours
  `NO_COLOR` and falls back on 16-colour terminals.
- **macOS binaries are now signed** with a Developer ID certificate and the
  hardened runtime. Notarization is wired up but **not yet working**: Apple's
  Notary Service has accepted every submission and then left it `In Progress`
  indefinitely, so a browser-downloaded archive still needs
  `xattr -dr com.apple.quarantine` for now.
- Prebuilt binaries for **Linux arm64** (Raspberry Pi, ARM servers), alongside
  macOS (Intel + Apple Silicon), Linux x86_64 and Windows x64.

## 0.2.1

### Added
- `/api/health` now reports the engine's own `version`. The desktop app ships
  the engine as a separate sidecar binary, so it can be older than the app
  bundling it — this lets a client show both without asking the user to run a
  diagnostic dump.

## 0.2.0

New device controls, plus a second audit pass that turned up a performance
problem and a couple of real bugs.

### Added
- `POST /api/connect` / `POST /api/disconnect` — reversibly drop the Bluetooth
  link while leaving the treadmill paired and the engine (and sync) running.
- `GET /api/steps/by-device` — daily step totals split by the device that
  recorded them, with `device_name` added to `/api/settings`.
- The daemon gives up auto-connecting after repeated failures and waits for a
  manual reconnect, instead of scanning forever.
- The README now documents the whole `/api` + `/ws` surface and the security
  model.

### Fixed
- **Today's totals are no longer recomputed on every Bluetooth poll.** They were
  recalculated 10–15 times a second, each time re-walking every raw sample of the
  day (~410 ms once a day had 50k samples) while holding the database lock, which
  made the engine progressively slower during a walk and stalled API reads behind
  it. Now cached for a second and invalidated on session boundaries.
- **`duration_running_s` was wrong by roughly 30×** — it converted a sample count
  to seconds with a hardcoded 2.5 s spacing that never matched the real rate.
- **A reconnect could be silently ignored.** A wake arriving in a narrow window
  was dropped, leaving the worker parked forever while `/api/connect` reported
  success.
- Raw samples are stored once a second rather than on every poll (~1M rows a day
  before), with status transitions always written through.
- A second `trot daemon` on the same data directory is now refused instead of
  both fighting over the adapter and the database.
- Failed database writes are logged instead of silently discarded, and
  `busy_timeout` lets a contended write wait rather than fail.
- The BLE worker and rollup loop are restarted if they panic; a panic can no
  longer poison a lock and take the API down with it.
- Config, snapshot and handshake files are created private (0600) *before* being
  written, and flushed to disk, closing a window where the API token was briefly
  world-readable.
- Responses carry `X-Content-Type-Options: nosniff`.

## 0.1.1

Hardening + correctness pass from a pre-launch security & code audit. No CLI or
`/ws` route/shape changes.

- **Security:** the request guard now rejects disallowed browser `Origin`s on
  every request, closing the `/ws` upgrade (which CORS does not cover) to
  cross-site pages. The daemon writes its `runtime.json` handshake (which holds
  the API token) `0600` and locks the data directory `0700` on Unix.
- **Correctness:** the analytics timeseries now de-glitches its raw tail the same
  way the daily totals do, so a single stale device frame can no longer spike a
  chart bucket. The step de-glitcher also drops a garbage stale-high opening
  frame instead of counting it as baseline steps.
- **Robustness:** config, settings, snapshot, and handshake files are written
  atomically (temp file + rename), so a crash mid-write can't leave a truncated
  file that resets your pairing/settings. `/api/data/reset` refuses to run on an
  already-empty database (no more clobbering a prior snapshot). `/api/analytics`
  rejects absurd range/resolution combinations that would ask SQLite for millions
  of buckets.
- Misleading security comments corrected to match the implementation.

## 0.1.0

First release.

- `trot daemon` — the engine: connects to your treadmill over Bluetooth Low
  Energy and serves a local **HTTP + WebSocket API**.
- `trot scan` / `pair` / `devices` / `unpair` — interactive pairing and device
  management; the daemon connects on start and disconnects cleanly on stop.
- `trot today` / `status` / `log` — read your activity straight from the terminal.
- **LifeSpan** under-desk treadmills via a native adapter; **generic FTMS**
  treadmills (walking pads and full-size) that broadcast the standard profile.
- Local-first: SQLite storage, no account, no cloud, no telemetry.
- Prebuilt binaries for macOS (Intel + Apple Silicon), Windows, and Linux, with
  shell / PowerShell installers.
