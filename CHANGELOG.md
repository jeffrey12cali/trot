# Changelog

All notable changes to `trot` are documented here.

## 0.1.1

Hardening + correctness pass from a pre-launch security & code audit. No CLI,
`/api`, or `/ws` route/shape changes — the public contract is unchanged.

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
