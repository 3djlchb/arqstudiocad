# ArqStudioCAD — Roadmap

The order of work towards the platform described in the
[project charter](PROJECT_CHARTER.md). Phases are a sequence, not dates: a
phase ends when its acceptance criteria are met and tested, not when its code
exists. Phases may overlap once their contracts are stable.

**Rule: contracts and architecture first, then a working CAD core, then IFC/BIM
and the specialized disciplines.**

Legend: ✅ done · ◐ in progress · ○ to do.

## Phase 0 — Specification ✅

Technical contract, ADRs, licenses, conventions, acceptance criteria
(contract §12). Charter and roadmap added on 2026-10-13 (ADR 0014).

## Phase 1 — Technological foundation ✅

Cargo workspace, uv, PyO3 + maturin, PySide6 window with i18n, local checks,
manual CI on every platform, packages for every target (ADR 0001–0006,
ADR 0009). Merged in PRs #1–#4.

## Phase 2 — CAD core ◐

| Milestone | Content | Status |
|---|---|---|
| 2.1 Geometry | `arq-geometry`: points, vectors, lines, circles, arcs, transformations, intersections, snaps (ADR 0010) | ✅ PR #5 |
| 2.2 Document | `arq-document` over opencadcodec: DWG/DXF open and save, entities, layers, ids, change recording; `arq-core`: transactions, groups, undo/redo, queries, document store; round-trip corpus (ADR 0011) | ✅ PR #6 |
| 2.3 Python API | `arq-python`: the whole core from Python, typed (ADR 0012) | ✅ PR #7 |
| 2.4 Application shell | Drawing primitives, render scene and CPU rasterizer, viewport, events, commands and command window, tabs, background open/save, panels (pinned or auto-hidden), Ribbon and Classic workspaces, dark and light themes (ADR 0013, ADR 0015) | ✅ PR #8 |
| 2.5 Editing workflow | Selection (pick, window, crossing), multifunction grips, object snaps and ORTHO, AutoCAD point syntax, LINE, CIRCLE, ARC, MOVE, COPY, ROTATE, SCALE, MIRROR, ERASE, blocks and polylines transformed as a whole, property editors, Spanish command names (ADR 0016) | ◐ branch `fase-2.5/edicion` |
| 2.6 Spatial index and GPU viewport | Spatial index (ADR 0011 D3.8); wgpu renderer with backend probing (ADR 0007); the CPU rasterizer stays as fallback | ○ |
| 2.7 Wider CAD entities | Editing polyline vertices, block contents and explode, texts; 3D primitives and the 3D document model (an ADR first) | ○ |
| 2.8 Projects | The project container: models, drawings, views, sheets, resources (an ADR first) | ○ |

## Phase 3 — CAD exchange and 2D documentation ○

DWG/DXF fidelity across versions, DWG as the default save format (contract
§10). Linetypes, lineweights, texts and fonts, dimensions, hatches, layouts
and sheets, plotting to PDF. Projections, sections and elevations as
**associative views** with graphic styles (ADR 0014).

## Phase 4 — IFC and BIM ○

IfcOpenShell integration: open, inspect, create and modify BIM models;
GlobalId, properties and relationships kept through every operation;
import/export validation; views and documentation generated from the model.
Never a collection of anonymous meshes. Model ownership decided by ADR first
(ADR 0014, open question 1). The BIM workspace takes ArchiCAD's interface as
its reference, in the Ribbon layout (ADR 0015).

## Phase 5 — Topography and civil engineering ○

Survey points, TIN, contour lines, slopes, profiles, cross sections, cut and
fill volumes; QGIS bridge for maps.

## Phase 6 — Disciplines ○

Architecture, structures (FEM engine verified against benchmarks), MEP,
environmental studies, transport and traffic — each an independent module
through public APIs and plugins.

## Phase 7 — Visualization ○

Scenes, PBR materials, lights, cameras; Blender + Cycles headless; renders,
image sequences and MP4 walkthroughs; a render queue with progress and
cancellation.

## Phase 8 — Interactive walkthroughs and coordination ○

Real-time navigation on the GPU renderer; federated models, clash detection,
quantities, change tracking, advanced associative documentation.

## Phase 9 — LuxCoreRender ○

Second render engine, after comparing quality, speed, compatibility and
upkeep with Cycles on shared test scenes.

## Phase 10 — Optimization and distribution ○

Large models and terrains, performance profiles, file safety, plugins,
signed installers, notarization, Flathub, AUR; public releases.

## Branches

Branch per milestone: `fase-<milestone>/<topic>`, e.g. `fase-2.5/edicion`.
Branches started before this roadmap keep their names (`fase-5/aplicacion`
is milestone 2.4). Small commits; a PR to `main` per milestone.
