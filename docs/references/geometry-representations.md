# Geometry representations: what exists and what the kernel needs

Written 2026-10-21, as background for the owned kernel
([ADR 0020](../architecture/adr/0020-progressive-own-geometry-kernel.md))
and the 3D document model
([ADR 0024](../architecture/adr/0024-3d-document-model.md)).

NURBS is one way among several to describe a shape. The B-rep is not one
of them: it is the container (lumps, shells, faces, loops, edges,
vertices), and each face's surface and each edge's curve can be of any
exact kind below.

## The main representations

| Representation | What it is | Where it is used | Exact |
|---|---|---|---|
| Analytic (quadrics and tori) | Closed formulas: plane, cylinder, cone, sphere, torus; line, circle, ellipse | Every CAD kernel; what `arq-kernel` reads today | Yes |
| NURBS | Piecewise rational polynomials driven by control points and weights; they also represent the analytic shapes exactly | Rhino, ACIS (AutoCAD), Parasolid, STEP, IFC advanced B-rep | Yes |
| Bézier and B-splines | Special cases of NURBS: one span, or no weights | Graphics, fonts, automotive surfacing | Yes |
| Procedural surfaces | Defined by how they are built: sweep, offset, blend, revolution, evaluated exactly rather than converted to NURBS | Inside ACIS and Parasolid: many AutoCAD `spline-surface` records are this, with an approximating NURBS beside them | Yes |
| Polygon meshes | Flat triangles or quads | SketchUp, Blender, STL, OBJ, games; terrain (TIN) | No: faceted |
| Subdivision surfaces (SubD) | A control mesh smoothed by refinement (Catmull-Clark, Loop) | Blender, Rhino 7+, film animation | A smooth limit surface; free-form editing |
| T-splines | NURBS with local refinement, without a full grid of control points | Autodesk Fusion | Yes |
| Constructive solid geometry (CSG) | A tree of operations: box minus cylinder, union sphere | OpenSCAD; IFC `IfcBooleanResult` (openings) | Yes, as a recipe |
| Implicit, signed distance fields, F-rep | A solid as a function: the distance to its surface | nTopology, libfive; lattices, additive manufacturing | Robust booleans and offsets |
| Voxels | A 3D grid of full and empty cells | Medicine, simulation, terrain, games | No: fixed resolution |
| Point clouds | Measured points, no surface | Laser scanning, surveys, scan-to-BIM | Data, not a model |

## What ArqStudioCAD needs, and when

- **Analytic and NURBS, with procedural surfaces read through their NURBS**:
  required. They are what DWG (ACIS), Rhino's 3DM and IFC's advanced B-rep
  carry; without NURBS some of what exists cannot be read. Analytic first
  (done, step 1 of ADR 0024 D4), NURBS next.
- **Extrusions and sweeps**: IFC defines walls, beams and slabs as profiles
  extruded or swept (`IfcSweptAreaSolid`), not as NURBS. Our B-rep holds
  them with analytic faces; creating them is step 5 of the kernel.
- **CSG**: IFC uses it for openings (a wall minus a window). It is solved
  by the boolean operations, step 7 of the kernel.
- **Meshes**: needed for terrain (TIN, phase 06), STL/OBJ import and
  rendering. A mesh is a view of a solid or a separate kind of data, never
  a solid's truth (ADR 0024 D2).
- **SubD, T-splines, implicit solids**: useful for free forms and lattices;
  not needed now. Each can join the kernel later as a capability without
  changing the document model.
- **Point clouds**: valuable for surveys and scan-to-BIM; a separate module
  if it comes.

## The kernel's roadmap

The owned kernel grows capability by capability (ADR 0020 D5): a temporary
backend computes what the kernel cannot do yet, behind an adapter, and a
capability moves to the kernel when its acceptance suite passes. Truck is
the first temporary backend, reviewed when first needed; OCCT stays a
reference for differential tests outside the product (ADR 0024 D3).

| Step | Capability | Who computes it |
|---|---|---|
| 1 | Read: analytic geometry | Owned kernel (done) |
| 1b | Read: NURBS curves and surfaces | Owned kernel, openNURBS as a mathematical reference (done) |
| 2 | See: tessellation; edges in plan, shading with the GPU viewport (02.7) | Owned kernel; Truck for comparison |
| 3 | Measure: exact bounds, area, volume, centroid | Owned kernel |
| 4 | Write ACIS from our B-rep (to save an edited solid) | Owned kernel, checked by reading back and in other CAD applications |
| 5 | Create: box, cylinder, cone, sphere, torus, wedge; extrusion and revolution of 2D profiles | Owned kernel (direct analytic constructions) |
| 6 | Transform as a whole: move, rotate, scale, mirror | Owned kernel |
| 7 | Booleans: union, subtraction, intersection | Truck first, then the owned kernel: planar polyhedra, then analytic, then NURBS |
| 8 | Fillets, chamfers, shelling, offsets, sweeps, lofts | Truck (OCCT as reference), the owned kernel in the long run |
| 9 | Retire the temporary backend | when no capability uses it |

Steps 1 to 6 need no backend: opening, seeing, measuring, creating and
moving solids come without third-party code. The backend arrives at step 7,
where a kernel takes longest to become robust.
