# Trot — licensing analysis

**Status:** internal analysis · prepared 2026-08-08 as `licensing-audit.md` ·
covers the tree at parent commit `bdfcb9c`. Renamed to `licensing-analysis.md`
on 2026-08-10: "audit" overstated what an AI-written internal document is, and
nothing in this file is an audit in the professional sense.

> **This is not legal advice, and it was not written by a lawyer.** It was
> written by an AI assistant for a project owner who is also not a lawyer. It
> is a structured summary of what the sources say and where the genuine
> uncertainty sits.

> **Corrections after external legal review (2026-08-10).** A lawyer reviewed
> the project's licensing position. The verdict: the core approach is probably
> lawful under EU/German law and the attribution is unusually conscientious —
> but this document and the notices were more confident than the evidence
> supports, and one factual claim was wrong. The corrections, which qualify
> everything below:
>
> 1. **Trot does contain literal upstream-derived protocol material.** The
>    former claim that Trot contains "no third-party source code" was wrong as
>    stated: the source contains cipher tables, device-name lists, command
>    frames, packet captures and test vectors that appear literally and derive
>    from upstream projects or from device manufacturers. These are very
>    likely functional protocol data rather than protectable expression, but
>    they exist, and the documentation now says so.
>    [`docs/provenance.md`](provenance.md) records each one.
> 2. **Article 5(3) of Directive 2009/24/EC is narrower than this document
>    implies.** It is not a general licence to study any program in any
>    circumstance — it applies to a person entitled to use a copy of the
>    program. Trot's actual basis is simpler and does not need Art. 5(3):
>    every source consulted was published under a licence permitting reading.
> 3. **The correct statement of the copyright position** is that functional
>    interface information is *generally* outside software-copyright
>    protection — not that every item this document classified as "protocol
>    knowledge" is conclusively unprotected. Material that could potentially
>    be protected includes upstream comments and prose, implementation control
>    flow not dictated by the protocol, distinctive error handling, and
>    creative selection or arrangement of a dataset.
> 4. **Licence compatibility does not resolve a hypothetical finding of
>    copying.** GPL, AGPL, Apache and MIT impose *different* conditions, and
>    some are not satisfied merely by listing the source in the notices. If a
>    provenance review identifies protected material, its exact licence
>    obligations will be applied source by source.
> 5. **"Non-commercial" plays no role in the legal analysis.** It is relevant
>    to practical enforcement risk, never to whether reproduction requires
>    authorisation, and it is not part of any justification below.
> 6. **Recommended next step (not yet done).** A targeted provenance and
>    non-literal-similarity review of `fitshow.rs`, `kingsmith_props.rs`,
>    `sperax.rs` and `urevo.rs`, classifying each significant similarity to
>    its upstream as (1) protocol-mandated, (2) independently derived,
>    (3) licensed literal material, or (4) potentially copied expression
>    needing remediation. **This review has not been performed.** This
>    document's "knowledge only" characterisations rest on reading Trot's
>    code and headers, not on a line-by-line similarity analysis (see the
>    caveat at the end of §10).

> **Addendum 2026-08-08:** the KingSmith app-cipher (R2/X21) driver
> (`kingsmith_props.rs`) was added after this analysis's snapshot. Its two
> sources are covered by the inventory below: the `cagnulein/qdomyos-zwift`
> row now includes it, and `LucasFrendorf/walkingpad-ble-footpod` (GPL-3.0,
> verified via the GitHub Licenses API on 2026-08-08) has its own row. The
> pattern is the one this analysis already covers — knowledge only, no code,
> control paths deliberately not ported, notices kept — with one addition in
> Trot's favour: where the upstream ships the cipher-table choice as a user
> setting, Trot's traffic-based table detection is an independent design with
> no upstream counterpart. A third public implementation (Kotlin, unlicensed)
> was identified and deliberately not consulted, per §6's guardrails.

> Every conclusion below is marked with a confidence level. Where something is
> unsettled, it says so. Points that warrant a real lawyer are collected in
> [§10](#10-what-i-could-not-determine--ask-a-lawyer-about-this). Read the
> body of this document through the corrections block above: where the two
> disagree, the corrections govern.

---

## 1. Plain-language summary

**Yes — reimplementing a wire protocol from knowledge gained by reading someone
else's source code is permissible, and it is what Trot did.** The reason is not
a loophole; it is the explicit structure of EU software copyright law. Copyright
protects the *expression* of a program — the code as written — and expressly
does **not** protect the ideas and principles underlying it, "including the
ideas and principles which underlie its interfaces" (Software Directive
2009/24/EC Art. 1(2); in Germany, UrhG § 69a(2)). A byte layout, an opcode
number, a checksum rule, a service UUID and a device-name string are facts
about how a machine talks. Facts are not authorship.

Three things make Trot's position stronger than the general case:

1. **Every upstream source was read under a licence that permits reading it.**
   These are published open-source repositories, not decompiled binaries and not
   leaked material. There is no "unauthorised access" question at all — the
   hardest part of the classic reverse-engineering analysis simply does not
   arise here. Trot never needed to rely on the decompilation safe harbour
   (Art. 6 / UrhG § 69e) and therefore never has to satisfy its narrow
   conditions.
2. **The output is an independent Rust implementation, not a translation.** It
   uses a different language, a different architecture (a driver trait feeding a
   neutral `Sample` struct), different naming, and in several places
   *deliberately diverges* from every upstream — dropped control frames, an
   independently derived checksum rule for Urevo, an inbound-checksum choice
   that rejects the primary upstream's rule, unit corrections that contradict
   published upstream comments. That divergence is not just good engineering; it
   is the best available evidence that this is a reimplementation from facts
   rather than a port of expression.
3. **The licences point the right way anyway.** Of twelve sources actually used,
   four are GPL-3.0 (same licence family as Trot), five are MIT, one is
   Apache-2.0, one is public domain, and one is AGPL-3.0. All of these are
   *one-way compatible into GPLv3*. Even in the worst case — if a court decided
   some of what was taken was protected expression after all — every one of
   those licences permits its use in a GPLv3 project, provided notices are kept.
   Trot keeps them, in `THIRD-PARTY-NOTICES.md`.

