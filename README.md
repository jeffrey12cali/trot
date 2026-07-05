# TROT

**TROT's Really Only Treadmilling.**

An honest little tracker for treadmill walking — a CLI + background daemon that
talks to your under-desk treadmill, records sessions, and exposes clean data.
No opinions about presentation; it does the modest thing and says so on the tin.

- **License:** GPLv3.
- **Status:** early days — `v0.1` builds a working `trot` CLI + daemon for
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
```
Cargo.toml               # workspace
crates/
  trot-core/             # engine library: ble · protocol · ftms · db · state · api · config
  trot-daemon/           # the `trot` binary: `daemon` serves /api + /ws; the other
                         # subcommands (scan/pair/today/…) drive it over that API
```

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

## Acknowledgements
Trot's LifeSpan / Omni protocol support was bootstrapped from and cross-checked
against [**blak3r/treadspan**](https://github.com/blak3r/treadspan) (MIT,
© 2025 Blake Robertson), which reverse-engineered the LifeSpan Omni console
protocol. See [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
