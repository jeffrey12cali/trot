# Third-party notices

Trot is licensed under **GPL-3.0-or-later**. It includes work derived from the
third-party project below, whose copyright and license notice are reproduced here
as required.

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
