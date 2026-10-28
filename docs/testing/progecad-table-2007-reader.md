# Native DWG 2007 table reader correction

Date: 2026-10-27. Corrected fork commit: `179aaeb0268d4f687406d6d46eb15bf8f9c60700`.
Follow-up to [the native table corpus baseline](progecad-tables-results.md)
and [the separate DXF-to-DWG format correction](progecad-table-instance-formatting.md).
[Hashes and measured results](progecad-table-2007-reader.json).

## Reproduction and proven cause

The original `tables-native-ac1021.dwg` contains four 5-row, 3-column tables.
AcadRust at `0d438b6` refuses a strict read of BORDERS (handle 0x2FA); failsafe
reading exposes only three tables. The reported truncation at bit 63057 is a
later consequence of an earlier stream displacement, not evidence that the
native source is damaged. ProgeCAD independently opens all four tables.

The first wrong read occurs in BORDERS cell index 4, whose legacy override
mask is 0x41120. It supplies a bottom-border lineweight without a visibility
payload. `read_table_cell_data` incorrectly consumed visibility with the
lineweight bit, then interpreted subsequent values at the wrong bit offset.
A second displacement occurs on shared virtual edges: they carry no local
border payload even when their property bits are set. The native central
border cell has virtual-edge mask 9 (top and left).

The [ODA DWG format reference](https://www.opendesign.com/files/guestdownloads/OpenDesign_Specification_for_.dwg_files.pdf),
version 5.4.1, printed page 223, documents independent visibility bits and
virtual-edge payload omission. This is a format reference; no ODA source code
was copied. Native stream tracing and independent reopening establish the
behavior of this fixture.

## Correction and scope

In the fork's `src/io/dwg/dwg_stream_readers/object_reader/entities.rs::read_table_cell_data`,
color, lineweight and visibility now use their separate presence bits. Virtual
edges consume no border payload. Existing CellBorder override metadata retains
which individual properties were actually present.

`src/io/dwg/dwg_stream_writers/object_writer/entities.rs::write_table_cell_data`
uses the same native presence/virtual-edge rules. Older DXF/API styles with a
combined weight/visibility mask receive an explicit native visibility bit when
needed, preserving their existing meaning.

`src/entities/table.rs::TableCell::binary_layout` preserves the exact border
property metadata read from native flat cells during modern conversion and
omits virtual edges; its existing projection for older DXF/API masks remains.
There is no alternate retry parser, boundary extension or recovery allowance.
The strict reader and damaged-record checks remain active.

## Independent result

The fixed reader opens **four tables in strict mode, without recovery**. The
native regression checks all 60 stored cell texts, column widths, row heights,
merge spans and the red 0.50 mm right borders. It rewrites a fresh AC1021 DWG
and reads all four tables again strictly, preserving the checked cell styles.

ProgeCAD 2026 independently reopened `review/table-2007-fixed.dwg`.
The unchanged strict capture/comparison tools measured **249 properties with
zero differences** against `control-ac1021.tsv`. The original source hash is
unchanged. Only a new review copy and observation capture were created.

## Regression coverage

- Two independent binary-layout tests in the reader module exercise all four
  edges with each of seven nonempty property combinations, plus virtual shared
  edges. Main-data, text and handle sentinels expose a shifted read; the test
  does not use ObjectWriter to construct the format being checked.
- `tests/table_2007_native.rs::flat_dwg_border_presence_survives_rewriting`
  checks weight-only, visibility-only and combined overrides in three versions.
- Its optional `native_2007_reads_all_four_tables_strictly` reads the private
  source and validates the strict rewrite against native DXF measurements.

```powershell
$env:ARQ_NATIVE_TABLE_ROOT = 'D:/PROGRAMACION/EJERCICIOS DWG/Tables-ProgeCAD-20261026/source'
cargo test --lib legacy_table_
cargo test --test table_2007_native -- --include-ignored
```

ARQ_2007_TABLE_OUTPUT optionally writes a DWG to a fresh destination, refusing
to overwrite any existing file. Original private drawings are not committed.
This bounded reader correction does not close milestones 03.4/03.5 or resolve
the separate resource-manager refusal, older TABLESTYLE-owner refusals or all
same-format DXF border encodings.

## Full fork and comparison checks

The default fork suite passed **2,292 tests**, with zero failures and 84
optional/documentation tests ignored. The focused native read/rewrite run passed
two tests, and all six previous table-instance formatting regressions passed,
including the native DXF-to-DWG fixture. ProgeCAD reopened the latter output
again with both fixes present: all 249 properties still match the native DWG
control. The manifest records this additional capture separately.

The release comparison against `0d438b6` ran **464 jobs / 232 paired operations**
on 34 drawings (25 public, six Xrefs, three native tables). Public and Xref
success states and summaries are unchanged. The only changed paired summary is
the original AC1021 read: four model tables instead of three, recovery false
instead of true, 96 total entities instead of 95, and 23 diagnostics instead of
25. Cached LINE/MTEXT counts are unchanged. No job changes success status.
The same three forced-AC1032 write refusals per library remain. These are
status/inventory comparisons, not timing or broad semantic-fidelity claims.

## Local integration with updated main

The product working copy includes main `4200286`. It was rebuilt with the
exact local fork source at `179aaeb`; the native AC1021 source opens through
CadApplication as four entities without recovery. Its normal safe-save API
writes AC1021 DWG, and the saved copy reopens as four entities without recovery.
ProgeCAD reopened this integrated output (`table-2007-product-local.dwg`):
**249/249 properties match the native AC1021 control**. The source hash is
unchanged. This capture is distinct from the low-level codec output above.

Local updated-main checks passed: **462 default Rust tests** (11 optional
ones ignored in that invocation), **five external core corpus groups**, all
**six optional document tests**, **614 Python tests** (nine existing warnings),
workspace Clippy with warnings denied, Rust formatting, Ruff (197 files), mypy
(122 source files), stubtest (one module) and dependency checks. Cargo-deny
reports the allowed Git source as unused while the temporary local path is
being tested; licenses, advisories, bans and source checks pass.

The integrated three-source matrix now reads all three inputs cleanly, with
four tables each. Five saves verify strictly. The remaining AC1032 DWG-to-DXF
resource-manager loss is still refused, so the strict matrix correctly returns
exit 1 rather than accepting that unrelated loss.

The corrected commit is now published as the annotated immutable tag
[arqstudiocad/on-c5555a5-table-2007-reader](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-table-2007-reader),
with explicit maintainer authorization. The product dependency pins its exact
Git revision; the temporary local-path override is removed. The suite counts
above were measured with the same exact source revision in the local candidate.
The final Git-source build/runtime checks are recorded separately below.
Main receives the correction only when the product branch is merged.

## Final immutable Git-pin verification

The product was rebuilt with `uv sync --locked` from the published Git revision
`179aaeb0268d4f687406d6d46eb15bf8f9c60700`, after removing the local-path override.
The rebuilt API opens the original as four tables without recovery, saves a
fresh AC1021 DWG (`table-2007-product-pinned.dwg`) through safe save, and reopens
four tables without recovery. ProgeCAD independently reopened that exact final
output: **249/249 properties match** `control-ac1021.tsv`. Its capture and output
hashes are in the manifest; all three original source hashes remain unchanged.

The Git-pin matrix repeats three clean four-table reads and five verified saves,
with the same single resource-manager refusal. Git-source workspace Clippy
(warnings denied), dependency checks and stubtest passed. Full suites already
passed against the identical clean source revision as documented above; the
source-URL change introduces no new codec code. No local dependency path remains.
