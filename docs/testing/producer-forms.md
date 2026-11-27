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
| Closed spline through fit points | to confirm: none in Autodesk's samples | clamped, first fit point again at its end | ProgeCAD's (since #259), until AutoCAD's is known | [SP3](commands-02-14-windows.md#elliptical-arcs-joined-spline-and-the-spline-as-a-curve-2026-11-23) |
| Polyline of one width all along | the constant width (43): 1,200 polylines in Autodesk's samples, 4 with the width at each vertex instead | the width at each vertex; reads the constant width | the constant width, AutoCAD's, which ProgeCAD reads: kept (2026-11-26) | [M2](commands-02-14-windows.md#join-on-polylines-matchprop-and-ellipse-2026-11-20), [Autodesk's samples](#autodesks-sample-files-2026-11-26) |
| Spline's fit points with a knot method other than Chord | to confirm: Autodesk's samples hold 6 fitted splines, all Chord | none kept | kept | SP4 |
| Offset of an ellipse or a spline | to confirm | quadratic, rational, every weight 1 | cubic, not rational | SC11–SC13 |
| Multileader style in DWG 2004 | EED of `ACAD_MLEADERVER` | refuses a drawing without it | AutoCAD's | [evidence](progecad-dwg2004-mleader-version.md) |
| Constant-width polyline in DXF | 43 alone | reads 43 alone | AutoCAD's | [evidence](lwpolyline-dxf-width.md) |

**To confirm** means no public AutoCAD sample has shown it yet (ADR
0045 D4).

## How a producer is known

Since 2026-11-26 (ADR 0045 delivery 1), from a DWG's AppInfo section:

| Producer | What its AppInfo says | Evidence |
|---|---|---|
| AutoCAD | `name ="AutoCAD"`, a Trusted DWG comment | LibreDWG's 2004–2013 samples; Autodesk's 18 sample files, AutoCAD 17.0 to 18.1 |
| The ODA's Teigha | `name ="Teigha®"`, an Open Design Alliance comment | LibreDWG's 2018 `Constraints.dwg` |
| Ours | our product name, given by `product.py` | our own saves |
| ProgeCAD, BricsCAD, ARES Commander, GstarCAD, Edificius, Vectorworks | to confirm: known by the names they give themselves | none yet |

A DXF, and a DWG before 2004, carry no AppInfo: their producer is unknown.

## Autodesk's sample files (2026-11-26)

Autodesk's own AutoCAD sample files (`autodesk.com/autocad-samples`: the
architectural, civil, mechanical, visualization, blocks-and-tables,
title-block and other examples) were downloaded by the maintainer and read
on the Linux machine. They are never committed or copied; the test
`local_autocad_drawings_name_autocad` reads them where `ARQ_AUTOCAD_DIR`
names them.

- **The producer:** all 18 DWGs name AutoCAD, versions 17.0 to 18.1
  (formats 2007 and 2010).
- **One width all along:** 1,200 lightweight polylines carry the constant
  width (group 43). Four, in the two blocks-and-tables drawings, carry the
  same width at each vertex instead. AutoCAD's form is the constant
  width, the one ours writes and ProgeCAD reads, so it is kept (D3).
- **Splines:** six through fit points, all open, all with Chord knots.
  So neither SP3 (closed) nor SP4 (other knot methods) is shown.
- **Read, saved and read back:** the corpus's local round trip
  (`ARQ_CORPUS_DIR`) on the 18 drawings, as DWG and DXF, then edited and
  undone. 17 pass. `visualization_-_conference_room.dwg` does not save as
  DXF: the safe save refuses it, nothing lost, because six references to
  its block "Fluorescent Fixture" read back with another box, reaching the
  origin. The block holds a 3D solid already drawn as a box; in DXF a body
  is kept without the DWG's wireframe cache and boxed from its vertices
  ([matrix](../DWG_DXF_INTEROPERABILITY.md)). A fix of its own is to come.
