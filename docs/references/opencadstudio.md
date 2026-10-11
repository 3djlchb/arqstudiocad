# Reference analysis: Open CAD Studio

Analyzed 2026-10-11 from a shallow clone of `main` (app version 2026.41.0).
Repository: <https://github.com/HakanSeven12/OpenCADStudio> · License: GPL-3.0.

## What it is

A full 2D/3D CAD application written entirely in Rust (~576,000 lines in the
app repository alone), with a native desktop build and a WebAssembly build.
Single main author, very active (commits daily).

## Architecture

The application is split across four repositories by the same author:

| Repository | Role | License | Size |
|---|---|---|---|
| OpenCADStudio | Application: UI, scene, commands, plugins, packaging | GPL-3.0 | ~576k lines |
| [opencadcodec](https://github.com/HakanSeven12/opencadcodec) | DWG/DXF read/write, ACIS SAT/SAB | **MPL-2.0** | ~212k lines |
| [opencadkernel](https://github.com/HakanSeven12/opencadkernel) | 2D curve algebra + B-rep solids | **MPL-2.0** | ~76k lines, early |
| [opencadgraph](https://github.com/HakanSeven12/opencadgraph) | Node-graph (dataflow) engine | **MPL-2.0** | — |

The app depends on the other three by **pinned git revision** (they are not
published on crates.io as of this analysis).

| Concern | How Open CAD Studio does it |
|---|---|
| UI toolkit | [iced](https://iced.rs) (Rust-native GUI), pinned git revision, plus a fork of `iced_aw` |
| Rendering | wgpu 29 through iced; WGSL shaders; `lyon` tessellation |
| Document model | **Uses `opencadcodec::CadDocument` directly** as the in-memory document (`src/app/document.rs`); entities are addressed by DWG `Handle` |
| Undo/redo | Transactional, symmetric deltas: per touched handle a before-image and after-image ("first-touch entity delta"); table deltas for layers/styles; a `Full(CadDocument)` snapshot only as a fallback |
| Spatial index | Compact world-XY **grids** for plan views; **BVH** (leaf size 16) for projected/tilted 3D views (`src/scene/pick/interaction_index.rs`) |
| Commands | Module registry (`src/modules/{draw,annotate,insert,layout,manage,model,parametric,view}`) |
| i18n | Project Fluent (`i18n-embed`, `.ftl` files in `locales/<lang>-<REGION>/`), ~20 languages, fallback `en-US` |
| Plugins | Versioned `HostApi` crate (`ocs_plugin_api`), plugins as `cdylib`s; out-of-process for isolation |
| Python | Bundled plugin embedding **RustPython** (not CPython), sandboxed, out of process, `PY_EVAL`/`PY_RUN` commands |
| GPU robustness | `src/gpu_backend.rs`: probes dx12/vulkan/gl in short-lived child processes, crash sentinel, software-adapter fallback (WARP/llvmpipe) |
| Packaging | WiX MSI (`packaging/windows`), signed macOS bundle script, Homebrew formula, Snap, AppStream metainfo, desktop file |

## What we adopt

1. **opencadcodec as the DWG/DXF engine** (proposal, decision in phase 3/8).
   - Read/write DXF R12–R2018+ and **DWG R13–R2018+**, ASCII and binary DXF,
     48 entity types, tables, layouts, dynamic blocks, XDATA, ACIS data.
   - Fidelity features we need for a DWG-native CAD: untouched DWG records are
     written back verbatim, failsafe reads with structured diagnostics,
     ~40 legacy code pages.
   - MPL-2.0 is file-level copyleft and explicitly compatible with GPL-3.0:
     we can depend on it (and modify it) as long as changes to its files stay
     MPL-2.0.
   - This replaces "write our own DWG reader/writer", which ADR 0002 identified
     as the hardest part of the project.
2. **The document *is* the file database.** Open CAD Studio keeps
   `CadDocument` as its model; IngeCAD does the same with ezdxf. Both arrived at
   it for fidelity: anything the app does not understand survives a save.
   `arq-document` should wrap the codec document behind our own API rather than
   translate into a separate model (decision in phase 3).
3. **Undo as symmetric per-handle deltas** (before/after images), the same
   allocation moving between the undo and redo stacks. Matches contract D
   ("never copy the whole document for a small edit").
4. **Spatial index: grid for 2D plan views, BVH for 3D**, measured before chosen.
5. **GPU fallback probing.** Old GPUs and drivers crash at startup; whatever
   graphics API we pick needs an equivalent safe mode / software fallback.
6. **Precision via local frames** (from opencadkernel, see below).

## What we do differently, and why

| Open CAD Studio | ArqStudioCAD | Reason |
|---|---|---|
| Everything in Rust, iced UI | Rust core + Python app + PySide6 | Qt's mature widgets, docking, accessibility, platform integration; Python for commands and plugins (ADR 0001) |
| RustPython, sandboxed out-of-process | CPython in-process | Full Python ecosystem (numpy, pyproj...) for users and plugins. Trade-off: plugins are trusted code; isolation is a later decision |
| Fluent `.ftl` catalogs | IngeCAD-style JSON packs (ADR 0006) | Simpler for translators; keeps the English-command-line rules. Revisit if plural/gender rules become a problem — Fluent handles them natively |
| Web/WASM build | Desktop only | Not a goal; Qt does not target the browser |

## opencadkernel (MPL-2.0)

"Early. The structure and the invariants are settled; most of the operations
are not written yet." Layering: `geom2d` (curves, intersection, tessellation,
offset) → `brep` (topology, booleans) → `acis` (lift/lower SAT data via the
codec). Two invariants worth adopting regardless of whether we use the crate:

- **Local frames:** every operation lifts inputs into a translation-only local
  frame before arithmetic and shifts results back. Survey coordinates (~1e6)
  otherwise spend half of `f64`'s precision before any computation. This is
  the geometry-side counterpart of our renderer's camera-relative rule.
- **Provenance:** every B-rep node remembers the record it came from and
  whether it was edited; clean nodes are written back as original bytes.

Proposal: build `arq-geometry` ourselves for the 2D basics in phase 2,
**adopting the local-frame invariant**; evaluate `opencadkernel::geom2d` /
`brep` when we need offsets, NURBS and booleans.

## Risks

- **Single maintainer** for codec, kernel and app. Mitigation: pin a git
  revision, keep our own round-trip test corpus, be ready to fork (MPL allows it).
- Codec is git-only: `deny.toml` must allow that specific git source.
- Codec types must stay behind `arq-document`; they never reach the PyO3 API.

## Also noted

- The author's `acadifc` crate converts IFC ↔ CAD on top of codec + kernel —
  relevant later for architecture/BIM. License not yet checked.
- `docs/cadcodec-reader-gaps.md`, `docs/tessellation.md`,
  `docs/plugin-architecture.md` are worth reading before phases 7–8.
