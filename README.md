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
- A real terminal surface:
  ```
  trot daemon           # run the engine (serves the API)
  trot scan             # find nearby treadmills
  trot pair <id>        # pair one and make it active
  trot devices          # list paired treadmills (* = active)
  trot unpair           # forget the active treadmill
  trot status           # is the daemon up? treadmill connected?
  trot today            # what you did today
  trot log --week       # the week's ledger
  ```

## Pairing a treadmill
Sessions are tracked against the **active** paired treadmill, which is remembered
across restarts. First-time setup:

```
trot scan                              # prints each treadmill's name + id
trot pair <id> --name "My treadmill"   # pair it and make it active
trot daemon                            # start tracking; auto-connects to it
```

`scan`/`pair` work **with or without** the daemon running. If the daemon is up it
owns the Bluetooth adapter, so the commands go through its API and it connects to
a newly paired treadmill immediately — no restart. If the daemon is down, they run
locally and just record your choice, so the next `trot daemon` picks it up.

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
