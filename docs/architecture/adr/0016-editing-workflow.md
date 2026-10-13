# 0016. Editing workflow: selection, snaps, grips, first commands

Date: 2026-10-14 · Status: Accepted · Builds on [0006](0006-i18n.md),
[0010](0010-geometry-conventions.md), [0011](0011-document-model-and-transactions.md),
[0013](0013-application-shell.md), [0015](0015-workspaces.md)

## Context

Milestone 2.4 shows drawings; 2.5 makes them editable: selecting, picking
points precisely, drawing lines, circles and arcs, and transforming what
is selected. The core edits only lines, circles and arcs, but the
maintainer's real drawings are mostly blocks (INSERT) and lightweight
polylines (one has 21 blocks, 20 polylines and 8 lines in model space), so
with that limit almost nothing in them could be moved. Users know AutoCAD;
its conventions are the reference unless a decision below says otherwise.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on 2026-10-14.

### D1 — What can be transformed *(maintainer)*

Move, rotate, scale, mirror and copy work on **lines, circles, arcs, blocks
(INSERT) and lightweight polylines (LWPOLYLINE)**; the last two move from
milestone 2.7 to here, transformed **as a whole**:

- INSERT: insertion point, rotation and scale factors (a mirror negates the
  X scale and adjusts the rotation, as AutoCAD does).
- LWPOLYLINE: vertices and bulges (a mirror flips the bulges' sign) and
  widths (scaled with the factor); its elevation is kept.

Editing polyline vertices, exploding and editing a block's content stay in
2.7. Every other kind (texts, hatches, dimensions…) can be selected, erased
and given another layer or color; transforming or copying one fails the
whole operation with a clear message until its phase. Scaling is uniform.

### D2 — Picking and selection queries in Rust

The core answers, for every drawable entity (blocks as a whole, from the
drawing primitives of ADR 0013 D5.2):

- `pick(point, aperture)`: the entity whose drawn geometry passes closest
  to the point within the aperture (world units), topmost on ties;
- `select_window(area, crossing)`: entities wholly inside the area
  (window), or whose geometry touches it (crossing) — exact, not by box.

Entities on off or frozen layers are never picked; on locked layers they
are picked but every operation refuses them (as today). Queries still scan
(ADR 0011 D3.8): the spatial index comes in 2.6. Budget: a hover answer on
the reference machine within one frame for the maintainer's drawings
(a few thousand entities); benchmarks record it for 100,000.

### D3 — Selection, as AutoCAD *(maintainer)*

- A click **adds** the entity under the cursor; **Shift+click removes** it;
  **Esc** empties the selection.
- Dragging **left to right** is a **window** (blue, solid: what lies wholly
  inside); **right to left** a **crossing** (green, dashed: whatever it
  touches).
- Selecting first and then running a command (noun–verb) and running a
  command that asks "Select objects:" (verb–noun) both work; at that prompt
  the same gestures apply and Enter ends the selection. An editing command
  empties the selection when it finishes.
- The selection is per document, in Python (`Session`), published as
  `SelectionChanged`.

### D4 — How selection looks *(maintainer)*

As OpenCADStudio: what is selected is drawn in **blue** with its grips as
blue squares; whatever the cursor rests on is **highlighted** before the
click (drawn thicker). Window and crossing rectangles are translucent blue
and green. All of it is an overlay painted over the rendered image with the
camera's transform, so hovering and selecting never re-render the drawing.

### D5 — Object snaps and ORTHO *(maintainer)*

- **Running object snaps**, on and off with **F3** and from the status bar,
  where the active modes are chosen: Endpoint, Midpoint, Center, Quadrant,
  Intersection, Perpendicular, Tangent, Nearest (the kinds of
  `arq-geometry`'s snaps). Endpoint, Midpoint, Center and Intersection are
  on by default.
- At a point prompt, typing a mode's English key (`END`, `MID`, `CEN`,
  `QUA`, `INT`, `PER`, `TAN`, `NEA`) forces it for that point.
- Snaps work on blocks and polylines too, through their drawn segments, and
  between any two entities for intersections. They are computed in Rust
  within an aperture in pixels around the cursor; the marker is AutoCAD's
  glyph for the kind, with its translated name beside it.
- **ORTHO**, on and off with **F8** and from the status bar, keeps the next
  point horizontal or vertical from the last one. Polar and object tracking
  come later.

### D6 — Typing points and values *(maintainer)*

AutoCAD's syntax, angles in degrees at this boundary (ADR 0010):
`x,y` absolute; `@dx,dy` relative to the last point; `@distance<angle`
relative polar; `distance<angle` absolute polar; a lone distance goes that
far in the cursor's direction (direct distance entry). On an empty command
line, Enter or Space repeats the last command; Space works as Enter, as in
AutoCAD.

### D7 — Multifunction grips *(maintainer)*

Grips show on selected entities (up to a limit, as AutoCAD's
`GRIPOBJLIMIT`, 100 by default): line ends and midpoint; circle center and
quadrants; arc ends, midpoint and center; one grip for a block (its
insertion point) and one for a polyline (its first vertex), which only move
them until vertex editing (2.7).

Clicking a grip makes it **hot** (red). Its first mode is **Stretch**
(a line's end moves, a circle's quadrant changes the radius, a midpoint or
center moves the entity); **Space or Enter** cycles **Stretch → Move →
Rotate → Scale → Mirror** around the hot grip, with the options Base point,
Copy (each pick makes a copy), Undo and eXit. Every grip edit is one undo
step; with Copy, the copies made in one session of the grip are one step.

### D8 — First commands

`LINE` (Undo, Close), `CIRCLE` (center and radius or Diameter, 2P, 3P),
`ARC` (three points; Start, Center, End), `MOVE`, `COPY` (multiple),
`ROTATE` (angle), `SCALE` (factor), `MIRROR` (erase the source or not),
`ERASE`, with AutoCAD's aliases (`L`, `C`, `A`, `M`, `CO`/`CP`, `RO`, `SC`,
`MI`, `E`). The core gains `copy_entities`. Commands that combine operations
use groups (ADR 0011 D3.9).

