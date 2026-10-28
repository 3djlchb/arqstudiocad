# Native ProgeCAD table corpus

Milestones 03.4 (compatibility matrix) and 03.5 (external checks).
[Measured results](progecad-tables-results.md) and [snapshot manifest](progecad-tables-results.json).
This exercise creates original ACAD_TABLE entities in ProgeCAD, independently
of AcadRust. It adds no table editor or new product dependency.

## Fixture and acceptance

Use a new empty metric drawing, model space, world coordinates. Load
`tools/progecad_table_corpus.lsp` inside ProgeCAD, then run `(arq:create-tables)`.
The helper refuses a nonempty model space. It creates four 5-row, 3-column
tables at X = 0, 160, 320 and 480 model units. All have a combined title row;
the stored text in the two hidden title cells is measured too.

| Case | Horizontal / vertical margins | Text height | Column widths |
|---|---|---|---|
| BASE, X=0 | 1.5 / 1.5 | 2.5 | 40 / 40 / 40 |
| MARGINS, X=160 | 4.5 / 3 | 3 | 25 / 55 / 40 |
| MERGED, X=320 | 1.5 / 1.5 | 2.5 | 40 / 40 / 40 |
| BORDERS, X=480 | 2 / 2 | 4 | 40 / 40 / 40 |

The requested row height is 14; the MARGINS fourth row is set to 20.
ProgeCAD may expand other rows to fit their text. Capture actual heights,
not the requested values. No assumption about drawing-unit conversion is made.

In BORDERS select the cell at zero-based row 2, column 1. Use the native cell
toolbar's **Edit borders** dialog: choose red, lineweight 0.50 mm, apply the
border preset and accept. Adjacent cells can receive shared-border overrides;
the raw cell capture records the actual result. The COM interface in this
build exposes neither MergeCells nor the per-cell border getters; do not
substitute invented API calls. The title-row spans and hidden-cell flags are
measured through native `entget` data.

Save separate DWG 2007 and DWG 2018 sources and export ASCII DXF 2010 at
16-digit precision using ProgeCAD's format controls. Check actual headers:
AC1021, AC1032 and AC1024 respectively. A filename or a COM SaveAs argument
alone does not establish the file version. Keep all originals unchanged.
The recorded run lives in the maintainer's local folder
`D:\PROGRAMACION\EJERCICIOS DWG\Tables-ProgeCAD-20261026\source`.
The DWGs/DXF are not committed or downloaded by the sample-fetch tool.

Acceptance requires clean reads of all four tables, strict saves without
accepted losses, and independent reopening of retained outputs with matching
measurements. A partial read, blocked save or missing observation is not a pass.
A core-verified save alone does not establish external fidelity.

## Measure the integrated core

Run the existing tool unchanged from the task worktree:

```powershell
uv run python tools/compatibility_matrix.py "D:/PROGRAMACION/EJERCICIOS DWG/Tables-ProgeCAD-20261026/source" --format json --output target/progecad-table-matrix.json
```

Retain output copies through `CadDocument.save`, only after confirming
`recovery_problems` is empty. Do not accept losses. Use fresh paths and record
SHA-256 values before and after the run. Keep any exploratory serialization
of a recovered drawing separate from accepted outputs.

## Independent observation and comparison

Reopen byte-identical control copies and each retained product output in
ProgeCAD. Open the drawing first, then load the helper again: Lisp functions
are local to the drawing. Do not combine opening another drawing and calling
helpers defined in the previous drawing's namespace.

```lisp
(load "D:/PROGRAMACION/ARQSTUDIOCAD/arqstudiocad-progecad-table-corpus/tools/progecad_table_corpus.lsp")
(arq:audit "D:/PROGRAMACION/EJERCICIOS DWG/Tables-ProgeCAD-20261026/observations/new-capture.tsv")
```

The audit refuses to overwrite an observation. It captures rows, columns,
actual row heights, column widths, total heights, margins, table-level text
heights, style name, all 60 stored cell texts/states and 60 selected raw cell
records, including merge spans and border override fields. Runtime entity
references are retained separately as raw evidence and excluded from comparison.

```powershell
uv run python tools/compare_progecad_tables.py baseline.tsv candidate.tsv --baseline-drawing tables-native-ac1032.dwg --candidate-drawing from-tables-native-ac1032-dwg.dwg --tables 4 --output target/native-table-comparison.json
```

The comparator validates the drawing name and requires a complete capture for
the declared table/cell dimensions. It rejects duplicate keys, malformed values,
nonfinite numbers and observation overwrite paths. Numeric tolerance is
absolute 1e-8 model units; strings and record structure compare exactly. Exit
codes: 0 = measured match, 1 = differences requiring review, 2 = invalid or
missing evidence. Raw-record differences can include explicit encodings of
implicit defaults; report and investigate them rather than silently normalizing
them or claiming every difference proves a visible defect.

This fixture does not cover formulas, data links, block-valued cells,
custom fonts, combined data rows, editing/recomputation or every version pair.
It does not close either milestone or provide visual table editing in the app.
