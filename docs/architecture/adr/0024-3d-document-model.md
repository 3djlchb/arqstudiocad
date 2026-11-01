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
- **Step 1b, NURBS (2026-10-22):** `arq-kernel` adds NURBS curves and
  surfaces (rational when weighted; de Boor evaluation, closest point by
  sampling and Newton refinement). B-spline curves (`intcurve`, `bs3`) and
  surfaces (`spline-surface`, `bs3-surface`) read through AcadRust's
  decoded nets: homogeneous control points, a surface's `v` varying
  fastest. All 75 bodies now read and check, and every body of the
  comparison corpus's 196 LibreDWG drawings, DWG and DXF. Offset and mesh
  surfaces wait (procedural geometry kept, not translated).
- The order of a spline surface's net (`v` fastest) is AcadRust's reading,
  not yet confirmed: the corpora hold only square nets with equal knot
  vectors, where both orders give the same surface. A net read in the
  wrong order fails the body's check, so the body stays kept and drawn as
  before, never wrong.
- **Step 2, see (2026-10-22):** a body's wireframe, like a 2D wireframe
  view: its edges, sampled within a chord tolerance, and isolines on its
  curved faces, as many as the drawing's ISOLINES (0 to 64). Cylinders and
  cones get generators; spheres meridians and the equator; tori circles
  both ways; NURBS surfaces interior iso-curves; planes none. An isoline
  is clipped to its face by ACIS's rule that a face lies to the left of
  its loops seen from outside, trusted only near the boundary (within
  0.15 of its size): farther, a curved surface may fold back and fool the
  rule, so the state is carried along the isoline from its sample nearest
  the boundary, and an isoline that never comes near the boundary is left
  out rather than guessed (a large face around a small hole may show
  fewer isolines than AutoCAD). Crossings are found by bisection.
- The plan view draws that wireframe instead of the bounding box, also for
  bodies inside blocks (the block's transform moves the points; the body
  itself is not transformed), and the bodies' extents are the drawn ones.
  Wireframes are cached by the hash of the ACIS data and the isoline
  count, so an edited body is never drawn from a stale one. A body that
  does not translate keeps its bounding box. Shading waits for the GPU
  viewport (02.7).
- **Step 3, measure, the kernel (2026-10-31):** `Body::mass_properties`
  gives a body's volume, surface area, centroid and bounds, in three pull
  requests: the kernel, then the document and the Python API, then
  MASSPROP (Spanish PROPFIS) on the command line, in the ribbon's new 3D
  tab; moments of inertia come later (maintainer, 2026-10-31).
  - **How (maintainer, 2026-10-31):** integrating over each face's
    boundary on the edges' exact curves, not triangulating the faces. The
    divergence theorem turns the volume and the centroid into integrals
    over the faces; Green's theorem turns each face's integral over its
    parameters into one along its loops. The triangulation comes with
    shading (02.7) and will be checked against these measures.
  - Each surface has a chart: `(u, v)`, its derivatives and its inverse.
    Cones, cylinders and spheres close in `u`, tori both ways, a NURBS
    surface where its ends meet (its parameters swapped to put that way
    first). The 1-form along the loops is chosen periodic, so no seam
    enters the sums. A sphere's poles and a cone's apex add the line
    around them when the face holds them, which follows from how its
    loops wind around the axis, and, when they do not, from the sign of
    its area. A torus face whose loops wind both ways is refused.
  - Gauss–Legendre rules on the edges, cut at a spline's knots, are
    doubled until each quantity settles to `1e-11` of the body's size to
    its power. The inner integrals are exact for planes, cones, spheres
    and tori; an elliptic section's settles on its own; a spline's is
    refined with the loops.
  - A closed body's outward vector area is nothing: one that is not
    (within `1e-6` of its area) is refused as not closed, as sheets are.
  - The bounds are the edges' (exact for lines and arcs, sampled for
    splines), the apices and poles inside faces, and the points of
    spheres and tori that stand out inside their faces, told by the rule
    the isolines use; a spline face's own bulge is not sought yet.
  - **Checked:** solids built by hand with closed-form answers (a box,
    cylinders whole and halved, a cone to its apex, a sphere whole, as a
    hemisphere either way up and cut off its axis, a torus, an elliptic
    cylinder, a box with a spherical void, a box topped by a trimmed
    NURBS patch) to `1e-10`; every 3DSOLID of LibreDWG's drawings (59)
    measures, and measures the same moved far away (to `2e-13`); the
    open sheets are refused. The maintainer's MASSPROP results on some of
    them are to be the reference values.
  - The kernel and `arq-geometry` are built optimized in development
    profiles too: the corpus test took minutes otherwise.

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
