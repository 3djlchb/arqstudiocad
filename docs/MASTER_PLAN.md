# ArqStudioCAD — Master plan

Version 2.1 · October 2026 · The project's governing document, adopted by the
maintainer on 2026-10-13; 2.1 (same day) keeps DWG as the working format, after
BricsCAD, with an own internal model and the native format reserved
(ADR 0018). The other governing documents
([index](README.md)) develop it; where they disagree, this plan and the
ADRs that implement it prevail.

The plan guides ArqStudioCAD from its foundations to a professional,
modular, cross-platform, open-source CAD/BIM platform. Its most important
strategic decision: **an own core in Rust, an application layer in Python and
an interface in PySide6**. External libraries are integrated through
**adapters**, so that no format, rendering or BIM dependency shapes the
central architecture permanently.

## 1. Vision and objectives

ArqStudioCAD is an integrated technical platform for drawing, modeling,
design, analysis, documentation and visualization of engineering and
architecture projects. Guiding principle:

> **One coordinated model, many disciplines, specialized analyses, and
> technical and visual deliverables tied to the project.**

Objectives:

- an own geometric and document core, efficient and maintainable;
- interoperability with DWG, DXF, IFC, 3DM and other relevant formats;
- architecture, structures, BIM, topography, building services and civil
  works, integrated;
- modeling, calculation engines, interface and external formats kept apart;
- open technologies first; optional commercial integrations only when they
  bring a justified advantage;
- Windows first, without compromising Linux and macOS;
- code published under a clear license policy; official releases only when
  they meet verifiable quality criteria.

## 2. Official architectural decisions

| Area | Decision |
|---|---|
| Core language | Rust |
| Application logic | Python |
| Graphical interface | PySide6 |
| Rust–Python bridge | PyO3 + maturin |
| Python dependencies | uv |
| Rust workspace | Cargo workspace |
| Working format | **DWG**, as BricsCAD does, with an **own internal model** from the start; native `.ascad` designed and reserved, decided by a multidisciplinary test ([ADR 0018](architecture/adr/0018-native-format.md)) |
| DWG/DXF interoperability | **AcadRust as the base, with validated improvements from opencadcodec** ([ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md)) |
| BIM | IFC through a decoupled IfcOpenShell integration |
| Geometry | Build and own the Rust geometry kernel progressively; temporary external backends may be used behind reviewed adapters and replaced capability by capability after declared parity tests ([ADR 0020](architecture/adr/0020-progressive-own-geometry-kernel.md)) |
| Initial rendering | Blender + Cycles |
| Animation and video | FFmpeg |
| Tests | Rust tests, pytest and interoperability tests |
| Version control | Git + GitHub |
| IDE and agent | Visual Studio Code + Claude Code |

No library choice is irreversible: a dependency that stops being adequate is
replaced without rewriting what does not depend on it.

## 3. System architecture

```
Presentation — PySide6      windows, panels, properties, views, tools, accessibility
Application — Python        commands, workflows, automation, plugins, coordination
Bridge — PyO3               typed API, data conversion, error control
Core — Rust                 geometry, documents, entities, spatial indexes, transactions, stable API
Formats                     DWG, DXF, 3DM, IFC        External engines   render, FEM, other analyses
```

- **Rust:** 2D/3D geometry, transformations, tolerances; CAD entities and the
  document model; spatial indexes, selection, geometric queries; operation
  history, transactions, undo/redo; references, dependencies, persistent
  objects; performance-critical operations; an internal public API,
  versioned and tested.
- **Python:** commands and user tools; discipline workflows; Blender, IFC
  and external-tool integrations; plugins, automation, scripting;
  coordination between subsystems.
- **PySide6:** the user interface; properties and panels; project tree,
  layers, scenes and layouts; settings, progress, notifications and
  diagnostics; accessibility, internationalization, customization.

Rule: intensive geometric operations are not implemented in Python without
need, and the Rust core never depends on interface classes. Details:
[system architecture](SYSTEM_ARCHITECTURE.md).

