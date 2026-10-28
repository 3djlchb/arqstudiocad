# Native table-instance formatting: DXF to DWG

AcadRust commit `0d438b632c7d81d5aab347cd2b1b46e6a27200b1` is retained by
[the immutable fork tag](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-table-instance-format),
on the same upstream `c5555a5` (0.6.7) base as the previous pin.

This follow-up addresses the native AC1024 DXF-to-DWG losses recorded in
[the table corpus baseline](progecad-tables-results.md). It does not close
milestones 03.4 or 03.5 or claim general table editing/rendering support.

## Proven causes and correction

The native DXF exposes an ACAD_TABLE legacy projection and a complete
TABLECONTENT linked through its owned extension dictionary and the
ACAD_XREC_ROUNDTRIP record, in the ACAD_ROUNDTRIP_2008_TABLE_ENTITY section.
The entity projection lacks the complete base and row styles. The previous
modern DWG writer serialized that projection alone, restoring default margins
and table-level row text heights instead of the stored instance format.

The fork's `src/io/dwg/table_format.rs::with_companion_format` now supplies
missing base/row/column formatting from this companion before DWG serialization.
It checks dictionary, record and content ownership, style identity and matching
row/column counts. A modern entity with its own base/row format remains
authoritative. Values, widths, heights and cells remain those of the entity;
explicit legacy margin and row text-format overrides take precedence. The
caller document is not mutated. The helper intentionally does not invent a
format when the companion is absent, inconsistent or belongs to another owner.

A second defect mixed two distinct representations. Public CellEdgeFlags use
RIGHT=2 and INSIDE_HORIZONTAL=32, while serialized GRIDFORMAT indexes use
RIGHT=32 and INSIDE_HORIZONTAL=2. The fork now translates these masks at the
DWG and TABLECONTENT DXF read/write boundaries. Additional raw grid indexes
remain raw. The DWG reader consumes every border record, including unfamiliar
indexes, to preserve stream alignment. Public flag values remain unchanged.

Implementation locations in the AcadRust fork:

- `src/entities/table.rs::CellEdgeFlags::{from_grid_mask,grid_mask}`.
- `src/io/dwg/table_format.rs::{companion,with_companion_format,row_override}`.
- `src/io/dwg/dwg_stream_readers/object_reader/entities.rs::read_cell_style`.
- `src/io/dwg/dwg_stream_writers/object_writer/entities.rs::{write_table_cell_style,write_table}`.
- `src/io/dxf/reader/section_reader/table_content.rs` and
  `src/io/dxf/writer/section_writer.rs`: TABLECONTENT grid-format conversion.

## Independent native measurements

ProgeCAD 2026 build 13.1.508.P.VC17.x64.26.0.12.5 reopened the candidate DWG
produced from the original `tables-native-ac1024.dxf`. The same strict capture
and comparator used for the baseline measured four tables and 249 properties.

| Comparison | Before | Corrected candidate |
|---|---:|---:|
| Against native DWG control, measured properties differing | 19 / 249 | 0 / 249 |
| Against native DXF control, scalar differences | 16 | 0 |
| Against native DXF control, raw cell records differing | 52, including lost right-border properties | 52, only CELL_VALUE group 93 flags |

The corrected margins are 4.5/3 for MARGINS and 2/2 for BORDERS. Table-level
row-type text heights retain 2.5, 3 or 4, as authored. BORDERS cells 6 and 7
retain their red (index 1), 0.50 mm right-border overrides. Dimensions, merge
spans and all captured text/state records match the native DWG control.

The 52 group-93 flag changes (0 to 6) also occur between independently
reopened native DXF and DWG controls. The comparator remains strict and reports
them; no equivalence exemption was added. This is bounded property evidence,
not a claim that all native table properties or pixels have been checked.

## Regression coverage and reproduction

`tests/table_instance_format.rs` in the fork covers companion ownership and
layout rejection, modern-format and explicit legacy-edit precedence, preservation of edited entity
values/widths and caller immutability, and distinct right/inside-grid borders.
Its optional native test compares base/row styles, widths and merge spans with
the native DWG control and checks both red right borders.

Run in the fork, with the private source corpus available:

```powershell
$env:ARQ_NATIVE_TABLE_ROOT = 'D:/PROGRAMACION/EJERCICIOS DWG/Tables-ProgeCAD-20261026/source'
cargo test --test table_instance_format -- --include-ignored
```

The optional ARQ_TABLE_FORMAT_OUTPUT path writes the generated DWG with
create-new semantics. Choose a fresh destination. Original drawings are never
overwritten. Reopen that output in ProgeCAD, load
`tools/progecad_table_corpus.lsp` from the product worktree, and capture with
`(arq:audit "D:/path/to/fresh-capture.tsv")`. Compare using
`tools/compare_progecad_tables.py` with explicit drawing names and `--tables 4`.

The integrated core also saved `table-format-product-candidate.dwg` through
its normal safe-save API. ProgeCAD reopened that retained output on 2026-10-27;
all 249 captured properties match the native DWG control. The manifest records
the independent integrated-output capture separately from the codec test output.

## Codec checks

The full default fork suite passed **2,288 tests**, with 83 optional/documentation
examples ignored and zero failures. The final focused run passed **six tests**,
including the optional native corpus case and explicit legacy-edit precedence.
The full suite preceded the final additional test-only case; the production
source was unchanged between these runs. New module/test formatting and the
patch whitespace check passed.

A release comparison against the previous fork `59aaf4c` ran **464 jobs / 232
paired operations** on 34 drawings: 25 public codec/LibreDWG samples, six private
Xref drawings and the three new native table inputs. No pair changed read/write
success status or the benchmark's entity/object/count/geometry summary. The
same three forced-AC1032 write refusals per library remain (a legacy surface's
ACDBASSOCEDGEACTIONPARAM and DATATABLE in two older examples). These inventory
summaries do not measure table formatting; the native captures establish the
formatting improvement separately. No timing, memory or cross-platform claim
is made from this run.

## Remaining scope

- Native AC1021 BORDERS reading was separate from this formatting fix; the
  subsequent [reader correction](progecad-table-2007-reader.md) reads and
  rewrites all four tables strictly, with 249/249 native measurements matching.
- Native AC1032 DWG-to-DXF still refuses MAPRESOURCEMANAGEROBJECT loss.
- The five raw border differences in same-format DXF need independent
  edit/regeneration investigation; this correction does not declare them safe.
- The two older R2000 TABLESTYLE-owner losses remain safely refused.
- The product's generic save verifier does not establish complete ACAD_TABLE
  formatting fidelity. These focused codec tests and independent captures add
  evidence without weakening that verifier or claiming comprehensive coverage.
- No new dependency or technology is introduced; the existing fork strategy
  and MPL-2.0 obligations in ADR 0017 continue to apply.
