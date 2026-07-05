# Changelog

All notable changes to `trot` are documented here.

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
