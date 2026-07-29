# Trot audit — prioritized fix checklist

## Audit #2 (v0.2.0) — full re-audit, everything below is DONE

Covered performance, correctness, concurrency, security, packaging, licensing,
CI and portability. Findings were re-verified against the tree as it stood, which
by then included three commits (BLE pause/resume, connect backoff, per-device
attribution) that the first audit never saw.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| H1 | High | `today_payload()` re-walked every raw sample of the day on **every BLE poll** (~10–15 Hz) under the DB lock. Measured 75 ms @10k samples, 412 ms @50k | 1 s cache, keyed by local date, invalidated on session/data boundaries |
| H2 | High | Lost wakeup — `notify_waiters()` stores no permit, so a wake between the flag check and the await parked the worker forever; `/api/connect` reported success | register with `Notified::enable()` before reading flags; same future reused for backoff |
| H3 | High | No single-instance guard, no `busy_timeout`, and `SQLITE_BUSY` discarded by `let _ =` | refuse a second daemon (live probe, so a stale handshake still allows restart), `busy_timeout=5000`, log failed writes |
| M1 | Medium | ~1M raw rows per day of walking | throttled to 1 row/s, status transitions written through |
| M1b | Medium | **New find:** `duration_running_s` converted sample counts to seconds with a hardcoded 2.5 s spacing — wrong by ~30× | shared `db::SAMPLE_INTERVAL_S` used by writer and both timeseries branches |
| M2 | Medium | Unauthenticated GET reads | documented the threat model honestly in the README (same-user processes can read the SQLite file anyway); added `nosniff` |
| M3 | Medium | `atomic_write` temp file got default perms; token file briefly world-readable; no fsync | open 0600 before writing, `sync_all()` before rename |
| M4 | Medium | 3 new routes shipped with no changelog/README/version | README API table + security section, 0.2.0 changelog, version bump |
| M5 | Medium | Detached tasks unsupervised; a panic killed ingestion silently | `supervise()` restarts with backoff; AppState locks recover from poisoning |
| L1–L9 | Low | doc-comment hijack, by-device lag not signalled, `device_name` control chars, `now_ts()` panic, no CI, no `cargo audit`, not rustfmt-clean, Windows orphan gap | all fixed except Windows (documented: needs a parent-side Job Object) |

**Verified, not assumed:** the single-instance guard and 0600/0700 permissions were
tested against a running daemon; `cargo audit` reports no advisories across 272
deps; release archives already ship `LICENSE` (GPLv3 §4 satisfied).

**Consciously declined:** rate limiting (loopback-only, writes token-gated, and the
one real amplifier — `/api/analytics` — is now bucket-capped); token-gating GET
reads (would break the published contract for Nowhere for no gain against a
same-user attacker).

---

# Audit #1 (v0.1.1) — prioritized fix checklist

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
