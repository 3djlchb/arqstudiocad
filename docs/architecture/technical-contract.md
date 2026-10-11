# ArqStudioCAD technical contract

Version 0.1 · Status: **accepted** · Applies to every change in this repository.

This document is the architecture rulebook. A feature that works on screen but
breaks one of these rules is not accepted. Decisions and their rationale live
in [adr/](adr/); this file states the rules that result from them.

---

## 1. Ownership

| Layer | Technology | Owns |
|---|---|---|
| CAD core | Rust | The document and all persistent state: geometry, entities, layers, blocks, operations, transactions, history, spatial queries, DWG/DXF I/O, derived render data |
| Application | Python | Commands, use cases, plugins, scripting, background-task coordination, events, i18n |
| Presentation | PySide6 | Windows, menus, panels, command line, dialogs, input events, viewport widget |

Python ↔ Rust communication is in-process through **PyO3 + maturin**. No HTTP,
no JSON, no IPC.

```
PySide6 (UI) ──► Python (application) ──► PyO3 (arq-python) ──► Rust core
                         ▲                                         │
                         └──────────── CadChangeSet ◄──────────────┘
```

The core never calls Python or Qt. It returns results and change sets; Python
turns them into events.

**Inviolable rule:** every crate except `arq-python` builds and passes its tests
without Python, PySide6 or a graphics system. `cargo test` at the repository
root runs exactly those crates (`default-members`), and CI verifies that none of
them depends on `pyo3`.

## 2. Repository layout

```
Cargo.toml                 Rust workspace (version source of truth)
pyproject.toml             maturin build + Python tooling
crates/
  arq-geometry/            geometric types and algorithms (no dependencies)
  arq-document/            entities, layers, blocks, symbol tables
  arq-core/                facade: operations, transactions, history, queries
  arq-render/              derived render data, independent of Qt
  arq-python/              PyO3 bindings -> arqstudiocad._core (only crate aware of Python)
python/arqstudiocad/
  commands/ events/ plugins/ scripting/ ui/  (+ i18n/ from phase 1)
tests/                     Python and integration tests
docs/architecture/         this contract and the ADRs
docs/references/           analysis of reference projects
```

Crate dependency direction is one-way:
`arq-geometry ← arq-document ← arq-core ← arq-render`, and `arq-core ← arq-python`.
Modules are created when needed, not in advance.

## 3. Contract A — Rust types and data ownership

Rust is the only owner of geometric entities and persistent state. Python and
PySide6 request operations; they never mutate internal structures.

```rust
pub type EntityId = u64;   // == DWG handle (see §10)
pub type LayerId = u64;    // == handle of the LAYER table record
pub type DocumentId = u64; // session-only, never persisted

pub struct Point2 { pub x: f64, pub y: f64 }
pub struct Vector2 { pub x: f64, pub y: f64 }
pub struct Bounds2 { pub min: Point2, pub max: Point2 }

pub enum Geometry2 {
    Line { start: Point2, end: Point2 },
    Circle { center: Point2, radius: f64 },
    Arc { center: Point2, radius: f64, start_angle: f64, end_angle: f64 },
}
```

These are the target shapes; final signatures are set when each crate is built.
`Document` fields are private and accessed only through controlled methods.

Geometry rules:

1. Coordinates are `f64` **model units**; never assume millimetres. Units are a
   document property (DWG `$INSUNITS`).
2. Angles in the API are **radians**. The UI may display degrees.
3. Non-finite values (NaN, ±∞) are rejected by every operation that takes them.
4. Radii must be positive; each operation validates its own degenerate cases.
5. Geometry changes only through core methods.
6. Render data is never the source of truth.
7. Tolerances are explicit parameters or documented constants, never ad-hoc literals.

## 4. Contract B — Rust → Python API (PyO3)

Only a small, stable set of classes is exposed; Rust internals are not.

| Python class | Responsibility |
|---|---|
| `CadApplication` | Create, open, save and close documents |
| `CadDocument` | Query and modify one document |
| `CadEntityInfo` | Immutable description of an entity |
| `CadLayerInfo` | Immutable description of a layer |
| `CadBounds2` | Geometric bounds |
| `CadChangeSet` | Summary of a committed change |
| `CadError` (+ subclasses) | Core errors as Python exceptions |

Minimum operations:

