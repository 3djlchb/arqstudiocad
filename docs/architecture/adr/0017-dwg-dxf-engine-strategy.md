# 0017. DWG/DXF engine: AcadRust as the base, behind a contract of ours

Date: 2026-10-13 · Status: Accepted (interoperability validation pending) ·
Amends [0008](0008-opencadcodec.md) · Evidence:
[DWG/DXF library comparison](../../references/dwg-library-comparison.md)

## Context

ADR 0008 made opencadcodec the DWG/DXF engine. The comparison of
2026-10-13 (220 drawings, 6,600 operations, each library in its own
process) found that opencadcodec is a fork of **AcadRust** (crates.io,
MPL-2.0), close to AcadRust 0.6.1, while AcadRust has moved on to 0.6.4;
each line has work the other lacks. GNU LibreDWG, measured on the same
files, read no drawing the codecs could not, writes weak DXF and marks most
dynamic-block and associative classes unstable.

The comparison also found two ways to lose data silently: converting a DWG
to another version drops the custom objects the codec cannot re-encode
(opencadcodec; AcadRust refuses instead), and a truncated DWG opens as an
empty drawing with no error (both codecs).

## Decision

### D1 — AcadRust is the production base *(maintainer)*

The DWG/DXF adapter is built on **AcadRust from crates.io**, pinned to an
exact version (`=0.6.4`, the version measured; `=0.6.5` since 2026-10-20,
see the log). Upgrades are deliberate:
each runs the round-trip corpus and the comparison bench
(`tools/dwg-bench`) before it is accepted, and the version, its results
and the date are recorded in this ADR's log below.

### D2 — opencadcodec's improvements are recovered one by one

