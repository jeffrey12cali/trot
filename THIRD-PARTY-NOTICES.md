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
qdomyos-zwift is distributed under the **GNU General Public License,
version 3** — the same license as Trot; see [LICENSE](LICENSE) for the full
text.

## QWalkingPad — `DorianRudolph/QWalkingPad`

The WiLink field offsets and belt-state semantics were **cross-checked
against [QWalkingPad](https://github.com/DorianRudolph/QWalkingPad)**,
Copyright (C) 2021 Dorian Rudolph, a third independent implementation of the
WalkingPad protocol (`Protocol.cpp`). QWalkingPad is distributed under the
**GNU General Public License, version 3** — the same license as Trot; see
[LICENSE](LICENSE) for the full text.
