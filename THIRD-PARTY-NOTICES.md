# Third-party notices

Trot is licensed under **GPL-3.0-or-later**.

**Trot is an independent Rust implementation of the cited treadmill
protocols. It reproduces functional protocol data where necessary for
interoperability and testing — UUIDs, opcodes, device identifiers, cipher and
lookup tables, command bytes, and selected packet captures or test vectors.
The provenance of those items is recorded below, and item by item in
[`docs/provenance.md`](docs/provenance.md). No upstream implementation
routines or prose are intended to be incorporated. To the extent any
reproduced material is copyrightable, Trot relies on and complies with the
applicable licence; otherwise these notices are retained as attribution and
provenance.**

The notices below reproduce copyright and licence text as a matter of
**credit to the people whose reverse engineering made this possible**, and to
record precisely what was learned from whom. Several go beyond what the
licences require. To the extent Trot reproduces copyrightable material, the
applicable licence terms are followed; if a provenance review identifies
protected material, its exact licence obligations will be applied source by
source. See `docs/licensing-analysis.md` for the project's internal analysis.

## TreadSpan — `blak3r/treadspan`

Trot's LifeSpan / Omni protocol support (the "A1" command set over GATT
`FFF0`/`FFF1`/`FFF2`) was **bootstrapped from and cross-checked against
[TreadSpan](https://github.com/blak3r/treadspan)** by Blake Robertson, an
open-source project that reverse-engineered the LifeSpan Omni console protocol.
TreadSpan's documented opcode map and field encodings informed Trot's
independent reimplementation in Rust.

Trot's Urevo (E1L) driver additionally builds on TreadSpan: it independently
implements the proprietary status-stream protocol on the same
`FFF0`/`FFF1`/`FFF2` block using the wake write and status-frame field map
documented by `arduino/src/TreadmillDeviceUrevoProtocol.h`, and it reproduces
frames from TreadSpan's annotated raw captures of a real E1L
(`protocol-analysis/urevo-E1L/`) as the fixture frames in Trot's tests,
against which every field was re-verified (Trot's checksum rule and
0.01-mile distance unit are derived from those captures). TreadSpan's Sperax
RM-01 service dump and app capture (`protocol-analysis/sperax-rm-01/`) also
serve as the cross-check that the hyphenated RM-01 speaks FTMS rather than
the proprietary Sperax protocol.

TreadSpan is distributed under the MIT License:

```
MIT License

Copyright (c) 2025 Blake Robertson

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## ph4-walkingpad — `ph4r05/ph4-walkingpad`

Trot's KingSmith WiLink support (the legacy WalkingPad protocol over GATT
`FE00`/`FE01`/`FE02`) was **bootstrapped from and cross-checked against
[ph4-walkingpad](https://github.com/ph4r05/ph4-walkingpad)** by Dusan Klinec,
the canonical open-source reverse engineering of the WalkingPad protocol. Its
documented status-frame layout, additive checksum and measured 690 ms minimum
command spacing informed Trot's independent reimplementation in Rust, and the
captured frames published in its README are used as test fixtures.
ph4-walkingpad is distributed under the MIT License:

```
MIT License

Copyright (c) 2017, CRoCS, Dusan Klinec (ph4r05)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## QZ (qdomyos-zwift) — `cagnulein/qdomyos-zwift`

Trot's KingSmith WiLink driver **independently implements protocol behaviour
using interface facts documented by
[QZ (qdomyos-zwift)](https://github.com/cagnulein/qdomyos-zwift)** by Roberto
Viola: the five-frame init handshake including its two `F7 A5 61 …` magic
variants and their model-name routing
(`src/devices/kingsmithr1protreadmill/kingsmithr1protreadmill.cpp`), the
20-byte status-frame length requirement, and the advertised-name device list
with its `KS-HD-Z1D` FTMS carve-out (`src/devices/bluetooth.cpp`).

Trot's FTMS driver additionally reproduces, from qdomyos-zwift, the verified
advertised-name list of real-world FTMS walking pads (Urevo, Merach, Sunny
Health & Fitness, CitySports, WellFit, Mobvoi, Sportstech, YPOO, TheRun,
Anplus, Focus, KingSmith, Sperax — `src/devices/bluetooth.cpp`, including the
`SPERAX_RM-01`-is-FTMS / `SPERAX_RM01`-is-proprietary carve-out).

Trot's Sperax driver **independently implements protocol behaviour using
interface facts learned from and cross-checked against qdomyos-zwift's**
`src/devices/speraxtreadmill/speraxtreadmill.cpp`, the only known
implementation of the proprietary `F5 … FA` protocol: the init and poll
frames (sent byte-identically), the ≥24-byte packet length requirement, the
big-endian step-count offset and the end-anchored speed offset, plus the
`SPERAX_RM01`/`SPERAX_RM-02` advertised-name routing
(`src/devices/bluetooth.cpp`). The frame envelope's CRC-16 parameters and
`F0`-escape rule were derived by Trot from the captured frames embedded in
that source and verify against all 64 of them.

Trot's FitShow driver **independently implements protocol behaviour using
interface facts learned from and cross-checked against qdomyos-zwift's**
`src/devices/fitshowtreadmill/fitshowtreadmill.{cpp,h}`, the most widely
deployed implementation of the FitShow `02 … 03` protocol: the frame
envelope and XOR trailer, the status-response field map in both its byte
orders (standard little-endian and the big-endian "anyrun" variant, which
Trot implements for tests but deliberately does not auto-select — upstream
gates it behind a user setting because no on-wire discriminator exists),
the status-code table, the three transport layouts (`FFF0`, `AE00`,
`FFE0`/`FFE4`) with their deployed preference order, and the
advertised-name matcher with its carve-outs (`FS-YK-`, the
NoblePro/`SW`-rule FTMS routing, the Tunturi T80/T60/T90 split —
`src/devices/bluetooth.cpp`). qdomyos-zwift's belt-control paths (its
`0x53` control-family frames, the user-data login among them) are
deliberately not implemented, and none of their bytes appear in this tree —
Trot's checksum tests use synthetic control-family vectors.

Trot's PitPat/Deerrun/SupeRun driver **independently implements protocol
behaviour using interface facts learned from
qdomyos-zwift's** `src/devices/deerruntreadmill/deerruntreadmill.cpp` and
its `PITPAT-T*` device matcher (`src/devices/bluetooth.cpp`): the Deerrun
transport variant on service `0xFFF0` with the notify/write roles swapped
relative to LifeSpan, and the status-query frame `6A 05 FD F8 43` with its
`4D 00 <seq> <len>` transport envelope. qdomyos-zwift's belt-control paths
(its speed/start/stop frames and its unlock preamble) are deliberately not
implemented; four of its captured frames (the unlock preamble, an
uncharacterised init frame, and two actuation-family frames) appear in
Trot's tests solely as checksum vectors, never transmitted (each is recorded
in `docs/provenance.md`).

