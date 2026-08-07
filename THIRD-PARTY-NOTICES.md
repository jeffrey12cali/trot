# Third-party notices

Trot is licensed under **GPL-3.0-or-later**. It includes work derived from the
third-party projects below, whose copyright and license notices are reproduced
here as required.

## TreadSpan — `blak3r/treadspan`

Trot's LifeSpan / Omni protocol support (the "A1" command set over GATT
`FFF0`/`FFF1`/`FFF2`) was **bootstrapped from and cross-checked against
[TreadSpan](https://github.com/blak3r/treadspan)** by Blake Robertson, an
open-source project that reverse-engineered the LifeSpan Omni console protocol.
TreadSpan's documented opcode map and field encodings informed Trot's
independent reimplementation in Rust. TreadSpan is distributed under the MIT
License:

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

Trot's KingSmith WiLink driver **ports protocol knowledge from
[QZ (qdomyos-zwift)](https://github.com/cagnulein/qdomyos-zwift)** by Roberto
Viola: the five-frame init handshake including its two `F7 A5 61 …` magic
variants and their model-name routing
(`src/devices/kingsmithr1protreadmill/kingsmithr1protreadmill.cpp`), the
20-byte status-frame length requirement, and the advertised-name device list
with its `KS-HD-Z1D` FTMS carve-out (`src/devices/bluetooth.cpp`).

Trot's FTMS driver additionally ports from qdomyos-zwift: the verified
advertised-name list of real-world FTMS walking pads (Urevo, Merach, Sunny
Health & Fitness, CitySports, WellFit, Mobvoi, Sportstech, YPOO, TheRun,
Anplus, Focus, KingSmith, Sperax — `src/devices/bluetooth.cpp`, including the
`SPERAX_RM-01`-is-FTMS / `SPERAX_RM01`-is-proprietary carve-out) and the
Merach unlock characteristic and payload
(`src/devices/horizontreadmill/horizontreadmill.cpp`).

qdomyos-zwift is distributed under the **GNU General Public License,
version 3** — the same license as Trot; see [LICENSE](LICENSE) for the full
text.

## walkingpad-controller — `mcdax/walkingpad-controller`

Trot's FTMS driver hardening **ports protocol knowledge from
[walkingpad-controller](https://github.com/mcdax/walkingpad-controller)** by
mcdax — specifically its derived documentation
(`docs/ftms-protocol-reference.md`), produced from vendor-app analysis plus
four real `btsnoop_hci` captures of a KingSmith KS-MC21: the mandatory
staggered CCCD-enable timing (100/200/300 ms), the KingSmith bit-13
step-count extension to Treadmill Data, the `d18d2c10-…` ODM unlock
characteristic and its magic payload, the Control Point
indication-may-never-arrive behaviour with success signalled via Fitness
Machine Status (0x2ADA), and the no-application-keepalive finding.
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
