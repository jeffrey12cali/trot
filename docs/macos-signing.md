# Signing & notarizing the macOS binaries

Releases currently ship **unsigned, un-notarized** macOS binaries, so a user who
downloads an archive **in a browser** gets *"trot cannot be opened because the
developer cannot be verified"* and has to run
`xattr -dr com.apple.quarantine ./trot`. (The `curl … | sh` installer is
unaffected — it never sets the quarantine bit — so this only bites browser
downloads.)

This document is the plan to remove that friction entirely.

---

## The one fact that shapes everything

**Codesigning alone does not remove the warning.** Since macOS 10.15, Gatekeeper
requires *notarization* for downloaded software. A signed-but-not-notarized
binary is still blocked; the dialog merely changes to *"Apple cannot check it for
malicious software."*

And **dist does not notarize**. Verified against dist 0.32.0: there is no
`notarytool`, `notarize` or `stapler` anywhere in the binary. It only shells out
to `/usr/bin/codesign`.

So the setup is deliberately two halves:

| Half | Who does it | Where |
| --- | --- | --- |
| Codesign with Developer ID + hardened runtime | dist, during the release build | `macos-sign = true` |
| Notarize the signed binaries | our own workflow, after the release publishes | `.github/workflows/notarize-macos.yml` |

Bare CLI binaries **cannot be stapled** (stapling only works on `.app`/`.dmg`/`.pkg`),
so the notarization ticket lives on Apple's servers and Gatekeeper checks it
online at first launch. That's why the workflow notarizes the binaries **in
place** and does not need to rewrite or re-upload any release archive.

---

## What you need (one time)

1. **Apple Developer Program membership** — $99/year. A *Developer ID Application*
   certificate only exists on a paid account; free accounts get "Apple Development"
   certs, which cannot sign software for distribution outside the App Store.
2. **A Developer ID Application certificate**
   Xcode → Settings → Accounts → your team → **Manage Certificates** → `+` →
   **Developer ID Application**, or create it at
   <https://developer.apple.com/account/resources/certificates>.
   Verify: `security find-identity -v -p codesigning` should list
   `Developer ID Application: Marcus Puchalla (TEAMID)`.
3. **An App Store Connect API key** for notarization
   <https://appstoreconnect.apple.com/access/integrations/api> → **Keys** → `+`.
   Give it the *Developer* role. Download `AuthKey_XXXXXXXXXX.p8` — **Apple lets
   you download it exactly once** — and note the **Key ID** and **Issuer ID**.

---

## Step 1 — add six repository secrets

Settings → Secrets and variables → **Actions** → *New repository secret*.

### For signing (read by dist)

| Secret | How to produce it |
| --- | --- |
| `CODESIGN_CERTIFICATE` | Keychain Access → right-click the *Developer ID Application* cert → **Export** as `.p12`, then `base64 -i cert.p12 \| pbcopy` |
| `CODESIGN_CERTIFICATE_PASSWORD` | the password you set during that export |
| `CODESIGN_IDENTITY` | the full identity string, e.g. `Developer ID Application: Marcus Puchalla (TEAMID)` |

> These three names are exact — I confirmed them by generating the workflow with
> `macos-sign = true` and diffing. Earlier revisions of this document claimed
> `APPLE_CERTIFICATE` / `APPLE_TEAM_ID` / `APPLE_API_KEY`; those are wrong.

### For notarization (read by `notarize-macos.yml`)

| Secret | How to produce it |
| --- | --- |
| `APPLE_API_KEY_P8` | `base64 -i AuthKey_XXXXXXXXXX.p8 \| pbcopy` — base64 so the multi-line private key survives as a single-line secret |
| `APPLE_API_KEY_ID` | the Key ID, e.g. `ABCD123456` |
| `APPLE_API_ISSUER_ID` | the Issuer ID (a UUID) shown above the keys table |

## Step 2 — turn signing on

Only after the secrets exist. In `dist-workspace.toml`:

```toml
[dist]
macos-sign = true
```

Then **`dist generate` and commit the result** — unlike a target change, this one
really does modify `.github/workflows/release.yml` (it injects the three
`CODESIGN_*` env vars), and the release job fails its integrity check if the
generated file is stale.

⚠️ **Do not enable this before the secrets are in place.** dist fails with
*"We failed to decode the certificate stored in the CODESIGN_CERTIFICATE
environment variable"* and takes the whole release build with it.

### Hardened runtime

Notarization requires it. dist reads a `CODESIGN_OPTIONS` environment variable
and passes it to `codesign --options`; set it to `runtime` for the build job. The
notarization workflow **hard-fails** if it finds a binary without the runtime
flag, rather than submitting something Apple will reject.

## Step 3 — cut a release

Tag as usual. The order is: dist builds and signs → the GitHub Release is
published → `notarize-macos.yml` fires on `release: published`, downloads the
`*-apple-darwin.tar.xz` archives, verifies each binary is signed and hardened,
zips it, and submits it to `notarytool --wait`.

If the secrets aren't configured the workflow **skips with a warning** instead of
failing, so it is harmless to have merged ahead of time.

Re-run a notarization by hand with **Actions → Notarize macOS → Run workflow**
and a tag.

## Step 4 — verify, then drop the workaround

On a Mac that has never seen the binary:

```sh
curl -LO https://github.com/marcuspuchalla/trot/releases/latest/download/trot-aarch64-apple-darwin.tar.xz
xattr -w com.apple.quarantine "0081;00000000;Safari;" trot-aarch64-apple-darwin.tar.xz  # simulate a browser download
tar xf trot-aarch64-apple-darwin.tar.xz && ./trot --version
```

It should run with no dialog. `spctl -a -vvv -t install ./trot` should report
*accepted / source=Notarized Developer ID*.

Once that passes, remove the `xattr` note from `README.md` (Install section) and
from the landing page's install block in the `trot-web` repo.

---

## Appendix — signing one binary by hand

```sh
codesign --force --options runtime --timestamp \
  --sign "Developer ID Application: Marcus Puchalla (TEAMID)" ./trot
ditto -c -k --keepParent ./trot trot.zip
xcrun notarytool submit trot.zip --key AuthKey_XXXX.p8 --key-id KEYID --issuer ISSUERID --wait
codesign --verify --strict --verbose=2 ./trot
```

## Appendix — Windows signing

dist also supports SSL.com's cloud signing for Windows via `SSLDOTCOM_USERNAME`,
`SSLDOTCOM_PASSWORD`, `SSLDOTCOM_CREDENTIAL_ID` and `SSLDOTCOM_TOTP_SECRET`.
Windows SmartScreen warnings are a smaller problem than Gatekeeper's hard block,
and an EV code-signing certificate is a separate yearly cost, so this is noted
only for completeness.