### D9 — Command names in Spanish *(maintainer)*

Commands are written, documented and tested in English, and each one gets
its Spanish name in the same step that creates it, not later
*(maintainer)*: the Spanish pack adds AutoCAD's Spanish names and aliases
(`LINEA`, `CIRCULO`, `ARCO`, `DESPLAZA`, `COPIA`, `GIRA`, `ESCALA`,
`SIMETRIA`, `BORRA`…) in `locales/es/commands.json`, starting with the
commands of 2.4 (`NUEVO`, `ABRIR`, `GUARDAR`, `CAPA`…). As ADR 0006 requires, English names
and aliases always work, `_LINE` forces English, and a pack may add a name
but never shadow an English name or alias; a test checks it.

### D10 — Editing from Properties *(maintainer)*

The property grid shows the selection: its kind as the caption ("Line",
"Circle", "*Varies* (3)" for mixed kinds). Editable for any selection,
mixed included: **Layer** (a list) and **Color** (a combo with a swatch:
ByLayer, ByBlock, the ACI colors, more colors…). Editable for lines,
circles and arcs: their **geometry numbers** (start and end; center and
radius; center, radius and start and end angles, in degrees). Derived values
(length, delta, angle, area, circumference) and the rest (linetype,
lineweight, a block's or polyline's data) are shown, not edited, until
their phase. Each edit is one undo step; a value the core refuses is
reported on the command line and the grid shows the drawing as it is.

## Consequences

- The maintainer's drawings become editable as a whole in 2.5; their
  contents (vertices, block definitions) in 2.7.
- The core grows: transforms for INSERT and LWPOLYLINE, `copy_entities`,
  `pick`, `select_window` and snap candidates; all bound to Python, typed,
  and tested in Rust and Python.
- The viewport gains an overlay layer; the drawing is re-rendered only when
  the document or the camera changes.
- Selection by scanning is measured now; if the budget fails on large
  drawings, the spatial index of 2.6 is moved earlier.
