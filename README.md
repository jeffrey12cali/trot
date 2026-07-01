# TROT

**TROT's Really Only Treadmilling.**

An honest little tracker for treadmill walking — a CLI + background daemon that
talks to your under-desk treadmill, records sessions, and exposes clean data.
No opinions about presentation; it does the modest thing and says so on the tin.

- **License:** GPLv3.
- **Status:** to be extracted from the current Tauri monolith (see
  [`../PLAN.md`](../PLAN.md)). Nothing has moved yet.

## What it does
- Device connectivity for under-desk treadmills (LifeSpan/Omni + generic FTMS
  first; architected for more adapters).
- Session lifecycle (start/stop/pause/resume), streaks, local-first history.
- Metrics: distance, time, cadence, speed, derived calories.
- A stable local **HTTP + WebSocket API** — the contract anything on top consumes.
- A real terminal surface, e.g.:
  ```
  trot daemon           # run the engine (serves the API)
  trot today            # what you did today
  trot log --week       # the week's ledger
  trot start | stop     # session control
  trot scan | pair      # find and pair a treadmill
  ```

## Planned structure
```
Cargo.toml               # workspace
crates/
  trot-core/             # engine as a library: ble · protocol · ftms · db · state · api · config
  trot-daemon/           # `trot daemon` — runs start_engine(), serves /api + /ws
  trot-cli/              # `trot` — subcommands over the daemon API
```

## Design rules
- Presentation-agnostic. Local-first. Privacy-respecting (no cloud required).
- The API is the product's public surface — keep it stable and documented.
- Voice: honest, modest, self-aware. You're not "training," you're trotting along
  at 3 km/h answering email, and the tool says so.
