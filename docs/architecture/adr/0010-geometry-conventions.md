# 0010. Geometry conventions (`arq-geometry`)

Date: 2026-10-11 · Status: Accepted

## Context

Phase 2 builds `arq-geometry`, the dependency-free base of the CAD core:
points, vectors, bounds, lines, circles, arcs, transformations and
intersections. Its conventions are hard to change later, because every other
layer and the DWG/DXF files depend on them.

## Decision

### Types

- **Own types**, no math library: `Point2` (a position) and `Vector2` (a
  displacement) are distinct types, so the compiler rejects meaningless
  operations (point + point). Point − point = vector; point + vector = point.
- **`f64`** in model units (contract §3).
- **Validated construction.** Fields are private; constructors return
  `Result<_, GeometryError>` and reject non-finite values and degenerate
  shapes (radius ≤ 0, zero-length segment, zero sweep). An invalid geometry
  cannot exist, so operations never re-validate. Read access is through
  methods. This refines the public-field sketch in the contract.
- **2D only in phase 2.** `Point3`/`Vector3` and the entity plane (DWG's
  object coordinate system) arrive with opencadcodec in phase 3.

### Angles

- **The core and the Python API use radians, always.** No function of the
  core accepts or returns degrees.
- **0 points East (+X); counterclockwise is positive** (AutoCAD/DWG).
- **Arcs follow DWG:** `center`, `radius`, `start_angle`, `end_angle`; the arc
  runs **counterclockwise** from start to end; both angles are normalized to
  `[0, 2π)`; the sweep is in `(0, 2π)`. Nothing to translate when reading or
  writing DWG/DXF.
- **The user sees and types degrees.** Commands and the interface convert at
  the presentation boundary (Python), when reading input and when displaying.
  A later UNITS command may change the displayed format — decimal degrees,
  degrees/minutes/seconds, grads, surveyor bearings — stored per drawing like
  DWG's `$AUNITS`, `$ANGBASE`, `$ANGDIR`. The core stays in radians.

### Tolerance

- An explicit **`Tolerance { linear, angular }`** value is passed to every
  operation that compares or intersects. No literal epsilons in the code.
- `Tolerance::DEFAULT`: **linear 1e-9 model units, angular 1e-10 rad** —
  absolute, which stays meaningful at survey coordinates (~1e6, where `f64`
  still resolves ~1e-10). A document may adjust it to its units later.

### Precision: local frames

Adopted from opencadkernel (`docs/references/opencadstudio.md`): algorithms
that combine coordinates (intersections, projections, containment) first
translate their inputs into a local frame near the data, compute there, and
translate results back. Georeferenced drawings keep their precision.

### Own implementation

The basics are written here. `opencadkernel` (MPL-2.0) is re-evaluated when
polylines with bulges, NURBS, offsets and booleans are needed.

### Tests

Unit tests for every operation and edge case, plus **property tests**
(`proptest`, dev-dependency, MIT/Apache) for laws that must always hold, plus
precision tests repeating cases near the origin and at ~1e6.

## Consequences

- The Python layer owns degree ↔ radian conversion and the angle display
  format; Rust never needs to know about it.
- DWG import/export maps arcs and angles one-to-one.
- Constructors are fallible: callers handle `GeometryError`, mapped to
  `CadError::InvalidGeometry` in phase 3.
