# How each entity is kept, by producer

[ADR 0045](../architecture/adr/0045-producer-compatibility.md) D3:
- **Written:** each entity is written in one form, the one AutoCAD and
  every producer reads the same way (AutoCAD, ProgeCAD, BricsCAD, ARES
  Commander, GstarCAD, Edificius, Vectorworks). Where there is none, it is AutoCAD's,
  unless one of them breaks on it.
- **Read:** each producer's form is taken into one model (D2).
- **Evidence:** this table records the choice for each entity and its
  evidence. A change of form goes through it.

Columns for BricsCAD, ARES Commander, GstarCAD, Edificius and Vectorworks are added
as their evidence comes (D4); until then each is "to confirm".

| Entity | AutoCAD | ProgeCAD | Written by ours | Evidence |
|---|---|---|---|---|
| Closed spline through fit points | to confirm | clamped, first fit point again at its end | ProgeCAD's (since #262), until AutoCAD's is known | [SP3](commands-02-14-windows.md#elliptical-arcs-joined-spline-and-the-spline-as-a-curve-2026-11-23) |
| Polyline of one width all along | to confirm | the width at each vertex | the constant width | [M2](commands-02-14-windows.md#join-on-polylines-matchprop-and-ellipse-2026-11-20) |
| Spline's fit points with a knot method other than Chord | to confirm | none kept | kept | SP4 |
| Offset of an ellipse or a spline | to confirm | quadratic, rational, every weight 1 | cubic, not rational | SC11–SC13 |
| Multileader style in DWG 2004 | EED of `ACAD_MLEADERVER` | refuses a drawing without it | AutoCAD's | [evidence](progecad-dwg2004-mleader-version.md) |
| Constant-width polyline in DXF | 43 alone | reads 43 alone | AutoCAD's | [evidence](lwpolyline-dxf-width.md) |

**To confirm** means no public AutoCAD sample has shown it yet (ADR
0045 D4).
