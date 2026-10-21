# ArqStudioCAD — System architecture

The layers of the [master plan](MASTER_PLAN.md) (§3, §7, §8), what each one
owns, and how today's repository maps onto the target layout. The binding
details of each boundary are in the
[technical contract](architecture/technical-contract.md).

## Layers

```
┌──────────────────────────────────────────────────────────────────────┐
│ Presentation — PySide6   python/arqstudiocad/ui                      │
│   windows, viewport, panels, properties, workspaces, diagnostics     │
├──────────────────────────────────────────────────────────────────────┤
│ Application — Python     python/arqstudiocad                         │
│   session, commands, events, i18n, plugins, scripting, workspaces    │
├──────────────────────────────────────────────────────────────────────┤
│ Bridge — PyO3            crates/arq-python → arqstudiocad._core       │
│   typed API, conversions, exceptions, GIL policy                     │
├──────────────────────────────────────────────────────────────────────┤
│ Core — Rust              crates/arq-core, arq-document, arq-geometry, │
│   arq-kernel, arq-render: geometry, B-rep solids, documents,         │
│   entities, transactions, undo, queries, render representation       │
├───────────────────────────────────┬──────────────────────────────────┤
│ Format adapters                   │ External engines                 │
│   DWG/DXF: AcadRust in            │   Blender/Cycles, FFmpeg, FEM…   │
│   arq-document (ADR 0017);        │   as external processes          │
│   own model (ADR 0018);           │                                  │
│   IFC, 3DM later                  │                                  │
└───────────────────────────────────┴──────────────────────────────────┘
```

| Layer | Owns | Never |
|---|---|---|
| Rust core | 2D/3D geometry and tolerances; entities and the document model; spatial indexes, selection, geometric queries; history, transactions, undo/redo; references, dependencies, persistent objects; performance-critical work; an internal API, versioned and tested | calls Python or Qt; depends on a format library outside its adapter |
| Python application | commands and user tools; discipline workflows; Blender, IFC and external-tool integrations; plugins, automation, scripting; coordination between subsystems | mutates core state except through the PyO3 API; implements intensive geometry without need |
| PySide6 interface | the user interface; properties, panels, project tree, layers, scenes and layouts; settings, progress, notifications, diagnostics; accessibility, i18n, customization | talks to Rust directly; holds model state |

## Design rules

The ten rules of the master plan (§8) are binding: core independent of
PySide6 and of the DWG/DXF library; modules through defined interfaces only;
stable identifiers and clear ownership; transactional changes; progress and
cancellation for long tasks; external-format errors as diagnostics, never
crashes; no needless copies, concurrency only when safe; versioned public
APIs with compatibility tests; an explicit policy for unknown data; every
new dependency justified (function, performance, upkeep, license) under the
rule for new technologies (§16).

## Adapters

A format or engine library lives behind a contract of ours, in one place:

| Adapter | Where today | Contract |
|---|---|---|
| DWG/DXF (AcadRust) | `crates/arq-document` | read with diagnostics, write to an explicit version, capabilities, round-trip validation (ADR 0017 D4) |
| Own internal model | `crates/arq-document` (02.6) | ADR 0018 D2; `.ascad` reserved (D5) |
| Rendering on the CPU (tiny-skia) | `crates/arq-render` | scene from change sets, camera, image (ADR 0013) |
| IFC (IfcOpenShell), 3DM, Blender | not yet | phase 05, 08 |

When an adapter grows beyond its crate it moves to its own crate
(`arq-interop`) or to `adapters/`, without changing its contract.

## Today's tree and the target layout

The target layout of master plan §7 is reached **incrementally**: a folder
or crate is created when there is code or a concrete need for it; what works
is not moved for the sake of the layout (maintainer, 2026-10-13).

| Target | Today | When it changes |
|---|---|---|
| `crates/arq-core` | `crates/arq-core` (document with transactions, undo, queries, store) | — |
| `crates/arq-geometry` | `crates/arq-geometry` | — |
| `crates/arq-kernel` | `crates/arq-kernel` (B-rep solids, ADR 0024) | grows capability by capability (ADR 0020) |
| `crates/arq-document` | `crates/arq-document` (model + DWG/DXF adapter) | the own internal model (02.6) separates model and adapter |
| `crates/arq-entities` | inside `arq-document` | when the own entity model outgrows it (02.6, 02.8) |
| `crates/arq-spatial` | R*-tree in `arq-core::spatial_index` (ADR 0021) | if a crate other than `arq-core` needs the index |
| `crates/arq-transactions` | `arq-core` history | if other models (IFC) need the same transactions |
| `crates/arq-render` | `crates/arq-render` | — |
| `crates/arq-interop` | — | when a second exchange format (IFC, 3DM) arrives |
| `crates/arq-python` | `crates/arq-python` | — |
| `apps/desktop` | `python/arqstudiocad/ui` + `application.py` | with packaging work (phase 11), if useful |
| `python/arq_app` | `python/arqstudiocad` (the name of ADR 0004) | not renamed: the package name is public API |
| `adapters/`, `modules/` | — | with the first IFC adapter (05) and the first discipline module (05–07) |
| `tests/unit`, `integration`, `interoperability`, `performance`, `fixtures` | Rust tests beside their crates; `tests/python`; corpus in `crates/arq-core/tests/corpus.rs` | interoperability and performance suites get their own folders when they grow beyond one file |
| `benchmarks/` | `crates/*/benches`, `tools/dwg-bench` | — |
| `docs/decisions` | `docs/architecture/adr` | kept: links from code and commits point there |
| `docs/references`, `docs/architecture` | the same | — |
| `docs/formats`, `docs/testing`, `docs/licenses` | the governing documents in `docs/` | split when one document grows too large |
| `tools/development`, `tools/packaging` | `tools/`, `packaging/` | — |
