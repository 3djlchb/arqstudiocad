# Technology review: Truck, the first boolean backend

Reviewed 2026-11-07 for ADR 0024 step 5b (booleans), under the master plan's
rule for new technologies (§16) and ADR 0020 (amendment 2: any reviewed
reference may be a temporary backend, behind an adapter). Repository:
<https://github.com/ricosjp/truck> · License: Apache-2.0.

## What it is

A Rust B-rep modeling kernel by RICOS (Japan), a workspace of crates:
`truck-base` (cgmath-based math, tolerances), `truck-geotrait`,
`truck-geometry` (knot vectors, B-splines, NURBS), `truck-topology`
(vertex, edge, wire, face, shell, solid), `truck-modeling` (builders: sweeps,
revolutions, attached planes), `truck-meshalgo` (tessellation),
`truck-shapeops` (boolean `and` / `or` on solids; subtraction is `and` with
the other solid turned inside out), `truck-stepio` (STEP), and rendering
crates we would not use. 1.7k stars, 33 open issues at review.

## Versions

| | Published (crates.io) | Repository `master` |
|---|---|---|
| Revision | `truck-shapeops` 0.4.0, `truck-modeling` 0.6.0, `truck-meshalgo` 0.4.0 | `55548a27de0af3e87cceeb677b809aa110a87028` |
| Date | 2024-09-20 | 2026-11-02 (commits through October and November 2026) |
| API | `builder::rsweep(v, origin, axis, angle)` | `rsweep` takes a division count too: the API moves |

The last release is two years old; fixes since (intersection curves,
STEP conics, CI) are only on `master`.

## Measured (isolated spike, not in the repository)

Three booleans on solids built with `truck-modeling`, volumes from a
0.002-chord tessellation, Windows release build:

| Operation | 0.4.0 / 0.6.0 | `master` 55548a2 |
|---|---|---|
| Union of two 2-unit cubes overlapping by one unit (exact 15) | 15.0, 5.6 ms | 15.0, 4.0 ms |
| Their intersection (exact 1) | 1.0 | 1.0 |
| Cube minus a cylinder r 0.5 through it (exact 6.429204) | 6.433593 (+0.07 %), 180 ms | 6.431739 (+0.04 %), 95 ms |

The excess on the curved case comes from the approximated intersection
curves and the tessellation used to measure; our own measure of the
converted result will tell them apart.

- **Build:** about 80–85 s from scratch for the three crates; the spike's
  release binary 1.6 MB.
- **Dependencies:** 125 crates (published), 138 (`master`).
- **Licenses:** all pass our `deny.toml`.
- **Advisories:** our `deny.toml` fails, on both revisions:
  - `quick-xml` (two denial-of-service advisories, three on the published
    revision with a decompression one), only through `vtkio`, the VTK
    exporter `truck-meshalgo` carries; we would never read or write VTK;
  - `cgmath` unmaintained: Truck's own linear algebra, through
    `matext4cgmath` and `truck-base`;
  - `proc-macro-error` unmaintained: a compile-time macro crate of
    `ruststep`, through `truck-stepio`, which `truck-shapeops` depends on;
    we would not read STEP through Truck.

## Against the rule (§16)

| Question | Answer |
|---|---|
| Does it improve a concrete objective? | Yes: union, subtraction and intersection of solids (ADR 0024 step 5), which the owned kernel cannot do yet. |
| Does it respect the core's independence? | Yes, behind an adapter crate of its own: `arq-kernel` does not depend on it, its types never cross the adapter (ADR 0020 D4). |
| Does it keep the integration contract stable? | Yes: the document and Python see our B-rep, before and after. |
| Is its license compatible? | Apache-2.0, compatible with GPL-3.0-or-later (one-way); notices kept in `THIRD_PARTY.md`. |
| Upkeep, documentation, tests? | Active upkeep; thin API documentation (no tolerance semantics stated); tests in the repository. No release for two years. |
| Does it improve a capability we need? | Yes, the booleans; tessellation and STEP we already have or do not need from it. |
| Can we replace it? | Yes: by capability, as the owned kernel passes each acceptance suite (ADR 0020 D5). |

It passes the essentials with two conditions: the advisories are handled
explicitly, and the geometry it returns is converted carefully (below).

## The real cost: geometry in and out

- **In:** our faces (planes, cones and cylinders, spheres, tori, NURBS) and
  edges (lines, ellipses, NURBS) are translated to Truck's surfaces and
  curves (plane, revolved curve, B-spline or NURBS surface; line, B-spline,
  NURBS curve).
- **Out:** Truck returns the new edges where solids cut each other as
  intersection curves approximated within the tolerance. AutoCAD returns an
  exact circle where a cylinder meets a plane. The ACIS writer writes lines,
  ellipses, planes, cones, spheres and tori today, no NURBS curve.
- **Tolerance:** Truck's operations take an absolute tolerance; ours scale
  with the model (architecture in millimetres spans thousands of units).

## Decisions *(maintainer, 2026-11-07)*

1. **Revision:** `master` pinned by exact commit (`55548a2` at review),
   recorded in `THIRD_PARTY.md` and allowed as a Git source in `deny.toml`;
   each update repeats this review's measurements.
2. **Advisories:** justified exceptions in `deny.toml`, each with its reason
   (VTK and STEP code never called; unmaintained crates), and a test that the
   adapter never reads or writes VTK or STEP through Truck; reviewed at each
   update.
3. **Result geometry:** exact where it can be, as AutoCAD: edges and faces
   recognized as lines, circles, ellipses, planes, cylinders and cones are
   kept exact; only free-form remains NURBS, for which the ACIS writer learns
   NURBS curves (and the surfaces it does not write yet).
4. **Delivery, four pull requests:** 5b-1 this review; 5b-2 the adapter
   crate, conversions both ways and UNION of planar solids; 5b-3 curved
   surfaces and NURBS in the ACIS writer; 5b-4 SUBTRACT and INTERSECT
   (Spanish DIFERENCIA, INTERSEC), with AutoCAD's prompts.

## Amendment: coplanar faces *(maintainer, 2026-11-07)*

The spike above tested only solids in general position. Before building the
adapter, the cases architecture makes every day were tried on `master`
`55548a2`:

| Case | Result |
|---|---|
| Two boxes overlapping in general position | correct, 12 faces, volume exact |
| Two boxes apart | correct, two shells |
| Two boxes touching face to face, centred or offset | `None` |
| A box inside another | panic: "This shell is not oriented and closed" |
| Two boxes sharing the floor, or flush at the top | `None` |
| Two walls in millimetres (4000 × 200 × 2700 and a crossing one), same level | `None`, with tolerance 0.05 or 5 |
| A slab minus a cylinder longer than it | correct |
| A slab minus a cylinder flush with it; a blind hole flush at the top | `None` |

Truck's booleans fail whenever a face of one solid lies on a face of the
other, which in architecture is the rule. Decision 1 above is withdrawn:
Truck does not become a dependency. The planar booleans are owned instead
([ADR 0032](../architecture/adr/0032-owned-planar-booleans.md)); Truck stays a
reference, and a comparison in tests in general position. Any backend
proposed later, for curved solids, gets a coplanar spike like this one before
its review is accepted.
