# 0014. Platform direction: from CAD to a CAD/BIM multimodeler

Date: 2026-10-13 · Status: Accepted · Amends the phase list of the technical
contract (§15)

## Context

ArqStudioCAD started as a 2D/3D CAD with DWG as its native format. On
2026-10-13 the maintainer set a wider goal: an open platform covering a
construction project's whole life cycle — survey and terrain, architecture,
structures, building services, environmental and traffic studies — around
coordinated information models (IFC), with associative CAD documentation and
visualization deliverables. The goal is written in the
[project charter](../../PROJECT_CHARTER.md); the order of work in the
[roadmap](../../ROADMAP.md).

The work done until then (phases 0–4 and most of 5) was reviewed against the
new goal.

## Decision

1. **Direction.** ArqStudioCAD is a CAD/BIM multimodeler platform with ten
   modules (M01–M10) and a visualization line, as the charter states. CAD 2D
   and 3D stay central capabilities.
2. **No restart.** The existing architecture — Rust core, Python application,
   PySide6 interface, PyO3/maturin, uv, DWG/DXF through opencadcodec, the
   technical contract and ADRs 0001–0013 — is the foundation of the
   platform. Phases 0–4 and the application shell are its "foundation" and
   "CAD core" (roadmap phases 1–2).
3. **Roadmap.** The roadmap's phases 0–10 replace the contract's phase list.
   The former phases 2–7 are milestones 2.1–2.6 of the CAD core.
4. **Names and layout stay.** Crates keep the `arq-` prefix; new ones follow
   it (`arq-spatial`, `arq-sections`, `arq-drafting`…). The interface stays
   inside the Python package, which is how it is packaged. No renaming
   without a functional reason.
5. **Licenses.** Code stays GPL-3.0-or-later. Documentation (`docs/`, READMEs,
   CONTRIBUTING) is CC BY-SA 4.0.
6. **Repository.** It stays public — among other reasons because GitHub
   Actions runs without minute limits for public repositories, so every
   platform can be tested there; releases wait for maturity.
7. **Coding agent.** Claude Code replaces Codex. `AGENTS.md`, which
   `CLAUDE.md` includes, remains the binding instructions for any agent.
8. **Rendering engines are external.** Blender + Cycles first, run headless
   in its own process (it is GPL); LuxCoreRender later, after evaluation.
   Neither enters the Rust core.
9. **CI on every pull request.** Amending ADR 0005, the CI workflow runs on
   every pull request to `main`, every push to `main` and by hand, since the
   public repository gets GitHub Actions without minute limits. Local checks
   stay mandatory; the package workflow stays manual.
10. **The 2D CAD comes first.** The CAD core is made complete and stable
   (milestones 2.4–2.8, then phase 3) before IFC, BIM and the discipline
   modules start.

### Open questions, each to be decided by its own ADR before its phase

1. **Who owns the BIM model** (before phase 4). The contract says "Rust owns
   the document". IfcOpenShell (C++, with Python bindings) is the natural
   owner of an IFC model. The likely answer is that Rust owns CAD documents,
   IfcOpenShell owns IFC models behind a Python adapter, and Rust receives
   tessellated geometry for rendering and sections — without duplicating
   data or losing it on save.
2. **The project container** (milestone 2.8). Models, drawings, views,
   sheets, terrain, renders and resources, with relative paths that survive
   moving the project to another machine; a project browser.
3. **Workspaces by discipline** (milestone 2.4, step 5.7). A workspace is a
   discipline's tools (CAD, BIM architecture, structures, MEP…) shown in a
   layout (Classic or Ribbon, ADR 0013 D5.1). Step 5.7 builds the mechanism
   with the CAD discipline in both layouts.
4. **3D in the document model** (milestone 2.7). What the core edits in 3D,
   and how 3D solids (ACIS in DWG) and IFC geometry meet. *Settled by ADR
   0024 (2026-10-21).*
5. **Associative views** (phase 3). How a view records its source and
   parameters, detects that it is stale, and regenerates while keeping manual
   annotations; graphic styles by discipline, class, scale and view type.

## Consequences

- Each new design leaves room for discipline modules, IFC, associative
  documentation and rendering, without building them early.
- The contract (§15) now points to the roadmap.
- Pending decisions are listed here rather than settled by chance in code.
