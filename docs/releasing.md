# Releasing

Trot and Nowhere are separate programs with separate release pipelines, joined
by one optional hand-off. This is how both work.

## The short version

1. Write what changed under `## Unreleased` in `CHANGELOG.md`, as you make the
   change.
2. Actions → **Cut release** → pick `patch` / `minor` / `major` → Run.
3. Done. Everything else — tests, five platform builds, signing, notarization,
   the GitHub Release — already happens on its own.

If the engine change matters to Nowhere users, set **Also update Nowhere** in
step 2 and fill in the three prose fields. Otherwise leave it at `no`.

## What "Cut release" actually does

It promotes `## Unreleased` to a version heading, bumps both crates, refreshes
`Cargo.lock`, commits, tags, and pushes. The tag push is the trigger the
existing `release.yml` (dist) has always used, so nothing about the build
pipeline changed — the same test gate, the same five targets, the same
codesigning, the same post-announce notarization.

**It will not write your changelog.** If `## Unreleased` is empty, the workflow
fails before doing anything else. This is on purpose. The 0.3.5 entry explains
that `0xFFF0` is squatted by at least five vendors and that one of them swaps
the notify and write roles — no tool that reads commit messages produces that
sentence, and a release note that says `fix: correct FFF0 matching` is worse
than the one you would have written. The version number automates. The prose is
yours.

## The hand-off to Nowhere

Nowhere bundles `trot` as a sidecar binary, so a new engine means a new app
build. Which engine an app contains is recorded in `.trot-version` in the
nowhere repo, and `desktop.yml` checks the engine out at exactly that tag —
so rebuilding a Nowhere tag a year from now produces the same app. Before this
was pinned, CI took whatever `main` happened to be at build time and the
release recorded nothing about it.

"Cut release" can dispatch to that repo, and there are two outcomes:

| You supply | What happens |
|---|---|
| title + summary + why | Nowhere bumps, tags and **publishes** — no further input |
| any of them blank | Nowhere opens a **pull request** with the bump done and the notes stubbed |

The split exists because Nowhere's `changelog.json` structurally requires a
`summary` and a `why` on every entry — the app shows them to users in its News
section. Neither workflow is permitted to invent them, so if you have not
written them, you get a PR to write them in rather than a release with
`TODO` in it. Choosing `pull request` explicitly always gives you the PR.

## Secrets this needs

| Secret | Where | Scope |
|---|---|---|
| `RELEASE_PAT` | trot | fine-grained PAT, `contents: write` on **trot and nowhere** |
| `RELEASE_PAT` | nowhere | fine-grained PAT, `contents: write` + `pull-requests: write` on nowhere |
| `TROT_REPO_TOKEN` | nowhere | existing — read access to trot, for the engine checkout |

These cannot be the built-in `GITHUB_TOKEN`. GitHub deliberately does not fire
workflows for pushes made with it, so a tag pushed that way would be created
and then silently build nothing. It is the same rule that forces
`notarize-macos.yml` to run as a dist post-announce job rather than
`on: release` — see the comment in `dist-workspace.toml`.

## A known gap on the Nowhere side

`desktop.yml` holds no Apple credentials, so the macOS bundles it publishes are
**unsigned and unnotarized** — only local builds via `npm run build:mac` are
signed. The workflow now checks the produced `.app` and says so in the job
summary. Setting the repo variable `REQUIRE_SIGNED_MACOS=true` turns that from
a warning into a build failure; do that once the signing secrets are in place.

This matters because the failure is quiet by nature. A build that signs but
skips notarization still exits 0 — it emits one warning line in a long log and
produces a `.dmg` Gatekeeper rejects on every install. That is exactly what
happened building 0.1.13 locally: the App Store Connect key was present but
named `NOWHERE_API_*` while Tauri only reads `APPLE_API_*`, so notarization was
skipped silently. `scripts/build-macos.sh` now maps the names and notarizes the
disk image itself, which Tauri does not do.
