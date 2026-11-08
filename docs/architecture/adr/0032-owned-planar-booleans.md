# 0032. Owned booleans, planar solids first

Date: 2026-11-07 · Status: Proposed

## Context

ADR 0024's step 5 edits solids: 5a moves them as a whole (done); 5b adds
union, subtraction and intersection. ADR 0024 D3 named Truck the first
temporary backend for them, and its [technology review](../../references/truck.md)
was accepted with the plan to adopt Truck's `master` behind an adapter
(maintainer, 2026-11-07).

Before the first line of the adapter, a spike on the cases architecture
makes every day showed Truck's booleans work only in general position, where
no face of one solid lies on a face of the other:

| Case | Truck (`master` 55548a2) |
|---|---|
| Two boxes overlapping in general position | correct |
| Two boxes apart | correct (two shells) |
| Two boxes touching face to face (walls meeting) | no result |
| A box inside another | panics ("This shell is not oriented and closed") |
| Two boxes on the same floor, or flush at the top | no result |
| Two walls in millimetres crossing at the same level | no result, at any tolerance |
| A slab minus a cylinder longer than it | correct |
| A slab minus a cylinder flush with it, or a blind hole flush at the top | no result |

In architecture coplanar faces are the rule: walls start on the same floor
and reach the same height, openings are flush with walls, slabs with slabs.
A backend that fails there cannot carry the booleans. ADR 0020 aims the
owned kernel at booleans anyway (the roadmap's kernel step 7: "planar, then
analytic, then NURBS"); this ADR brings its first part forward.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on
2026-11-07.

### D1 — The owned kernel computes the booleans of planar solids *(maintainer)*

- `arq-kernel` unions, subtracts and intersects solids whose faces are all
  planes and whose edges are all lines: boxes, wedges, extruded polylines
  without arcs, walls, slabs, rectangular openings. One algorithm gives the
  three operations.
- A solid with curved faces is refused with a clear message, the drawing
  unchanged; curved booleans are their own later decision (owned analytic
  surfaces, or a reviewed backend), with this ADR's tests as their base.
- Truck is not a dependency. Its review is amended with this evidence; it
  stays a reference, and a comparison in tests where it works.

### D2 — The classical B-rep method *(maintainer)*

As CAD kernels do, so the result is clean, as AutoCAD's is (no seams):

1. **Intersect:** each face of one solid against each face of the other
   (boxes first, so most pairs are skipped): crossing planes give segments
   on both faces; coplanar faces give the overlap of their polygons, a 2D
   problem in their plane (D3).
2. **Split:** every face along the segments and overlaps that fall in it,
   into fragments that are each wholly inside, outside or on the other
   solid.
3. **Classify:** each fragment as inside, outside, or on the other solid's
   boundary with the same or the opposite orientation.
4. **Select:** union keeps what is outside the other, plus one copy of each
   same-oriented coplanar fragment; intersection keeps what is inside; A − B
   keeps A outside B, B inside A turned over, and the opposite-oriented
   coplanar fragments of A.
5. **Assemble and tidy:** vertices welded within the tolerance, the
   fragments joined into shells, coplanar adjacent faces merged and
   collinear edges joined, so a union of two touching walls is one block
   with no line where they met. Disjoint results stay separate pieces
   (lumps) of one solid, as AutoCAD keeps them.

Tolerances scale with the model, as the rest of the kernel does (ADR 0010):
millimetre architecture spans thousands of units.

### D3 — Our own 2D polygon booleans *(maintainer)*

- Coplanar faces need union, intersection and difference of polygons with
  holes in their plane: `arq-geometry` gains them, with our tolerances and
  tests, no outside library.
- The same machinery merges coplanar faces after the operation (D2.5).

### D4 — How correctness is shown *(maintainer)*

- **Architecture cases:** walls meeting at the same level in T, L and
  cross; walls touching end to end; slabs with flush openings; a box inside
  a box; boxes sharing a face, an edge, a vertex; in millimetres.
- **Volume identities on random solids:** vol(A ∪ B) = vol(A) + vol(B) −
  vol(A ∩ B) and vol(A − B) = vol(A) − vol(A ∩ B), with the exact measure of
  ADR 0024 step 3, over many generated boxes and prisms, aligned and
  touching on purpose; every result a valid closed body.
- **Against Truck where it works:** in general position, the same volume and
  bounds; Truck only in tests, never in the product.
- **ProgeCAD:** results saved and reopened, measured and audited there, as
  the other solids (`docs/testing`).

### D5 — The commands

- **UNION, SUBTRACT, INTERSECT** (Spanish UNION, DIFERENCIA, INTERSEC) with
  AutoCAD's prompts: SUBTRACT asks for the solids to subtract from, then
  the solids to subtract.
- The result takes the first selected solid's properties (layer, color,
  linetype, lineweight); the others are erased; one undo step (maintainer,
  2026-11-07).
