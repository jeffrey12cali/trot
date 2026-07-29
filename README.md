# TROT

**TROT's Really Only Treadmilling.**

An honest little tracker for treadmill walking — a CLI + background daemon that
talks to your under-desk treadmill, records sessions, and exposes clean data.
No opinions about presentation; it does the modest thing and says so on the tin.

- **Website:** [trot.fmp.dev](https://trot.fmp.dev)
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

## The local API
The daemon serves JSON over HTTP plus a live WebSocket stream on an **ephemeral
loopback port**. On start it writes `runtime.json` (`{port, token}`) into the data
directory; that's how the CLI and any UI find it.

| Route | Method | What it gives you |
|---|---|---|
| `/api/health` | GET | Is the daemon up, is a treadmill connected |
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
