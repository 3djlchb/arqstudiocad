# UNION, SUBTRACT and INTERSECT checked in ProgeCAD

ADR 0032 D4 asks for the solids the booleans make to be saved, reopened,
measured and audited in another application. This page records that check
(2026-11-07, ProgeCAD 2026 Professional, by script on copies, with the
maintainer's permission; the maintainer closed their own ProgeCAD first).

## The drawings

One drawing of seven results, made through the document API the commands
use (`union_solids`, `subtract_solids`, `intersect_solids`), in millimetres:

| Result | Made by | Exact volume (mm³) |
|---|---|---|
| Walls in L at the same level | UNION | 5 280 000 000 |
| Walls in T | UNION | 4 680 000 000 |
| Walls crossing | UNION | 5 280 000 000 |
| A 200 mm slab less a flush opening and a notch at its edge | SUBTRACT | 4 508 000 000 |
| Two boxes touching along an edge: one solid of two lumps | UNION | 2 000 000 000 |
| A hollow box: a void | SUBTRACT | 26 000 000 000 |
| The overlap of two boxes | INTERSECT | 1 000 000 000 |

Saved in DWG 2018 and DXF from a new drawing, and in DWG 2000 on LibreDWG's
native `2000/Constraints.dwg`, the solids placed apart from its contents.

## Results

| Drawing | Solids listed | Worst volume difference | Worst centroid offset | AUDIT |
|---|---|---|---|---|
| DWG 2000 | 7 of 7 | 1.1e-15 | 8.4e-14 | 0 errors |
| DWG 2018 | 7 of 7 | 1.1e-15 | 8.4e-14 | 0 errors |
| DXF | 7 of 7 | 1.1e-15 | 8.4e-14 | 0 errors |

The differences are relative, the volume to ours and the centroid offset to
each solid's size.

The first run found the hollow box measured as solid in DWG 2000: the codec
lost voids and second lumps writing SAT 400. That was corrected first
([evidence](progecad-acis400-topology.md), #138); this run is on the
corrected codec.

## How

- The drawings were built by `combine_samples.py` and measured by
  `measure.lsp`, which writes each 3DSOLID's volume, centroid and bounds
  through ProgeCAD's own ActiveX properties.
- The audit ran with `AUDITCTL` on.
- `compare.py` matched each solid against our `measure_solids` and flagged
  any solid ProgeCAD did not list.
- These are the same scripts as the earlier solid checks; none are part of
  the repository.
