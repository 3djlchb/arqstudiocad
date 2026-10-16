# 0019. 3D geometric foundations

Date: 2026-10-15 · Status: Accepted · Supersedes the timing statement in ADR 0010 about when `Point3` and `Vector3` arrive.

## Context

The CAD core has validated 2D value types in `arq-geometry`. The roadmap places
3D primitives and the 3D document model in milestone 02.8, but ADR 0010 still
says that 3D point and vector types arrive with the DWG/DXF engine. The codec
already uses its own 3D types to preserve format data; those are not the
platform's geometric API or an owned 3D document model.

We need a small, dependency-free 3D mathematical layer that can be tested and
used by later CAD, documentation, rendering and BIM adapters without deciding
those systems' ownership or implementing a solid kernel prematurely.

## Decision

1. 3D foundational types live in `arq-geometry`, alongside the existing 2D
   types. They use private `f64` fields, fallible constructors for external
   values, and no third-party math library.
2. Coordinates are right-handed Cartesian model coordinates: X and Y retain
   the existing 2D plane, positive Z points up, and `X × Y = +Z`. Values remain
   in document model units; the geometry layer does not assume metres or
   convert units.
3. `Point3` is a position and `Vector3` a displacement. Point/vector
   arithmetic follows the 2D rules. Vector dot and cross products use the
   right-handed convention.
4. `Bounds3` is a non-empty, axis-aligned box. Empty collections are
   represented by `Option<Bounds3>::None`, as with `Bounds2`.
5. `Transform3` represents a finite affine map with a 3×3 linear part, a local
   pivot and a translation. Applying it uses `pivot + M * (point - pivot) +
   translation` to preserve precision at large coordinates. Composition,
   inversion, translation, axis-angle rotation and positive per-axis scaling
   are provided. Inversion of a singular map returns a geometry error.
6. Geometry algorithms use explicit tolerances and local frames. Unit and
   property tests cover the algebra, invalid input, transform laws and
   coordinates near 1e6 model units.
7. This decision adds mathematical values only. It does not define 3D document
   entity ownership, DWG/ACIS conversion, IFC geometry, curves, surfaces,
   tessellation, B-rep topology, boolean operations, or a rendering pipeline.
   ADR 0014's open question about 3D in the document model remains open and
   must be decided before those features are implemented.

## Consequences

- The 3D foundation can be developed in the existing `arq-geometry` crate;
  no crate or external geometry engine is introduced.
- Existing 2D types and behavior remain available and unchanged.
- ADR 0010 remains the source for existing 2D angle and tolerance conventions;
  this ADR supersedes only its outdated 3D timing statement.
- Milestone 02.8 gains a testable first step, while document integration and
  solid modeling remain separately scoped work.