Trot's KingSmith app-cipher (R2/X21) driver **independently implements
protocol behaviour using interface facts learned from qdomyos-zwift's**
`src/devices/kingsmithr2treadmill/kingsmithr2treadmill.{cpp,h}`, the most
widely deployed implementation of the obfuscated `props` text protocol:
the seven substitution-cipher tables and the transport pipeline (UTF-8 →
base64 → per-character substitution → `0x0D` terminator → 16-byte
write-without-response chunks), the three service/characteristic address
spaces (`0x1234`/`FED7`/`FED8` and its `0001…`/`0002…` variants) with
their per-model routing and fallbacks, the init message sequence and the
observed reply to each frame, the `props <key> <value>…` response grammar
with its `Error`/`mcu_version`/`goal` special cases, the telemetry key
list, and the advertised-name matcher (`src/devices/bluetooth.cpp`).
qdomyos-zwift's belt-control paths — its `props CurrentSpeed`/`runState`/
`ControlMode` setter writes — and its clock-setting `time_posix` init
write are deliberately not implemented. Where qdomyos-zwift exposes the
cipher-table choice as a user setting, Trot detects the table from the
traffic instead — an independent design, documented in the driver.

qdomyos-zwift is distributed under the **GNU General Public License,
version 3** — the same license as Trot; see [LICENSE](LICENSE) for the full
text.