- Documents: `create_document(name)`, `open_document(path)`,
  `save_document(document_id, path)`, `close_document(document_id)`
- Geometry: `add_line`, `add_circle`, `get_entity`, `delete_entities`,
  `move_entities`, `rotate_entities`
- Query/edit: `get_bounds`, `query_entities(bounds)`, `get_layers`,
  `create_layer`, `set_entity_layer`

Rules:

- Bulk queries return compact data; mutations take lists of IDs.
- No Python object may hold a reference into Rust memory across calls.
- Every exposed item has a `.pyi` stub so the API is typed for Python users.

### Errors

```rust
pub enum CadError {
    InvalidGeometry(String),
    EntityNotFound(u64),
    LayerNotFound(u64),
    DocumentNotFound(u64),
    InvalidOperation(String),
    Io(String),
    UnsupportedFormat(String),
}
```

Operations return `Result<T, CadError>`. PyO3 maps each variant to a specific
exception (`InvalidGeometryError`, `EntityNotFoundError`, ...). Each error
carries a **stable code** and an English technical message; user-facing,
translated text is produced in Python (§13). PySide6 never sees Rust error types.

## 5. Contract C — Python commands

Python coordinates user actions and never duplicates geometric algorithms.

Every command:

- has a unique **English** name (e.g. `MOVE`) and a parameter definition;
- receives a controlled application context;
- validates user input before mutating the document whenever possible;
- mutates only through the core API;
- returns a structured result;
- supports cancellation where the operation allows it;
- takes part in undo/redo if it changes the document.

A command never edits widgets or document internals.

## 6. Contract D — Transactions and undo/redo

The history belongs to the Rust core; Python drives the command lifecycle.

```
Python command → begin transaction → validate & mutate in Rust → commit
                                                                   ├─ record history
                                                                   └─ emit CadChangeSet → UI update
```

```rust
pub struct CadChangeSet {
    pub created: Vec<EntityId>,
    pub modified: Vec<EntityId>,
    pub deleted: Vec<EntityId>,
}
```

- A failed operation leaves the document unchanged: all or nothing.
- Undo never copies the whole document for a small edit; the strategy is
  decided in phase 3 (ADR).

## 7. Contract E — Events and UI synchronization

The core never calls PySide6. Change sets become Python events, which a
`QObject` bridge re-emits as Qt signals.

Initial events: `DocumentOpened`, `DocumentSaved`, `DocumentChanged`,
`SelectionChanged`, `ActiveLayerChanged`, `CommandStarted`,
`CommandFinished`, `CommandCancelled`, `LanguageChanged`.

Events are frozen dataclasses. The UI updates only what a change affects.

## 8. Contract F — Viewport and renderer

Three separate things: the **CAD model** (Rust, exact), the **render
representation** (derived vertices, segments, styles, bounds) and the
**renderer** (GPU caches, drawing, camera).

```
RenderScene: update(changes) · remove_entities(ids) · set_visibility(layer, visible)
             rebuild_if_needed() · render(view_state)
```

- `RenderScene` owns derived data only and is independent of Qt.
- Geometry is sent to the GPU **relative to the camera origin**, so large
  (georeferenced) coordinates survive `f32` conversion.
- Moving the mouse never re-uploads the whole scene.
- **One renderer on wgpu** with WGSL shaders; the backend (Vulkan, DirectX 12,
  Metal, OpenGL/GLES, software) is chosen at runtime by out-of-process probes
  ([ADR 0007](adr/0007-graphics-api.md)).
- The core drawing path works within wgpu **downlevel limits** (OpenGL ES 3.0
  class: no compute shaders, no storage buffers); anything beyond is optional.
- The reference low-end machine (GeForce GT 730 2 GB, LMDE 7) is a release gate.

Selection: pointer + camera → screen-to-world → Rust spatial query → `EntityId`
→ Python selection → highlight. Spatial index chosen after measuring.

## 9. Contract G — Threads and performance

- Qt main thread: windows, widgets, UI updates.
- One committed mutation stream per document. No `&mut Document` is held across
  Python calls; no two tasks mutate the same document without synchronization.
- Queries may run in parallel once proven safe.
- A background operation must define: cancellation, behaviour if the document
  changes meanwhile, when results apply, progress reporting, failure rollback.