**The residual risk is therefore small and it is mostly not a licence-compliance
risk.** It is the narrower question of whether any *particular* thing taken was
protected expression rather than fact — and if it was, whether the notices Trot
already carries would satisfy the obligation. For the permissive and GPL sources
the answer is "the notices already do". For the one AGPL source it would matter
more, which is why the AGPL source gets its own treatment in [§5](#5-agpl-30-the-one-that-needs-care).

**Nothing in this analysis blocks publication.** There are four wording fixes worth
making first, listed in [§7](#7-fix-before-publishing-ordered-by-priority). The
most important one is a single line in `lifespan.rs` that cites a source not in
the notices file — it turns out to be the owner's own earlier Python project,
but nobody reading the repo can tell that, and an auditor would flag it as an
unattributed upstream.

**Overall confidence: high** that the approach is lawful and the obligations are
met. **Medium** on two narrow sub-questions (the device-name list as a
compilation; the AGPL characterisation), both addressed below.

---

## 2. The inventory (verified)

Every licence in the table was verified on 2026-08-08 against the GitHub
Licenses API (`GET /repos/{owner}/{repo}/license`) and, for the permissive ones,
against the actual copyright line in the upstream `LICENSE` file. The
"what Trot took" column is drawn from the module headers in
`crates/trot-core/src/drivers/` and from `THIRD-PARTY-NOTICES.md`, both read in
full.

| Source | Licence (verified) | What Trot took (code / knowledge only) | Compatible with GPLv3? | Obligations we must meet | Currently met? | Risk |
|---|---|---|---|---|---|---|
| `cagnulein/qdomyos-zwift` | **GPL-3.0** (repo `LICENSE` = GPLv3 text, 674 lines; no per-file "or later" grant found) | Knowledge only, across 5 drivers + FTMS: WiLink init handshake + name list; FTMS advertised-name list; Sperax `F5…FA` frames (**frames sent byte-identically**) and field offsets; FitShow envelope, field map, status codes, transports, name matcher; PitPat Deerrun transport + poll frame; KingSmith R2/X21 cipher tables, transport pipeline, address spaces, init sequence, `props` grammar + name matcher (post-audit addendum) | ✅ Yes — same licence | If code were copied: keep GPL notices, licence whole under GPL, state changes (§5). For knowledge: nothing legally required | ✅ Attributed in notices + 5 module headers | **Low** |
| `LucasFrendorf/walkingpad-ble-footpod` | **GPL-3.0** (verified 2026-08-08; bare GPLv3 `LICENSE`, no "or later" grant) | Knowledge only — KingSmith R2/X21 cross-check: both GATT address spaces on real G1C hardware, 16-byte WWR chunking, G1C v6 table default, the poll-driven steady state (post-audit addendum) | ✅ Yes — same licence | Same as above | ✅ Notices + `kingsmith_props.rs` header | **Low** |
| `peteh/pacekeeper` | **GPL-3.0** (repo `LICENSE`; sources carry no per-file header) | Knowledge only — FBA0 service layout, status-frame field map incl. steps, subscribe-and-push interaction model | ✅ Yes — same licence | Same as above | ✅ Notices + `pitpat.rs` header | **Low** |
| `DorianRudolph/QWalkingPad` | **GPL-3.0-or-later** (per-file header in `Protocol.cpp`: "either version 3 … or (at your option) any later version") | Knowledge only — independent confirmation of WiLink field offsets and belt-state semantics | ✅ Yes | Same as above | ✅ Notices + `kingsmith_wilink.rs` header | **Low** |
| `sirfergy/HomeAssistantWalkingPad` | **GPL-3.0** | Knowledge only — one finding: PitPat wire stays metric when the panel displays miles | ✅ Yes | Same as above | ✅ Notices + `pitpat.rs` header | **Low** |
| `ph4r05/ph4-walkingpad` | **MIT**, © 2017 CRoCS, Dusan Klinec (ph4r05) — verified verbatim | Knowledge only + **captured frames from its README used as test fixtures** | ✅ Yes (one-way MIT→GPLv3) | Retain copyright + permission notice if any "substantial portion" is included | ✅ Full MIT text reproduced in notices | **Low** |
| `blak3r/treadspan` | **MIT**, © 2025 Blake Robertson — verified verbatim | Knowledge only (LifeSpan opcode map; Urevo wake write + field map) + **annotated raw captures used as test fixtures**; Sperax service dump as cross-check | ✅ Yes | Same as above | ✅ Full MIT text in notices; also credited in `README.md` §Acknowledgements | **Low** |
| `mcdax/walkingpad-controller` | **MIT**, © 2026 mcdax — verified verbatim | Knowledge only, from its *documentation* (`docs/ftms-protocol-reference.md`): CCCD stagger timing, bit-13 step extension, 0x2ADA behaviour, no-keepalive finding | ✅ Yes | Same as above | ✅ Full MIT text in notices | **Low** |
| `azmke/pitpat-treadmill-control` | **MIT**, © 2025 Alexander — verified verbatim | Knowledge only + **one 52-byte real capture used as a test fixture** | ✅ Yes | Same as above | ✅ Full MIT text in notices | **Low** |
| `aradix85/fitshow-treadmill-accessible` | **MIT**, © 2026 aradix85 — verified verbatim | Knowledge only, from `docs/PROTOCOL.md` — one finding (FS-BT-C1 modules are plain FTMS) | ✅ Yes | Same as above | ✅ Full MIT text in notices | **Low** |
| `dudanov/python-pyftms` | **Apache-2.0** | Knowledge only — used as a *cross-check* of the 0x2ADA opcode map against the FTMS v1.0 spec. Notices state explicitly "No code was copied" | ✅ Yes, **one-way** (Apache-2.0 → GPLv3 only; not the reverse) | If code were used: retain `NOTICE` file contents, mark modified files, keep attribution; patent grant flows through | ✅ Attributed; nothing further owed for knowledge | **Low** |
| `KeiranY/PitPat-WebBT` | **The Unlicense** (public domain dedication) — verified | Knowledge only — independent confirmation of PitPat field offsets and the heartbeat frame | ✅ Yes (no conditions at all) | None. Attribution is pure courtesy | ✅ Attributed anyway | **None** |
| `sstjohn/milltender` | **AGPL-3.0** ⚠️ — verified | **Knowledge only, explicitly no code** (notices and `fitshow.rs` both say so): hardware findings — bare `02 51 51 03` poll with no login, FFF0 role arrangement, ≥12-byte floor, XOR validation, imperial wire scales | ✅ Yes as knowledge. Even as *code* it would be combinable via GPLv3 §13 — but with consequences; see [§5](#5-agpl-30-the-one-that-needs-care) | For knowledge: nothing. For code: AGPL §13 network-source obligation would attach to the combination | ✅ Attributed, with the AGPL correctly identified | **Low–Medium** — the only entry where the code/knowledge line carries real weight |
| FitShow OEM protocol doc (Chinese, v1.1; mirrored at `limdongkyu/fitshow-device-protocol`) | **No licence** → all rights reserved | Facts only, used to *verify* the envelope, the little-endian rule, field order and status-code table. Notices state "No text or code was taken from it; every ported byte comes from the licensed sources" | N/A — not a licensed input; see [§6](#6-unlicensed-material) | Do not reproduce its text, tables or diagrams. Do not vendor the file into the repo | ✅ Not vendored (verified: `git ls-files` shows no third-party source or PDF in the tree) | **Low** |
| `duhow/ftms-bridge` | **No licence** → all rights reserved | **Nothing.** Verified unused: no reference anywhere in the tree (`grep` across `*.rs`, `*.md` returns zero hits) | N/A | None | ✅ Correctly absent from notices | **None** |
| `lifespan_sc110` (Python) — *the owner's own earlier project* | Proprietary, "© 2026 Marcus Puchalla. All rights reserved" | **Code** — `lifespan.rs` says "Ported faithfully from … (parser.py + `__init__`.py)" | ✅ Yes — the owner is the sole copyright holder and may licence his own work under GPLv3 | None legally. But the repo does not say whose it is | ⚠️ **Not clear from the repo** — see [§7.1](#71-p0--make-the-lifespan_sc110-line-self-explanatory) | **Low legally, Medium reputationally** |

### Dependency tree (separate question, checked for completeness)

`cargo license` over the full workspace returns only GPLv3-compatible terms:
174 × `Apache-2.0 OR MIT`, 51 × `MIT`, 18 × `Unicode-3.0`, plus small numbers of
BSD-2/3-Clause, ISC, 0BSD, Zlib, BSL-1.0 and `MIT OR Unlicense`. Three
individually noteworthy entries, all fine:

- `option-ext` — **MPL-2.0**. GPL-compatible; MPL 2.0 § 3.3 expressly allows
  distribution under a "Secondary License" (GPL/LGPL/AGPL). *(Confidence: high — this is the MPL's stated design and the FSF lists MPL 2.0 as GPL-compatible.)*
- `webpki-roots` — **CDLA-Permissive-2.0**. A permissive *data* licence for the
  Mozilla root store, not code. No copyleft, no notice-retention obligation
  beyond the usual. Not FSF-reviewed, which is why it is worth naming here, but
  no realistic conflict. *(Confidence: medium-high.)*
- `sync_wrapper` — bare **Apache-2.0** (not dual-licensed). One-way compatible
  into GPLv3; nothing to do.

No GPL-incompatible dependency is present. The only GPL-3.0-or-later entries are
Trot's own two crates. *(Confidence: high — machine-generated from the resolved
lockfile.)*

---

## 3. Are protocols, wire formats and byte layouts copyrightable?

**Short answer: no, not as such — and this is one of the better-settled areas of
software copyright in the EU.** The longer answer distinguishes what is settled
from what is not.

### 3.1 EU statute — settled law

**Directive 2009/24/EC (the Software Directive), Art. 1(2):**

> "Protection in accordance with this Directive shall apply to the expression in
> any form of a computer program. Ideas and principles which underlie any
> element of a computer program, including those which underlie its interfaces,
> are not protected by copyright under this Directive."

That clause was written for exactly this situation. A treadmill's BLE protocol is
the interface between two programs; the ideas and principles underlying it are
carved out of protection by name. Recital 11 of the Directive elaborates that the
"logic, algorithms and programming languages" comprising a program's ideas and
principles are not protected, and that where interconnection interfaces are
concerned, "the parts of the program which provide for such interconnection and
interaction … are generally known as 'interfaces'". *(Confidence: high — this is
statute, directly on point.)*

**Art. 5(3)** — the observe/study/test right:

> "The person having a right to use a copy of a computer program shall be
> entitled, without the authorisation of the rightholder, to observe, study or
> test the functioning of the program in order to determine the ideas and
> principles which underlie any element of the program …"

**Art. 6** — decompilation for interoperability, subject to conditions
(indispensability, the information not already readily available, limitation to
the necessary parts, and a prohibition on using it to create a substantially
similar program or for anything other than interoperability).

**Art. 8** makes contractual terms contrary to Art. 5(2), 5(3) and Art. 6 null
and void — you cannot licence away the study right.

**German implementation — UrhG §§ 69a–69g.** § 69a(2) sentence 2 mirrors
Art. 1(2) verbatim ("Ideen und Grundsätze, die einem Element eines
Computerprogramms zugrunde liegen, einschließlich der den Schnittstellen
zugrunde liegenden Ideen und Grundsätze, sind nicht geschützt"). § 69d(3) is the
observe/study/test right, § 69e the decompilation right, and § 69g(2) voids
contrary contractual provisions. *(Confidence: high.)*

**Why Art. 6 / § 69e does not constrain Trot.** This matters and is easy to get
backwards. Art. 6's conditions are the price of an *extraordinary* permission —
making unauthorised reproductions/translations of a program (decompiling a
binary) that would otherwise infringe. Trot did none of that. It read published
source code, under licences (GPL, MIT, Apache, Unlicense) that affirmatively
grant the right to use, copy and study. There was no infringing act to excuse,
so there is no safe harbour to qualify for, and Art. 6's "must not create a
substantially similar program" condition — which people sometimes cite at
projects like this — is not a constraint that applies here at all. *(Confidence:
high on the reasoning; this is the standard reading, though I found no CJEU case
squarely restating it in the "reading licensed source" posture, because the
question is too obvious to litigate.)*

### 3.2 CJEU — C-406/10 *SAS Institute v World Programming* (2 May 2012)

The leading EU authority, and it is close to on point. Holdings that matter:

- **Para. 39:** "neither the functionality of a computer program nor the
  programming language and the format of data files used in a computer program
  in order to exploit certain of its functions constitute a form of expression of
  that program" — and so are not protected by the Software Directive.
  A treadmill wire format is a data format in exactly this sense.
- **Paras. 61–62:** a person who has obtained a copy under a licence may, without
  authorisation, observe, study and test the program to determine the ideas and
  principles underlying it; copyright is not infringed where that person "did not
  have access to the source code … but merely studied, observed and tested" in
  order to reproduce the functionality in a second program.

**The honest caveat — para. 45.** The Court expressly left open that a
programming language or data file format *might* be protected "as works" under
the InfoSoc Directive 2001/29 if they constitute the author's own intellectual
creation. This is a real, unresolved gap and it should not be glossed over. In
practice it has never been successfully used against a wire-format
reimplementation that I am aware of, and it is hard to see how a byte layout
dictated by sensors, bandwidth and an 8-bit checksum could clear the
originality threshold (*Infopaq*, C-5/08: "author's own intellectual creation").
But "hard to see how" is not "decided". *(Confidence: high that Trot's protocol
facts are unprotected; medium that no theory exists under which any of them
could be argued protected.)*

### 3.3 CJEU — C-13/20 *Top System v Belgian State* (6 Oct 2021)

Holds that a lawful acquirer may decompile a program, in whole or in part, to
correct errors, and that this rests on Art. 5(1) rather than requiring Art. 6's
conditions. Its relevance to Trot is indirect but real: it shows the CJEU reading
the user-side exceptions *broadly and functionally*, not as grudging exceptions
to be construed narrowly. It also confirms the general direction of travel — EU
law treats getting software to work with other software as a protected interest.
*(Confidence: high on the holding, medium on how much weight it carries here —
it is supportive context, not authority on Trot's facts.)*

### 3.4 United States — relevant because the upstreams and GitHub are US-based

- **17 U.S.C. § 102(b):** "In no case does copyright protection for an original
  work of authorship extend to any idea, procedure, process, system, method of
  operation, concept, principle, or discovery …" The statutory analogue of
  Art. 1(2), tracing to *Baker v. Selden*, 101 U.S. 99 (1879).
- **Google LLC v. Oracle America, Inc., 593 U.S. 1 (2021).** Widely cited as "APIs
  are fair game", and that is an overstatement worth correcting. The Court
  **assumed copyrightability arguendo** and expressly declined to decide it,
  then held that Google's copying of ~11,500 lines of Java SE declaring code was
  **fair use as a matter of law**. Its most useful reasoning for Trot is on
  factor two: declaring code sits "further than most computer programs from the
  core of copyright" because it is functional in nature and "inherently bound
  together" with uncopyrightable ideas. That reasoning applies with more force to
  a treadmill status frame than it did to the Java API. But note what this means
  structurally: *Google* is a **fair-use** holding, which is a US-only defence
  with no EU equivalent. If you are relying on *Google*, you are relying on a
  defence, not on the absence of a right. Trot's actual EU position — that there
  is no protected subject matter in the first place — is the stronger one.
  *(Confidence: high on what the case held and on its limits.)*
- **Sega Enterprises v. Accolade**, 977 F.2d 1510 (9th Cir. 1992) and **Sony
  Computer Entertainment v. Connectix**, 203 F.3d 596 (9th Cir. 2000):
  intermediate copying in the course of reverse engineering, undertaken to reach
  unprotected functional elements, is fair use. Notably, *Connectix* found fair
  use **without** a formal clean room. These are the closest US analogues, and
  they involved *disassembling binaries* — a far more aggressive act than reading
  published source. *(Confidence: high.)*
- **Computer Associates v. Altai**, 982 F.2d 693 (2d Cir. 1992): the
  abstraction–filtration–comparison test, which filters out elements dictated by
  efficiency, by external factors (hardware, compatibility requirements,
  standards) and by the public domain before comparing. Every protocol constant
  in Trot is dictated by an external factor — the treadmill firmware. *(Confidence:
  high.)*
- **Lotus v. Borland**, 49 F.3d 807 (1st Cir. 1995), aff'd by an equally divided
  Court, 516 U.S. 233 (1996): a command hierarchy was an uncopyrightable "method
  of operation". Affirmance by an equally divided Court means no nationwide
  precedent — cite it as persuasive, not binding. *(Confidence: high, including
  on the limitation.)*

### 3.5 The one item that is *not* purely a protocol fact

Everything above concerns byte layouts, opcodes, offsets and checksum rules —
comfortably facts. There is one input in Trot that is qualitatively different and
deserves to be named:

**The advertised-name lists** (`ADV_NAME_PREFIXES` in `ftms.rs`, and the matcher
carve-outs in `fitshow.rs`, `kingsmith_wilink.rs`, `sperax.rs`, `pitpat.rs`),
described in the headers as "ported from qdomyos-zwift's device matcher". Each
individual string ("URTM", "KS-MC", "SPERAX_RM-01") is a bare fact about what a
device broadcasts. But the *selection* — which twenty-odd devices, with which
carve-outs — is the product of years of someone's field testing, and a curated
selection is the classic shape of a protected compilation.

My assessment, with the reasoning shown so you can weigh it:

- Under **Art. 3(1) Database Directive 96/9/EC** and *Football Dataco*
  (C-604/10), a selection is protected only where the author expresses creative
  ability through free and creative choices; where the selection is "dictated by
  technical considerations, rules or constraints which leave no room for creative
  freedom", it is not. The selection criterion here is purely functional and
  binary: *does this device speak this protocol?* There is no room for creative
  choice — the treadmill firmware decides.
- The **sui generis database right** (Art. 7) requires substantial investment in
  obtaining, verifying or presenting the contents. Arguably present upstream.
  But it protects against extraction of a substantial part of *the database*,
  and qdomyos-zwift's device matcher is a scattered set of `if` conditions in a
  general-purpose source file, not a database being exploited as such. This is
  the weakest link in my analysis and I flag it as such.
- **It does not matter much either way**, because qdomyos-zwift is GPL-3.0 and
  Trot is GPL-3.0-or-later. If the list is protected, it is licensed to Trot on
  terms Trot already complies with (notice retained in `THIRD-PARTY-NOTICES.md`
  and in three module headers). See [§4.4](#44-gpl-30--gpl-30-the-easy-case-with-a-version-footnote) for the one
  version wrinkle.

*(Confidence: medium-high that the lists are unprotected facts; high that the
GPL path makes the question academic.)*

---

## 4. Licence-by-licence: what would be required if code *were* used

Trot's position is that only knowledge was taken (with the fixture-frames
nuance below). This section states the obligations anyway, both because the
distinction is not always crisp and because the owner should know which of
Trot's current practices are **obligations** and which are **courtesy**.

### 4.1 A note on the test fixtures — the sharpest code/knowledge boundary

Three drivers use captured frames published by upstream projects as test
fixtures: ph4-walkingpad's README captures (`kingsmith_wilink.rs`), treadspan's
568-frame annotated E1L capture (`urevo.rs`), azmke's 52-byte idle frame
(`pitpat.rs`), and qdomyos-zwift's 64 embedded captured Sperax frames
(`sperax.rs`). Trot also *sends* several frames byte-identically to upstream
(Sperax `F5 07 00 01 26 D8 FA`, PitPat `6A 05 FD F8 43`, FitShow `02 51 51 03`).

This is the closest anything in Trot comes to copying, so it is worth being
precise:

- **Frames the treadmill emitted are recordings of machine output, not authored
  works.** A packet capture is a record of facts. Under EU law a work requires
  the author's own intellectual creation; a device's telemetry has no author.
  *(Confidence: high.)*
- **Frames Trot sends are dictated by the device.** There is exactly one byte
  sequence that makes a Sperax start streaming. Merger doctrine (US) and the
  Art. 1(2) interface carve-out (EU) both cover this squarely. *(Confidence: high.)*
- **The upstream's surrounding annotation and commentary would be expression** —
  and Trot did not take it; the module headers demonstrably reason about the
  captures independently, twice contradicting the upstream's own labels
  (treadspan's "0.1 miles" comment, qdomyos' inbound checksum variant).
- **All four fixture sources are MIT or GPL anyway.** Even on the most
  pessimistic characterisation, the notices Trot carries satisfy them.

*(Overall confidence: high. This is not where a problem lives.)*

### 4.2 MIT → GPLv3

MIT is one-way compatible into GPLv3: the combined work ships under GPLv3, and
the MIT-licensed portions retain their own terms as well. The only condition MIT
imposes is notice retention — "The above copyright notice and this permission
notice shall be included in all copies or substantial portions of the Software."

**What that requires in practice:** the *full* MIT text plus the *exact*
copyright line, distributed with the binary as well as the source. Trot does
this. All five MIT copyright lines in `THIRD-PARTY-NOTICES.md` were checked
character-for-character against the upstream `LICENSE` files and all five match
exactly:

- `© 2025 Blake Robertson` ✓
- `© 2017, CRoCS, Dusan Klinec (ph4r05)` ✓
- `© 2026 mcdax` ✓
- `© 2025 Alexander` ✓
- `© 2026 aradix85` ✓

And `dist-workspace.toml` line 29 ships `THIRD-PARTY-NOTICES.md` inside every
release archive, which is the part projects usually forget. **This obligation is
fully met — and, since only knowledge was taken, it is met to a higher standard
than required.** *(Confidence: high.)*

### 4.3 Apache-2.0 → GPLv3

One-way compatible with **GPLv3 only** — not GPLv2. The FSF's position, and the
Apache Software Foundation's, agree on this: Apache-2.0's patent-termination and
indemnification clauses are additional restrictions that GPLv2 cannot accommodate
but GPLv3 § 7 can. The direction is strictly Apache → GPLv3; GPLv3 code cannot go
back into an Apache-2.0 project.

If pyftms code were used, Trot would owe: retention of the licence, copyright and
attribution notices; propagation of any `NOTICE` file contents; and a statement
of changes on modified files (§ 4(b)). Trot took none, and says so explicitly in
the notices ("No code was copied"). Nothing further is owed. Since Trot is
`GPL-3.0-or-later`, the GPLv3-only restriction is satisfied.
*(Confidence: high.)*

### 4.4 GPL-3.0 → GPL-3.0 — the easy case, with a version footnote

Same licence family, so compatibility is trivial. But note what would still apply
if code *were* copied, because "same licence" is not the same as "no
obligations": GPLv3 § 5 requires that modified files carry prominent notices
stating that you changed them and the date; that the whole work be licensed under
GPLv3; and that all copyright notices and warranty disclaimers be preserved.
Attribution is not optional under the GPL; it is § 5(a).

**The version footnote — worth knowing, not worth acting on.** Trot declares
`GPL-3.0-or-later` (`Cargo.toml` line 7). Of the four GPL upstreams, only
QWalkingPad carries an explicit "or (at your option) any later version" grant
(verified in its `Protocol.cpp` header). qdomyos-zwift, pacekeeper and
HomeAssistantWalkingPad ship the bare GPLv3 licence text with no per-file "or
later" statement in the files I checked. The conservative reading of a bare GPLv3
`LICENSE` with no "or later" grant is **GPL-3.0-only**.

Practical consequence: **none today**, because no code was taken. If code were
ever taken from those three, the resulting combined work would be effectively
GPL-3.0-only for redistribution purposes even though Trot's own crates remain
`or-later`. That is a normal, well-understood situation, not a violation — but it
would be worth a sentence in the notices if it ever becomes true.
*(Confidence: medium-high — the "bare LICENSE means only" reading is the standard
conservative interpretation, but it is a genuine grey area in the community and
some maintainers would say they intended "or later".)*

---

## 5. AGPL-3.0 — the one that needs care

`sstjohn/milltender` is **AGPL-3.0** (verified). Trot's notices and `fitshow.rs`
both state, explicitly and in those words, that **protocol knowledge only** was
taken and **no code**.

**Is taking knowledge only from an AGPL project into a GPLv3 project acceptable?
Yes — and the reasoning is not a technicality.** Copyleft is a condition attached
to the *copyright licence*, and a copyright licence only bites where you do
something that copyright controls: copying, adapting, distributing. Reading a
published program and learning a fact from it — that a treadmill answers a bare
`02 51 51 03` poll with no login — is not an act restricted by copyright. The
AGPL cannot reach it because copyright does not reach it, and Art. 1(2) /
UrhG § 69a(2) put the point beyond argument by excluding interface ideas and
principles from protection outright. No licence can create copyright in an
unprotected fact by asserting it in a `LICENSE` file. *(Confidence: high.)*

**What would change if code had been copied.** Three things, and they compound:

1. **The AGPL section could not be relicensed as GPL-3.0.** There is no
   downgrade path. AGPLv3 § 13 has no "or any later/other version" escape into
   plain GPL. The only lawful routes would be (a) the copyright holder
   relicensing, or (b) removing the code.
2. **GPLv3 § 13 provides the combination route — but with a condition.** Trot's
   own `LICENSE` (lines 552–561) contains it:

   > "you have permission to link or combine any covered work with a work
   > licensed under version 3 of the GNU Affero General Public License into a
   > single combined work, and to convey the resulting work. The terms of this
   > License will continue to apply to the part which is the covered work, but
   > **the special requirements of the GNU Affero General Public License,
   > section 13, concerning interaction through a network will apply to the
   > combination as such.**"

   So the combination is *permitted* — this is a real and often-overlooked
   answer. But the AGPL's network clause then attaches to the combination.
3. **And that clause would actually bite here, which is the uncomfortable part.**
   AGPLv3 § 13 requires that if you modify the program and *users interact with
   it remotely through a computer network*, you must offer those users the
   Corresponding Source. Trot is a daemon that serves an HTTP `/api` and a
   WebSocket `/ws` — the Nowhere app is precisely a remote client of it. Whether
   a localhost-bound API constitutes "interacting remotely through a computer
   network" is **genuinely unsettled** and I will not pretend otherwise; the
   better view is probably that a loopback-only, single-user daemon does not
   trigger it, but Trot's API can be bound beyond loopback and the moment anyone
   does that, the argument gets much harder. This is exactly the kind of question
   that should go to a lawyer *if* it ever becomes live.

**It is not live today.** Nothing to do. But this is the single item in this analysis
where I would want the "knowledge only, no code" claim to be true in fact and not
merely in the header comment — and, having read `fitshow.rs`, I believe it is:
milltender is Python, the Rust implementation shares no structure with it, and
what Trot attributes to milltender is a list of *findings* ("answers a bare poll
with no login", "≥12-byte floor", "0.1 mph / 0.001 mile on that hardware"), not
routines.

**Recommendation:** keep it. Do not drop milltender. Its findings are what let
Trot justify *not* sending FitShow's login frame — a safety-relevant design
decision — and the citation is honest. Dropping the attribution while keeping the
knowledge would be strictly worse in every dimension, legal and ethical.
*(Confidence: high on the knowledge/code conclusion; medium on the § 13
localhost question, which is why it is flagged.)*

---

## 6. Unlicensed material

Two items: the FitShow OEM protocol document (Chinese, v1.1, mirrored at
`limdongkyu/fitshow-device-protocol`) and `duhow/ftms-bridge`. Neither repository
has a licence — the GitHub API returns `null` for both, verified.

**The default is "all rights reserved."** GitHub's Terms of Service (§ D.5) give
other users a limited right to view and fork public repositories, but grant no
licence to use, modify or redistribute. So: reproducing the document, translating
and republishing it, or vendoring it into Trot would be infringement.

**But extracting facts from it is a different act.** Copyright protects the
document's expression — its prose, its tables as laid out, its diagrams, its
selection and arrangement — not the protocol it describes. That "little-endian is
the protocol-wide rule" and that a certain status code means "running" are facts
about a machine, and facts are not protected in any jurisdiction relevant here
(EU: Art. 1(2) / § 69a(2) and the originality requirement; US: § 102(b), *Feist
Publications v. Rural Telephone*, 499 U.S. 340 (1991)). Reading an unlicensed
specification and implementing what it describes is the ordinary way protocols
have always been implemented. *(Confidence: high.)*

**Reading a specification document vs. decompiling a binary — the distinction the
brief asks about.** They are not close. Decompiling makes a reproduction and a
translation of a protected work, both restricted acts, which is precisely why
Art. 6 / § 69e exists to excuse them under conditions. Reading a document makes
no reproduction at all: you loaded a page a rightsholder published publicly, and
took an unprotected fact out of it. There is nothing to excuse and therefore no
conditions to satisfy. The unlicensed status of the document restricts what you
may do *with the document*; it does not create a right in the protocol. *(Confidence: high.)*

**Two practical guardrails, both currently satisfied:**

1. **Do not vendor it.** Verified: `git ls-files` shows no third-party source,
   PDF or specification anywhere in the tree. ✅
2. **Do not paraphrase it closely.** The `fitshow.rs` header describes what the
   spec *settles* (two specific findings) rather than reproducing its content,
   and states "No text or code was taken from it; every ported byte comes from
   the licensed sources." That framing is correct and should be kept. ✅

**`duhow/ftms-bridge`:** verified genuinely unused — zero references across all
`.rs` and `.md` files. Correctly absent from the notices. Nothing to do. If it is
ever consulted, add a notices entry noting the unlicensed status, as was done for
the OEM document.

**A separate point worth raising, briefly:** an OEM specification is more likely
than a hobby repo to have been distributed under confidentiality. Trot obtained
it from a public GitHub mirror, not from FitShow, and is not bound by an NDA it
never signed — German law has no general "third-party recipient" trade-secret
liability absent knowledge of a breach. Under **GeschGehG § 3(1) Nr. 2** (the
German implementation of the EU Trade Secrets Directive 2016/943),
**reverse engineering is expressly lawful** where the product was lawfully
acquired and the acquirer is under no restricting duty. Trot's actual protocol
knowledge comes from open-source implementations and hardware observation, not
from the document, which the notices already say. *(Confidence: medium-high — the
GeschGehG § 3(1) Nr. 2 reverse-engineering permission is clear statute; the
"unknowing recipient of a leaked spec" analysis is more fact-dependent and is on
the lawyer list.)*

---

## 7. Trademark

**Nominative / referential use is the correct frame, and Trot is using it
correctly.** The EU provision is **Art. 14(1)(c) EUTMR** (Regulation
2017/1001), which permits use of a mark "for the purpose of identifying or
referring to goods or services as those of the proprietor of that trade mark",
subject to Art. 14(2)'s requirement that the use be in accordance with honest
practices in industrial or commercial matters. The German equivalent is
**MarkenG § 23(1) Nr. 3**. The leading cases are **C-63/97 *BMW v Deenik*** (a
repairer may say it repairs BMWs) and **C-228/03 *Gillette v LA-Laboratories***,
which sets the honest-practices criteria: the use must not suggest a commercial
connection, must not take unfair advantage of or damage the mark's repute, must
not denigrate it, and must not present the goods as imitations. The US analogue
is the *New Kids on the Block* test (971 F.2d 302, 9th Cir. 1992).

**Is the README's Trademarks section adequate? Yes — it is better than most.** It
does the three things that matter: it disclaims affiliation and endorsement in
bold, it identifies the marks as belonging to their owners, and it states that
use is "for identification and compatibility purposes only". It also correctly
invokes GPLv3 § 7(e) to reserve Trot's own name and mark, which is a real
provision and correctly cited. Two gaps, both trivial to close:

- **Two brands appear in shipped code but not in the Trademarks list:**
  **Anplus** (`"ANPLUS-"`, `"ANPIUS-"` in `ftms.rs`) and **Focus Fitness**
  (`"FOCUS M3"`). Add both.
- **VirtuFit** appears in `THIRD-PARTY-NOTICES.md` (the TR600i) and is not
  listed. Add it, or drop the model reference.
- *(Also: `CLAUDE.md` says the Trademarks section must cover Woodway and
  NordicTrack; NordicTrack is there, Woodway is not. Either add it or fix
  `CLAUDE.md` — an internal consistency point, not a legal one.)*

**Is naming a driver module after a brand a problem? No.** `lifespan.rs`,
`sperax.rs`, `fitshow.rs`, `pitpat.rs`, `urevo.rs`, `kingsmith_wilink.rs` are
internal source filenames identifying which device the module reads. That is
descriptive, non-branding, referential use in its purest form: there is no other
practicable way to say "this file handles Sperax treadmills", which is precisely
the *Gillette* necessity criterion. They are not product names, they do not
appear in commerce as an indication of origin, and no consumer encounters them.

The line to keep watching is a different one and it is worth naming: **the
product name is "Trot", not "TrotPad" or "LifeSpan Companion"**, and the
README already commits to that. Brand names must stay in the *predicate*
("works with LifeSpan") and never migrate into the *subject* (a product called
"LifeSpan-something"). Trot is on the right side of that line today. The one
place to be slightly careful in future is marketing copy and app-store
listings, where prominence and typography start to matter in a way they do not
in a source tree.

*(Confidence: high on the framework and on the module-naming conclusion;
medium-high on adequacy, since trademark adequacy is fact- and
presentation-sensitive and the landing page and any future app-store listing are
outside this repo and were not reviewed.)*

---

## 8. The clean-room question — honest risk assessment

The brief states the situation plainly: **there was no formal clean room.** The
same agent read the upstream source and then wrote Trot's implementation. How
much does that matter?

### 8.1 Less than people think — but it is not nothing

**No jurisdiction requires a clean room.** There is no statute in the EU, in
Germany or in the US that conditions the legality of a reimplementation on
procedural separation between the reader and the writer. A clean room is an
**evidentiary device**, not a legal element. Its purpose is to make independent
creation provable — to give you an unimpeachable answer when a plaintiff says
"you had access, and the similarity is explained by copying". It manages
litigation risk; it does not create a right you would otherwise lack.

The supporting evidence for that reading:

- **The Software Directive assumes access.** Art. 5(3) grants a right to observe,
  study and test *in order to determine the underlying ideas and principles* —
  the whole point is that the person doing the studying may then use what they
  learned. Art. 8 makes that right unwaivable. A regime that required the studier
  and the implementer to be different people would have said so; instead the CJEU
  in *SAS* (paras. 61–62) blessed studying-then-reimplementing by the same party.
- **US courts have found fair use without a clean room.** *Sony v. Connectix*
  is the clearest example: Connectix's engineers disassembled the PlayStation
  BIOS and wrote the emulator, and the Ninth Circuit found fair use. *NEC v.
  Intel* (N.D. Cal. 1989) is the canonical case where a clean room *helped* — but
  it helped as proof, and NEC's engineers had actually seen the Intel microcode
  first.

### 8.2 Where the real line sits

The line is not "did you look" — it is **"what did you take"**. The
*Altai* abstraction–filtration–comparison framework is the clearest articulation,
and EU courts reason similarly even without the label. Things that indicate
copying of expression:

| Indicator of copied expression | Present in Trot? |
|---|---|
| Verbatim or near-verbatim code | **No** — different language (Rust vs C++/Python/JS), no transliterated routines found |
| Copied comments (the classic smoking gun) | **No** — the headers are original prose that *argues with* upstream |
| Copied identifier and variable names | **No** — Trot's naming is its own (`Sample`, `select_transport`, `CommandSpacer`, `belt_state`); it *cites* upstream names (`noOpData`, `minimal_cmd_space`, `TreadmillData.__init__`) as references, which is attribution, not copying |
| Same structure, sequence and organisation | **No** — Trot's architecture (a `Driver` trait, a registry, a neutral `Sample`, engine/driver separation) has no upstream counterpart; qdomyos-zwift is a Qt device-class hierarchy, pacekeeper is Arduino, ph4 is asyncio |
| Copied code layout / formatting | **No** — `rustfmt` |
| Idiosyncratic errors reproduced | **Mostly no, with one disclosed exception.** Trot corrects four documented upstream errors (treadspan's "0.1 miles", the 0.006225680934 claim, qdomyos' inbound checksum variant, and its `(value[9] << 8) & 0xff` expression, provably always zero). Exception: `sperax.rs` deliberately keeps upstream's raw-wire-offset reader although it documents the wire as escaped, because no inbound capture exists and upstream is the only implementation verified on hardware. Disclosed in the module header and `provenance.md`. |

The last row is worth dwelling on, because in a copying dispute it is the kind of
evidence that ends the argument. Copied code carries the original's mistakes.
Trot's module headers document, in writing and with reasoning, four places where
it examined the upstream's claim, tested it against raw captures, found it wrong,
and did something different. That is a written record of independent analysis,
timestamped in git. **A deliberately constructed defensive record could not
easily be better than what the ordinary engineering process produced here.**

Similarly, the systematic *refusal* to port whole categories of upstream code —
every belt-control path, in all five drivers where upstream had one — demonstrates
selective, purposive extraction of specific facts rather than wholesale porting.

### 8.3 The realistic risk profile

Ranked, with my honest estimate:

1. **A GPL/MIT upstream author complains publicly about attribution or
   derivation.** *Most likely by a wide margin, and it is a reputational event,
   not a legal one.* Trot's mitigation is already strong: twelve sources
   attributed, file-level provenance in every driver header, full licence texts
   reproduced, notices shipped in the binary archives. Trot's attribution is
   **substantially more than any of these licences require** — see § 9.3.
2. **Someone argues the device-name list is a protected compilation.** Low
   probability, and it fails on the merits (functional selection, no creative
   freedom) — and it is moot anyway because the source is GPL-3.0 and Trot is
   GPL-compatible with notices retained.
3. **A treadmill manufacturer objects.** Low, and mostly not a copyright theory —
   it would be trademark, or a claim about circumvention or terms of service.
   Trot's observe-only posture (it never writes a control frame; `CONTRIBUTING.md`
   makes this a stated policy and tests pin the write sets) removes most of the
   surface here. It is also, incidentally, excellent evidence of good faith.
4. **A copyright claim over the protocol implementations themselves.**
   Low. It would have to overcome Art. 1(2), § 69a(2), *SAS* para. 39, and — for
   a US plaintiff — § 102(b) and *Altai* filtration, on facts where the defendant
   wrote in a different language with a different architecture and demonstrably
   corrected the plaintiff's own errors.
5. **The AGPL § 13 question becoming live.** Currently zero, because no code was
   taken. It would only arise if that changed.

*(Confidence: high on the ranking; the probabilities are judgement, not
measurement.)*

### 8.4 One honest caveat about the process

The brief is candid that the same agent read upstream and wrote Trot. The
strongest possible version of Trot's defence would have been a two-person split
(one reads and writes a facts-only spec; another implements from the spec alone).
That was not done, and this document should not pretend otherwise.

What Trot has instead is a **documented substitute**: the module headers function
as exactly the facts-only specification a clean room would have produced, they
are unusually detailed, they cite provenance per field, and git records that they
were written alongside the code. That is weaker than a true clean room as
*procedure* and roughly equivalent as *evidence*. Given that no protected
expression was taken in the first place, the procedural gap is very unlikely to
matter.

**Practical suggestion for the future** (not a fix, a habit): for any new driver,
write the protocol notes in `docs/drivers/` **first**, commit them, then
implement from the committed notes. That produces a genuine
paper trail at essentially zero cost, and it makes the separation visible in git
history rather than asserted after the fact.

---

## 9. Fix before publishing (ordered by priority)

### 9.1 P0 — Make the `lifespan_sc110` line self-explanatory

**File:** `crates/trot-core/src/drivers/lifespan.rs`, line 2.

**Current text:**

```rust
//! Ported faithfully from the Python `lifespan_sc110` package (parser.py + __init__.py).
```

**The problem:** this is the only place in the tree that says code was *ported*
from a named package, and that package does not appear in
`THIRD-PARTY-NOTICES.md`. Anyone auditing the repo — a packager, a Debian/AUR
reviewer, a curious user, or an upstream author checking whether they were
credited — will read that line as an unattributed third-party dependency. It is
the single most likely thing in this repo to generate an accusatory GitHub issue.

**The reality (verified):** `lifespan_sc110` is the owner's *own* earlier Python
project at `~/projects/lifespan`, whose `LICENSE` reads "Copyright © 2026 Marcus
Puchalla. All rights reserved." Sole copyright holder, therefore free to licence
that work under GPL-3.0-or-later in Trot. **There is no legal problem here at
all.** The problem is purely that the repo does not say so.

**Suggested replacement:**

```rust
//! Ported from the author's own earlier Python implementation
//! (`lifespan_sc110`, © 2026 Marcus Puchalla), relicensed by its copyright
//! holder under GPL-3.0-or-later for use here. Not a third-party dependency.
```

*(Confidence: high that this should be changed. Effort: one line.)*

### 9.2 P1 — Soften the opening sentence of `THIRD-PARTY-NOTICES.md`

**File:** `THIRD-PARTY-NOTICES.md`, lines 3–5.

**Current text:**

> "Trot is licensed under **GPL-3.0-or-later**. It includes work derived from
> the third-party projects below, whose copyright and license notices are
> reproduced here as required."

**The problem:** "includes work derived from" is a legal term of art. "Derivative
work" is the exact concept that triggers copyleft obligations, and this sentence
volunteers that Trot is one — for *all twelve* sources, including the AGPL one,
including the ones where the body of the same document says the opposite
("Protocol knowledge only is ported — no code", "No code was copied"). The
header contradicts the body, and the header is the part people quote.

This is not about hiding anything; the per-source descriptions below it are
detailed and honest and should not change. It is about the summary sentence
accurately reflecting what those descriptions actually say.

**Suggested replacement:**

> "Trot is licensed under **GPL-3.0-or-later**. It was built with reference to
> the third-party projects below. For most of them Trot reimplemented protocol
> knowledge independently rather than copying code — the per-project notes say
> which, in each case. Their copyright and licence notices are reproduced here
> in full, whether or not retention is strictly required, so that anyone
> redistributing Trot carries them forward."

*(Confidence: high that the current wording is unhelpful and inaccurate relative
to the document's own body; medium on the exact replacement — the owner should
phrase it in his own voice.)*

### 9.3 P2 — Add three brands to the README Trademarks list

**File:** `README.md`, lines 314–321. Add **Anplus**, **Focus Fitness** (both
appear as matched advertised-name prefixes in `ftms.rs`) and **VirtuFit**
(appears in `THIRD-PARTY-NOTICES.md`). Optionally **Woodway**, which `CLAUDE.md`
asserts is covered but which is not in the list.

*(Confidence: high. Pure housekeeping; low stakes but zero cost.)*

### 9.4 P3 — Two cosmetic staleness fixes

- **`dist-workspace.toml` line 26** comments the include as "the third-party
  (treadspan MIT) notice". There are now twelve sources. The *behaviour* is
  correct (`THIRD-PARTY-NOTICES.md` ships in every archive — good, and the part
  most projects get wrong); only the comment is stale.
- **`README.md` §Acknowledgements** credits only treadspan, from when Trot had
  one driver. Seven drivers and twelve sources later, a reader of the README
  cannot see the other eleven. Adding a line pointing at
  `THIRD-PARTY-NOTICES.md` for the full list would be both fairer and more
  accurate. **This is courtesy, not obligation** — the notices file is the
  operative document and it is complete.

### 9.5 Explicitly **not** recommended

- **Do not drop milltender** (or any other source). The attribution is honest and
  the knowledge is legitimately usable. Dropping the citation while keeping the
  knowledge would be worse in every respect.
- **Do not remove or trim the module headers** to reduce the "paper trail". The
  instinct is exactly backwards: those headers are Trot's best evidence of
  independent analysis, and they are the reason § 8.2 could be answered
  confidently. Keep them.
- **Do not add a "clean room" claim.** There was not one. Saying so would be
  false and would be far more damaging if challenged than the absence ever could
  be.

### 9.6 What is obligation vs. courtesy — so the owner knows the difference

| Practice | Status |
|---|---|
| Full MIT licence texts + exact copyright lines in notices | **Obligation** if code/substantial portions used. Trot took knowledge only → this is **courtesy**, performed to the obligation standard |
| Shipping `THIRD-PARTY-NOTICES.md` in release archives | **Obligation** if MIT/Apache code is used (notice must accompany binaries). Currently **courtesy**. Keep it — it costs nothing and it is what makes the "if a court disagreed" fallback work |
| Per-module provenance headers with file-level citations | **Courtesy**, entirely. No licence requires this granularity. High value as evidence |
| Naming the AGPL source and its licence | **Courtesy**. High value: it demonstrates awareness rather than concealment |
| Naming the unlicensed OEM spec and stating nothing was taken from it | **Courtesy**. High value for the same reason |
| Attributing the Unlicense/public-domain source | **Pure courtesy** — the Unlicense waives all conditions |
| README Trademarks section | Not a legal *obligation*, but materially strengthens an honest-practices / nominative-use position under Art. 14(2) EUTMR and *Gillette* |

**Trot's attribution substantially exceeds what any of these licences require.**
That is not a problem — over-attribution never is — but the owner should know
that if he ever needs to trim, there is room, and the things listed as "courtesy"
above are where the room is. My recommendation is to trim nothing.

---

## 10. What I could not determine / ask a lawyer about this

Ordered by how much it would matter if it went the wrong way.

1. **Whether Trot's localhost HTTP/WS API would count as "interaction through a
   computer network" under AGPLv3 § 13.** Currently moot (no AGPL code). Would
   become live and consequential if any milltender code were ever incorporated,
   or if any future dependency is AGPL. **Genuinely unsettled** — there is no
   authoritative case law on loopback-only services, and the FSF's own guidance
   does not squarely address a single-user local daemon whose API can also be
   bound to a LAN address. *(Ask a lawyer only if it becomes live.)*
2. **Whether the curated advertised-name lists could attract sui generis database
   right protection under Art. 7 of Directive 96/9/EC.** My analysis says no
   (functional selection; not exploited as a database; *Football Dataco*
   creativity threshold not met), but this is the weakest link in this analysis and
   I mark it **medium** confidence rather than high. Mooted in practice by the
   GPL-3.0 source licence, which is why it sits at #2 rather than #1.
3. **The *SAS* para. 45 residue** — whether a wire format could ever be protected
   as a work under InfoSoc Directive 2001/29 even though it is not protected under
   the Software Directive. The CJEU expressly left this open in 2012 and, as far
   as I could determine, it has not been resolved since. I could find no case
   applying it successfully to a binary wire protocol. It is a theoretical
   exposure, not a practical one, but it is real and I am not able to close it.
4. **Whether the bare-GPLv3-`LICENSE` upstreams (qdomyos-zwift, pacekeeper,
   HomeAssistantWalkingPad) are GPL-3.0-only or GPL-3.0-or-later.** I applied the
   conservative reading (only). Moot while no code is taken. If code were ever
   taken, the cheapest resolution is to ask the maintainer directly rather than
   to ask a lawyer.
5. **The provenance and confidentiality status of the FitShow OEM document.** I
   verified the mirror is unlicensed; I could not determine whether the original
   was distributed under confidentiality obligations, nor whether the mirror was
   posted lawfully. My assessment is that this does not affect Trot (no NDA,
   facts not expression, GeschGehG § 3(1) Nr. 2 permits reverse engineering, and
   the notices state nothing was taken from it) — but if the owner wants to be
   thorough before publishing, this is a reasonable thing to put in front of a
   German IT lawyer alongside item 6.
6. **Kleingewerbe / commercial-status interaction.** Trot is GPLv3, free and
   non-commercial, but the owner is forming a business and Trot sits next to a
   commercial product (Nowhere) that consumes its API. Two things I flag but
   cannot assess:
   (a) **Trademark honest-practices analysis is sensitive to commercial context**
   — nominative use in a hobby project's README and in a business's marketing are
   evaluated differently under Art. 14(2) EUTMR / *Gillette*.
   (b) **The GPL boundary between Trot and Nowhere.** If Nowhere is a separate
   program communicating with the Trot daemon over HTTP/WebSocket, the standard
   FSF position is that this is not a derivative work and Nowhere need not be
   GPL. That is the widely-accepted **industry practice** position and it is
   probably right. But "arm's-length process communicating over a documented
   protocol" is the classic GPL grey zone, and the analysis turns on facts I did
   not review (does Nowhere bundle the Trot binary? does it ship it? is the
   coupling intimate?). **This is outside the scope of this analysis and is the item
   I would most recommend putting in front of a lawyer**, not because it is
   likely to be a problem, but because it is the one with money attached.
7. **Product liability / CE-marking questions** arising from software that reads
   fitness equipment, and any consumer-protection duties attaching once a
   Kleingewerbe exists. Entirely outside this analysis's scope; noted only so it is
   not assumed to have been covered.

**A general caveat on the whole document:** I verified every licence claim
mechanically against the GitHub API and against the upstream `LICENSE` files, and
I read the notices and every driver header in full. What I did **not** do is a
line-by-line diff of Trot's Rust against each upstream's source to independently
confirm the "knowledge only" characterisation. My assessment that the
characterisation is accurate rests on reading Trot's code and headers and on the
strong structural evidence (different language, different architecture, corrected
upstream errors, systematically omitted features) — which I regard as convincing,
but which is not the same as an exhaustive similarity analysis.

---

## Sources consulted

**Statute and directives**
- Directive 2009/24/EC on the legal protection of computer programs — Art. 1(2), 5(3), 6, 8; Recital 11
- Directive 96/9/EC on the legal protection of databases — Art. 3(1), Art. 7
- Directive (EU) 2016/943 (Trade Secrets); Germany: GeschGehG § 3(1) Nr. 2
- Regulation (EU) 2017/1001 (EUTMR) — Art. 14(1)(c), Art. 14(2)
- UrhG §§ 69a(2), 69d(3), 69e, 69g(2) — <https://www.gesetze-im-internet.de/urhg/__69a.html>
- MarkenG § 23(1) Nr. 3
- 17 U.S.C. § 102(b)

**Case law**
- CJEU C-406/10 *SAS Institute v World Programming* (2 May 2012) — <https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=celex:62010CJ0406>
- CJEU C-13/20 *Top System v Belgian State* (6 Oct 2021) — <https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:62020CJ0013>
- CJEU C-5/08 *Infopaq*; C-604/10 *Football Dataco*
- CJEU C-63/97 *BMW v Deenik*; C-228/03 *Gillette v LA-Laboratories*
- *Google LLC v. Oracle America, Inc.*, 593 U.S. 1 (2021) — <https://supreme.justia.com/cases/federal/us/593/18-956>
- *Sega Enterprises v. Accolade*, 977 F.2d 1510 (9th Cir. 1992)
- *Sony Computer Entertainment v. Connectix*, 203 F.3d 596 (9th Cir. 2000)
- *Computer Associates v. Altai*, 982 F.2d 693 (2d Cir. 1992)
- *Lotus v. Borland*, 49 F.3d 807 (1st Cir. 1995), aff'd 516 U.S. 233 (1996)
- *Baker v. Selden*, 101 U.S. 99 (1879); *Feist v. Rural Telephone*, 499 U.S. 340 (1991)
- *NEC Corp. v. Intel Corp.*, 10 U.S.P.Q.2d 1177 (N.D. Cal. 1989)

**Licence texts and verification**
- GitHub Licenses API, `GET /repos/{owner}/{repo}/license`, all fourteen repositories, retrieved 2026-08-08
- Upstream `LICENSE` / `LICENSE.txt` files, retrieved and compared verbatim
- GPL-3.0 § 5, § 7(e), § 13 (this repo's `LICENSE`, lines 552–561)
- AGPL-3.0 § 13 — <https://www.gnu.org/licenses/agpl-3.0.html>
- Apache-2.0 § 4(b) — <https://www.apache.org/licenses/LICENSE-2.0>
- MPL 2.0 § 3.3
- GitHub Terms of Service § D.5 (public repository licence grant)

**Repo evidence**
- `THIRD-PARTY-NOTICES.md` (323 lines, read in full)
- `crates/trot-core/src/drivers/{mod,lifespan,ftms,kingsmith_wilink,urevo,sperax,pitpat,fitshow,util}.rs` — module headers, read in full
- `LICENSE`, `README.md`, `CONTRIBUTING.md`, `Cargo.toml`, `dist-workspace.toml`
- `cargo license` over the resolved workspace
- `git ls-files` (confirming no vendored third-party material)
