# Trot pre-launch audit — prioritized fix checklist

Derived from the Fable5 security/correctness audit (2026-07-21). Severity is judged
against the real threat model: a **single-user, loopback-only** desktop engine.

Status legend: `[x]` done in this pass · `[ ]` deferred (with reason).

## P0 — Fix before the public/HN launch (credibility + real gaps)

- [x] **A1 · False security comments.** `api.rs` router said "No CORS layer" while a
  `cors()` layer *is* applied; `app.rs`/`api.rs` claimed a token is "injected into the
  served index.html" and the UI is "served same-origin from this very server" — the
  engine serves **no HTML at all**. Comments rewritten to match reality.
  *Files:* `api.rs:106-118`, `app.rs:19-21`.
- [x] **A2 · `/ws` had no Origin check (CORS doesn't cover WebSockets).** Added an
  Origin allow-list check to the request `guard`, so it now covers every request —
  including the `/ws` upgrade and all GET reads. Non-browser clients (CLI, `ureq`)
  send no Origin and pass through. *File:* `api.rs` `guard`.
- [x] **A3 · `timeseries` raw tail used `MAX(col)-MIN(col)`** (the un-de-glitched
  method) while day/hour views de-glitch — a single stale frame could spike an
  analytics chart the totals suppress. Raw-tail branch now de-glitches (same rule as
  the rollup writer). *File:* `db.rs` `timeseries`.
- [x] **A4 · Double `/api/data/reset` destroyed the backup.** Reset unconditionally
  exported-then-overwrote the snapshot, so a second reset overwrote the good snapshot
  with an empty DB. Reset now refuses when the DB is already empty. *File:* `api.rs`
  `api_data_reset`.
- [x] **A5 · API token in a world-readable `runtime.json`.** Data dir is now created
  `0700` and `runtime.json` written `0600` on Unix. *Files:* `engine.rs`, `main.rs`.

## P1 — Also worth addressing (fixed this pass)

- [x] **B1 · De-glitch fooled by a garbage *first* frame.** The spike-drop rule only
  ran for interior samples, so a stale-high opening reading was counted as baseline
  steps. `deglitch_walk`/`deglitch_tail` now drop a first frame that the next sample
  contradicts by more than `spike`. *File:* `db.rs`.
- [x] **B2 · Non-atomic config/snapshot writes.** A crash mid-write left truncated
  JSON that silently reset pairing/settings to defaults. All config + snapshot writes
  now go through a temp-file + atomic `rename`. *Files:* `config.rs`, `api.rs`, `main.rs`.
- [x] **B3 · `/api/analytics` bucket amplifier.** `range_days=1825 & resolution=minute`
  asked SQLite for ~2.6M buckets. The handler now rejects requests whose
  `range ÷ resolution` exceeds a sane bucket cap. *File:* `api.rs` `api_analytics`.

## P2 — Deferred (deliberate, with reason)

- [ ] **C1 · Move blocking `rusqlite` off the Tokio workers (`spawn_blocking`).** Real,
  but a broad refactor across ~20 handlers + the `Db` API; impact is bounded on a
  single-user desktop, and the concrete DoS amplifier (B3) is already capped. Track as
  a follow-up; verify with a load test.
- [ ] **C2 · Make `/api/scan` a token-guarded POST.** It's a GET with a BLE side-effect,
  but the `/api` surface is the **stable public contract** consumed by Nowhere and the
  CLI — changing the verb is a breaking change that must be coordinated across repos.
  Mitigated for now by the new Origin guard (A2). Do it in a contract-versioned bump.
- [ ] **C3 · Two consecutive stale-*low* frames can still cause a false reset.** Fixing
  this safely needs a windowed reset-confirmation tuned against real hardware captures;
  guessing risks corrupting real users' totals (worse than the transient it fixes).
  Needs a `/api/diag` capture of a bad reconnect to tune. Residual is Medium and
  self-heals after the minute rolls up (~5 min).

## Nits (not blocking; noted for a later polish pass)
- Duplicated `RETENTION_DAYS`/`ROLLUP_INTERVAL_S` in `engine.rs` and `api.rs`.
- `let _ = STATUS_RUNNING;` dead-code silencer in `ble.rs`; re-parsing const UUIDs per call.
- `now_ts()` uses `.unwrap()` while `config::now()` uses `unwrap_or(0.0)` — make consistent.
