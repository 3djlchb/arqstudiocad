# Surface associative-edge DXF preservation investigation

Date: 2026-11-14. Status: investigation in progress; no codec correction
published or adopted. Current task base: main `eb6a7ee`. AcadRust pin:
`f10cc742384aa84baea9376d486cb8023a39b1e0` (including the merged SAT 400
lump/shell topology correction). The earlier measured baseline below used
main `30225fb` and pin `6e03fb1`.
ATMOS sweep-history and DIMTXSTY work belongs to the other task; the latter's
merged correction is included in this baseline. Production guards are unchanged.

## Previous measured main matrix (2026-11-07)

The extension was rebuilt with `uv sync` from the earlier `6e03fb1` Git pin. The same
34 drawings as the published DATATABLE matrix were checked: public samples
were fetched and hash-verified in this task's worktree, while the native
Xref/table inputs retain their original paths. Every input hash is checked
before and after the matrix.

Result: **34 clean reads, 63 verified saves, five declared-loss blocks and
zero semantic save errors**. All per-drawing fields match the preceding
published matrix after mapping the public corpus root and normalizing only
random temporary-directory names in refusal diagnostics. Exit 1 is intentional
while blocked losses remain. No loss-acceptance callback is used.

Evidence: `target/surface-main-matrix.json`,
`target/surface-main-matrix-comparison.json`,
`target/surface-main-matrix-summary.log`, and
`target/surface-current-main-sync.log`.

## Surface reproduction

Input: LibreDWG `2004/Surface.dwg`, actual version AC1018, SHA-256
`7ba7ea2e6e89a01094d4ffe104dd1cfc03513d1991bd223aeac1a3186a6c38d3`.
Seven ACDBASSOCEDGEACTIONPARAM objects exist; five decode. Handles `2E3`
(owner `2E2`, dependency `2E4`) and `50F` (owner `50E`, dependency `510`)
are preserved as 93-byte opaque DWG records and omitted by DXF serialization.
The product correctly blocks the resulting seven-to-five inventory loss.

A read-only diagnostic using the exact Cargo library artifact isolates both
failures: `read_edge_curve` in
`src/io/dwg/dwg_stream_readers/object_reader/associative.rs` reads action type
17 (ellipse), three vectors and four valid real values. A fifth real read
crosses the main-data boundary at bit 700: requested two bits, none remain.
The four stored scalars are the two radii and the start/end angles. The opaque
fallback prevents loss during same-version DWG preservation; it does not
supply a DXF representation. Native source bytes were not edited.

## Independent native observation so far

ProgeCAD 2026 opened a hash-identical private copy. A path-guarded read-only
AutoLISP capture resolves all seven handles. Both affected objects are ellipses
with the expected vectors/radii/angles, and ProgeCAD exposes an additional
fifth real equal to zero. This supports investigating a terminal legacy
implicit value; it does not justify accepting truncated scalars, nested curves,
or other versions without evidence.

The first capture used display precision. A second path-guarded capture uses
16 decimal places and resolves entity references to stable handles. Comparing
all seven objects in the original private copy with the initial codec candidate's
same-version DWG yields **215 property keys, 215 keys after reopening, and zero
exact-text differences** in ProgeCAD 2026. This validates these measured
properties of that earlier artifact, not the final rebased candidate or the
whole drawing. No native AUDIT acceptance is claimed.

The candidate DXF reopens with the two recovered ellipses matching, but handles
`36B` and `416` become proxies: 183 keys remain, with 72 property-key differences.
Each source object is a composite of four circular arcs. The pre-correction
codec probe already misreads `36B`'s part types as `[11, 858343468, 0, 0]` instead
of `[11, 11, 11, 11]`. This establishes that the compound-arc parsing defect
predates the ellipse correction; it is not a validated Surface DXF save.

A new external-corpus regression, committed as codec `9be42aa`, fails on this
same invalid part sequence before an arc fix. Its sample is the hash-verified
original; the drawing is not committed. Run with `ARQ_SURFACE_SAMPLE` pointing
to `2004/Surface.dwg` and the explicit ignored-test opt-in.

A native export through `vla-SaveAs` requesting DXF 2004 actually has header
AC1032 and is retained only as a modern control. A separate export through the
Save As dialog explicitly selects **AutoCAD 2004 ASCII DXF**; the resulting
`native-ui-2004.dxf` is independently checked as **AC1018**. Its arc composites
contain three stored real values per arc; the AC1032 control has a fourth zero.
Its ellipses contain four stored reals; the modern control adds a fifth zero.
ProgeCAD's in-memory `entget` exposes those extra zero values for both layouts.
This narrows the layout investigation; it does not justify silently accepting
partial scalars or unverified version promotion.

Evidence is under `target/surface-native/`: the source manifest, source and
candidate full-precision observations, `candidate-dwg-native-comparison.json`,
`candidate-dxf-native-comparison.json`, guarded helpers, and
`native-export-record-comparison-20261110.json` (hashes and actual versioned
records). Originals remain unchanged. Candidate and controls are diagnostic
artifacts, not production safe-save acceptance.

