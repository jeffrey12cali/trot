# Provenance of third-party-derived material in Trot

This file records, item by item, every **literal third-party-derived item** in
Trot's source and tests: cipher tables, device-name lists, command frames,
packet captures and test vectors. It is the item-level companion to
[`THIRD-PARTY-NOTICES.md`](../THIRD-PARTY-NOTICES.md) (per-project credits and
licence texts) and [`docs/licensing-analysis.md`](licensing-analysis.md) (the
project's internal analysis). If you add such an item, add a row here — see
CONTRIBUTING.md § Third-party code.

Four kinds of item are distinguished, because they are not the same thing:

- **frame** — an individual protocol frame (a command or a device response).
  A frame Trot *sends* is dictated by the device firmware: there is exactly
  one byte sequence that produces the behaviour. A frame a device *emitted*
  is a recording of machine output.
- **corpus** — a complete capture corpus published upstream. Trot vendors no
  corpus; where one was used, Trot reproduces *selected individual frames*
  from it and re-derived the semantics itself.
- **annotation** — upstream's prose commentary attached to captures or code.
  Annotations are upstream expression and are **not reproduced** in Trot;
  where Trot's conclusions differ from upstream's annotations, the module
  headers say so explicitly.
- **synthetic** — a fixture Trot generated itself. Synthetic fixtures are not
  third-party material; they are listed where their presence replaces a
  third-party vector, so the replacement is on record.

A recurring pattern worth stating once: many items below **originate with the
device manufacturer, not with the upstream project that published them**. The
KingSmith cipher tables are KingSmith's, extracted from KingSmith's app;
advertised names are chosen by manufacturers and broadcast by their firmware;
captured frames are what a machine emitted. An upstream open-source licence
cannot grant rights in material its author did not create — which is why the
primary basis for reproducing these items is their functional, interface
nature, with the upstream licences as an additional basis for
upstream-authored expression only.

## The matrix

| Item | Source | Literal or derived | Functionally necessary? | Licence | Appears in source/tests | Notes |
|---|---|---|---|---|---|---|
| **KingSmith app-cipher — `kingsmith_props.rs`** | | | | | | |
| Seven substitution-cipher tables v1–v7 (`CIPHER_TABLES`) | qdomyos-zwift `kingsmithr2treadmill.h` | Literal lookup tables | Yes — traffic cannot be decoded without them | GPL-3.0 (upstream); tables originate with **KingSmith** (extracted from KingSmith's app) | Source | qdomyos could not license these even if they were copyrightable; the basis for carrying them is their functional nature. Trot's table *detection* is an independent design with no upstream counterpart |
| Base64 alphabet + `=` (`PLAINTEXT_TABLE`) | RFC 4648 standard order | Literal | Yes | Public standard | Source | Not upstream-authored |
| Advertised-name list (10 `KS-…` prefixes) | qdomyos-zwift `bluetooth.cpp` | Literal device identifiers | Yes — the name gate is the device adjudication | GPL-3.0 (upstream); names originate with KingSmith/Xiaomi firmware | Source | |
| Init/poll messages (`""`, `shake`, `net`, `get_dn`, `get_pk`, `version`, `servers getProp 1 2 7 12 23 24 31`) | qdomyos-zwift + walkingpad-ble-footpod | Literal frames | Yes — the device answers only these | GPL-3.0 (both) | Source + tests | Protocol-mandated commands; `time_posix` and all `props …` setters deliberately not implemented |
| GATT address spaces (`0x1234`/`FED7`/`FED8` + two 128-bit variants) | qdomyos-zwift | Literal UUIDs | Yes | GPL-3.0 (upstream); UUIDs originate with KingSmith firmware | Source | |
| All test fixtures in `kingsmith_props.rs` | Trot | **Synthetic** | — | — | Tests | No public capture of this protocol exists; every fixture is generated from the two licensed sources' shared facts and labelled synthetic |
| **KingSmith WiLink — `kingsmith_wilink.rs`** | | | | | | |
| Status query `F7 A2 00 00 A2 FD` | ph4-walkingpad (`ask_stats`), qdomyos-zwift, QWalkingPad | Literal frame (sent) | Yes — the poll the protocol answers | MIT / GPL-3.0 / GPL-3.0-or-later | Source + tests | Protocol-mandated |
| Params query `F7 A6 00 00 00 00 00 A6 FD` | qdomyos-zwift (`initData3`), QWalkingPad (`queryParams`) | Literal frame (sent) | Yes | GPL-3.0 / GPL-3.0-or-later | Source + tests | One of two init frames provably queries; the `F7 A5 61 …` frames are deliberately absent from this tree |
| Speed frame `F7 A2 01 19 BC FD` (2.5 km/h example) | qdomyos-zwift (`forceSpeedOrIncline`) | Literal frame (checksum vector only) | Test value — pins the additive checksum on a third shape | GPL-3.0 | Tests only | **Never sent**; the write-set test proves it |
| Captured status frames used as fixtures | ph4-walkingpad README captures | Literal frames (device output) | Yes — the only real vectors for the 20-byte frame | MIT | Tests | Individual frames, not a corpus; machine output originates with the device |
| Advertised-name list (`WALKINGPAD`, `KINGSMITH`, `R1 PRO`, `KS-…`; `RE` exact; `KS-HD-Z1D` carve-out) | qdomyos-zwift `bluetooth.cpp` | Literal device identifiers | Yes | GPL-3.0 (upstream); names originate with manufacturers | Source | |
| **Urevo — `urevo.rs`** | | | | | | |
| Wake frame `02 51 0B 03` | treadspan (`TreadmillDeviceUrevoProtocol.h`, vendor-app capture) | Literal frame (sent) | Yes — the pad is silent without it | MIT | Source + tests | Protocol-mandated; the only frame this driver writes |
| E1L fixture frames (e.g. `RUNNING_FIXTURE`) | treadspan `protocol-analysis/urevo-E1L/` (568-frame corpus) | Literal frames selected from a **corpus** (device output) | Yes — the only published real captures | MIT | Tests | The corpus is not vendored; treadspan's **annotations are not reproduced** — Trot re-derived the semantics and twice contradicts them (distance is 0.01 mi not "0.1 miles"; the checksum rule is Trot's own derivation, treadspan validates nothing inbound) |
| Advertised name `URTM041` | Device broadcast, verified in treadspan's capture | Literal device identifier | Yes | MIT (capture); name originates with Urevo firmware | Source | `URTM024` and the rest of the range deliberately excluded (plain FTMS) |
| **Sperax — `sperax.rs`** | | | | | | |
| Hello frame `F5 07 00 01 26 D8 FA` | qdomyos-zwift `speraxtreadmill.cpp` (captured frame) | Literal frame (sent, twice, as upstream does) | Yes | GPL-3.0 | Source + tests | Protocol-mandated |
| Status query `F5 08 00 19 F0 0A 59 FA` | qdomyos-zwift (captured frame) | Literal frame (sent) | Yes — the frame data packets answer | GPL-3.0 | Source + tests | |
| Cmd 0x13 frame `F5 09 00 13 01 00 89 B8 FA` | qdomyos-zwift (captured frame) | Literal frame (CRC vector only) | Test value — pins CRC-16 on a payload-carrying shape | GPL-3.0 | Tests only | **Never sent** — its payload could be a setting; the write-set test proves it |
| Name prefixes `SPERAX_RM01`, `SPERAX_RM-02` (and `SPERAX_RM-01` in the FTMS list) | qdomyos-zwift `bluetooth.cpp` | Literal device identifiers | Yes — a hyphen is the whole revision split | GPL-3.0 (upstream); names originate with Sperax firmware | Source | The CRC-16 parameters and `F0`-escape rule are **Trot's own derivation** from the 64 captured frames embedded in upstream source (verified against all 64; corpus not vendored) |
| **PitPat / Deerrun / SupeRun — `pitpat.rs`** | | | | | | |
| Status query `6A 05 FD F8 43` | qdomyos-zwift, PitPat-WebBT, pitpat-treadmill-control | Literal frame (sent) | Yes — the poll/heartbeat all sources agree on | GPL-3.0 / Unlicense / MIT | Source + tests | Protocol-mandated |
| Transport envelope `4D 00 <seq> <len>` | pitpat-treadmill-control (`bluetooth_manager.py`) | Literal constant bytes | Yes — required on the enveloped transports | MIT | Source + tests | |
| Unlock preamble `6B 05 9D 98 43` | qdomyos-zwift `deerruntreadmill.cpp` | Literal frame (checksum vector only) | Test value | GPL-3.0 | Tests only | **Never sent** — exists to enable actuation, which Trot bans |
| Uncharacterised init `6A 05 D7 D2 43` | qdomyos-zwift | Literal frame (checksum vector only) | Test value | GPL-3.0 | Tests only | **Never sent** |
| Two 23-byte actuation frames (upstream's stop and start) | qdomyos-zwift | Literal frames (checksum vectors only) | Test value — pin the XOR rule across the 23-byte family | GPL-3.0 | Tests only | **Never built into traffic**; the write-set test proves it |
| 52-byte idle frame (`IDLE_CAPTURE`) | pitpat-treadmill-control (`treadmill_data.py` example payload) | Literal frame (device output) | Yes — the only published real capture of this protocol; the inbound-checksum decision rests on it | MIT | Tests | Machine output originates with the device |
| Name prefix `PITPAT-T` | qdomyos-zwift matcher | Literal device identifier | Yes | GPL-3.0 (upstream); name originates with the OEM | Source | `PITPAT-S*` (the bike) deliberately excluded |
| **FitShow — `fitshow.rs`** | | | | | | |
| Status query `02 51 51 03` | qdomyos-zwift; verified on hardware by milltender | Literal frame (sent) | Yes — the only frame this driver writes | GPL-3.0 / AGPL-3.0 (as sources of the *finding*; the frame is dictated by the device) | Source + tests | Protocol-mandated: one byte of command, checksum forced by the rule |
| Advertised-name matcher (`FS-`, `TR510-T`, `TUNTURI T80-`; `NOBLEPRO CONNECT`, `WINFITA`, `SW-BLE`, `BF70`; `FS-YK-` exclude; the 14-char `SW` rule) | qdomyos-zwift `bluetooth.cpp` | Literal device identifiers + a matching rule | Yes — the name gate is the whole adjudication on the contested `FFF0` block | GPL-3.0 (upstream); names originate with manufacturers | Source | |
| Status-code table (`0x00`–`0x0A` constants) | qdomyos-zwift; confirmed against the OEM spec | Literal opcodes | Yes | GPL-3.0 (upstream); codes originate with FitShow firmware | Source | |
| Checksum/framing vectors (5 frames incl. two control-family shapes) | Trot (2026-08-10) | **Synthetic** — trailers hand-computed | — | — | Tests | Replaced former third-party vectors: two OEM-spec worked examples (unlicensed document), milltender's stop command (AGPL-3.0) and qdomyos' user-data login (GPL-3.0). None of those four frames appears anywhere in this tree any more |
| Status fixtures (`build_status_frame` + hand-computed pin) | Trot | **Synthetic** | — | — | Tests | No public real capture of an inbound FitShow status frame exists; fixtures are built to the field map the three sources agree on, and labelled synthetic |
| **FTMS — `ftms.rs`** | | | | | | |
| Advertised-name list (18 prefixes: `URTM`, `MRK-T`, `SF-T`, `CITYSPORTS-LINKER`, `WELLFIT TM`, `MOBVOI …`, `SWALK LITE-`, `ANPLUS-`/`ANPIUS-`, `YPOO-MINI PRO-`, `THERUN  T15`, `FOCUS M3`, `KS-…`, `SPERAX_RM-01`) | qdomyos-zwift `bluetooth.cpp` | Literal device identifiers | Yes — routes real hardware to the right driver | GPL-3.0 (upstream); names originate with ~15 manufacturers | Source | The *selection* is the one item in Trot with a colourable compilation argument — see `licensing-analysis.md` §3.5. Selection criterion is purely functional (does the device speak FTMS?) |
| KingSmith family gate (`is_kingsmith_name` list) | qdomyos-zwift + walkingpad-controller captures | Literal device identifiers | Yes — gates the non-SIG bit-13 step extension | GPL-3.0 / MIT | Source | |
| FTMS opcodes, flag bits, field sizes | Bluetooth SIG FTMS v1.0.1 + GATT Supplement | Literal spec values | Yes | Bluetooth SIG published specification | Source | Cross-checked against python-pyftms (Apache-2.0); no code copied |
| Bit-13 step extension layout (uint16-LE + pad byte) | walkingpad-controller (`docs/ftms-protocol-reference.md`, from real KS-MC21 captures) | Derived fact | Yes | MIT | Source | The extension originates with KingSmith firmware |
| **LifeSpan — `lifespan.rs`** | | | | | | |
| A1 opcode map (`0x82`–`0x91`), frame format | The author's own `lifespan_sc110` (relicensed by its sole copyright holder); bootstrapped from and cross-checked against treadspan | Literal opcodes | Yes | Owner's own work under GPL-3.0-or-later; treadspan MIT | Source + tests | Not a third-party dependency — see the module header |
| Name prefixes `LifeSpan`, `ESP32` | Own hardware observation | Literal device identifiers | Yes | — | Source | |
| **Cross-cutting** | | | | | | |
| 16-bit GATT service/characteristic UUIDs (`FFF0`, `FE00`, `FBA0`, `AE00`, `FFE0`, `1826`, `2ACD`, `2ADA`, …) | Device firmware, via all sources | Literal UUIDs | Yes | Facts about the hardware / Bluetooth SIG assignments | Source | |
| Sperax inbound field geometry (steps at 15, speed at `len-7`) | qdomyos-zwift | Derived — behavioural | Yes, for parity with the only hardware-verified reader | GPL-3.0 | Source | Deliberately follows upstream's raw-wire offsets although this module documents the wire as escaped. **Not protocol-mandated** — disclosed in the module header and in licensing-analysis.md §8.2 |
| Six quoted upstream comment/expression fragments | qdomyos-zwift (4) · ph4-walkingpad (1) · QWalkingPad (1) | Literal (prose) | No — quoted for criticism | GPL-3.0 | Source comments | Never executed; each quoted to explain a deliberate divergence |
| `1910` / `2B10` / `2B11` transport triple, with roles | pacekeeper `src/platform.h` GATT-dump comment | Literal UUIDs + role assignment read off the dump's `[read,notify]` / `[read,write…]` annotations | Speculative — no implementation drives the protocol over it | GPL-3.0 (dump); UUIDs originate with the device firmware | Source | Probed last as unverified. qdomyos knows `1910` only as a fallback *unlock* service and never mentions `2B10` |

## What is deliberately absent

For completeness, upstream material that is **not** in this tree, by policy:

- Every upstream belt-control path (speed/start/stop/incline frames, Control
  Point writes, unlock preambles beyond the checksum vectors listed above).
- qdomyos-zwift's WiLink `F7 A5 61 …` init frames, its FitShow login and
  clock-carrying queries, its Sperax cmd 0x13 as *traffic* (vector only), the
  KingSmith `time_posix` clock write.
- The FitShow OEM protocol document (unlicensed): not vendored, no text or
  frame from it reproduced (its two worked examples were removed from the
  tests on 2026-08-10 and replaced with synthetic vectors).
- Anything from the unlicensed Kotlin KingSmith client and from
  `duhow/ftms-bridge` (never consulted / verified unused).
- Upstream implementation structure, throughout — and upstream prose, with
  three disclosed exceptions. Six short fragments are quoted *critically* in
  code comments, each in order to explain why Trot does something different:
  qdomyos-zwift's "the treadmill send the speed in miles always" and its `SW`
  name-matching expression (both `fitshow.rs`), an "update each 10 m /
  0.01 mile" comment (`kingsmith_props.rs`), and its speed-offset expression
  `17 + (len - 24)` (`sperax.rs`, quoted to explain why our constant is 7);
  ph4-walkingpad's `fix_crc` body `cmd[-2] = sum(cmd[1:-2]) % 256` (MIT,
  `kingsmith_wilink.rs`); and QWalkingPad's `padRunning = s != 0 && s != 5`
  (GPL-3.0, `kingsmith_wilink.rs`, quoted because we adopt the predicate whole). All GPL-3.0 into
  GPL-3.0-or-later, de minimis, quoted with attribution, never executed.

> **FTMS name-list ordering check (2026-08-11):** 0/18 positional matches against qdomyos-zwift `bluetooth.cpp`, LCS 7/18. Arrangement independent; membership functionally determined.

## Review log

- **2026-08-11 — FTMS advertised-name list, ordering.** Diffed against upstream
  `bluetooth.cpp` (4813 lines): 0/18 positional matches, longest common
  subsequence 7/18. Membership identical (it is the set of devices known to
  speak FTMS, and the names originate with their manufacturers); arrangement
  independent. Upstream's order falls out of an if-else dispatch chain; ours
  groups by vendor with a per-entry rationale.
- **2026-08-13 — non-literal similarity review** of `sperax.rs`, `fitshow.rs`,
  `kingsmith_props.rs` and `urevo.rs` against the upstream sources, function by
  function. Three of four clean. In each, the largest block of the module has no
  upstream counterpart at all: Sperax's CRC-16 and escape handling, KingSmith's
  cipher-table detection, Urevo's checksum derivation and 15-byte counter floor.
  The disclosed Sperax offset carryover was measured and is exactly two integers
  plus a length gate — the surrounding parser shares no structure with upstream.
  One finding in `fitshow.rs`: a test-only fixture builder followed milltender's
  `tests/conftest.py` layout (AGPL-3.0) in its payload width, its choice of
  which fields to parameterise, and its parameter order. Rewritten the same day
  to build from an offset table at a parameterised length, with the previously
  hard-zeroed fields parameterised; the layout is now attributed to
  qdomyos-zwift's field map and the OEM spec's little-endian rule, which is
  where it actually comes from.
- **Not yet reviewed:** `pitpat.rs` (four upstreams — the obvious next
  candidate), `kingsmith_wilink.rs`, `ftms.rs`, `lifespan.rs`, `util.rs`, and
  `walkingpad-ble-footpod` as a second KingSmith source.
- **2026-08-15 — `pitpat.rs`**, against all five upstreams (pacekeeper GPL-3.0,
  azmke MIT, KeiranY Unlicense, sirfergy GPL-3.0, qdomyos `deeruntreadmill`
  GPL-3.0). **Clean with two exceptions, both prose, both remediated the same
  day**: a passage on the imperial flag that preserved the clause order and most
  of the wording of sirfergy's `protocol.py` comment (rewritten from our own
  observation of all four decoders' behaviour), and a minimum-length doc comment
  that borrowed sirfergy's unusual "…before we trust it" idiom (rewritten to
  cite each decoder's actual bound). No code-expression carryover in any of the
  five pairings.

  Affirmative evidence: our read order is ascending where pacekeeper's is
  non-monotonic (a quirk KeiranY inherited); our state map differs from all four
  in structure and order and adds an `Other(u8)` arm none of them has; we did
  **not** inherit the `COUNTDOWN=0 … DISCONNECTED=100` enum that sirfergy copies
  verbatim from pacekeeper, though two of our five sources carry it; and the
  module's largest blocks — `select_transport`, `decode_notification`'s
  dual-interpretation arbitration, the error taxonomy, and 14 of 21 tests — have
  no upstream counterpart at all.

  Two things checked because they looked alarming and turned out not to be. The
  transport probe order matches qdomyos 3/3 (FBA0 → FFFF → FFF0), but is
  independently determined by our stated verification-strength rationale, arises
  in qdomyos from an unrelated cause (its driver began as FFF0 and had the others
  bolted on later), and selects on characteristic **roles** where qdomyos selects
  on **services** — which is what makes the swapped-FFF0 adjudication possible at
  all. And sirfergy's test-suite frame builder assigns the same fields at the
  same offsets as ours; rejected as a finding because the offsets are facts, the
  field *set* is determined by our own parser rather than theirs, and ours
  computes a checksum trailer sirfergy's cannot (its parser validates none),
  which is what lets our builder be a true round-trip inverse.

- **Methodology note.** Check upstream repositories for test directories via the
  GitHub tree API, not by inspecting whatever files happen to have been fetched.
  Two rounds, two test suites found that way (milltender, sirfergy), neither
  visible in the vendored file set, and one of them held the only real finding of
  its round.

- **2026-08-15 (second pass) — `kingsmith_wilink.rs`, `ftms.rs`, `lifespan.rs`,
  `util.rs`**, plus `kingsmith_props.rs` against `walkingpad-ble-footpod`.
  **Zero category-4 findings.** Affirmative evidence: the WiLink init drops
  three of qdomyos's five frames and its spacing formula differs from ph4's
  (ph4 sleeps the *elapsed* time and therefore under-waits); our FTMS parser
  decodes 18 fields where qdomyos's generic driver skips seven with `// TODO`,
  and that driver has **no** 0x2ADA opcode map at all, so our eight-variant
  `MachineStatus` cannot derive from it; every LifeSpan decoder — including the
  non-obvious `b2*100 + b3` speed rule — is a port of the owner's own
  `lifespan_sc110/parser.py`, and *contradicts* treadspan's `*256` reading;
  `util.rs` has no upstream counterpart and its `FrameAssembler` corrects a real
  defect in footpod's last-byte flush; footpod's distinctive `skip_keys`
  off-by-one resync hack was not taken.

  **Name-list arrangements, measured against `bluetooth.cpp`** (the check the
  first pass could not perform): WiLink **0/9** positional, LCS 5/9 —
  independent. **props was 10/10 — upstream's order exactly.** Membership there
  is functionally determined (which models speak the app-cipher protocol) and
  the names are KingSmith's, but a 10/10 arrangement match is the one thing an
  EU compilation argument could bite on, so the list was **regrouped by product
  generation** — the property that actually determines which cipher table a unit
  wants, and therefore what a reader needs — and now scores **0/10**.

  Test-suite probe found nothing: ph4's suite is `def test_main(): assert True`,
  pyftms's is two parametrize tables over a serializer DSL Trot does not have.

**The review is complete.** Every driver and the shared plumbing has been
compared function by function against every upstream that exists, in four
rounds. Two exceptions remain on record: `kingsmithr2treadmill.{cpp,h}` was
never fetched, so `kingsmith_props.rs`'s attributions to qdomyos for the
`Error`-terminates rule and the 300 ms init gap rest on the module header rather
than on a reading (footpod independently corroborates the `Error` rule); and the
FTMS spec itself is not in the file set, so spec-mandate claims rest on internal
consistency plus pyftms's independent agreement.
