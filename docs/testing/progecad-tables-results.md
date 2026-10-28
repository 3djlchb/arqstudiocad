# ProgeCAD table measurements: 2026-10-26

Status: historical baseline at fork `59aaf4c`; **03.4 and 03.5 remain open**.
The later [table-instance correction](progecad-table-instance-formatting.md)
resolves the measured DXF-to-DWG formatting losses; results below remain the
unchanged baseline, not the status of the corrected codec. The subsequent
[DWG 2007 reader correction](progecad-table-2007-reader.md) resolves the
recovered BORDERS-table read recorded below.
[Reproduction exercise](progecad-tables.md), [captured snapshot manifest](progecad-tables-results.json).
Application base `3d4459c` (PR #77 merged), branch `feature/progecad-table-corpus`.
AcadRust 0.6.7 fork pin `59aaf4cddee5a1a7c86a640c1246225be1bfe2bc`.
ProgeCAD 2026 Spanish build `13.1.508.P.VC17.x64.26.0.12.5`
(file version `13,1,508,33299`). No product/codec source or dependency changed.

## Inputs and core results

Three independently saved native inputs contain the same four ACAD_TABLE
entities, with 5 rows and 3 columns each. Each title spans three columns;
all 60 stored cell texts, including hidden title cells, are measured.
The nondefault border case was edited through ProgeCAD's native dialog.
Sources are local under `EJERCICIOS DWG/Tables-ProgeCAD-20261026/source`.
The JSON manifest records source versions, hashes, counts and output hashes.

| Native input | Product read | Strict DWG save | Strict DXF save |
|---|---|---|---|
| DWG AC1021 (2007) | Recovered: 3 of 4 tables | Not run by matrix | Not run by matrix |
| DXF AC1024 (2010) | Clean: 4 tables | Verified by core | Verified by core |
| DWG AC1032 (2018) | Clean: 4 tables | Verified by core | Refused: MAPRESOURCEMANAGEROBJECT 1 -> 0 |

The matrix intentionally returns exit 1 for this corpus. Its failure is a
measured compatibility limitation, not an omitted test or an accepted loss.
No source was overwritten. Exploratory outputs from the recovered input are
kept only as diagnostics in `review/core-3d4459c`; accepted clean-read outputs
are separately retained in `review/validated-3d4459c`.

`tools/compatibility_matrix.py::measure` records `recovery_problems` and skips
round trips after a recovered read; `roundtrip` calls the strict public save API.
The product's `inventory.rs::Document::save_losses` blocks the missing resource
manager object. Its semantic table use has not been established, so it was
not removed or exempted to force a DXF pass.

## Independent ProgeCAD reopening

Native sources were copied byte-for-byte to `review/native-controls` before
reopening. Their captures are in `observations/control-ac1021.tsv`,
`control-ac1024.tsv` and `control-ac1032.tsv`. Three clean-read product outputs
were independently opened and measured without saving over them. No repair
dialog appeared in these observed reopenings.

`tools/progecad_table_corpus.lsp::arq:audit-doc` captures the document's actual
name and properties; `arq:raw-measures` captures row heights and selected native
cell records. `tools/compare_progecad_tables.py::observation` requires the exact
expected drawing and all table/cell measurements; `compare` checks 249 records
per pair (including the schema, excluding drawing identity and raw entity
runtime-reference metadata). This is a defined set, not every possible table
property or a pixel-level rendering proof.

| Compared output against its own native source control | Recorded differences | Interpretation |
|---|---|---|
| AC1032 DWG -> DWG | 0 / 249 | All measured table properties match |
| AC1024 DXF -> DWG | 68 / 249 | 16 table-level margin/text-height differences plus 52 differing cell records; fidelity fails |
| AC1024 DXF -> DXF | 5 / 249 | All scalar measurements/texts match; five border records differ; representation review remains |

The native AC1021 and AC1032 control captures match all 249 measurements.
Thus ProgeCAD can reopen all four tables of the 2007 input, including the
border case the product cannot read. The independent native DXF and DWG
controls have 52 raw-cell differences, involving CELL_VALUE flags (group 93,
0 versus 6), with identical scalar measurements. Such cross-format native
normalization must not be confused with the product-specific losses below.

### Demonstrated DXF-to-DWG discrepancies

Compared with native DXF reopening:

- At X=160, horizontal/vertical margins change **4.5 / 3 -> 1.5 / 1.5**.
- At X=480, margins change **2 / 2 -> 1.5 / 1.5**.
- All four table-level `GetTextHeight` row-type values change to the default
  **4.5 / 6 / 4.5** from the fixture's 2.5, 3 or 4.
- Native cell records at X=480, indices 6 and 7 lose explicit right-border
  red color (DXF group 65=1) and 0.50 mm lineweight (275=50). Override masks
  also differ. The remaining raw differences include the format-dependent
  CELL_VALUE flags seen in the native controls.

These are persisted-property differences. The tables also carry anonymous
cached geometry: an apparently similar initial drawing does not prove that
these settings survive editing or regeneration. Actual per-cell text heights
and every rendering consequence have not been fully evaluated here.
The earlier 1.5-margin companion-map correction remains valid for its narrower
fixture, which contained no table entity; it did not establish table-instance
formatting fidelity.

`crates/arq-document/src/verify.rs::Document::validate_roundtrip` currently
checks public entities/layers, Xrefs and table-style companion conversion;
`inventory.rs::Document::losses_in` counts kinds. Neither establishes complete
ACAD_TABLE instance formatting preservation. Thus a core-verified save can
still fail these independent native measurements. No verifier was weakened.
The exact codec cause is not yet isolated. Relevant investigation points in
fork `59aaf4c` are `section_reader.rs::read_table_entity` (DXF reader),
`section_writer.rs::write_table_cell` (DXF writer) and the DWG table
reader/writer routines; these are triage locations, not proven root causes.

### DXF-to-DXF border representation differences

Five cells at X=480 (indices 4, 6, 7, 8 and 10) change stored override flags.
Some gain explicit group 285/286 values of 1. Colors, lineweights, dimensions,
text and merge spans remain in the captured data. In the pinned codec,
`section_writer.rs::write_table_cell` writes those visibility values as
`!border.invisible` with a lineweight override. This is evidence of explicit
visibility encoding, not sufficient proof of changed visible borders.
The strict raw comparator reports the differences; they have not been hidden
behind a normalization allowance. Independent edit/regeneration checks remain.

### DWG 2007 read failure

The source record at handle **0x2FA**, corresponding to the BORDERS table,
raises `type_code=-16`, truncated stream at bit 63057 (8 requested, 7 remain).
Only three tables reach the product model; recovery diagnostics are exposed.
The native control opens and matches all four tables, so this is an actionable
reader discrepancy. Repeated generic parsing is not a substitute for fixing
and regression-testing this case in the fork.

## Evidence and checks

Working-copy evidence is retained in `target/progecad-table-*`: the strict
matrix, retained-output manifest, native control comparisons and three output
comparisons. Actual observation TSVs and drawings are in the exercise folder;
large/private binary inputs are not committed. Source and output hashes were
checked again after native reopening and stayed unchanged.

The comparator has 16 tests covering completeness, wrong drawing/table count,
changed margins/text/merge spans/borders, invalid numbers and malformed Lisp,
duplicate keys, numeric representation and observation overwrite protection.
The full Python suite passed **553 tests**, with nine existing warnings;
Ruff (185 files), mypy (115 source files) and stubtest (one module) passed.
The default Rust suite passed **443 tests**, eight optional tests ignored.
Workspace Clippy with warnings denied, Rust formatting and cargo-deny
(licenses, advisories and sources) also passed.
No Linux/macOS, floor-CPU release, formula/data-link, broad table-editing or
complete visual-fidelity acceptance is claimed.

## Next implementation priorities

1. Add the native 2007 border table as a focused fork regression and fix the
   truncated read; require all four tables without recovery.
2. The [follow-up correction](progecad-table-instance-formatting.md) preserves
   the measured DXF -> DWG margins, row-type text heights and right borders.
   Broader save/read-back table-format validation remains open; count parity
   alone cannot establish it.
3. Resolve the five same-format DXF border encoding differences through native
   edit/regeneration comparisons before proposing any equivalence rule.
4. Investigate resource-manager preservation separately, then extend versions,
   formulas and other table types. The two pre-existing R2000 TABLESTYLE-owner
   refusals remain separate open work.
