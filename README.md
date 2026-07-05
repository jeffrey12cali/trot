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
  trot scan             # scan + interactively pick a treadmill to pair
  trot pair <id>        # pair a specific device id (non-interactive)
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
