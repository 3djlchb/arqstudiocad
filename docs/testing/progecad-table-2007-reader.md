# Native DWG 2007 table reader correction

Date: 2026-10-27. Follow-up to [the native table corpus baseline](progecad-tables-results.md)
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