## 4. Functional modules

| | Module | Content |
|---|---|---|
| M01 | CAD 2D/3D | Basic entities, drawing, editing, selection, layers, blocks, transformations, modeling, views |
| M02 | DWG/DXF interoperability | Reading, writing, conversion, diagnostics, compatibility by version, preservation of unknown objects |
| M03 | BIM/IFC | Building elements, properties, spatial relations, classification, quantities, BIM coordination |
| M04 | Topography and civil works | Points, surveys, TIN surfaces, contour lines, profiles, sections, earthworks |
| M05 | Architecture and structures | Architectural modeling, structural elements, loads, analysis and design through specialized engines |
| M06 | Building services (MEP) | Electrical, sanitary, water, ventilation, HVAC networks; interference detection |
| M07 | Environmental studies | Basins, drainage, territorial analysis, environmental variables, GIS where it applies |
| M08 | Transport and infrastructure | Roads, alignments, grades, corridors, linear works, infrastructure coordination |
| M09 | Associative technical documentation | Plans, dimensions, sections, elevations, labels, title blocks, scales, sheets tied to the model |
| M10 | Coordination | Model comparison, clash detection, change review, traceability |
| M11 | Automation and extensions | Public API, plugins, scripting, automated tasks, third-party tools |

Rendering, animation and walkthroughs are **transversal capabilities**, not a
module isolated from the data model. Advanced modules depend on the core
through defined contracts, never on each other's internals.

## 5. DWG/DXF strategy

Decided ([ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md),
evidence in the [comparison](references/dwg-library-comparison.md) of 220
drawings and 6,600 operations):

1. AcadRust is the base of the DWG/DXF adapter.
2. opencadcodec's valuable improvements are brought in selectively:
   geometric constraints, fidelity and diagnostics first.
3. LibreDWG is not part of the production flow; it may serve as a
   comparison tool.
4. `arq-document` keeps the contract between our document model and the
   external codecs.
5. Compatibility tests by version, entity and operation.

Mandatory protections: detect partial reads and damaged files; a failed save
never destroys the original; warn before a conversion that may lose
information; keep unknown data when technically possible; validate written
files by reading them back and in independent applications; keep a
**capability matrix** for dynamic blocks, constraints, ACIS solids, Xrefs,
tables, dimensions, hatches and custom BIM objects
([DWG/DXF interoperability](DWG_DXF_INTEROPERABILITY.md)).

**DWG is the working format and the document model is our own**
(amendment of 2026-10-13, [ADR 0018](architecture/adr/0018-native-format.md)):
the engine reads into our model and writes from it; whether a native format
(`.ascad`, reserved) is needed is decided by a real multidisciplinary test.
The architecture decision is taken; its functional validation closes after
external tests, Xrefs and BIM objects above all.

## 6. Formats and interoperability

| Format or technology | Purpose |
|---|---|
| DWG (working format) + own model | Daily work and persistence; a project database beside it if a multidisciplinary test shows the need |
| `.ascad` (reserved) | Native format, designed and kept for when DWG plus a project database are not enough |
| DWG/DXF | Exchange with CAD applications |
| IFC | BIM interoperability |
| 3DM / openNURBS | Exchange with Rhino |
| STEP and other 3D formats | Evaluated per use case |
| SVG/PDF | Documentation, printing, delivery |
| OBJ/glTF and mesh formats | Visualization, geometry exchange, walkthroughs |
| Blender/Cycles | High-quality external rendering |
| FFmpeg | Encoding of sequences and video |

Each format has an independent adapter; open libraries are used when their
capabilities, licenses and stability are adequate. The system distinguishes
**faithful import**, **partial import** (some entities converted, others
preserved or warned about), **lossy export** (incompatibilities identified
before the file is written) and **semantic compatibility** (relations,
properties and behaviors the format admits are kept, not only geometry).
Details: [data and formats](DATA_AND_FORMATS.md).

## 7. Repository organization

