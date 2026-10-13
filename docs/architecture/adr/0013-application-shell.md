# 0013. Application shell: workspaces, commands, panels, provisional viewport

Date: 2026-10-12 · Status: Accepted · Builds on [0006](0006-i18n.md),
[0007](0007-graphics-api.md), [0011](0011-document-model-and-transactions.md),
[0012](0012-python-api.md)

## Context

Phase 5 turns the walking skeleton into the application shell: window,
command window, panels, events and a provisional viewport (contract §15).
Drawing and editing commands are phase 6; the wgpu renderer is phase 7. The
maintainer's real drawings are mostly blocks and polylines, which the core
keeps but cannot describe for drawing yet, so the viewport needs more than
LINE, CIRCLE and ARC to be useful.

## Decision

### D5.1 — Two workspaces over one set of actions

Every user action is a command (D5.4) with one `QAction`: translated text,
shortcut, command name. Two **workspaces** lay the same actions out
differently, switched from the View menu and remembered:

- **Classic** (IngeCAD, AutoCAD classic): menu bar, toolbars, docked panels,
  command window.
- **Ribbon** (Open CAD Studio, AutoCAD ribbon): tabs (Home, Layers, View…)
  holding groups of tool buttons, the same docked panels and command window.

Both are Qt Widgets; the ribbon is our own widget (Qt has none). Tools show
their name as text until there are icons. QML is not used; it may be
reconsidered for specific panels. Dock layout is saved per workspace.

### D5.2 — Render representation now, in Rust

Every entity becomes drawable now, not only the editable kinds:

- `arq-document` describes each entity as neutral **drawing primitives** in
  world coordinates — polylines (with arcs), circles, text outlines — with
  blocks expanded (nested, with a recursion guard), so codec types still
  never leave it (ADR 0011).
- `arq-render` builds the **render scene** from them (contract §8): curves
  flattened at a chord tolerance, ByLayer/ByBlock colors resolved, entities on
  off or frozen layers hidden, results cached per entity and updated from each
  `CadChangeSet`. It has no Qt and no GPU code yet.
- Covered in this phase: LINE, CIRCLE, ARC, LWPOLYLINE/POLYLINE (bulges),
  ELLIPSE, INSERT and DIMENSION (their blocks), HATCH (boundary), TEXT/MTEXT
  (outline box until fonts exist), POINT. Anything else is drawn as its
  bounding box, marked as a placeholder.
- Python receives the scene as `(N, 2)` `float64` coordinate arrays plus
  per-polyline offsets, colors and entity ids, through the buffer protocol
  (ADR 0012 D4.1): NumPy can read them without a copy and is not required.

Linetypes, lineweights, real text and hatch patterns are later work.

### D5.3 — Provisional viewport: rasterized in Rust, shown by Qt

`arq-render` rasterizes the scene on the CPU into an RGBA image with
**tiny-skia** (BSD-3-Clause; no GPU, no Qt), and the viewport widget only
displays that image. No Python loop runs per entity, so pan and zoom stay
smooth on large drawings and on the 2011 reference CPU (D5.8), whatever the
graphics driver. The camera is `f64` (center, scale); the rasterizer works
relative to the view, so survey coordinates keep their precision. Pan with
the middle button, zoom with the wheel about the cursor, zoom extents.
Rasterizing releases the GIL. Phase 7 adds the GPU renderer beside it; this
one stays as the software fallback.

Measured (`cargo bench -p arq-render`, development laptop i7-8750H, one
core, 2026-10-12, 100,000 entities at 1920 × 1080): building the scene from
the document 59 ms, rendering the whole drawing 69 ms, a 1 % view 10 ms.

(First decided as `QPainter` from Python; changed on 2026-10-12 once the
reference machine was known: a Python loop per polyline would take about
half a second per frame there on large drawings.)

### D5.4 — Commands and the command window

Contract C: each command has an English name, aliases (`acad.pgp` style,
e.g. `L` for LINE), a parameter definition, and runs with a controlled
context (application, active document, view, prompt). The **command
window** is docked at the bottom, like AutoCAD and IngeCAD: a read-only
history of prompts and messages above an input line showing the current
prompt (`Command:`). Enter or Space on an empty line repeats the last
command, Esc cancels, typing completes command names. Commands are English
and always work; translated names may only be added (ADR 0006).