- Disjoint solids united make one solid of separate pieces (maintainer,
  2026-11-07).
- A selection holding a curved solid is refused, said why, nothing changed
  (maintainer, 2026-11-07).
- In the ribbon's 3D tab, a new group **Solid Editing** (Spanish *Edición
  de sólidos*) beside Modeling and Analysis, and in the Modify menu
  (maintainer, 2026-11-07).

### D6 — Delivery, four pull requests *(maintainer)*

1. This ADR, with the Truck review amended.
2. The kernel in general position: intersection, splitting,
   classification, selection and assembly, the three operations, the 2D
   polygon booleans they need.
3. The kernel with coincident geometry: coplanar, touching and contained
   faces, edges and vertices; merging coplanar faces; the full test suites
   of D4.
4. The commands of D5 in the document, the Python API and the ribbon, and
   the results checked in ProgeCAD.

### Progress

- **2 (done):** `Body::union`, `Body::intersection` and `Body::difference`
  in `arq-kernel`, on `Region`'s union, intersection and difference of
  polygons with holes in `arq-geometry`. Each face is split by the other
  solid's section in its plane, a 2D boolean that splits and classifies at
  once; a corner on that plane counts as above it (a symbolic
  perturbation), so a corner or an edge of one solid on a face's plane of
  the other is already handled. Coplanar faces that meet are refused
  (`BooleanError::Coplanar`), as are results touching along an edge or at a
  corner (`BooleanError::NonManifold`), until step 3. A box inside another
  already leaves a void. The comparison with Truck moves to step 3, with
  the coincident cases it fails.
- **3 (done):** coincident geometry. The other solid's section is taken
  on both sides of a face's plane, its corners on the plane counted
  inside (`L`) or outside (`U`): a part of the face in both lies inside
  the other solid, in `L` only on a face of it turned the same way, in `U`
  only on one turned the opposite way, in neither outside. Each operation's
  rule of D2.4 is then one 2D boolean per face (`F − U`, `F ∩ L`, `F − L`
  for the first solid; `G − (L ∪ U)` and `G ∩ L ∩ U` for the second), so
  coplanar faces and solids touching along a face, an edge or a corner need
  no case of their own, and `BooleanError::Coplanar` is gone. The kept
  faces in one plane turned the same way are merged, with their corners on
  straight runs. Where solids touch along an edge, its four faces are
  paired around it, each with the next across the solid between them, so
  each piece keeps its own closed surface (one body, two lumps).
- **D4 evidence:**
  - architecture cases in millimetres: walls meeting in L, T and cross at
    the same level, walls end to end, a slab with flush openings, a pocket
    flush with the top, boxes sharing a face, an edge, a corner, a void
    touching the outside along an edge;
  - 300 random pairs of boxes on a coarse grid, half turned alike,
    against the volume identities;
  - Truck in general position, run by hand in a crate of its own
    (`tools/truck-compare`, maintainer, 2026-11-07): where both answer
    they agree, except where Truck's own results miss the identities
    ([results](../../testing/truck-boolean-comparison.md)).
- **4 (done):** UNION, SUBTRACT and INTERSECT (Spanish UNION, DIFERENCIA,
  INTERSEC), in the 3D tab's Solid Editing group and Modify > Solid
  Editing.
  - The result is written into the first solid chosen, which keeps its
    handle and properties; the others are erased, in one undo step.
  - SUBTRACT, as AutoCAD's, unites the solids to subtract from and takes
    away every solid to subtract (maintainer, 2026-11-07).
  - Nothing changes, and the command says why, when a solid is curved or
    cannot be read, when too few are chosen, or when nothing would be left
    (maintainer, 2026-11-07).
  - The results reopen in ProgeCAD 2026 from DWG 2000 and 2018 and from
    DXF with the same volume and centroid, AUDIT clean
    ([evidence](../../testing/progecad-solid-booleans.md)). That check found
    voids and second lumps lost in DWG 2000, corrected first in the codec
    (#138).

## Consequences

- Booleans of the solids architecture uses most need no third-party
  kernel, and come exact: planes and lines in, planes and lines out.
- The owned kernel's step 7 starts here, ahead of the roadmap's order: its
  acceptance suite (ADR 0020 D6) is D4's.
- Curved booleans (round columns through slabs, cylindrical holes) wait for
  their own decision: [ADR 0033](0033-owned-booleans-milestone.md) makes
  them deliveries of milestone 02.15, owned too.
- The adapter crate planned for Truck is not built; ADR 0024 D3's backend
  stays open for curved solids, each candidate after its own review and a
  coplanar spike like this one.
