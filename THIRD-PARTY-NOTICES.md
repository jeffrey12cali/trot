# Third-party notices

Trot is licensed under **GPL-3.0-or-later**.

**Trot is an independent Rust implementation of the cited treadmill
protocols. It reproduces functional protocol data where necessary for
interoperability and testing — UUIDs, opcodes, device identifiers,
command bytes, and selected packet captures or test vectors.
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

## Removed drivers (2026-08-15)

Trot briefly shipped drivers for two further protocol families and
**deliberately removed both** before publication, as a licensing-prudence
decision: the KingSmith app-cipher generation (`kingsmith_props.rs`, whose
implementation necessarily reproduced KingSmith's substitution-cipher
tables) and the FitShow OEM family (`fitshow.rs`, part of whose protocol
record traces to an unlicensed OEM specification). With them, the notices
formerly carried here for `sstjohn/milltender` (AGPL-3.0),
`aradix85/fitshow-treadmill-accessible` (MIT) and
`LucasFrendorf/walkingpad-ble-footpod` (GPL-3.0) were retired — nothing
learned from those projects remains in the tree. The decision and its
reasoning are recorded in [`docs/provenance.md`](docs/provenance.md); the
removed notices remain available in git history.
