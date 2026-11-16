# Booleans of cylinders and cones checked in ProgeCAD

ADR 0033 D4 asks for the solids each delivery's booleans make to be saved,
reopened, measured and audited in another application. This page records
that check for delivery 2, planes with cylinders and cones, once UNION,
SUBTRACT and INTERSECT took them (2026-11-16, ProgeCAD 2026 Professional,
by script on copies; no other ProgeCAD was open).

## The drawings

One drawing of twelve results, made through the document API the commands
use (`union_solids`, `subtract_solids`, `intersect_solids`), in
millimetres, each with its exact volume in closed form:

| Result | Made by | Exact volume (mm³) |
|---|---|---|
| Two Ø400 columns 300 apart | UNION | 699 584 826.384 |
| Their overlap | INTERSECT | 54 397 410.477 |
| A Ø600 column notched by a Ø400 one 350 off its axis | SUBTRACT | 740 844 922.742 |
| A tube: Ø600 less a coaxial Ø400 | SUBTRACT | 471 238 898.038 |
| Two Ø400 columns stacked | UNION | 376 991 118.431 |
| Two Ø400 columns touching: one solid of two lumps | UNION | 753 982 236.862 |
| The overlap of a Ø500 and a Ø300 column at other heights | INTERSECT | 34 635 646.036 |
| A 1 m cube bored by a Ø400 column through it | SUBTRACT | 874 336 293.856 |
| The cube united with the column | UNION | 1 125 663 706.144 |
| A Ø600 column halved by a box along its axis | INTERSECT | 141 371 669.412 |
| The cube with a conical hole (a cone Ø800 to Ø200 through it) | SUBTRACT | 801 032 465.273 |
| What the cube keeps of the cone, a frustum Ø600 to Ø400 | INTERSECT | 198 967 534.727 |

The column overlaps are the lens two circles share times the shared
height. Saved in DWG 2018 and DXF from a new drawing, and in DWG 2000 on
LibreDWG's native `2000/Constraints.dwg`, the solids placed apart from its
contents.

## Results

| Drawing | Solids listed | Ours against exact | Ours against ProgeCAD | ProgeCAD's centroid offset | AUDIT |
|---|---|---|---|---|---|
| DWG 2000 | 12 of 12 | 2.1e-11 | 2.5e-6 | 7.0e-7 | 0 errors |
| DWG 2018 | 12 of 12 | 2.1e-11 | 2.5e-6 | 7.0e-7 | 0 errors |
| DXF | 12 of 12 | 2.1e-11 | 2.5e-6 | 7.0e-7 | 0 errors |

The differences are relative: volumes to the exact one or to ours, the
centroid offset to each solid's size. Ours were measured on the drawings
reopened.

- **ProgeCAD reads every solid** in the three formats, and AUDIT finds
  nothing.
- **Its volumes differ from the exact ones by up to 2.5e-6** in four
  results (the columns united, the notched column, the overlap at other
  heights and the half column). Ours match the exact volumes to 2.1e-11, so
  the difference is ProgeCAD's integration of curved faces: it measured a
  plain cylinder of volume 2π at 6.283178863, 1.0e-6 short, in an earlier
  check ([DWG 2004 multileader version](progecad-dwg2004-mleader-version.md)).
  The other eight agree within 1e-6.

## How

- The drawings were built by a script through the document API and
  measured by `measure.lsp`, which writes each 3DSOLID's volume, centroid
  and bounds through ProgeCAD's own ActiveX properties.
- Each drawing was opened by its own ProgeCAD from its command line, and
  the audit ran with `AUDITCTL` on.
- `compare.py` matched each solid against our `measure_solids` and flagged
  any solid ProgeCAD did not list, as in the
  [planar check](progecad-solid-booleans.md).
- These scripts are not part of the repository.
