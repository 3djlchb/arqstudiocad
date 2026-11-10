# 0035. Vertical and horizontal phases, and the first release

Date: 2026-11-10 · Status: Proposed · Amends [ADR 0014](0014-platform-direction.md)
(the roadmap's phases)

## Context

ADR 0014 replaced the technical contract's phase list with a roadmap of
phases in one sequence, and master plan 2.0 numbered them 00–11. The
maintainer reviewed that sequence on 2026-11-10, against the state of the
code:

- **Phase 02 does not yet meet its exit criterion, "create, edit, save and
  recover a basic drawing".** Its basic 2D commands are missing. The code
  draws lines, circles, arcs, polylines and texts, and moves, copies,
  rotates, scales, mirrors and erases them, but has no OFFSET, TRIM,
  EXTEND, FILLET, CHAMFER, RECTANG, ELLIPSE, SPLINE, ARRAY, BREAK, JOIN or
  MATCHPROP, and STRETCH only as a grip mode (checked 2026-11-10).
- **Free Geometry ([ADR 0031](0031-free-geometry.md)) was milestone 02.14
  of the CAD core.** It is a way of modeling built on the core, a workspace
  of its own, not part of what every drawing needs.
- **Nothing said when the first release comes.** Master plan §14 says
  when *a* release is ready, not which work ends before the first one.
- **Phases 05–11 are modules.** BIM, topography, the disciplines,
  rendering, coordination, extensions and distribution build on the core
  through its APIs. Their numbers read as a strict order they do not
  have, and a new module had no place but in the middle of the sequence.

## Options

1. **Keep one sequence, 00–11,** and insert Free Geometry and the basic
   commands where they fit, renumbering what follows each time.
2. **Vertical and horizontal phases:** the core in sequence up to the
   first release, then modules on top of it, numbered in the order they
   are added.

## Decision

Option 2 *(maintainer, 2026-11-10)*.

### D1 — Vertical and horizontal phases

- **Phases 00–04 are vertical:** the core, built in sequence. They end with
  the first release.
- **Phases 05 on are horizontal:** modules on the core, through its APIs.
  New phases are added as the project grows.

### D2 — Two rules for horizontal phases

1. **A horizontal phase never changes the core on its own.** What it needs
   of the core is asked for as a milestone of a vertical phase, or, when
   how the core changes must be decided, by an ADR.
2. **A horizontal phase's number tells when it was added, not a strict
   dependency.** New phases go at the end; none is renumbered.

### D3 — The first release closes the vertical phases

The first release is a milestone of phase 04, after the parametric engine
(04.1) and the documentation that completes the core
*(maintainer, 2026-11-10)*:

| Milestone | Content |
|---|---|
| 04.2 | Dimensions |
| 04.3 | Hatches |
| 04.4 | Layouts |
| 04.5 | Plotting to PDF |
| 04.6 | **The first release** |

Its exit criterion, beyond master plan §14:

- OpenGL drawing black on the Intel UHD 630 fixed, or excluded with the
  CPU as fallback ([ADR 0034](0034-gpu-viewport-direct-path.md) D6.4);
- the curved booleans of 02.15 done, or refused with a clear message;
- `docs/KNOWN_LIMITATIONS.md` (or its equivalent), a CHANGELOG, and a short
  guide to installing and using the application;
- the installer tried on a clean machine, the reference floor machine
  included, with `--check`;
- a trademark note in both READMEs;
- a legal review of the name before the release is publicized.

The official release stays subject to the maintainer's decision that the
application is mature (ADR 0014 D6).

### D4 — Milestone 02.14: the basic 2D commands

So that phase 02 meets its exit criterion, a new milestone 02.14 adds the
basic 2D commands, in the order architecture uses them *(maintainer,
2026-11-10)*:

1. RECTANG, OFFSET, TRIM, EXTEND: walls and openings;
2. FILLET, CHAMFER: corners;
3. STRETCH as a command, ARRAY, BREAK, JOIN: editing in bulk;
4. MATCHPROP;
5. ELLIPSE, SPLINE: new curves, which need new entities in the own model.

Each with its Spanish name, as every command (ADR 0016). Milestone 02.15,
owned booleans ([ADR 0033](0033-owned-booleans-milestone.md)), keeps its
number.

### D5 — Free Geometry becomes phase 05

Free Geometry leaves phase 02 and becomes phase 05, the first horizontal
phase, of module M01 CAD 2D/3D *(maintainer, 2026-11-10)*. Its milestones
05.1… are defined later with the maintainer; ADR 0031 D13 gives the order
to start from. The phases after it move one number up:

| Phase | Before | Now |
|---|---|---|
| Free Geometry | milestone 02.14 | 05 |
| BIM and IFC | 05 | 06 |
| Topography and surfaces | 06 | 07 |
| Architecture, structures and MEP | 07 | 08 |
| Rendering and animation | 08 | 09 |
| Coordination and walkthroughs | 09 | 10 |
| Extensions and advanced optimization | 10 (milestones 10.1–10.4) | 11 (11.1–11.4: ArqLISP, ArqBasic) |
| Cross-platform distribution | 11 | 12 |

## Consequences

- The core has an end: phases 00–04 and the first release. Work on a
  module no longer delays it, and a module no longer waits for its number.
- By D2.1, what Free Geometry needs of the core (shading, picking in 3D,
  the working plane, a MESH entity in the document, NURBS surfaces in the
  ACIS writer: ADR 0031 D3–D6, D13) is asked for as milestones of the
  vertical phases when its milestones 05.x are defined. This ADR does not
  place them.
- Phase 02 gains thirteen commands before it can close, and phase 04 four
  milestones and the release.
- Old numbers stay readable: the roadmap's table of old names maps them,
  and the ADRs that used them keep their history, each with a note that
  points here.
- The master plan goes to version 2.2: its table of phases, the two rules,
  and §9.1 (ArqLISP and ArqBasic, now phase 11).
