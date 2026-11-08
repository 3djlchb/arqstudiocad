# 0033. Owned booleans, a milestone of their own

Date: 2026-11-07 · Status: Proposed

## Context

[ADR 0032](0032-owned-planar-booleans.md) brought the owned kernel's
booleans forward for solids of flat faces, after Truck failed with coplanar
faces ([review](../../references/truck.md)). They are done:

- the 2D polygon booleans (`Region`) and the kernel in general position
  (#133);
- coincident faces, edges and corners, and merged faces (#137);
- UNION, SUBTRACT and INTERSECT, checked in ProgeCAD (#139).

On the way, ProgeCAD's check found voids lost in DWG 2000, corrected in the
codec (#138).

Curved solids are still refused. In architecture they come right after
walls and slabs:

- round columns through slabs, circular openings and drilled holes;
- domes and vaults;
- pipes, round ducts and handrails meeting each other.

ADR 0032 left them to "their own decision". ADR 0020 aims the owned kernel at
booleans "planar, then analytic, then NURBS". ADR 0024 D3 still names a
temporary backend for what the kernel cannot do.

The maintainer asked, once ADR 0032 was complete, for the owned booleans to
become a milestone with its own deliveries (2026-11-07).

## Decision

Each choice marked *(maintainer)* was made by the maintainer on
2026-11-07.

### D1 — Milestone 02.15, owned all the way *(maintainer)*

- **The milestone:** 02.15 **Owned booleans**, of phase 02. Milestone 02.8
  keeps what it delivered: ADR 0024's step 5b, the planar booleans.
- **No backend:** the kernel computes every boolean itself, curved solids
  included. No external kernel is a dependency for booleans; ADR 0024 D3's
  temporary backend stays open only for its step 8 (fillets, chamfers,
  shells, offsets, sweeps, lofts), each candidate after its own review and
  a coplanar spike as ADR 0032's.
- **Same method:** ADR 0032 D2, the classical B-rep one. Each face is split
  by the other solid where it meets it, the parts are classified inside,
  outside or on, and the operation keeps them by the rule of D2.4. The
  section is taken on both sides of a face, as for planes, so coincident
  faces need no case of their own.

### D2 — Deliveries, in the order architecture needs them *(maintainer)*

Each delivery is its own pull requests, accepted only with D4's evidence;
the commands of ADR 0032 D5 take each new kind of solid as soon as its
delivery is done, and refuse the rest as they do now:

| Delivery | Solids combined | Architecture |
|---|---|---|
| **1. Planar** *(done)* | Flat faces, straight edges | Walls, slabs, openings, boxes, wedges, extruded polylines |
| **2. Planes with cylinders and cones** | Any of them with flat-faced solids, and with each other where their axes are parallel | Round columns through slabs, circular openings, drilled holes, extruded circles and arcs, cones |
| **3. Spheres and tori** | With flat-faced solids, cylinders and cones on a common axis | Domes, vaults, rounded ends, rings |
| **4. Curved with curved** | Cylinders, cones, spheres and tori in any position | Pipes and ducts meeting, a round column through a vault, handrails |
| **5. NURBS** | Faces of NURBS surfaces, read from drawings or made by later commands | Free forms (ADR 0031) |

### D3 — Intersection curves, exact where they can be *(maintainer)*

- **Exact where they are conics:** the curves where a plane meets a
  cylinder, a cone, a sphere or a torus's plane of symmetry are lines,
  circles and ellipses. They are kept exact, as the kernel's `LineCurve`
  and `EllipseCurve`.
- **Rational NURBS for the other conics:** a parabola or hyperbola from a
  plane and a cone is written as an exact rational NURBS curve.
- **Fitted NURBS between two curved surfaces:** a fitted NURBS within the
  tolerance, as ACIS and Parasolid do. Each such curve records the
  tolerance it was fitted to, and its points lie on both surfaces within
  it.
- **Tolerances scale with the model,** as everywhere in the kernel (ADR
  0010).

### D4 — Evidence, for every delivery *(maintainer)*

As ADR 0032 D4, for the new kinds:

- **Architecture cases with exact volumes**, in millimetres where they are
  walls and slabs: a slab less a round column, flush and through; a drilled
  hole and a blind one; a dome on a drum; pipes meeting. Each result is
  measured exactly where a closed form exists (π, ellipse areas).
- **Volume identities on random solids:** vol(A ∪ B) = vol(A) + vol(B) −
  vol(A ∩ B) and vol(A − B) = vol(A) − vol(A ∩ B), aligned and coincident on
  purpose. Every result must be a valid closed body.
- **Truck where it works:** `tools/truck-compare`, run by hand, extended to
  curved solids in general position, where Truck's booleans worked (a slab
  less a cylinder longer than it).
- **ProgeCAD:** the results saved in DWG 2000 and 2018 and in DXF,
  reopened, measured and audited (`docs/testing`).
- **Tests and sabotage:** the kernel's tests at the right layer (contract
  §12), each change verified by sabotage.

### D5 — What each delivery needs first

- **Splitting curved faces:** a curved face is split in its own parameters
  `(u, v)`, as a planar one is in its plane. The 2D booleans therefore grow
  curved edges (arcs, and NURBS in parameter space), and closed surfaces
  are cut along their seams (delivery 2).
- **Classifying parts:** a part of a curved face is classified by a point
  of it against the other solid's boundary, curved faces included
  (delivery 2).
- **Writing the results:** the ACIS writer must write what the results
  hold. It already writes lines, circles, ellipses, planes, cones,
  cylinders, spheres and tori. NURBS curves (from deliveries 2 to 4) and
  NURBS surfaces (delivery 5) come with the delivery that first needs them.
- **Measuring:** every result measures exactly where its faces are analytic
  (ADR 0024 step 3, now in closed form for cones, cylinders, spheres and
  tori). Faces bounded by fitted curves measure to the curves' tolerance.

### D6 — Outside this milestone

- Fillets, chamfers, shells, offsets, sweeps and lofts: ADR 0024 step 8.
- Booleans of surfaces and regions (sheets, not solids).
- Making NURBS solids: ADR 0031 (Free Geometry) decides how they are made.
  This milestone only combines them.

## Progress

- **Delivery 2, four pull requests** (maintainer, 2026-11-07), as ADR
  0032's:
  1. the 2D booleans with curved edges;
  2. planes with cylinders in general position;
  3. the coincident cases (flush holes and columns), cones and cylinders
     on parallel axes;
  4. the commands, Truck and ProgeCAD.

  Curved edges are kept exact, their crossings found numerically
  (maintainer, 2026-11-07).
- **2.1 (done):** `CurvedRegion` in `arq-geometry`, regions bounded by
  exact trims:
  - **Trims:** segments, arcs of ellipses (circles among them), and graphs
    of rational trigonometric functions, what a plane leaves on a cylinder
    or a cone in its parameters.
  - **Method:** `Region`'s edge selection.
    - Crossings are seeded where fine polylines meet and refined by
      Newton's method on the exact curves; ends lying on other trims are
      found by projection; trims running along each other are split by
      their ends.
    - Pieces are told apart by their ends and their middle.
    - Contours are chained by the leftmost turn, tangent ties broken by
      curvature, and consecutive pieces of one curve are joined again.

## Consequences

- Solids architecture uses most combine without a third-party kernel. Each
  delivery widens what UNION, SUBTRACT and INTERSECT accept, and the
  refusal message narrows with it.
- The kernel grows curved 2D booleans in parameter space, curve–surface and
  surface–surface intersection, and NURBS in the ACIS writer. Each is built
  when a delivery needs it, and each serves later work: drawing, sections,
  and ADR 0031.
- ADR 0024 D3's temporary backend no longer covers booleans. Its review,
  when step 8 comes, has a smaller scope.
- ADR 0020's kernel step 7 is this milestone; its acceptance suite is D4's.
- The milestone is long. Its deliveries are ordered so that each one is
  useful on its own.
