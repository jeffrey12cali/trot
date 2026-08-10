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
| Advertised-name list (19 prefixes: `URTM`, `MRK-T`, `SF-T`, `CITYSPORTS-LINKER`, `WELLFIT TM`, `MOBVOI …`, `SWALK LITE-`, `ANPLUS-`/`ANPIUS-`, `YPOO-MINI PRO-`, `THERUN  T15`, `FOCUS M3`, `KS-…`, `SPERAX_RM-01`) | qdomyos-zwift `bluetooth.cpp` | Literal device identifiers | Yes — routes real hardware to the right driver | GPL-3.0 (upstream); names originate with ~15 manufacturers | Source | The *selection* is the one item in Trot with a colourable compilation argument — see `licensing-analysis.md` §3.5. Selection criterion is purely functional (does the device speak FTMS?) |
| KingSmith family gate (`is_kingsmith_name` list) | qdomyos-zwift + walkingpad-controller captures | Literal device identifiers | Yes — gates the non-SIG bit-13 step extension | GPL-3.0 / MIT | Source | |
| FTMS opcodes, flag bits, field sizes | Bluetooth SIG FTMS v1.0.1 + GATT Supplement | Literal spec values | Yes | Bluetooth SIG published specification | Source | Cross-checked against python-pyftms (Apache-2.0); no code copied |
| Bit-13 step extension layout (uint16-LE + pad byte) | walkingpad-controller (`docs/ftms-protocol-reference.md`, from real KS-MC21 captures) | Derived fact | Yes | MIT | Source | The extension originates with KingSmith firmware |
| **LifeSpan — `lifespan.rs`** | | | | | | |
| A1 opcode map (`0x82`–`0x91`), frame format | The author's own `lifespan_sc110` (relicensed by its sole copyright holder); bootstrapped from and cross-checked against treadspan | Literal opcodes | Yes | Owner's own work under GPL-3.0-or-later; treadspan MIT | Source + tests | Not a third-party dependency — see the module header |
| Name prefixes `LifeSpan`, `ESP32` | Own hardware observation | Literal device identifiers | Yes | — | Source | |
| **Cross-cutting** | | | | | | |
| 16-bit GATT service/characteristic UUIDs (`FFF0`, `FE00`, `FBA0`, `AE00`, `FFE0`, `1826`, `2ACD`, `2ADA`, …) | Device firmware, via all sources | Literal UUIDs | Yes | Facts about the hardware / Bluetooth SIG assignments | Source | |
| Sperax inbound field geometry (steps at 15, speed at `len-7`) | qdomyos-zwift | Derived — behavioural | Yes, for parity with the only hardware-verified reader | GPL-3.0 | Source | Deliberately follows upstream's raw-wire offsets although this module documents the wire as escaped. **Not protocol-mandated** — disclosed in the module header and in licensing-analysis.md §8.2 |
| Three quoted upstream comment/expression fragments | qdomyos-zwift | Literal (prose) | No — quoted for criticism | GPL-3.0 | Source comments | Never executed; each quoted to explain a deliberate divergence |

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
  three disclosed exceptions. Three short fragments are quoted *critically* in
  code comments, each in order to explain why Trot does something different:
  qdomyos-zwift's "the treadmill send the speed in miles always" and its `SW`
  name-matching expression (both `fitshow.rs`), and an "update each 10 m /
  0.01 mile" comment (`kingsmith_props.rs`). All GPL-3.0 into
  GPL-3.0-or-later, de minimis, quoted with attribution, never executed.

> **FTMS name-list ordering check (2026-08-11):** 0/18 positional matches against qdomyos-zwift `bluetooth.cpp`, LCS 7/18. Arrangement independent; membership functionally determined.
