# Releasing Trot — build & distribution plan

How we cut prebuilt binaries for macOS / Windows / Linux, publish a GitHub
Release, and get onto Homebrew (and later Debian/Arch).

---

## TL;DR

- **You cannot build every platform locally.** Trot's BLE layer (`btleplug`) has a
  different backend per OS, and each needs that OS's SDK:
  - macOS → **CoreBluetooth** (`objc2-core-bluetooth`) — build on the Mac.
  - Windows → **WinRT** (`windows` crate, MSVC) — needs Windows.
  - Linux → **BlueZ over D-Bus** (`dbus`, `bluez-async`) — needs `libdbus-1-dev`.
- **Locally you can cover:** macOS **Intel + Apple Silicon** (both targets already
  installed) and **Linux** via `cross` (Docker is installed). **Windows** is not
  practical to cross-build from a Mac.
- **Recommended path:** adopt **`dist` (formerly `cargo-dist`)**. One config +
  a git tag → GitHub Actions builds the whole matrix, makes checksummed archives,
  a GitHub Release, shell/PowerShell installers, **and a Homebrew tap formula**.
  It solves the Windows problem for free and is reproducible.
- A **fully-local first cut** (macOS + Linux, skip Windows) is also fine for a
  `v0.1.0` — commands below.

---

## 1. Target matrix

| Target triple | Platform | Build where | Notes |
|---|---|---|---|
| `aarch64-apple-darwin` | macOS Apple Silicon | **Local (Mac)** | target installed |
| `x86_64-apple-darwin` | macOS Intel | **Local (Mac)** | target installed |
| *(universal2)* | macOS both | **Local** | `lipo` the two above |
| `x86_64-unknown-linux-gnu` | Linux x64 | **Local via `cross`** or CI | needs `libdbus-1-dev` |
| `aarch64-unknown-linux-gnu` | Linux arm64 | `cross` or CI | needs `libdbus-1-dev:arm64` |
| `x86_64-pc-windows-msvc` | Windows x64 | **CI only** (windows-latest) | WinRT/MSVC |

`rusqlite` is built with the **bundled** SQLite, so every build host needs a C
compiler (all CI images and `cross` images have one — no system SQLite needed).

---

## 2. Option A — build locally (macOS + Linux)

### macOS, both architectures → universal binary
```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin   # already installed
cargo build --release -p trot-daemon --target aarch64-apple-darwin
cargo build --release -p trot-daemon --target x86_64-apple-darwin

# fuse into one universal binary
lipo -create -output trot \
  target/aarch64-apple-darwin/release/trot \
  target/x86_64-apple-darwin/release/trot

tar czf trot-v0.1.0-macos-universal.tar.gz trot
shasum -a 256 trot-v0.1.0-macos-universal.tar.gz
```

### Linux via `cross` (uses Docker — already installed)
```bash
cargo install cross --git https://github.com/cross-rs/cross
```
Add a `Cross.toml` at the repo root so the container has the D-Bus dev lib:
```toml
[target.x86_64-unknown-linux-gnu]
pre-build = ["apt-get update && apt-get install --assume-yes libdbus-1-dev pkg-config"]
```
```bash
cross build --release -p trot-daemon --target x86_64-unknown-linux-gnu
tar czf trot-v0.1.0-linux-x86_64.tar.gz -C target/x86_64-unknown-linux-gnu/release trot
sha256sum trot-v0.1.0-linux-x86_64.tar.gz
```
> arm64 Linux: add `[target.aarch64-unknown-linux-gnu]` with
> `libdbus-1-dev:arm64` (multiarch) in `pre-build`.

### Windows
Not worth cross-compiling from macOS (WinRT + MSVC). Either **skip it for `v0.1.0`**
or build it once on any Windows machine / a free GitHub Actions `windows-latest`
run. This is exactly why Option B is recommended.

---

## 3. Option B — recommended: `dist` + GitHub Actions

[`dist`](https://opensource.axo.dev/cargo-dist/) generates a release pipeline from
one config. On a tag push it builds the full matrix on GitHub's runners (so Windows
"just works"), uploads checksummed archives to a **GitHub Release**, and emits
installers + a **Homebrew formula**.

```bash
cargo install cargo-dist          # provides the `dist` binary
dist init                         # interactive: pick targets + installers
```
Then set, in `Cargo.toml` (`[workspace.metadata.dist]`) or `dist-workspace.toml`:
- **targets:** the six triples above.
- **installers:** `shell` (curl | sh), `powershell`, `homebrew`.
- **Homebrew tap:** `marcuspuchalla/homebrew-trot` (create that repo first).
- **system deps for Linux** (so CI installs D-Bus):
  ```toml
  [workspace.metadata.dist.dependencies.apt]
  libdbus-1-dev = "*"
  pkg-config = "*"
  ```
