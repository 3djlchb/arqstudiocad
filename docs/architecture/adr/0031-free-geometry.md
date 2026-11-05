# 0031. Free Geometry: a workspace for organic forms, by subdivision

Date: 2026-11-05 · Status: Proposed

## Context

CAD and BIM programs are strict about how volumes are made: primitives,
extrusions, revolutions, booleans, walls and slabs. Organic forms (a
curved canopy, a sculpted bench, a landscape mound, a free-form façade)
are hard or impossible to make with them. The maintainer asked for a
workspace of its own, **Geometría Libre** (Free Geometry), for such forms,
living beside the CAD tools without breaking them: its tabs open only when
the workspace is chosen (maintainer, 2026-11-04).

The reference is [IngeTrazo](https://github.com/ingelibre/ingetrazo)
(GPL-3.0-or-later), a free push/pull modeler from the same family as
IngeCAD ([analysis](../../references/ingetrazo.md)). It shows the tools
and the way of working (drawing on faces with inference, push/pull,
offset, follow me, bridge, typed lengths), but its geometry cannot be
taken over: flat polygons in single-precision floats, found by matching
positions, curves made of facets, owned by Python and Qt. ArqStudioCAD's
solids are an exact B-rep in `f64`, owned by Rust (ADR 0024 D1).

What we have: the B-rep kernel reads, draws, measures and writes ACIS for
analytic surfaces (ADR 0024 steps 1 to 4b); the model is seen from
AutoCAD's preset views and selected in them by its edges (ADR 0030); the
viewport draws wireframes only, the GPU viewport (02.7) waiting for the
reference machine. DWG has an entity for exactly this: **MESH**,
AutoCAD's subdivision mesh, with smoothing levels and creased edges;
AcadRust reads and writes it.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on
2026-11-05.

### D1 — A workspace of its own *(maintainer, 2026-11-04)*

- **Free Geometry** (Spanish *Geometría Libre*) is a discipline of its own
  (ADR 0015), ribbon only, with its own tabs; they show only while it is
  the chosen workspace. The 2D drafting workspace and its 3D tab are
  unchanged.
- It is milestone **02.14** of the roadmap.

### D2 — Subdivision, with push/pull on its faces *(maintainer)*

- A free form is a **subdivision mesh** (Catmull-Clark): a control cage of
  vertices, edges and faces, smoothed into a limit surface, with
  **creases** where an edge should stay sharp, as Blender, Rhino's SubD
  and AutoCAD's MESH work. Moving a few points of the cage reshapes the
  whole surface smoothly: that is what makes forms organic.
- On the cage, the face tools of a push/pull modeler, after IngeTrazo:
  push/pull (extrude a face), offset (inset), bridge two loops, bevel,
  loop cut; and moving, turning and scaling vertices, edges and faces.
- Starting shapes: mesh primitives (box, cylinder, sphere, torus…) and a
  solid made into a mesh.

### D3 — Saved as MESH, made a solid on request *(maintainer)*

- A free form is saved as a DWG **MESH**: its cage, its smoothing level
  and its creases. It stays editable in AutoCAD and ProgeCAD as AutoCAD's
  own subdivision mesh, and a MESH drawn there opens here for editing.
- A command makes it a **solid** when it must be measured exactly, cut or
  combined: a 3DSOLID whose faces are the exact NURBS of its limit
  surface (bicubic B-spline patches where the cage is regular, a fitted
  patch within tolerance around its irregular points). This needs the
  ACIS writer to write NURBS surfaces (ADR 0024 step 4 extended).
- The other way, a solid becomes a mesh (AutoCAD's MESHSMOOTH).
- A mesh is its own kind of data, never a solid's truth (ADR 0024 D2; the
  [geometry representations](../../references/geometry-representations.md)
  reference): converting creates a new entity.

### D4 — Rust owns the meshes

- The cage, its smoothing and every edit live in Rust: a mesh module of
  `arq-kernel` (vertices, edges, faces with their adjacency; open meshes
  allowed, a closed one needed to become a solid). Python drives
  commands; it never holds geometry (contract, ADR 0001).
- Every edit is a transaction, all or nothing, one undo step, as in CAD.
- IngeTrazo is a reference for behaviour and tests; an algorithm taken
  from it is reimplemented in Rust and recorded in `THIRD_PARTY.md`. Its
  code is not embedded.

### D5 — Simple shading on the CPU *(maintainer)*

- The CPU rasterizer learns to shade faces: triangles of the smoothed
  surface (and of solids' tessellations), a fixed light, a depth buffer,
  the cage's edges drawn over it. No textures, shadows or transparency.
- It is what free forms are modeled with on the reference machine, and it
  stays as the GPU viewport's fallback (ADR 0013 D5.8) when 02.7 comes.
- Its speed is measured on the reference machine before the workspace
  ships: a smoothing level and a face count that stay interactive.

### D6 — Picking in 3D, and a working plane

- Vertices, edges and faces of a cage are picked in any view by the line
  of sight through the cursor, the nearest first (ADR 0030 selects by
  projected edges only).
- A **working plane**: the plane of a face picked, or of a preset view,
  where points are put and push/pull moves, as IngeTrazo infers it. It is
  a first step toward the user coordinate systems (ADR 0024 D5), which
  stay their own work.
- Lengths and angles are typed on the command line, as in CAD.

### D7 — Commands everywhere, tabs in their workspace *(maintainer)*

- Free Geometry's commands are typed in any workspace, as every command
  is today; only their tabs belong to Free Geometry.
- A free form is seen, selected and measured in CAD as any entity:
  MASSPROP measures it once it is a solid, or warns that a mesh is
  measured through its smoothed surface.

### D8 — Ties with IngeTrazo and BIM

- Exchange through common formats (OBJ, glTF; `.skp` through OpenSKP, MIT,
  as IngeTrazo does) when they are needed, each its own decision.
- IngeTrazo's way of tagging free geometry as IFC objects (walls, slabs…)
  for quantities and budgets fits phase 05 (BIM and IFC) and the budgets
  (ADRs 0027, 0028); it is decided there.
- A collaboration with IngeTrazo's author (our DWG and ACIS engine for
  their planned DWG support, their push/pull experience for ours) is the
  maintainer's to propose.

### D9 — Order

1. **Shading** on the CPU (D5), for solids first.
2. **Read and see MESH**: AutoCAD's meshes drawn smoothed, selected,
   saved unchanged.
3. **Picking in 3D and the working plane** (D6).
4. **The workspace and its first shapes**: mesh primitives, a solid made a
   mesh, the smoothing level, creases.
5. **Editing the cage**: moving, turning and scaling its vertices, edges
   and faces.
6. **Face tools**: push/pull, offset, bridge, bevel, loop cut.
7. **A mesh made a solid** (D3), with NURBS in the ACIS writer.
8. Later, each its own step: symmetry, sweeping (follow me), deformers
   (bend, twist, lattice), sculpting brushes, materials.

## Consequences

- Organic forms get a way of working of their own without loosening the
  CAD tools: the same document, the same DWG, the same core.
- A free form travels to AutoCAD and ProgeCAD as their own MESH, and as an
  exact solid when made one.
- The kernel gains a second kind of geometry, meshes, beside the B-rep;
  the two meet only in conversions.
- Shading on the CPU arrives before the GPU viewport and serves CAD's
  solids too.
- 02.14 depends on no backend: Truck's booleans (ADR 0024 step 5) are not
  needed to model free forms, only to combine them once solids.
