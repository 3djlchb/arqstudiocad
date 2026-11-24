# Associative arrays: samples from ProgeCAD

[ADR 0044](../architecture/adr/0044-associative-array.md) D4. Ours reads
the associative arrays others make before it makes any. ProgeCAD makes
the files it reads first. To run on the Windows laptop, which has
ProgeCAD; nothing here runs on Linux.

## Making them

**Environment:**
- ProgeCAD 2026 Professional;
- a blank drawing from its own `pcadISO (Metric).dwt`;
- World UCS and the Top view.

The tool sets `OSMODE`, `ORTHOMODE` and `SNAPMODE` to 0 and
`ARRAYASSOCIATIVITY` to 1 for the run, and restores them after it.

**Run:**

```lisp
(load "tools/progecad_command_compare.lsp")
(arq:assoc-array-samples "C:/path/to/aa" "C:/path/to/aa.log")
```

**What it makes:** each case is saved as `aa-<case>.dwg` and
`aa-<case>.dxf`, both 2018, and described in the log
(`arq:dump-full`). Every source is synthetic.

| Case | Sources | What ProgeCAD does |
|---|---|---|
| AA1 | the line (0,0)–(1,0.5) | `-ARRAY` Rectangular, 3 rows × 4 columns, 2 / 3 apart |
| AA2 | the line on `ARRAY-TEST` in red, a circle, an arc, a closed polyline 0.25 wide | `-ARRAY`, 2 × 3, 4 / 15 apart |
| AA3 | the line | `-ARRAY`, 3 × 4, 2 / 3 apart, `SNAPANG` 30° |
| AA4 | the line | `ARRAYRECT`: Count 2 × 2, Spacing 3 / 2, Levels 2, 5 apart |
| AA5 | the line | AA1, then the array moved by (10,5), turned 30° and scaled by 2 about (10,5) |
| AA6 | the line | AA1, then `ARRAYEDIT` Columns 6 |
| AA7 | AA2's sources | AA2, then the array exploded |

**If a step fails:** where ProgeCAD's prompts differ (AA4's `ARRAYRECT`
options, AA6's `ARRAYEDIT`), the log says `step error`, and the case is
saved as it stands. Make that case by hand then, as the table says, and
save it with the same name.

## Bringing them here

The files are synthetic, but they stay off the repository until the
maintainer decides whether they become test fixtures (ADR 0044,
Consequences).
- Copy `aa-*.dwg`, `aa-*.dxf` and the log to the Linux machine, in a
  folder outside the repository.
- Give the agent its path.

## What they show

To be filled in when they arrive:
- the objects' names and fields, read from the DXF;
- how the INSERT and the action body's transform stay consistent (AA5);
- what changes when an array regenerates (AA6);
- what EXPLODE gives (AA7).