Phase 5 commands are those of documents and views: NEW, OPEN, SAVE, SAVEAS,
CLOSE, UNDO, REDO, ZOOM (extents, in, out), LAYER (opens the panel).

### D5.5 — Events

Contract E: frozen dataclasses (`DocumentOpened`, `DocumentChanged` with its
change set, `ActiveLayerChanged`, `CommandStarted`…) published on a Python
event bus; one `QObject` bridge re-emits them as Qt signals. Panels and the
viewport update only what a change set names.

### D5.6 — Panels

Docked and closable: **Layers** (on/off, freeze, lock, color, current,
create, rename, delete, all through the core, with undo), **Properties**
(the selected entity from phase 6; until then the drawing's own data:
name, file, version, entity counts, current layer), **History** (the undo
and redo steps by name; choosing one undoes or redoes up to it) and the
**command window**.

### D5.7 — Documents

One tab per open document, each with its own view. Opening and saving run on
a worker thread (the core releases the GIL), with a busy indicator, so the
window keeps responding. Closing a modified document or quitting asks to
save. Save As offers DXF first until the round-trip corpus makes DWG the
default (contract §10, phase 8).

### D5.8 — A guaranteed floor, scaling up from it

The reference machine is now known (recorded in ADR 0007): GT 730 Kepler on
**nouveau**, whose Vulkan is **software only**, an **i7-2600** (Sandy Bridge,
no AVX2), 16 GB, LMDE 7 on X11. It is the **floor**: everything works there.
Above it, the application **detects at run time** what each machine offers
and uses it, so one build runs well on as many configurations as possible,
from that machine to current hardware. It amends ADR 0007 as follows.

The floor:

- **Build baseline: x86-64-v2 at most** (ARM64 and Apple Silicon builds use
  their own baseline). No `target-cpu=native`, no `x86-64-v3`/AVX2/AVX-512 in
  any build; a test fails if one appears.
- **The CPU rasterizer of D5.3 always works**, whatever the graphics driver:
  it is the guaranteed fallback, before llvmpipe or WARP.
- **The GPU core path stays within wgpu's downlevel limits** (ADR 0007), as
  OpenGL ES 3.0 on this machine; nouveau runs Kepler at low clocks, so that
  path sends little work per frame.

Scaling up, decided at run time:

- **CPU instructions:** hot loops (flattening, rasterizing) may carry AVX2 or
  NEON variants chosen by runtime detection, only where a benchmark shows the
  gain; the baseline variant always exists.
- **Cores:** parallel work uses the cores the machine has (four on the floor
  machine, many more on current ones), never a fixed count.
- **GPU:** the probe prefers hardware adapters — Vulkan, DirectX 12 or Metal
  where available — and rejects an adapter whose device type is CPU while a
  hardware one exists (on the floor machine its Vulkan is software, so it
  uses OpenGL ES). Features beyond the downlevel path (MSAA, compute picking,
  larger buffers) are enabled when the adapter offers them.
- **Displays:** high-DPI scaling and both X11 and Wayland on Linux.

Each scaled-up path is tested against the floor path on the same input, so
a faster machine never draws something different. `tools/check_cpu_floor.sh`
runs every test suite on an emulated Sandy Bridge CPU with Intel SDE (a free,
proprietary emulator each developer downloads; never shipped), after proving
that the emulator refuses AVX2: QEMU was tried and does not, it hides AVX2
from CPUID but still executes it. `tools/reference_machine_report.sh` runs
the build, tests, self-check and timings on the real floor machine and
records its graphics stack. Both run before each release.

## Consequences

- A command written once appears in both workspaces, the command window and
  any later menu or ribbon without extra code.
- The real drawings are visible from this phase; phase 7 replaces only the
  painter, not the scene.
- `arq-render` and the drawing primitives are new Rust surface to test; the
  round-trip corpus gains a check that every entity of a drawing produces
  primitives or a marked placeholder.
