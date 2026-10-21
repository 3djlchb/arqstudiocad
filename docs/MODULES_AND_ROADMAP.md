# ArqStudioCAD — Modules and roadmap

The order of work of the [master plan](MASTER_PLAN.md) (version 2.1,
2026-10-13), and where the project stands. Phases are a sequence, not
dates: a phase ends when its exit criterion is met and tested, not when its
code exists. Phases may overlap once their contracts are stable.

Legend: ✅ done · ◐ in progress · ○ to do.

## Where we are (2026-10-21)

- **Technical MVP: 8 of 8** (master plan §15): milestone 02.6 saves and
  recovers the working document (DWG) from the own internal model.
- **Working format:** DWG, as BricsCAD does, with an own internal model; the
  native format `.ascad` is designed and reserved (ADR 0018).
- **Data protections (03.3):** done (ADR 0017 D5).
- **On hold:** milestone 02.7's GPU viewport waits for the reference
  machine (GT 730 on LMDE 7), its release gate (ADR 0007); the work
  resumes when the maintainer has it at hand (maintainer, 2026-10-18).
- **Current:** milestone 02.8: texts are done, single-line and basic
  multiline (ADR 0022); blocks are done (ADR 0023: EXPLODE with
  attributes, BLOCK and INSERT, and the block editor); dynamic blocks wait
  for the parametric engine, milestone 04.1 (D3a).
- **Before:** milestone 02.7 uses the spatial index for bounding-box
  queries and as the broad phase for picking, window/crossing selection, and
  object snaps; exact geometry checks remain in place. A crash-isolated wgpu
  probe checks adapter, pipeline, offscreen draw and readback; it passed locally
  on Windows with Intel UHD 630 / DX12. GPU viewport rendering and reference
  Linux validation remain open. Milestone 02.5 is complete and manually
  validated by the maintainer, including property editors, shared panel/table
  styling, and CAD color selection; 03.2 is done and 03.4 compatibility work
  is in progress.
- **Milestone 02.8** is in progress alongside 02.7: the 3D mathematical
  foundation, polyline vertex editing and block-reference EXPLODE are done;
  texts are done (ADR 0022); block content editing and the 3D document
  decision remain.
- Before the master plan the phases were numbered 0–10 (ADR 0014) and
  milestones 2.1–2.8; the table at the end maps old names to new ones.

## Phase 00 — Specification and contracts ✅

Technical contract, ADRs, licenses, conventions, acceptance criteria;
charter, master plan and governing documents (2026-10-13).
Exit: decisions documented and a compilable skeleton.

## Phase 01 — Technological foundation ✅

Cargo workspace, uv, PyO3 + maturin, PySide6 window with i18n, local checks,
CI on every pull request and push to `main`, packages for every target
(ADR 0001–0006, 0009). PRs #1–#4.
Exit: a minimal reproducible application on Windows showing a result
computed in Rust.

## Phase 02 — CAD core ◐

Exit: create, edit, save and recover a basic drawing.

