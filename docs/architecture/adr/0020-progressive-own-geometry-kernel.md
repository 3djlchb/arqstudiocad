# 0020. Progressive development of an owned geometry kernel

Date: 2026-10-15 · Status: Accepted

## Context

ArqStudioCAD's long-term goal is to own its geometry kernel in Rust. A complete
CAD kernel is too broad to build in one step, and early operations need
reference behavior and proven results. The project can study Open CASCADE
Technology (OCCT), Truck, opencadkernel, and OpenCADStudio while implementing
its own algorithms incrementally.

OCCT is primarily a C++ library and its upstream project publishes it under
LGPL-2.1 with the Open CASCADE exception. Truck publishes an Apache-2.0
license; opencadkernel publishes MPL-2.0; OpenCADStudio publishes GPL-3.0.
Those licenses and implementation languages make their roles different:
reference, temporary backend, dependency, and source reuse each need an
explicit decision.

## Decision

1. **Long-term ownership.** ArqStudioCAD will progressively implement and
   maintain its own geometry kernel in Rust. Existing 2D geometry remains
   supported and evolves alongside 3D.
2. **Use of references.** Study OCCT, Truck, opencadkernel, and OpenCADStudio
   for algorithms, architecture, test cases, and engineering practices. A
   reference does not by itself authorize copying source code or adding a
   dependency.
3. **Temporary OCCT backend.** For early 3D operations, OCCT may provide a
   temporary reference or execution backend for differential tests and, where
   a specific feature requires it, an adapter-backed implementation path.
   Each such integration must first pass the master plan's technology and
   license review, including the exact OCCT version, C++ bridge, packaging,
   supported platforms, and distribution obligations. This ADR does not add
   OCCT as a dependency.
4. **Stable boundary.** External kernel types and ownership stay behind an
   adapter. Public core and document contracts use ArqStudioCAD-owned types;
   the rest of the application must not depend on OCCT, Truck, or
   opencadkernel types. The temporary backend can be disabled or replaced
   without changing those contracts.
5. **Incremental substitution.** Compare and migrate by named capability
   (for example, curve evaluation, surface evaluation, tessellation, or a
   specific solid operation), not by claiming global parity with OCCT. Keep
   the external backend available for comparison while the owned
   implementation is developed. Route a capability to the owned kernel only
   after its acceptance suite passes; remove the temporary backend only when
   no supported feature or required comparison still uses it.
6. **Parity evidence.** Each capability's acceptance suite records reference
   inputs and outputs, geometric invariants, degeneracies and failure cases,
   tolerances and units, georeferenced precision, and deterministic behavior
   where required. Performance and memory are measured on reproducible inputs
   and reported separately from correctness. “At par” means passing the
   declared suite for that capability; it does not mean feature-for-feature
   equivalence to all of OCCT.
7. **Source reuse and attribution.** Any copied or adapted code is reviewed
   against the repository's GPL-3.0-or-later compatibility rule, keeps its
   required notices, and is recorded in `THIRD_PARTY.md`. Ideas and test
   methods are attributed there as design references. If source reuse is not
   clearly compatible, reimplement from independently derived requirements
   and tests instead of copying.
8. **No premature solids scope.** The 3D value types in ADR 0019 are the
   mathematical foundation only. Curves, surfaces, B-rep topology, solid
   validation, tessellation, and boolean operations each enter the roadmap as
   bounded capabilities with their own tests and acceptance criteria. This
   ADR does not decide 3D document ownership; ADR 0014 question 4 remains open.

## Consequences

- The owned Rust kernel remains the destination; OCCT is an optional,
  temporary bridge for specific early capabilities, not the public model.
- Differential testing can expose gaps before an operation is routed to the
  owned implementation.
- OCCT integration is gated by a separate adapter/build/license review and is
  not part of the already implemented `arq-geometry` value types.
- Kernel progress is reported as capability-level test evidence, not as a
  broad claim of matching another product.
