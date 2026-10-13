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

### D5.3 — Provisional viewport with QPainter

A widget paints the render scene with `QPainter` until phase 7 replaces it:
camera in `f64` (center, scale), pan with the middle button, zoom with the
wheel about the cursor, zoom extents. It reads the scene again only when a
change set arrives, and skips polylines outside the view. It works on any
machine, the GT 730 included.

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

## Consequences

- A command written once appears in both workspaces, the command window and
  any later menu or ribbon without extra code.
- The real drawings are visible from this phase; phase 7 replaces only the
  painter, not the scene.
- `arq-render` and the drawing primitives are new Rust surface to test; the
  round-trip corpus gains a check that every entity of a drawing produces
  primitives or a marked placeholder.
