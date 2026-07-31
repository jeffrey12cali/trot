# Signing & notarizing the macOS binary (manual)

Releases ship **unsigned, un-notarized** macOS binaries. On a user's Mac that means
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

## B. Automatic (codesigning only), in the `dist` release CI

**Two corrections to what this doc used to say**, both verified against the
`dist` **0.32.0** source and its generated workflow template:

1. The secret names are **not** `APPLE_CERTIFICATE` / `APPLE_TEAM_ID` /
   `APPLE_API_KEY`. The generated `release.yml` injects exactly three:
   `CODESIGN_CERTIFICATE`, `CODESIGN_CERTIFICATE_PASSWORD`, `CODESIGN_IDENTITY`.
2. **`dist` does not notarize.** There is no `notarytool` invocation anywhere in
   0.32.0. It codesigns only, so notarization stays the manual step in section A.
   Codesigning alone still leaves the "unidentified developer" prompt for
   browser-downloaded files; only notarization removes it.

So set these repo secrets (Settings → Secrets and variables → Actions):

| Secret | Value |
| --- | --- |
| `CODESIGN_CERTIFICATE` | base64 of your Developer ID Application cert exported as `.p12` — `base64 -i cert.p12 \| pbcopy` |
| `CODESIGN_CERTIFICATE_PASSWORD` | the password you set when exporting the `.p12` |
| `CODESIGN_IDENTITY` | the identity string, e.g. `Developer ID Application: Marcus Puchalla (TEAMID)` |

Then enable it in `dist-workspace.toml`:

```toml
[dist]
macos-sign = true
```

Re-run `dist generate` and commit — unlike a plain target change, **this one does
alter the generated workflow**, so the diff must be committed or the release job
fails its integrity check.

Until that's set up, releases ship unsigned and the README tells users to run
`xattr -dr com.apple.quarantine ./trot` for browser downloads.

> Export the `.p12` from **Keychain Access** → right-click the *Developer ID
> Application* cert → **Export**. Keep the `.p8`, its Key ID, and Issuer ID
> somewhere safe — Apple only lets you download the `.p8` once.
