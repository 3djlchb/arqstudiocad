# 0024. The 3D document model: our B-rep, ACIS kept

Date: 2026-10-21 · Status: Accepted · Settles ADR 0014 question 4

## Context

ADR 0014 left open what the core edits in 3D, and how 3D solids (ACIS in
DWG) and IFC geometry meet. ADR 0019 gave the 3D value types and ADR 0020
the way to an owned kernel: built capability by capability, with reviewed
references as temporary backends behind an adapter.

Today a DWG's 3DSOLID, REGION, BODY and SURFACE carry ACIS data (SAT text
or SAB binary) that the engine keeps byte for byte; we draw their edges in
plan or a box, and edit none of them. Our own model owns layers, LINE,
CIRCLE and ARC (ADR 0018 D2, stage 1). AcadRust parses SAT and SAB into
records (`SatDocument`) and writes them back, and builds a few primitives
as SAT. Shaded 3D views need the GPU viewport of milestone 02.7, on hold
for the reference machine.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on 2026-10-21.

### D1 — A solid is ours: a B-rep *(maintainer)*

The document represents a solid with ArqStudioCAD's own boundary
representation: bodies, lumps, shells, faces, loops, edges and vertices,
whose geometry is analytic (planes, cylinders, cones, spheres, tori, lines,
circles, ellipses) or NURBS (curves and surfaces). The types live in an
owned crate behind the kernel's contracts (ADR 0020 D4), built on
`arq-geometry`'s 3D values (ADR 0019); its name is set when it is created.
No engine or backend type appears in the document's or the core's API.

### D2 — ACIS is read into it and kept intact *(maintainer)*

- Reading a DWG translates each solid's ACIS records into our B-rep, in
  `arq-document`'s adapter, on AcadRust's SAT/SAB records.
- A solid not edited is saved with its original ACIS bytes, unchanged.
  Only an edited solid is written as ACIS generated from our B-rep, in the
  ACIS version its DWG version expects.
- A solid whose ACIS cannot be translated yet stays as now: kept byte for
  byte, drawn by its edges or box, not editable, and listed as such. No
  solid is ever lost or rewritten unasked (ADR 0017 D5).
- A mesh (tessellation) is a view of a solid, never the document's data.

### D3 — Truck is the first temporary backend *(maintainer)*

[Truck](https://github.com/ricosjp/truck) (Rust, Apache-2.0) is the first
backend that computes what the owned kernel cannot do yet (ADR 0020,
amendment 2): a Rust crate, no C++ bridge or extra packaging, on every
platform. Before it becomes a dependency it passes the master plan's
technology review (§16): exact version and crates, license, `cargo deny`,
build and size on the three platforms. OCCT stays a reference for
differential tests outside the product. A capability moves to the owned
kernel when its acceptance suite passes (ADR 0020 D5, D6).

### D4 — Read and see first, then create and edit *(maintainer)*

The 3D capabilities enter in this order, each as its own reviewed step with
its acceptance suite:

1. **Read:** ACIS to our B-rep: planar and analytic faces first, then NURBS
   (spline) surfaces and curves.
2. **See:** tessellate our B-rep; draw its edges in plan now, and shaded
   faces when the GPU viewport (02.7) arrives.
3. **Measure:** bounds, area, volume, centroid.
4. **Create:** primitives (box, cylinder, cone, sphere, torus, wedge),
   extrusion and revolution of 2D profiles.
5. **Edit:** move, rotate, scale and mirror a solid as a whole; then
   booleans (union, subtraction, intersection), first through the
   temporary backend.

### D5 — What waits

IFC geometry meets this model in phase 05 (ADR 0014 question 1): IFC solids
and surfaces map to the same B-rep and meshes stay views. 3D wireframe
entities with Z and user coordinate systems are a later step of 02.8, not
part of this ADR.

## Implementation notes

- **Step 1, read (2026-10-21):** the crate `arq-kernel` holds the B-rep
  (arenas of vertices, edges, coedges, loops, faces, shells and lumps named
  by typed ids) and the analytic geometry: lines and ellipses; planes,
  circular cones and cylinders, elliptic cylinders, spheres and tori. A body
  exists only after its check: indices, closed loops, and every vertex on
  its edges' curves and its faces' surfaces within ACIS's 1e-6, relative to
  the body's reach. `Document::solid` reads an entity's ACIS through
  AcadRust's records into it, or says why not (unreadable, not translated
  yet, invalid); the ACIS bytes are untouched.
- ACIS details met on the corpus: a cone or cylinder with a negative cosine
  has its normal toward the axis; a sphere or torus of negative radius, its
  normal inward; ACIS 1.x (R14) shells have no wire pointer; a body
  transform applies to row vectors (`p · R · s + t`).
- On LibreDWG's drawings 73 of 75 bodies read and check; the other two wait
  for spline geometry (a `spline-surface`, an `intcurve-curve`), the next
  step. The corpus test pins these counts.

## Consequences

- One solid model for DWG now and IFC later; ACIS stays a file format.
- Opening and saving lose nothing: untouched solids keep their bytes;
  untranslatable ones stay kept and visible.
- Writing ACIS from our B-rep is new work: edited solids need it, so
  creation and editing (D4 steps 4–5) wait for it, tested by reading the
  written ACIS back with AcadRust and in independent CAD applications
  (03.5).
- The test corpus already holds solids to measure progress: LibreDWG's
  `ATMOS-DC22S.dwg` (58 3DSOLID), `example_*.dwg`, `2004/Surface.dwg`, and
  the maintainer's drawings.
- Truck is a dependency only after its review; until then the steps that
  need no backend (read, see, measure) go first.
