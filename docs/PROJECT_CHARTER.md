# ArqStudioCAD — Project charter

ArqStudioCAD is an open-source platform for CAD drafting, BIM modeling,
engineering, territorial and environmental studies, and the automated
production of technical documentation. It aims to cover a construction
project from the survey of the land to the architectural, structural and
building-services design, with coordinated information models and CAD
drawings of professional quality.

This charter states what the project is and where it goes. How it is built is
in the [technical contract](architecture/technical-contract.md) and the
[ADRs](architecture/adr/); the order of work is in the [roadmap](ROADMAP.md).

## Mission

Build an open, extensible platform that connects geometric design, BIM
models, engineering and specialized analyses with precise technical
documentation that stays up to date.

**Principle: one coordinated model, many disciplines, specialized analyses,
and associative CAD documentation.**

## Guiding principles

- **Rust** is the geometric and computational core.
- **Python** is the application layer: commands, automation, integration,
  plugins.
- **PySide6** is the interface, decoupled from the model's logic.
- **IFC** is the main format for exchanging and keeping BIM information.
- **DWG** is the native CAD format and **DXF** the open alternative
  (ADR 0002, ADR 0008).
- **CAD 2D and 3D are central capabilities**, not accessories of BIM.
- Modular architecture, stable APIs, automated tests and measured performance.
- Every dependency is chosen for function, license, compatibility and upkeep.

## Scope: ten modules

| | Module | Content |
|---|---|---|
| M01 | CAD 2D and 3D | Lines, arcs, polylines, curves, solids, layers, blocks, dimensions, references, selection, transformations |
| M02 | BIM and IFC | Buildings, storeys, spaces, walls, doors, windows, slabs, materials, properties, relationships |
| M03 | Topography and civil works | Survey points, TIN surfaces, contour lines, profiles, grades, slopes, sections, earthworks |
| M04 | Architecture and structures | Parametric architectural elements; foundations, beams, columns; analytical models and structural engines |
| M05 | Building services (MEP) | Electrical, lighting, conduits, water, drainage, HVAC, equipment, connections |
| M06 | Environmental studies | Sun paths, shadows, solar radiation, daylight; energy and comfort through external engines |
| M07 | Transport and infrastructure | Road design, accesses, parking, turning paths, external networks, traffic studies |
| M08 | 2D documentation | Plans, sections, elevations, profiles, details, hatches, lineweights, dimensions, annotations, scales, sheets |
| M09 | Multidisciplinary coordination | Federated models, clash detection, change review, quantities, validation, issue tracking |
| M10 | Automation and extensions | Python API, scripts, plugins, custom commands, importers and exporters, external engines |

Mechanical modeling (parametric parts, assemblies, fabrication drawings) may
become a workspace once its scope is defined.

### Visualization and deliverables

Visualization is a line of work of its own:

- **Photorealistic renders** of exteriors and interiors, with PBR materials
  and natural and artificial light. **Blender + Cycles** is the first engine,
  run headless as an external process; **LuxCoreRender** is evaluated later
  as an alternative.
- **Walkthroughs** come in two kinds. *Cinematic* walkthroughs follow camera
  paths and are rendered frame by frame into video. *Interactive*
  walkthroughs are real-time navigation on the platform's own GPU renderer
  (ADR 0007).
- **Deliverables** are drawings, sheets, PDF, IFC, DXF/DWG, images (PNG,
  JPEG, OpenEXR), image sequences, MP4 video, reusable camera paths, and
  render reports. Each records the model version it shows.

### Associative documentation

Drawings are not throw-away exports. Every plan, section, elevation or detail
is a **view** that knows its source model, its parameters and its graphic
style. When the model changes, the view knows it is out of date and can be
regenerated **without destroying the manual annotations** that still apply.
Styles follow discipline, element class, scale and view type, not only layers
(after BricsCAD's sections and typed plans). The output is editable vector
geometry, never a screenshot; raster output is a separate presentation option.

## Technology

| Technology | Role |
|---|---|
| Rust + Cargo | Core crates and their tests |
| Python + uv | Application, integration, automation |
| PySide6 (Qt 6) | Interface |
| PyO3 + maturin | Rust–Python bridge |
| opencadcodec | DWG/DXF reading and writing (ADR 0008) |
| IfcOpenShell | IFC entities, relationships, properties, serialization |
| wgpu + tiny-skia | GPU renderer and CPU rasterizer (ADR 0007, ADR 0013) |
| Blender (Cycles), LuxCoreRender | Photorealistic rendering, as external engines |
| Visual Studio Code + Claude Code | Development environment and coding agent |

The Rust core builds and tests without Qt. Third-party libraries enter through
adapters, so none of them shapes the whole architecture.

## Licenses and platforms

- Code: **GPL-3.0-or-later**. Documentation: **CC BY-SA 4.0**
  ([docs/LICENSE](LICENSE)).
- Dependencies are preferred in this order: MIT, BSD-2-Clause, BSD-3-Clause,
  Apache-2.0, LGPL-3.0, then others that are compatible. The order is a
  preference, not an authorization: each one is checked (`cargo deny`,
  `THIRD_PARTY.md`).
- Development starts on Windows x86-64. Targets are Windows x86-64 and ARM64,
  Linux x86-64 (ARM64 when feasible) and macOS universal2, as `.deb`,
  `.rpm`, AppImage, Flatpak and AUR packages (ADR 0003, ADR 0009).
- The repository is public. Releases are published only when the
  application is mature.

## Success criteria

The project is not measured by the number of its tools. It is measured by
verifiable results:

- geometric precision and stability;
- IFC and CAD interoperability proven by tests (round-trip corpora);
- BIM data and relationships kept intact;
- graphic quality and exactness of drawings;
- reliable updating of documentation;
- performance measured on models and terrains of representative size, down
  to the low-end reference machine (ADR 0013, D5.8);
- real compatibility on every supported operating system;
- modularity, maintainability and a stable API;
- engineering results checked against recognized references.

## Not now

Each discipline arrives as a complete module on top of a stable common base,
not as features scattered across the core. Analyses (structural, energy,
electrical, traffic) are accepted only when verified against reference cases:
a result is not valid just because it was computed.