Commit the generated `.github/workflows/release.yml`, then:
```bash
git tag v0.1.0 && git push --tags
```
CI produces: per-platform `.tar.gz`/`.zip`, `SHA256SUMS`, a GitHub Release, the
`install.sh`/`install.ps1` scripts, and pushes the Homebrew formula to the tap.

---

## 4. The `v0.1.0` release itself

- Both crates are already at `version = "0.1.0"`. Confirm they match and add a
  short `CHANGELOG.md`.
- Tag **`v0.1.0`** (dist keys releases off the tag).
- Manual alternative to dist, once artifacts exist:
  ```bash
  gh release create v0.1.0 ./dist/* \
    --title "trot v0.1.0" --notes-file CHANGELOG.md
  ```

---

## 5. Homebrew

- **Now (easy): your own tap.** Create repo **`marcuspuchalla/homebrew-trot`**.
  `dist` writes/updates the formula there each release. Users then:
  ```bash
  brew install marcuspuchalla/trot/trot
  ```
- **Later: `homebrew-core`** (so plain `brew install trot`). Requires meeting
  Homebrew's notability bar (roughly: a maintained, reasonably popular project) and
  a PR to homebrew-core. Not a day-one thing.
- **Runtime note:** on first run macOS shows a **Bluetooth permission** prompt
  (TCC). Document it in the README so users aren't surprised.

---

## 6. Linux distro packages (later / "perspectively")

- **Debian / Ubuntu (`.deb`):** generate with **`cargo-deb`**
  (`cargo install cargo-deb && cargo deb -p trot-daemon`). Declare a runtime
  dependency on `libdbus-1-3`. Distribute by attaching the `.deb` to the GitHub
  Release, or host a tiny apt repo (Cloudflare Pages / an `apt`-repo action).
  Getting into **official Debian/Ubuntu** archives is a heavy, slow, sponsored
  process — treat as long-term, not near-term.
- **Arch (AUR):** write a `PKGBUILD` and push to the **AUR** (self-service git):
  - `trot` — builds from source (`makedepends: rust, dbus`).
  - `trot-bin` — repackages the GitHub release binary (fast for users).
  Official Arch `[extra]` needs a Trusted User to adopt it; the **AUR is the
  realistic target**.
- **Also worth considering:** Nix (`nixpkgs`), and for Windows `winget` / `scoop`.

---

## 7. Signing & notarization (do before wide distribution)

- **macOS:** unsigned binaries are quarantined by Gatekeeper — users must
  right-click → Open or `xattr -dr com.apple.quarantine trot`. For a smooth
  install you need an **Apple Developer ID** (**$99/yr**) to codesign + notarize;
  `dist` supports macOS signing. Fine to ship `v0.1.0` unsigned with a note; sign
  before promoting it widely (and before homebrew-core).
- **Windows:** unsigned `.exe` triggers SmartScreen. A code-signing certificate is
  optional and can wait.

---

## 8. What we need from you (decisions + accounts)

| Need | For | Cost / effort |
|---|---|---|
| Confirm the **target list** (incl. arm64 Linux? Windows arm64?) | build matrix | decision |
| Create **`marcuspuchalla/homebrew-trot`** repo | Homebrew tap | 2 min |
| GitHub Actions permissions / a PAT if the tap is a separate repo | dist pushing the formula | config |
| **Apple Developer ID** (optional for 0.1, recommended soon) | signed macOS | $99/yr |
| Windows code-signing cert (optional) | quiet SmartScreen | later |
| Short **CHANGELOG.md** | release notes | 10 min |

---

## 9. Recommended order of operations

1. Add `CHANGELOG.md`; confirm crate versions = `0.1.0`.
2. `cargo install cargo-dist`; `dist init`; set targets + installers
   (`shell`, `powershell`, `homebrew`) + the apt deps for Linux.
3. Create the `homebrew-trot` tap repo.
4. Commit the dist config + generated `.github/workflows/release.yml`.
5. `git tag v0.1.0 && git push --tags` → CI builds + publishes the Release + tap.
6. Verify: download each artifact, `brew install marcuspuchalla/trot/trot`, run it.
7. (Later) `cargo-deb` `.deb` + an AUR `PKGBUILD`/`trot-bin`; then consider
   signing/notarization and homebrew-core.
```