A monorepo: a Cargo workspace, a Python application, adapters kept apart.
The **target** layout is below; folders are created only when there is code
or a concrete need for them ([system architecture](SYSTEM_ARCHITECTURE.md)
maps today's tree onto it).

```
apps/desktop/                 main.py, ui/, resources/
crates/                       arq-core, arq-geometry, arq-document, arq-entities, arq-spatial,
                              arq-transactions, arq-render, arq-interop, arq-python
python/arq_app/               commands/, services/, plugins/, integrations/
adapters/                     dwg_dxf/, ifc/, rhino_3dm/, blender/, other_formats/
modules/                      cad/, bim/, topography/, structures/, mep/, environmental/,
                              infrastructure/, documentation/, coordination/
tests/                        unit/, integration/, interoperability/, performance/, fixtures/
benchmarks/
docs/                         architecture/, decisions/, references/, formats/, testing/, licenses/
tools/                        development/, packaging/
```

Governing documents: project charter, system architecture, technical
contract, modules and roadmap, data and formats, DWG/DXF interoperability,
testing and performance, licenses and distribution, CONTRIBUTING, and the
ADRs.

## 8. Internal contracts and design rules

1. The Rust core is independent of PySide6 and of the chosen DWG/DXF
   library.
2. Modules do not reach into other modules' internals; they use defined
   interfaces.
3. Documents and entities have stable identifiers and clear ownership rules.
4. Operations that modify the model are transactional where it applies.
5. Long tasks report progress and handle cancellation.
6. External-format errors become understandable diagnostics, never crashes.
7. Intensive operations avoid needless copies and use concurrency only when
   it is safe.
8. Public APIs are versioned and have compatibility tests.
9. Unknown data has an explicit preservation policy.
10. Every new dependency justifies its function, performance, upkeep and
    license.

## 9. Phases

Incremental phases; dates are set once the real pace is known, never
invented. Phases are sequential where there are technical dependencies and
may run in parallel once contracts are defined. Status and milestones:
[modules and roadmap](MODULES_AND_ROADMAP.md).

| | Phase | Exit criterion |
|---|---|---|
| 00 | Specification and contracts | decisions documented and a compilable skeleton |
| 01 | Technological foundation | a minimal reproducible application on Windows showing a result computed in Rust |
| 02 | CAD core | create, edit, save and recover a basic drawing |
| 03 | DWG/DXF | a documented compatibility matrix and no known silent losses in the cases covered |
| 04 | Associative technical documentation | dimensions, texts, annotations, views, scales, sheets, printing, documentation tied to the model |
| 05 | BIM and IFC | semantic elements, properties, spatial relations, IFC import/export, exchange validation |
| 06 | Topography and surfaces | points, triangulation, contour lines, profiles, sections, terrain calculations |
| 07 | Architecture, structures and MEP | specialized modeling, links between disciplines, loads and analysis through engines; results independently verifiable |
| 08 | Rendering and animation | Blender/Cycles, model transfer, materials, lighting, sequences, FFmpeg |
| 09 | Coordination and walkthroughs | clash detection, revision comparison, multidisciplinary review, interactive visualization |
| 10 | Extensions and advanced optimization | plugin API, automation, performance profiles, large models, additional libraries, LuxCoreRender evaluation |
| 11 | Cross-platform distribution | installers, bundled dependencies, signing where it applies, updates, installation docs, tests on Windows, Linux, macOS |

## 10. Performance and quality

Performance is an architectural property, not a last-minute optimization.
Measured: opening and saving time by file size and entity count; peak memory
and growth while editing; selection, regeneration and spatial-query time;
import/export time and exchange fidelity; cost of transactions, undo/redo
and model updates; multi-core scalability; start-up time and base
consumption. Every benchmark names its input files, environment and
repetitions, and is reproducible; figures from incompatible machines are not
compared. Test pyramid: Rust unit tests; Rust–Python integration; interface
and user flows; format reading, writing and round trip; performance and
memory; cross-platform and installation; regression with real files.
Engineering calculations add reference cases, units, numerical tolerances
and independent verification. Details:
[testing and performance](TESTING_AND_PERFORMANCE.md).

## 11. Licenses and open-source policy

Own software GPL-3.0-or-later; documentation CC BY-SA 4.0; each dependency
reviewed for license, provenance and obligations; preference for MIT,
BSD-2-Clause, BSD-3-Clause and Apache-2.0, then other compatible licenses
after review; no incompatible dependency without an explicit legal and
technical evaluation; copyright notices, attributions, a dependency
inventory and a record of modifications; commercial-only libraries assessed
separately, as optional adapters whose absence never prevents using the
fundamental capabilities. Being open source does not remove a library's
distribution obligations: they depend on its license, how it is linked, the
files modified and how the product is distributed. Details:
[licenses and distribution](LICENSES_AND_DISTRIBUTION.md).

## 12. Platforms and distribution

Development starts on Windows x86-64. Later targets: Windows x86-64 and
ARM64; Linux x86-64 and ARM64 (native packages where feasible, AppImage,
Flatpak); macOS Apple Silicon and Intel, subject to toolchains and
dependencies. A platform is supported only once the application and its
critical dependencies are built, installed and tested on it.

## 13. Workflow

Any coding agent may contribute (including Claude Code, Codex and other agents),
and work may continue across sessions or days. The repository state and this
master plan are the shared source of truth; an agent's identity or prior chat
does not establish a separate implementation direction. Before planning code
or changing logic, every agent must freshly scan the repository, current branch,
uncommitted diff, relevant implementation and tests, then read the governing
documents and applicable ADRs. Continue from the checked-out state, preserve
existing work, and record durable decisions and progress in project documents.
The complete procedure is in [AI agent workflow](AI_AGENT_WORKFLOW.md).

1. Pick a concrete task from the plan.
2. Read the relevant architecture documents and ADRs.
3. Define the contract and the acceptance criteria before implementing.
4. Ask the coding agent for limited, reviewable changes with tests.
5. Run the relevant tests and benchmarks.
6. Review the diff and the dependency changes.
7. Fix problems and update the documentation.
8. Small, descriptive commits.
9. Integrate through branches and reviews.
10. Publish official releases only when the quality criteria are met.

Branches per feature or fix: `feature/<topic>`, `fix/<topic>` (for example
`feature/dwg-adapter`, `fix/document-roundtrip`). Important architectural
decisions are recorded as ADRs.

## 14. When a release is ready

Not because it compiles. It completes the flows it declares; passes its
automatic and regression tests; documents its known limitations; keeps
licenses and attributions right; installs and starts on the platforms it
announces; destroys no data when an operation fails; warns clearly before
lossy conversions; has reproducible build and packaging procedures; has
enough documentation to install, use and extend it.

## 15. First official milestone: the technical MVP

Prove the architecture end to end before widening the scope.

- [x] Repository and governing documents.
- [x] Cargo workspace and the Python environment with uv.
- [x] A Rust library reachable from Python through PyO3.
- [x] PySide6 showing a result computed by Rust.
- [x] A minimal CAD document with identified entities.
- [x] **Save and recover the working document (DWG) from the own internal model** ([ADR 0018](architecture/adr/0018-native-format.md); originally "the native document").
- [x] A first DXF test with its round trip validated.
- [x] Automated tests and a first continuous integration.

Status as of 2026-10-17: 8 of 8 (see the [roadmap](MODULES_AND_ROADMAP.md)).

## 16. Rule for new technologies

Before a library, engine or technology is integrated: Does it improve a
concrete objective? Does it respect the Rust core's independence? Does it
keep the integration contract stable? Is its license compatible with the
intended distribution? Has it enough upkeep, documentation and tests? Does
it improve performance, fidelity or a capability we need? Can we replace it
if it stops being adequate? If it fails the essentials, it does not enter
the core; it may be studied as an optional extension or discarded.
