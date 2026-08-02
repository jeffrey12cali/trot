# Security policy

## Reporting a vulnerability

**Please don't open a public issue for a security problem.**

Use GitHub's private reporting:
[**Report a vulnerability**](https://github.com/marcuspuchalla/trot/security/advisories/new).
It's visible only to the maintainer, and it lets us prepare a fix and an advisory
before anything is public.

Trot is a one-person project, so please be realistic about response times: I aim
to acknowledge within a few days and to have a fix or a clear explanation within
a couple of weeks for anything genuinely exploitable.

If you'd rather not use GitHub, email the address listed on
[@marcuspuchalla](https://github.com/marcuspuchalla).

## Supported versions

Only the **latest release** is supported. Trot is pre-1.0 and moves quickly;
fixes go into the next release rather than being backported.

## What Trot's security model actually is

Worth reading before reporting, because some things that look like findings are
deliberate.

Trot runs a local HTTP + WebSocket API bound to `127.0.0.1` on an ephemeral port,
and writes a `runtime.json` handshake containing that port and a per-launch
token.

**Enforced:**

- The data directory is created `0700` and `runtime.json` `0600`, so other users
  on a shared machine can't read your data or drive the API.
- Every state-changing call requires the per-launch token (`x-sc110-token`).
- Requests must carry a loopback `Host` header, which defeats DNS rebinding.
- Any browser `Origin` must be on a small allow-list. This covers the `/ws`
  upgrade, which CORS does not.
- Responses carry `X-Content-Type-Options: nosniff`.

**Deliberately not enforced — please don't report these as vulnerabilities:**

- **Read-only endpoints don't require the token.** A process running as your user
  could equally well open the SQLite file directly, so a token there would be
  theatre. Your user account is the trust boundary.
- **`GET /api/scan` triggers a Bluetooth scan without a token.** Same reasoning;
  it's on the list to become a POST in a future contract-versioned change.
- **No rate limiting.** It's a loopback service for one user.

**In scope and worth reporting:**

- Anything that lets a *remote* origin or a *different local user* read data or
  drive the API — a bypass of the Host or Origin guard, or of the file
  permissions.
- Token leakage beyond the 0600 handshake file.
- Memory-safety issues, or a crash reachable from device input (a malicious or
  malfunctioning treadmill sending crafted BLE frames is a legitimate threat).
- Anything in the release pipeline that would let a third party ship a binary
  under Trot's name.

## Dependencies

CI runs `cargo audit` against the RustSec advisory database on every push, and
the release pipeline is gated on the test suite. If you spot an advisory we've
missed, an ordinary issue is fine — no need for private disclosure.
