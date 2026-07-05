# Signing & notarizing the macOS binary (manual)

The v0.1 release ships **unsigned** macOS binaries. On a user's Mac that means
Gatekeeper shows *"trot cannot be opened because the developer cannot be
verified."* Workarounds today:

- Installed via `curl … | sh` (the shell installer): usually **no** prompt, because
  the file isn't tagged with the browser quarantine bit.
- Downloaded via a browser: right-click → **Open** once, or run
  `xattr -dr com.apple.quarantine ./trot`.

You have a paid **Apple Developer** account, so you can sign + notarize and remove
that prompt entirely. Two ways: do it by hand for a one-off, or wire it into the
`dist` CI so every release is signed automatically.

---

## What you need (one time)

1. **Developer ID Application certificate** (this is *not* the Mac App Store cert):
   - Xcode → Settings → Accounts → your team → **Manage Certificates** → `+` →
     **Developer ID Application**. Or create it at
     <https://developer.apple.com/account/resources/certificates>.
   - Confirm it's installed: `security find-identity -v -p codesigning`
     → you want a line like `Developer ID Application: Marcus Puchalla (TEAMID)`.
2. Your **Team ID** (10 chars) — <https://developer.apple.com/account> → Membership.
3. An **App Store Connect API key** for notarization (preferred over app-specific
   passwords): <https://appstoreconnect.apple.com/access/integrations/api> →
   **Keys** → `+`. Download the `AuthKey_XXXXXXXXXX.p8` (once only), and note the
   **Key ID** and **Issuer ID**.

---

## A. Manual, for a single binary

```sh
# 1. Sign (hardened runtime is required for notarization)
codesign --force --options runtime --timestamp \
  --sign "Developer ID Application: Marcus Puchalla (TEAMID)" \
  ./trot

# 2. Zip it (notarization takes an archive, not a bare binary)
ditto -c -k --keepParent ./trot trot.zip

# 3. Submit and wait for the ticket
xcrun notarytool submit trot.zip \
  --key   /path/to/AuthKey_XXXXXXXXXX.p8 \
  --key-id KEYID \
  --issuer ISSUERID \
  --wait

# 4. (CLI binaries can't be "stapled" — stapling only works on .app/.dmg/.pkg.
#    A notarized bare binary still passes Gatekeeper because the ticket is
#    published by Apple and checked online. If you ship a .dmg/.pkg later,
#    staple it: xcrun stapler staple trot.dmg)

# Verify
codesign --verify --strict --verbose=2 ./trot
spctl -a -vvv -t install ./trot   # for a .dmg/.pkg
```

---

## B. Automatic, in the `dist` release CI

`dist` supports macOS signing via GitHub Actions secrets — no workflow edits, it
reads them if present. Add these repo secrets (Settings → Secrets and variables →
Actions):

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | base64 of your Developer ID Application cert exported as `.p12` — `base64 -i cert.p12 | pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the password you set when exporting the `.p12` |
| `APPLE_TEAM_ID` | your 10-char Team ID |
| `APPLE_API_KEY` / `APPLE_API_ISSUER` / the `.p8` | App Store Connect API key for notarytool |

Then enable it in `dist-workspace.toml`:

```toml
[dist]
macos-sign = true
```

Re-run `dist generate`, commit, and the next `vX.Y.Z` tag produces signed +
notarized macOS archives. Check exact secret names against the dist version you
run (`dist` config reference → *macOS signing*), since they've been refined
across releases.

> Export the `.p12` from **Keychain Access** → right-click the *Developer ID
> Application* cert → **Export**. Keep the `.p8`, its Key ID, and Issuer ID
> somewhere safe — Apple only lets you download the `.p8` once.