- Releasing the GIL is an explicit, reviewed decision per operation.

## 10. Contract H — Files and persistence

**DWG is the native format; DXF is the open alternative.**
See [ADR 0002](adr/0002-dwg-native-format.md). Both are read and written by
**opencadcodec** (MPL-2.0), wrapped by `arq-document`; codec types never leave
that crate ([ADR 0008](adr/0008-opencadcodec.md)).

- The document model mirrors DWG: `EntityId` is the entity handle, layers and
  linetypes are symbol-table records with handles, model space and paper space
  are block records. Handles are persistent and never reused in a document.
- Data that DWG cannot represent is stored in XDATA / XRECORD under the
  registered application `ARQSTUDIOCAD`, together with our schema version.
- The file "format version" is the DWG/DXF version written, plus that schema
  version.
- Saving defaults to DXF until our round-trip corpus (open → edit → save →
  reopen, every supported DWG/DXF version) is green; then DWG becomes the default.
- GPU buffers and Qt objects are never persisted.
- The core owns file I/O; Python asks to open/save; PySide6 shows the dialogs.

## 11. Contract I — Versioning and compatibility

- The PyO3 classes and methods are the public API of the core for Python.
- Internal Rust changes must not force UI changes.
- Incompatible API changes require a new major version (once at 1.0).
- Plugins use the public Python API only, never core internals.
- The project version lives in `[workspace.package]` of `Cargo.toml` and
  starts at 0.1.0. The Python package reads it from there.

## 12. Contract J — Mandatory tests

| Layer | Tests |
|---|---|
| arq-geometry | Coordinates, intersections, tolerances, transformations |
| arq-document | IDs/handles, layers, creation, editing, deletion |
| arq-core | Transactions, errors, undo/redo, queries |
| arq-python | PyO3 types, conversions, exceptions |
| Python | Commands, cancellation, plugins, events, i18n |
| PySide6 | Input, viewport, selection, visual updates |
| Integration | Create, save, open and modify a complete drawing |

Every change passes its layer's tests and the relevant integration tests.
Performance tests (e.g. 100,000 entities) are reproducible measurement targets,
not promises. A check that inspects nothing is a failure, not a pass.

## 13. Contract K — Internationalization

See [ADR 0006](adr/0006-i18n.md). In one sentence:
**everything the user reads is translatable; everything the user types is English.**

- Source language is English everywhere: code, comments, docs, UI strings.
- UI text goes through `tr()`; a missing translation falls back to English,
  never to a key or a crash.
- Translations are language packs (`i18n/<code>/meta.json`, `ui.json`,
  `commands.json`); adding a language changes no Python code.
- Command names and prompt option keys are English and **always** work in every
  language. Localized command names may only be *added*; they never shadow an
  English name or alias.
- A translated prompt keeps the English key visible: `[Referencia(R)]`.
- The Rust core emits no translated text.
- Spanish (`es`) is the first maintained language and must stay at 100 %
  coverage; other packs are reported, never release-blocking.

## 14. Platforms

Windows x86-64 and ARM64; macOS universal2 (Apple Silicon + Intel); Linux
x86-64 (+ ARM64 when feasible) as `.deb`, `.rpm`, AUR, Flatpak and AppImage.
Day-to-day testing is local (Windows + WSL2); GitHub Actions CI runs on manual
dispatch and covers the platforms not available locally. No public release
until the application is mature. See [ADR 0003](adr/0003-target-platforms.md)
and [ADR 0005](adr/0005-development-conventions.md).

## 15. Implementation phases

0. Foundation: workspace, tooling, CI, this contract.
1. Walking skeleton: Rust → PyO3 → PySide6 window with i18n, packaged for every platform.
2. `arq-geometry`.
3. `arq-document` (over opencadcodec) + `arq-core`: handles, layers, transactions, undo/redo.
4. `arq-python`: full binding API.
5. PySide6 application: window, command line, panels, events, provisional viewport.
6. First complete workflow: LINE, CIRCLE, select, MOVE, ROTATE, ERASE, UNDO/REDO.
7. GPU renderer (wgpu, backend probing, Qt integration spike) and spatial index, with benchmarks.
8. DWG/DXF round-trip corpus and fidelity; DWG becomes the default save format.
9. Public distribution: signing, notarization, Flathub, AUR.