## Candidate and retest status

The local unpublished ellipse candidate preserves the four complete terminal
R2004 scalars and guards unverified DWG promotion. Its four focused tests pass
again on the rebased `f10cc74` ancestry, including explicit-value retention,
partial/truncated scalar rejection and unverified-version controls. Earlier
red commits are retained (rebased IDs `dbe2f4c` and `3588de4`).

The product worktree fast-forwarded to `e786ffa` without conflicts, preserving
its documentation. The codec test commits were rebased onto `f10cc74` without
conflicts, preserving the candidate source diff. A new product extension build
and complete codec suite were started on 2026-11-10; neither is yet recorded as
passed here. The 34-drawing matrix must be repeated after that build. The
local combined candidate now probes only a complete flat four-arc AC1018
record on a cloned reader, retaining its three stored scalars per arc. It
accepts the probe only with no read error and an exact main-record boundary;
other layouts use the existing reader. Its unverified modern DWG promotion is
blocked. The new compound-arc and promotion reproductions are codec commits
`9be42aa` and `85ddcf2`; the latter fails before the guard. Explicit fourth
scalars across five versions have an additional regression control. Combined
tests and fresh native artifacts are still pending; this is not an adopted
codec fix.

## Reference review

Open CAD Studio `5da54173512735d7056afeebc6dd3bf45cde321d` pins
opencaddocument `49ec1ce3e9c6761b9380a32cd056f88903f9d4a0`, whose actual
codec is upstream AcadRust `b583abed0e6873f097edb64ce701b60a5ebdfc62`.
Its lockfile confirms the same revision. Current upstream HEAD inspected:
`008c6391bbb5deb7baf4bd44495b7b1cfc53eceb`.

Both inspected readers still expect five ellipse scalars. They contain a
narrow terminal AC1024 arc exception, guarded against nested curves, partial
scalars and prior errors; this does not resolve these AC1018 ellipses.
Reference reader/writer sources, manifest chain, hashes and MPL-2.0 license
are retained under `target/surface-reference-review`. No reference source has
been copied wholesale. The terminal-boundary guard design is informed by the
exact upstream arc precedent, adapted independently for the measured R2004
ellipse; provenance is recorded in `THIRD_PARTY.md`. No new dependency is
adopted by this investigation.

## Remaining acceptance gates

- Complete the current-pin matrix and full codec retest.
- Correct the reproduced compound-arc parsing defect with bounded layout tests.
- Establish the precise native legacy representation and version scope.
- Validate any scoped correction with regression tests, repeated DWG/DXF
  conversion and independent full-precision native observations.
- Repeat the paired corpus and product checks before publishing or adopting
  a new codec pin. Keep the other task's accepted fixes in its ancestry.

The other four matrix blocks remain the dynamic-block purge-preventer DXF
omissions. Wider interoperability and milestones 03.4/03.5 remain open.

## Windows continuation (2026-11-14)

Agent: Codex, Windows development and independent ProgeCAD validation. The
maintainer agreed to finish Surface before taking a new task. Scope: the
measured R2004 ellipse/composite layouts and Plane/Revolved surface action
bodies; no Xref loading, ATMOS history work or acceptance-policy relaxation.
Expected areas are the codec associative readers/writers, their regression
tests, product provenance, roadmap and this evidence document. Integration
depends on completing the codec tests, sabotage checks, paired corpus, native
reopening and product acceptance before adopting a new immutable pin. No
other open product/fork PR reserved Surface at the initial GitHub check.

The product worktree fast-forwarded to `eb6a7ee`, retaining the published
`f10cc74` pin. A documentation conflict retained both the newly integrated
command references and the Surface provenance. Main itself was not changed.
The rebuilt baseline matrix again reports **34 clean reads, 63 verified
saves, five declared-loss blocks and no semantic save errors**, with no
changed rows and every source hash unchanged. The nine native exercise inputs
were located after a directory rename and verified against their earlier
hashes; no private drawing was copied. Evidence:
`target/surface-main-20261114-matrix-comparison.json`. The matrix's exit 1
continues to mean declared-loss blocks, not an unhandled semantic error.

The current unpublished codec candidate passes all **11 focused tests**
(5 ellipse, 3 native action body, 3 composite), with the public Surface
fixture hash checked before execution. This includes preserving an explicit
R2004 Plane parameter trailer and refusing unverified legacy scalar promotion
in DXF as well as DWG. Fresh diagnostic DWG, DXF and DXF-to-DWG artifacts
retain seven decoded edge parameters and the Plane dependency `50D`/marker 7
in codec read-back. Those checks do not establish independent native validity.
The full current codec suite and fresh ProgeCAD captures remain in progress.

Product baseline formatting, Clippy, Rust tests and cargo-deny pass on the
updated base. Python acceptance passes 1,066 tests, with three explicitly
skipped; Ruff, format checking, mypy and stubtest also pass. These results precede any
codec adoption and must not be presented as candidate product acceptance.
Temporary continuation scripts and logs are outside the repository.
