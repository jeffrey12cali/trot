# AUR packaging

Two packages:

- **`trot`** — source package, builds from the release tarball with `cargo`
  (the conventional AUR package for a Rust CLI).
- **`trot-bin`** — installs the prebuilt Linux x86_64 binary from the GitHub
  release (fast, no Rust toolchain). Mutually exclusive with `trot`.

## Before publishing (each release)

```sh
cd packaging/aur/trot        # or trot-bin
# bump pkgver / reset pkgrel=1 in PKGBUILD if needed
updpkgsums                   # fill sha256sums from the real sources
makepkg -f                   # test the build locally
namcap PKGBUILD              # optional lint
makepkg --printsrcinfo > .SRCINFO   # REQUIRED for AUR
```

## Publishing to AUR (manual — needs your AUR account + SSH key)

1. Add your SSH public key at <https://aur.archlinux.org> → My Account.
2. Clone the package repo (name = pkgname; empty for a new package):
   ```sh
   git clone ssh://aur@aur.archlinux.org/trot.git        # or trot-bin.git
   ```
3. Copy `PKGBUILD` + `.SRCINFO` in, commit, push:
   ```sh
   cp PKGBUILD .SRCINFO /path/to/clone/ && cd /path/to/clone
   git add PKGBUILD .SRCINFO
   git commit -m "trot 0.1.0"
   git push
   ```

On each new release: bump `pkgver`, `updpkgsums`, regenerate `.SRCINFO`, commit, push.

> Note: `dist` currently builds Linux **x86_64 only**. Add `aarch64` to
> `dist-workspace.toml` targets first if you want an ARM Arch package.
