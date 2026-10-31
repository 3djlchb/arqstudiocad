# Native table margins and text heights: DWG to DXF

Date: 2026-10-30. Status: correction published; Git-pinned application acceptance passed.
Application base: main `11fe628` (including PR #99). Codec base: immutable
`a1a9a56965d67230295777d3dc419edc9b6571b3`, AcadRust 0.6.8 plus private patches.
Both working copies use a new `fix/table-dxf-format` branch. Production main
and original test drawings remain untouched.

## Reproduced problem

The existing native corpus records 88 differences across 249 measured properties
after exporting `tables-native-ac1032.dwg` to DXF. Fresh ProgeCAD 2026 reopening
of the unchanged baseline reproduces all 88: eight margins, twelve row-type text
heights, eight merged-title text values, and sixty complete cell records.
The new codec tests fail before the fix for both modern margins and row-type
text heights, including the actual native source. The red log is retained under
`target/table-dxf-red.log` in the codec working copy.

## Proven causes and correction

Modern DWG tables retain base and row styles directly. The DXF entity writer
serializes a legacy ACAD_TABLE projection and originally looked only at
`legacy_style_override`; it omitted those modern instance margins and heights.
A projection now emits asserted symmetric margins and uniform row-type heights
in the existing legacy fields, without mutating the source. Unasserted defaults,
asymmetric margins and conflicting heights within a row type are not collapsed
into a single table-wide override. Existing explicit modern values take
precedence over stale legacy projections; legacy-only tables retain their data.

Native reopening proved that this projection alone corrects the margins but
is insufficient for ProgeCAD's row-type text-height getter. ProgeCAD's own export
preserves the modern format in an owned TABLECONTENT referenced by
`ACAD_XREC_ROUNDTRIP`, section `ACAD_ROUNDTRIP_2008_TABLE_ENTITY`. A diagnostic
SetTextHeight experiment changed the native getter while leaving the captured
legacy prefix identical. Changing the legacy table-data flag to the native
value also failed to restore the heights. These diagnostic artifacts remain
under ignored target directories.

The writer now retains a complete native TABLECONTENT companion when exporting
a modern table to DXF 2010 or later. It uses the already-supported object and
XRecord encodings, attached through the table's owned extension dictionary.
The generated handles belong only to the writer's prepared copy, before class
registration and handle enumeration. A foreign dictionary or any pre-existing
round-trip record is left untouched. Rewriting the resulting DXF does not
create duplicate companions. This does not add a dependency, a new product
model or table editing support.

Implementation in the AcadRust fork:

- `src/io/dxf/writer/section_writer/table_format.rs::with_legacy_format`.
- `src/io/dxf/writer/section_writer.rs::write_acad_table`.
- `src/io/dxf/writer/table_roundtrip.rs::prepare`.
- `src/io/dxf/writer/mod.rs::write_dxf`.
- `tests/table_dxf_format.rs`: synthetic, native and binary regressions.

## Independent ProgeCAD evidence

The unchanged product helper `tools/progecad_table_corpus.lsp` captures four
tables and the same 249 properties. `tools/compare_progecad_tables.py` retains
strict comparison and document-name validation. Each observation uses a fresh
path; none overwrites an earlier capture. The baseline and candidate drawings
were reopened in ProgeCAD rather than merely read back by the codec.

| Candidate stage | Margin differences | Row text-height differences | Merged-title text differences | Cell-record differences | Total |
|---|---:|---:|---:|---:|---:|
| Production baseline a1a9a56 | 8 | 12 | 8 | 60 | 88 |
| Legacy projection alone | 0 | 12 | 8 | 60 | 80 |
| Complete modern-style companion | 0 | 0 | 0 | 60 | 60 |

All 189 non-cell-record properties match the control after the complete fix.
The base and merged tables retain margins 1.5 and text heights 2.5; MARGINS
retains margins 4.5/3 and text heights 3; BORDERS retains margins 2/2 and text
heights 4. All eight previously missing merged-title values are recovered.
No exception or numeric-normalization rule was added to the comparator.

The remaining sixty cell-record differences include the explicit per-cell
text-height flag/value and CELL_VALUE flags. Effective inherited row formatting
is now retained, but this result is not a 249/249 raw-record match. Further
cell-level fidelity investigation remains separate; milestones 03.4/03.5 stay
open. Visual/semantic equivalence of every unmeasured property is not claimed.

## Regression checks and reproduction

The focused suite passes twelve tests, including ownership, caller
immutability, repeat-export idempotence, existing vendor-record preservation,
modern/legacy precedence, helper-object reactor isolation, asymmetric-margin
and distinct-row protection,
unasserted defaults, and ASCII/binary DXF 2010/2013/2018. The native case checks
all four tables' expected margins and heights.

```powershell
$env:ARQ_NATIVE_TABLE_ROOT = 'D:/PROGRAMACION/EJERCICIOS DWG/Tables-ProgeCAD-20261026/source'
cargo test --test table_dxf_format -- --include-ignored
```

Optional `ARQ_TABLE_DXF_OUTPUT` writes a fresh candidate DXF with create-new
semantics. Reopen that file in ProgeCAD, load the product's existing LISP audit
helper, and capture a new TSV. Use the native `control-ac1032.tsv`, the exact
observed candidate name (including the read-only suffix, if present), and the
unchanged comparator. A nonzero comparator exit remains expected while the
sixty cell-record differences persist; its complete report must be inspected.

The source DWG SHA-256 is
`6f4050e24aaa4413e46e290cc4a3fdcd4f39f1c8783c566c8002a00fe5705678`.
The retained codec evidence includes `native-baseline-20261030.tsv`,
`native-companion-candidate.tsv` and their strict comparison JSON files.
Full-suite, corpus and integrated-application results are recorded below.
The initial checks below used a temporary local Cargo path. The integration
branch now pins the published Git revision; its acceptance is recorded separately.

## Format references

The public DXF references describe horizontal/vertical margins and row-type
text-height overrides; the complete companion linkage was established from
native ProgeCAD files, not inferred from our own reader:

- [Autodesk DXF 2014 reference](https://images.autodesk.com/adsk/files/autocad_2014_pdf_dxf_reference_enu.pdf), TABLE group codes.
- [Autodesk TableStyleOverrides](https://help.autodesk.com/cloudhelp/2018/ENU/OARX-RefGuide/files/OREF-AcDbTable__TableStyleOverrides.html).

No external implementation or documentation text was copied into the codec.

## Baseline comparison bench

The same wrapper compares the production codec `a1a9a56` with this candidate in
separate child processes, on 25 public samples, six native Xref drawings and
three native table sources. Both codec builds use the same unoptimized profile;
concurrent validation means timings are not interpreted as performance evidence.

| Profile | Jobs | Paired operations | Changed success states | Changed summaries |
|---|---:|---:|---:|---:|
| Preserve source version | 476 | 238 | 0 | 2 |
| Force DWG AC1032 | 464 | 232 | 0 | 2 |

In each profile, both summary differences are reads of the corrected AC1032
DWG-to-DXF output, by the candidate and by the baseline reader. They show exactly
four additional TABLECONTENT objects, four dictionaries and four XRecords.
Entity/type counts, geometry aggregates, recovery state, other object counts
and the Map proxy remain unchanged. No other source/output summary differs.
All source-version jobs succeed. Forced AC1032 retains the same three write
refusals in each codec: the custom edge-action parameter in Surface.dwg and
DATATABLE objects in example_2000.dwg / example_r14.dwg. These are the existing
version-conversion guards, not new failures.

Reports are under `target/table-dxf-bench-{source,2018}-{public,xrefs,tables}`;
`target/table-dxf-benchmark-summary.json` pairs every operation and retains the
complete changed-summary and refused-write evidence.

The final fixture DXF is byte-identical to the one independently reopened in
ProgeCAD, SHA-256
`f046f2f8ff894554267118798bff9d347da68e28d356af88272114b3e020f85c`.
A separate regression ensures the new helper object never inherits entity
reactors; those remain attached to the original entity. That final metadata
correction does not change this native fixture's output bytes.

## Integrated safe-save artifact

The rebuilt application's public `CadApplication` / `CadDocument.save` API opens
the unchanged native source and writes a fresh
`target/table-format-product-candidate.dxf`. Strict read-back succeeds without
recovery or consent to loss. That output is byte-identical to the independently
ProgeCAD-validated codec artifact, including SHA-256 `f046f2f8...20f85c` above.
The original native source hash is unchanged.

The complete paired corpus was repeated after the helper-reactor correction.
Final reports use `target/table-dxf-final-bench-*` and
`target/table-dxf-final-benchmark-summary.json`; all final paired outcomes and
summaries agree exactly with the earlier reviewed comparison. The separate
metadata regression covers the case not present in the native source.

## Final acceptance and publication state

Published codec commit: `f1b0980d181fa64d36d5740dd8140f656f16b9fb`, parent `a1a9a56`.
Published immutable fork tag: `arqstudiocad/on-ea66abe-table-dxf-format`.
Remote annotated tag object: `25bd6a70988b4f874e3f88c9ef4d0df387c79f7b`;
its peeled commit matches the full revision above. The integration branch
replaces the temporary path with that exact Git revision, retaining `=0.6.8`.
Provenance, the ADR log and roadmap describe the same scope. Git-fetched build
acceptance passed as recorded below. The maintainer creates the PR manually.

| Check | Result |
|---|---|
| Full default codec suite, including doctests | 2,420 passed, 0 failed, 87 optional tests ignored |
| Focused new formatting suite, including the native fixture | 12 passed |
| Existing native Map integration regressions | 4 passed |
| Untouched native Map DWG record regression | 1 passed |
| Existing native table-instance regressions | 6 passed |
| Existing native DWG 2007 table regressions | 2 passed |
| Product `cargo test` on main base 11fe628 | 488 passed, 0 failed, 12 optional tests ignored |
| External core corpus groups | 4 public codec groups and 1 local six-file Xref group passed |
| Optional document tests | 7 passed |
| Full Python suite (`QT_QPA_PLATFORM=offscreen`) | 753 passed, no skips, 9 existing warnings |
| Ruff check / format check | Passed; 224 files inspected by formatting |
| mypy / stubtest | Passed; 142 source files / one extension module |
| Workspace/all-target Clippy with warnings denied | Passed |
| Cargo formatting and cargo-deny checks | Passed |
| Integrated 34-drawing matrix | 34 clean reads; 58 verified saves, 8 blocked losses, 2 known safe errors |

The matrix tool exits 1 because the two pre-existing R2000 TABLESTYLE ownership
errors remain in the report; this is not an all-green conversion claim. No save
guard, loss allowance or comparator rule was relaxed. Corpus and property
measurements are local Windows validation, not Linux/macOS CI or release CPU-floor
coverage. The full default suite is the required fork acceptance here; a new
all-features run was not needed for this non-feature-gated writer change.

Evidence hashes are recorded in `target/table-dxf-evidence-manifest.json` in the
application working copy. Private drawings, native captures, generated CAD files
and raw logs are not committed.

## Published Git-pin acceptance

The published revision was fetched by Cargo and `uv sync` rebuilt and installed
its extension successfully (10 minutes 23 seconds on this run). The manifest
and lockfile pin the full commit above, with no temporary local path. Comparing
all 233 lockfile package entries proves only AcadRust's Git source changed.

- `cargo test --locked`: 488 passed, zero failures, 12 optional tests ignored.
- Four public core corpus groups, the six-file local Xref corpus group and all
  seven optional document tests passed, including native unloaded-reference data.
- Full Python suite on the rebuilt extension, with `QT_QPA_PLATFORM=offscreen`:
  753 passed, no skips, nine existing warnings (102.97 seconds).
- Cargo formatting, locked workspace/all-target Clippy with warnings denied,
  cargo-deny, Ruff check/format, mypy (142 files) and stubtest (one module) passed.
- Repeated matrix: 34 clean reads, 58 verified saves, eight blocked losses and
  the same two R2000 TABLESTYLE-owner errors. Every full source path, source hash,
  read inventory and save outcome matches the local-candidate report; only
  randomized temporary-directory names in error messages were normalized.
- The actual published-build safe-save output,
  `target/table-format-git-product-candidate.dxf`, has the same full SHA-256
  `f046f2f8ff894554267118798bff9d347da68e28d356af88272114b3e020f85c` as the
  independently ProgeCAD-validated artifact. The original source is unchanged.

The matrix's exit status 1 remains expected for its two reported safe errors;
all 34 records and 68 outcomes were inspected. No new native reopening or paired
codec benchmark is claimed after publication: those checks already tested this
identical committed codec source. Git-pin evidence uses the
`target/table-dxf-git-*` prefix and its hashes are added to the evidence manifest.
No product code, guard, comparator or loss-consent rule changed in this adoption.

## Subsequent per-cell investigation

PR #101 merged this correction. The [2026-10-31 cell-cache follow-up](progecad-table-cell-fidelity.md) corrects the demonstrated inherited cell-height and shared-border losses and distinguishes raw representation differences using a fresh native DXF control. The measurements above remain the historical baseline for this earlier codec revision; the follow-up records the new published revision and its separate acceptance.
