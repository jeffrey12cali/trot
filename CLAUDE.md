# CLAUDE.md — trot

**TROT** — the open-source (GPLv3) Rust engine + CLI for under-desk treadmills.
This repo is the **source of truth** for how the tool behaves. Crates:
`trot-core` (engine library: ble · drivers [one file per treadmill protocol,
see docs/drivers/README.md] · telemetry · db · state · api · config)
and `trot-daemon` (the `trot` binary: `daemon`, `scan`, `pair`, `devices`,
`unpair`, `status`, `today`, `log`).

## ⚠️ A change here ripples outward — update the downstream artifacts

Trot is presented to users in several hand-maintained places that do NOT update
themselves. **When you change the CLI or engine, update all of the affected ones
in the same change.** A change "here" includes: command names/flags, output
format of any subcommand, the `/api` + `/ws` routes, supported devices/brands, or
the install steps.

Downstream checklist:

1. **`README.md`** (this repo) — the canonical docs: command list, pairing flow,
   Supported treadmills, Trademarks. Keep it accurate.
2. **The Trot landing page** — repo `github.com/marcuspuchalla/trot-web`
   (deployed to trot.puchalla.dev). It has a **hand-authored terminal ANIMATION that
   simulates real `trot` output** (`trot daemon` boot + `trot today`), plus Install
   command blocks, an API endpoint table, and a Supported-treadmills section — all
   must mirror this tool. See that repo's `CLAUDE.md`.
3. **The Nowhere app** — repo `github.com/marcuspuchalla/nowhere` — where Trot is
   described in the UI/About and where the app talks to the `/api` + `/ws`
   contract. Keep both in sync (the contract is the parity boundary).
4. **Any presentation / slides / deck** that shows Trot's commands or output.

## Conventions

- **Output is data, not chatter.** CLI subcommands print only their result — no
  taglines or verbose banners. The "it's really only treadmilling" line lives ONLY
  as the `--help` `about` string (the deliberate easter egg); do not reintroduce
  it into command output.
- **Naming:** prefer the brand **"LifeSpan"** generally in user-facing text rather
  than the specific model number ("SC110"), which is an internal/protocol detail.
- **Trademarks:** third-party brand names (LifeSpan, NordicTrack, Peloton, Woodway,
  Technogym, Bluetooth) must stay covered by the README's Trademarks section. Add
  any new brand there too.
- The **`/api` + `/ws` surface is public and stable** — it's the contract Nowhere
  and any third-party client build on. Treat breaking changes as breaking.
