# JOIN command

Milestone 02.14, group 3, with AutoCAD 2021 as the behavioral baseline.
Scope approved by the maintainer (2026-11-15): two pull requests, lines
and arcs first, polylines second; a source line joins lines only, as
AutoCAD documents. Delivery 1, lines and arcs, is implemented.

## Behavior

`JOIN`, alias `J`, Spanish command `UNIR`, in the Modify menus, the
Classic toolbar and the Ribbon through one shared action. Top view only,
as BREAK and STRETCH today.

### Prompts

1. `Select source object or multiple objects to join at once:` the
   viewport selects as at any selection prompt (picks, windows,
   crossings), Enter ending it. One object selected is the **source**;
   several are joined **many at once**. What is selected before JOIN
   answers this prompt. Unlike AutoCAD, where one pick makes the source at
   once, Enter follows that pick: the viewport's selection serves both.
2. With a source: `Select lines to join to source:`, or `Select arcs to
   join to source [Close(L)]:` for a source arc; any selection method,
   Enter ending it. Close turns the source arc into its circle.

### With a source object

The result is the source, which keeps its id and every property (layer,
color, linetype, lineweight, XDATA…); the objects joined to it are
erased.

| Source | Joins | Rule |
|---|---|---|
| Line | lines | Collinear, gaps allowed: the result spans the farthest ends. |
| Arc | arcs | Same center and radius, gaps allowed, joined counterclockwise from the source. A full turn gives a circle. `cLose` alone turns the source arc into its circle. |
| Open polyline | lines, arcs, open polylines | Contiguous (ends touching within the model tolerance) and in the polyline's plane and elevation: one polyline, segments in path order, arcs as bulges. |

A circle or a closed polyline cannot be a source.

### Many at once

The objects are grouped as AutoCAD does:

- collinear lines become one line;
- arcs with the same center and radius become one arc, or a circle when
  they make a full turn;
- lines, arcs and polylines whose ends touch become one polyline.

Each group's result takes the properties of the group's first object
selected.

### What it says

`Joined to the source: {count}.`, or `Joined: {count}, into
{results}.` many at once. Objects that cannot join are left untouched and
counted: `Discarded from the operation: {count}.` Nothing joined is not
an error (`Nothing joined.`): the drawing is left as it was, with no undo
step.

### All or nothing

One operation, one `JOIN` undo step, one change set. Cancelling at any
prompt, or a failed join, leaves the document unchanged. Objects on a
locked layer are discarded, as AutoCAD does.

## Deliveries

1. **Lines and arcs** (done): line with lines, arc with arcs, `cLose`,
   many-at-once for lines and arcs. No polyline is made or changed; a
   polyline as source is refused for now.
2. **Polylines:** an open polyline as source; lines, arcs and polylines
   whose ends touch become one polyline, with source and many-at-once.
   Widths, thickness and elevation to be decided in that pull request.

## Reference and validation

Behavioral reference: AutoCAD 2021 Help, "JOIN (Command)".
The implementation is original; no Autodesk source or artwork is copied.
The public reference is a behavioral specification, not external
validation.

Tests belong to geometry (collinearity, arc merging, counterclockwise
spans, full turns, tolerances), core (atomicity, kept id and properties,
erased objects, change sets, spatial index, undo/redo), bindings
(what they return, unknown ids, DWG and DXF round trips) and Python/Qt (prompts, both selection modes,
`cLose`, messages, Spanish text). Each new test is verified by sabotage.

Where the code lives: `arq_geometry::{join_lines, join_arcs,
join_groups}`; `CadDocument::join_entities` and `close_arc` (arq-core),
with `Document::arc_to_circle`; the `join_entities` and `close_arc`
bindings; `arqstudiocad.commands.joining`.

Validation on LMDE 7 is internal: joined lines and the circle an arc
becomes are written to DWG and DXF and read back by the own codec.
Reopening them in ProgeCAD is noted in the pull request for the Windows
laptop.