## PaceKeeper — `peteh/pacekeeper`

Trot's PitPat/Deerrun/SupeRun driver **independently implements protocol
behaviour using interface facts documented by
[PaceKeeper](https://github.com/peteh/pacekeeper)** by peteh, the primary
open-source implementation of the PitPat OEM treadmill protocol, verified on
real hardware (a PitPat-T01 / SupeRun BA06-B1): the `FBA0`/`FBA1`/`FBA2`
service layout (`src/platform.h`), the full status-frame field map including
the step counter (`src/TreadmillHandler.cpp`), and the subscribe-and-push
interaction model (PaceKeeper reads the telemetry stream without writing a
single frame). PaceKeeper's belt-control functions are not implemented.

PaceKeeper is distributed under the **GNU General Public License,
version 3** — the same license as Trot; see [LICENSE](LICENSE) for the full
text.

## pitpat-treadmill-control — `azmke/pitpat-treadmill-control`

Trot's PitPat/Deerrun/SupeRun driver additionally **implements protocol
behaviour using interface facts learned from
[pitpat-treadmill-control](https://github.com/azmke/pitpat-treadmill-control)**
by azmke (Alexander), whose decoder (`src/treadmill_data.py`) carries
vendor-app-level detail: the inbound XOR checksum rule (validated by no
other implementation), the `FFFF`/`FF01`/`FF02` transport variant with its
4-byte `4D 00 <seq> <len>` envelope (`src/bluetooth_manager.py`), the
firmware-conditional duration unit (milliseconds on firmware ≥20, seconds
before), and the only published real capture of a status frame — the 52-byte
idle frame used as a test fixture in Trot's driver.
pitpat-treadmill-control is distributed under the MIT License:

```
MIT License

Copyright (c) 2025 Alexander

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## PitPat-WebBT — `KeiranY/PitPat-WebBT`

The PitPat status-frame field offsets and the characterization of
`6A 05 FD F8 43` as the protocol's heartbeat/status query were
**cross-checked against
[PitPat-WebBT](https://github.com/KeiranY/PitPat-WebBT)** by Keiran Young
(`treadmill.js`), an independent Web Bluetooth implementation released into
the **public domain** (The Unlicense).

## HomeAssistantWalkingPad — `sirfergy/HomeAssistantWalkingPad`

The finding that the PitPat wire protocol stays **metric even when the
console's panel is set to miles** (the imperial flag describes the display
only, so a decoder must not rescale) comes from
**[HomeAssistantWalkingPad](https://github.com/sirfergy/HomeAssistantWalkingPad)**
by sirfergy (`custom_components/walkingpad/protocol.py`), a
PaceKeeper-derived Home Assistant integration. HomeAssistantWalkingPad is
distributed under the **GNU General Public License, version 3** — the same
license as Trot; see [LICENSE](LICENSE) for the full text.

## milltender — `sstjohn/milltender`

Trot's FitShow driver **independently implements protocol behaviour using
interface facts (no code) established on real hardware by
[milltender](https://github.com/sstjohn/milltender)** by sstjohn
(`milltender.py`, `phase0/fitshow_probe.py`), the only FitShow telemetry
implementation verified on real hardware (a TX6 Glow-Up walking pad on a
FitShow FS-BT-D2 module): the finding that the status stream answers a
bare `02 51 51 03` poll with no login or init write of any kind, the
`FFF0` transport's LifeSpan-shaped roles (write FFF2, notify FFF1), the
≥12-byte status-payload floor, inbound XOR validation, and the imperial
wire scales on that hardware (0.1 mph speed, 0.001 mile distance, and the
0.1 kcal calorie reading whose conflict with qdomyos-zwift is why Trot
reports no calories from this protocol). milltender is distributed under
the **GNU Affero General Public License, version 3**
(<https://www.gnu.org/licenses/agpl-3.0.html>).

## fitshow-treadmill-accessible — `aradix85/fitshow-treadmill-accessible`

The finding that newer FitShow modules (FS-BT-C1, still advertising
`FS-…`) speak plain standard FTMS with the vendor `FFF0` service reduced
to a notify-only side channel — which is why Trot's FitShow driver
verifies the vendor *write* role and lets such hardware fall through to
the FTMS driver — comes from
**[fitshow-treadmill-accessible](https://github.com/aradix85/fitshow-treadmill-accessible)**
by aradix85 (`docs/PROTOCOL.md`, a reverse-engineered protocol note for
the VirtuFit TR600i). fitshow-treadmill-accessible is distributed under
the MIT License:

```
MIT License

Copyright (c) 2026 aradix85

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## walkingpad-controller — `mcdax/walkingpad-controller`

Trot's FTMS driver hardening **implements protocol behaviour using interface
facts documented by
[walkingpad-controller](https://github.com/mcdax/walkingpad-controller)** by
mcdax — specifically its derived documentation
(`docs/ftms-protocol-reference.md`), produced from vendor-app analysis plus
four real `btsnoop_hci` captures of a KingSmith KS-MC21: the mandatory
staggered CCCD-enable timing (100/200/300 ms), the KingSmith bit-13
step-count extension to Treadmill Data, the finding that start/stop/pause
transitions are signalled via Fitness Machine Status (0x2ADA), and the
no-application-keepalive finding.
walkingpad-controller is distributed under the MIT License:

```
MIT License

Copyright (c) 2026 mcdax

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## python-pyftms — `dudanov/python-pyftms`

The FTMS driver's Fitness Machine Status (0x2ADA) opcode map was
**cross-checked against
[python-pyftms](https://github.com/dudanov/python-pyftms)**, Copyright 2024
Sergey Dudanov, a clean FTMS v1.0 reference implementation
(`src/pyftms/models/machine_status.py`). No code was copied; the
implementation follows the FTMS v1.0 specification. python-pyftms is
distributed under the **Apache License, Version 2.0**
(<https://www.apache.org/licenses/LICENSE-2.0>).

## QWalkingPad — `DorianRudolph/QWalkingPad`

The WiLink field offsets and belt-state semantics were **cross-checked
against [QWalkingPad](https://github.com/DorianRudolph/QWalkingPad)**,
Copyright (C) 2021 Dorian Rudolph, a third independent implementation of the
WalkingPad protocol (`Protocol.cpp`). QWalkingPad is distributed under the
**GNU General Public License, version 3** — the same license as Trot; see
[LICENSE](LICENSE) for the full text.

## walkingpad-ble-footpod — `LucasFrendorf/walkingpad-ble-footpod`

Trot's KingSmith app-cipher (R2/X21) driver was **cross-checked against
[walkingpad-ble-footpod](https://github.com/LucasFrendorf/walkingpad-ble-footpod)**
by LucasFrendorf, a client for the KS-NGCH-G1C built on the same protocol
(`kingsmith_g1c.py`). It confirms both 128-bit address-space variants on
real G1C hardware revisions, the 16-byte write-without-response chunking,
and the G1C's default v6 cipher table — and it is the source that
establishes the poll-driven steady state: its monitor loop re-sends
`servers getProp …` at 1 Hz and reads the full telemetry from the `props`
replies. Its belt-control writes and its clock-setting `time_posix` init
frame are not implemented. walkingpad-ble-footpod is distributed under the
**GNU General Public License, version 3** — the same license as Trot; see
[LICENSE](LICENSE) for the full text.

A further public implementation of this protocol exists (a Kotlin Android
client) but carries no license; per this project's rules nothing was taken
from it and it is deliberately absent from these notices.