No wholesale merge of the two code bases. The concrete differences are
listed (the comparison's annex and section 12) and each valuable one is
brought in as a small change with a regression drawing that proves it:
first the **geometric-constraint groups** (`ACDBASSOC2DCONSTRAINTGROUP`),
then entity-fidelity fixes and diagnostics. Our changes live in **our own
fork** of AcadRust ([3djlchb/acadrust](https://github.com/3djlchb/acadrust)),
used through `[patch.crates-io]` pinned to a commit, with its origin and
license recorded in `THIRD_PARTY.md`. *(Amended 2026-10-13, maintainer:)*

- The fork's `main` **follows the original** (`hakanaktt/acadrust`), as a
  mirror with none of our changes.
- **Our features live only in the fork**, on top of the AcadRust version
  we use. They are not sent upstream. While an upgrade is prepared they sit
  on a branch (`arqstudiocad/on-<base>`); once ArqStudioCAD pins it, the
  branch becomes an annotated **tag** of the same name, so the fork keeps
  only `main` and every commit a build ever pinned stays reachable
  *(maintainer, 2026-10-24; before, one branch per base)*. Tags today:
  `arqstudiocad/preserve-undecoded` (0.6.4) and `arqstudiocad/on-2b12197`
  (after 0.6.5).
- We take the original's changes **only when they favour us** (a fix or
  feature we need): our branch then moves to that version, re-applying our
  changes, and its regression drawings, the round-trip corpus and the
  comparison bench must pass before it is adopted (D1).

### D3 — LibreDWG stays out of the product

LibreDWG is not linked into the application. It stays an optional
reference in `tools/dwg-bench`, and is reconsidered only if drawings appear
that AcadRust cannot process and LibreDWG can.

### D4 — The model is ours; the codec is an adapter

The rule of ADR 0008 (codec types never leave `arq-document`) becomes an
explicit contract of `arq-document`, so the engine can be replaced without
touching the core, BIM data or the interface:

| Operation | Contract |
|---|---|
| `read_document` | read a file; return the document **and its diagnostics** (records skipped or recovered, whether it is partial) |
| `write_document` | write with an **explicit target version** |
| `inspect_capabilities` | which entities and operations the engine supports, as the core uses them |
| `validate_roundtrip` | what survives writing and reading again |

These are our contract's names, not the library's.

### D5 — Four mandatory protections

1. **Damaged files are reported.** A file read partially, or one that
   reads as empty although it has content, yields an explicit diagnostic;
   it is never taken for a valid empty drawing, and it cannot be saved over
   the original without the user's confirmation.
2. **Safe save.** Write to a temporary file, read it back, validate it
   against the document; only then replace the original. On failure the
   original stays untouched.
3. **Unknown objects are preserved.** Their original data, source version
   and identity are kept. If a version conversion cannot keep them, the
   user is warned and can cancel; a loss is never silent. Some proprietary
   objects cannot survive a version change at all: then the conversion is
   stopped, not promised.
4. **Regression drawings.** Every improvement brought in from opencadcodec,
   and every protection, has a drawing that shows it working and that the
   rest still round-trips.

### D6 — Dynamic blocks and constraints: three levels, our engine

The adapter reads and writes parameters, actions, references and
associative data; the Rust document model represents and keeps them; an
ArqStudioCAD **parametric engine** evaluates constraints, visibility,
stretches and dependencies; the interface edits parameters. Three levels of
compatibility are distinct: **keeping** a dynamic block's definition,
**showing** its current state (the evaluated block stored in the file) and
**recomputing** its geometry when a parameter changes. Open CAD Studio
(GPL-3.0) is a technical reference for the third level: we learn from its
design; we do not copy modules without reviewing their dependencies and
obligations.

### D7 — Licenses

The product stays GPL-3.0-or-later. AcadRust's files keep MPL-2.0, and so do
our changes to them (MPL-2.0 file-level obligations); every dependency and
every change brought in is recorded in `THIRD_PARTY.md`, after checking the
copyright notices of the specific files.

## Pending

- **Interoperability:** the comparison's output files, opened in an
  independent CAD application, must confirm the measurements before this
  decision's validation is closed.
- **Xrefs and BIM drawings** (AutoCAD Architecture, Civil 3D, Revit and
  ArchiCAD exports) are not yet in the corpus.

## Consequences

- One Rust engine, published and maintained upstream, with our own fixes
  kept in our fork; upgrades cost re-applying them.
- The editing work of milestone 2.5 is re-validated on AcadRust before it
  goes on.
- Saving costs a re-read; for the drawings measured it is milliseconds.

## Log

| Date | Engine | Corpus | Notes |
|---|---|---|---|
| 2026-10-13 | AcadRust 0.6.4 (measured) | 220 drawings, all three libraries | comparison; decision taken |
| 2026-10-13 | AcadRust 0.6.4 + our patch `b2358ab` ([fork](https://github.com/3djlchb/acadrust/tree/arqstudiocad/preserve-undecoded)) | round-trip corpus; LibreDWG `Constraints.dwg` R14–2018 | 03.2: the fork's constraint-group reading turned out to be garbage for R14–2010 (old layout); instead, class objects that fail to decode are kept verbatim, so those drawings open intact and keep their constraints byte for byte in a same-version save. Decoding the old layout is pending. Our changes stay in our fork, not sent upstream (D2 amended). |
| 2026-10-13 | AcadRust 0.6.4 + our fork at `f250d80` | LibreDWG `Surface.dwg` (2004), `ACEB10.dwg` (R11), `ATMOS-DC22S.dwg`, `example_*.dwg` | 03.2, checked against independent evidence: the fork read the 2004 lofted surface correctly (AcadRust 0.6.4's issue-#129 change skipped its binary extra modeler block; our fork allows only that self-validating probe); its "recovery" of ACEB10 was an empty drawing, while AcadRust mis-sized R11 linetype indices (16-bit, as LibreDWG reads them; fixed, 1,593 entities). Their round trip found engine bugs still to fix: a RAY rewritten in R14/2000 DWG, ACIS bodies and RAY/TOLERANCE ids in DXF output; the safe save refuses those saves. |
| 2026-10-14 | AcadRust 0.6.4 + our fork at `7cbde43` | the seven LibreDWG drawings, full round trip | Three of the four bugs the round trip found were ours, not the engine's: a RAY's far-out box compared with an absolute tolerance (now relative), and ACIS bodies boxed from a wireframe cache DXF does not keep (now boxed and compared by their ACIS vertices, equal in SAB and SAT). The engine's: a TOLERANCE's line separator written as `\P` in DXF (now `^J`) and its box collapsing on a zero stored height. All seven drawings now round-trip. |
| 2026-10-20 | AcadRust after 0.6.5 (the original's `2b12197`) + our fork at `5f473cc` ([branch](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-2b12197)) | round-trip corpus (codec samples, the LibreDWG drawings, the maintainer's drawings); bench, 220 drawings, AcadRust only, old fork against new | Upgrade taking the original's R11/R12 DWG support, truncated R13–R2000 recovery and decoded geometry dependencies. Bench: 5 operations better (an R1.4 DXF read, two DWG saves with geometry dependencies), none worse, same time and memory. Our R11/R12 linetype-index patch dropped (the original reads them so); kept: class objects kept verbatim (the original preserves only a few auxiliary classes, now notes too), surfaces' extra modeler block, TOLERANCE in DXF; new: R11/R12 current layer, style and linetype names, and DXF hatch elliptic edge angles in degrees (the original kept them so in the model, where radians are expected). Not taken: the original's newest commit `daaa37a`, whose mesh limits, scaled from the machine's memory, make reading a maintainer's drawing ask for 40 GB. The engine's explode now follows AutoCAD (since `c08fc93`: a SOLID does not explode, a HATCH explodes into its pattern, and on that drawing exhausts memory by scaling pattern lines already scaled), so hatch boundaries are outlined in `arq-document` and hatches are never exploded; ellipses are drawn from points on them (the codec's spline approximation drew them 0.5 % small), and what explode keeps whole (SOLID, SPLINE, 3DFACE, meshes) goes through the codec's `flatten`; a clockwise hatch edge stores its angles mirrored (every clockwise arc of the maintainer's drawings meets its neighbors only so), which both explodes got wrong; R11 `ACEB10.dwg` is now read in paper space, as the file says. |
| 2026-10-24 | AcadRust 0.6.7 (the original's `c5555a5`) + our fork at `955bdd8` ([branch](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5), a tag once pinned) | AcadRust's own tests (2,279, none failing); round-trip corpus (codec samples, LibreDWG drawings, the maintainer's drawings); bench, 217 drawings, AcadRust only, old fork against new | Upgrade taking 110 of the original's commits, among them the 38 improvements it merged from opencadcodec (D2's one-by-one recovery now arrives through the original): old-layout (R14–2010) constraint groups and solid history (`ACSH_*`) are decoded, so nothing is kept undecoded in those drawings any more and they save in DWG and DXF without loss; bounded DWG allocations (the maintainer's "Cimentación de zapata", 40 GB with `daaa37a`, reads normally); decode failures returned as errors instead of panics (a damaged file is reported once); AutoCAD Architecture classes read, and left out of a DXF save, which then asks before losing them; DXF elliptic hatch edges stored as circular angles (AutoCAD's rule, replacing our degrees patch). Bench: 6 operations better (DWG saves of the constraint drawings and ATMOS), none worse, 41 % less time, same peak memory (120 MB). Kept from our fork: class objects kept verbatim in every read (the original rejects them in a strict read; its test adapted), surfaces' binary extra modeler block (beside the original's own R2004 probe), TOLERANCE in DXF, R11/R12 current names. The fork keeps only `main` (D2 amended): our earlier branches are tags. |
| 2026-10-25 | AcadRust 0.6.7 + fork `1ca0ae2` ([immutable tag](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-xref-unloaded)) | Fork suite: 2,282 passed; 31-drawing baseline/patch comparison: 422 jobs, no new status/summary differences; public and six-DWG local product corpora pass | Preserve the explicit unloaded-reference marker (BLOCK group 71=1) in DXF reading and writing. Native ProgeCAD 2026 AC1027 exports and reopening establish the encoding; unresolved flags alone are not treated as unload state. Three synthetic codec tests cover 68 version/mode/state/encoding round trips; the product keeps its reference-level save guard and corrupt-marker refusal test. Other lockfile dependencies are unchanged. [Evidence and scope](../../testing/progecad-dxf-unloaded.md). |
| 2026-10-25 | AcadRust 0.6.7 + fork `59aaf4c` ([immutable tag](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-xref-table-conversion)) | Fork: 2,284 passed, zero failures; 31 drawings / 422 benchmark jobs, zero new status/summary differences; native ProgeCAD DWG and DXF reopening | Retire a legacy CELLSTYLEMAP only after exact full-cell equivalence and ownership checks. The product independently proves conversion before normalizing its inventory and rejects corrupt or broken companions. Final native outputs retain all 90 compared style properties, margins 1.5 and unloaded Xref state. Actual TABLESTYLE loss in two R2000 fixtures remains safely refused. No blanket DATAOBJECT allowance; [evidence and limitations](../../testing/progecad-dxf-unloaded.md). |
| 2026-10-27 | AcadRust 0.6.7 + fork `179aaeb` ([immutable tag](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-table-2007-reader)) | Fork: 2,292 default tests passed; 34 drawings / 464 comparison jobs, no new status regressions; native ProgeCAD table reopening | Complete the measured DXF-to-DWG table-instance margins, row formatting and right borders from owned TABLECONTENT; distinguish native grid indexes. Decode/write legacy border properties by independent presence bits and omit virtual-edge payload. The native AC1021 source now reads and rewrites all four tables strictly, without recovery; codec and integrated safe-save outputs match all 249 native measurements. Public/Xref benchmark summaries are unchanged; the sole improved summary is the AC1021 read (three recovered tables to four clean tables). Updated-main local integration: 462 default Rust tests, five external core corpus groups, six optional document tests and 614 Python tests passed. Resource-manager and older TABLESTYLE-owner refusals remain separate limits. [Formatting evidence](../../testing/progecad-table-instance-formatting.md), [reader evidence and scope](../../testing/progecad-table-2007-reader.md). |