| Milestone | Content | Status |
|---|---|---|
| 02.1 Geometry | `arq-geometry`: points, vectors, lines, circles, arcs, transformations, intersections, snaps (ADR 0010) | ✅ PR #5 |
| 02.2 Document | `arq-document`: entities, layers, ids, change recording; `arq-core`: transactions, groups, undo/redo, queries, document store; round-trip corpus (ADR 0011) | ✅ PR #6 |
| 02.3 Python API | `arq-python`: the whole core from Python, typed (ADR 0012) | ✅ PR #7 |
| 02.4 Application shell | Drawing primitives, CPU rasterizer, viewport, events, commands and command window, tabs, background open/save, panels, Ribbon and Classic workspaces, themes (ADR 0013, 0015) | ✅ PR #8 |
| 02.5 Editing workflow | Selection, multifunction grips, object snaps and ORTHO, AutoCAD point syntax, LINE…ERASE, blocks and polylines transformed as a whole, property editors, Spanish command names (ADR 0016) | ✅ steps 2.5.1–2.5.8 complete and manually validated by the maintainer |
| 02.6 Own internal model | Stage 1 of ADR 0018 D2: layers, current layer, LINE, CIRCLE, ARC owned by our model; the DWG engine reads into it and writes from it; save and recover the DWG from the own model; written through to the engine, checked after every transaction in debug builds | ✅ |
| 02.7 Spatial index and GPU viewport | Spatial index (ADR 0021); wgpu renderer with backend probing (ADR 0007); CPU rasterizer as fallback | ◐ R*-tree accelerates bounding-box queries and supplies candidates for picking, window/crossing selection, and object snaps; exact geometry checks remain; the offscreen backend probe passes locally on Windows/DX12; GPU viewport rendering and reference Linux validation remain open |
| 02.8 Wider entities and 3D | Polyline vertices, block contents and explode, texts; staged 3D geometry foundation and later document model (ADRs 0019 and 0014 question 4) | ◐ 3D mathematical foundation complete (PR #24); polyline vertex grips and transactional vertex edits complete (PR #35); block-reference EXPLODE complete (PR #36; references with attributes are refused until their data can be preserved); texts complete (ADR 0022: drawn with their letters in a built-in stroke font, transformed as a whole, one grip, content, height and rotation in Properties, TEXT and basic MTEXT commands with word wrapping; the drawing's fonts and formatted MTEXT in phase 04); block content editing and the 3D document decision remain open |

## Phase 03 — DWG/DXF ◐

Exit: a documented compatibility matrix and no known silent losses in the
cases covered. Module M02. Decision: [ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md);
detail: [DWG/DXF interoperability](DWG_DXF_INTEROPERABILITY.md).

| Milestone | Content | Status |
|---|---|---|
| 03.1 Engine on AcadRust | AcadRust 0.6.4 from crates.io behind `arq-document`; corpus and tests green | ✅ |
| 03.2 opencadcodec's improvements | One by one with regression drawings: geometric constraints first, then fidelity fixes and diagnostics; kept in our AcadRust fork | ✅ every measured difference checked and the right ones brought in (R2004 lofted surfaces); the fork's wrong readings replaced by keeping objects verbatim; R11 linetypes, TOLERANCE in DXF and our own RAY and ACIS checks fixed; LibreDWG's seven regression drawings round-trip |
| 03.3 Data protections | Damaged files reported, safe save with read-back validation, no silent loss on version conversion, regression drawings | ✅ damaged files refused or recovered and marked; saves read back and checked; losses asked before saving; regression drawings made by the tests |
| 03.4 Compatibility matrix | By version, entity and operation; dynamic blocks, constraints, ACIS, Xrefs, tables, dimensions, hatches, custom BIM objects | ◐ first matrix from the comparison |
| 03.5 External validation | Outputs opened in independent CAD applications; Xref and BIM drawings in the corpus | ○ |
| 03.6 Fidelity across versions | DWG as the default save format once the corpus allows it (contract §10); linetypes, lineweights, layouts | ○ |

## Phase 04 — Associative technical documentation ○

Dimensions, texts and fonts, annotations, hatches, views, scales, sheets,
title blocks, plotting to PDF; projections, sections and elevations as
**associative views** that know their source and regenerate keeping manual
annotations (ADR 0014); the **project container** (models, drawings, views,
sheets, resources; an ADR first). Module M09.

It opens with the engine its associativity stands on *(maintainer,
2026-10-21)*:

| Milestone | Content | Status |
|---|---|---|
| 04.1 Parametric engine | ArqStudioCAD's own engine (ADR 0017 D6; an ADR first): a dependency graph that regenerates what depends on what changed; 2D geometric and dimensional constraints, solved and kept through DWG; dynamic blocks recomputed when a parameter changes (visibility, stretch, flip, lookup…), so the block editor accepts them (ADR 0023 D3a). Associative dimensions and views are built on it; BIM objects (phases 05 and 07) reuse it | ○ |

## Phase 05 — BIM and IFC ○

IfcOpenShell through a decoupled integration: semantic elements, properties,
spatial relations, IFC import/export, exchange validation; GlobalId,
properties and relations kept through every operation; never a collection of
anonymous meshes. Model ownership decided by ADR first (ADR 0014, open
question 1). The BIM workspace takes ArchiCAD's interface as its reference
(ADR 0015). The **multidisciplinary test** of ADR 0018 D4 runs here: it
decides whether DWG plus a project database suffice or the reserved `.ascad`
format is implemented. Module M03.

## Phase 06 — Topography and surfaces ○

Survey points, triangulation (TIN), contour lines, profiles, sections,
slopes, cut and fill volumes; QGIS bridge for maps. Module M04.

## Phase 07 — Architecture, structures and MEP ○

Specialized modeling, links between disciplines, loads, analysis through
appropriate engines (structural FEM verified against reference cases);
results independently verifiable. Modules M05 and M06; environmental
studies (M07) and transport and infrastructure (M08) follow the same pattern
as independent modules on public APIs.

## Phase 08 — Rendering and animation ○

Blender + Cycles headless, model transfer, PBR materials, lighting, cameras,
image sequences, FFmpeg for video, a render queue with progress and
cancellation. Transversal capability.

## Phase 09 — Coordination and walkthroughs ○

Clash detection, revision comparison, multidisciplinary review,
traceability; interactive real-time navigation on the GPU renderer. Module
M10.

## Phase 10 — Extensions and advanced optimization ○

Plugin API, automation, performance profiles, large models and terrains,
additional libraries, LuxCoreRender evaluated against Cycles on shared
scenes. Module M11.

## Phase 11 — Cross-platform distribution ○

Installers, bundled dependencies, signing and notarization where they apply,
updates, installation docs, tests on Windows (x86-64, ARM64), Linux (x86-64,
ARM64; native packages, AppImage, Flatpak) and macOS (Apple Silicon, Intel);
public releases only when the release criteria of the master plan (§14) are
met.

### Milestone 02.8 sequence

Milestone 02.8 proceeds in independently reviewable steps:

1. **3D mathematical foundation (ADR 0019):** `Point3`, `Vector3`, `Bounds3`
   and `Transform3` in `arq-geometry`, with unit/property tests and precision
   cases near 1e6 model units. This does not add 3D document entities.
2. **Wider 2D entities:** polyline vertex editing, block contents and explode,
   and text support under the existing document and editing contracts. The
   polyline vertex grips and transactional vertex editing (PR #35) and the
   transactional EXPLODE of block references (PR #36) are complete; block
   references with attributes are refused until their data can be preserved
   by the operation. Texts are complete (ADR 0022: drawn with their letters
   in a built-in stroke font, transformed as a whole, one grip, content,
   height and rotation in Properties, the TEXT command; the drawing's own
   fonts and formatted MTEXT stay in phase 04). Block content editing follows, with
   stage 2 of the own model (ADR 0018 D2).
3. **3D document model (ADR 0014 question 4):** decide which 3D entities the
   core owns and how DWG ACIS and IFC geometry are represented before adding
   persistent 3D entities or format conversion.
4. **Owned kernel, progressively:** implement and test each geometry capability
   in Rust. OCCT may serve as a temporary, adapter-backed reference or execution
   path after its version, bridge, packaging and license review. Compare
   capability by capability; route a capability to the owned kernel only after
   its declared correctness, precision, degeneracy and performance evidence is
   reviewed (ADR 0020).
5. **Curves, surfaces and solids:** add separate, bounded capabilities with
   their own contracts and tests. B-rep topology and boolean operations are
   later steps, not part of the 3D value-type foundation.

## Modules and phases

| Module | Main phase |
|---|---|
| M01 CAD 2D/3D | 02 |
| M02 DWG/DXF interoperability | 03 |
| M03 BIM/IFC | 05 |
| M04 Topography and civil works | 06 |
| M05 Architecture and structures | 07 |
| M06 Building services (MEP) | 07 |
| M07 Environmental studies | 07 (pattern) |
| M08 Transport and infrastructure | 07 (pattern) |
| M09 Associative technical documentation | 04 |
| M10 Coordination | 09 |
| M11 Automation and extensions | 10 (the Python API exists since 02.3) |

## Old names (ADR 0014 roadmap)

| Before 2026-10-13 | Now |
|---|---|
| Phases 0, 1 | 00, 01 |
| Milestones 2.1–2.5 | 02.1–02.5 |
| 2.6 Spatial index and GPU viewport | 02.7 |
| 2.7 Wider CAD entities | 02.8 |
| 2.8 Projects | Phase 04 (project container) |
| Phase 3 CAD exchange and 2D documentation | 03 (exchange) and 04 (documentation) |
| Phases 4–8 | 05–09 |
| Phase 9 LuxCoreRender, 10 Optimization and distribution | 10, 11 |

## Branches

One branch per feature or fix: `feature/<topic>`, `fix/<topic>` (master
plan §13), a pull request to `main` each. Branches started before
2026-10-13 keep their names (`fase-2.5/edicion`).
