# JOIN command

Milestone 02.14, group 3, with AutoCAD 2021 as the behavioral baseline.
Scope approved by the maintainer (2026-11-15): two pull requests, lines
and arcs first, polylines second; a source line joins lines only, as
AutoCAD documents. Both deliveries are implemented: lines and arcs
(#189, compared in ProgeCAD, [evidence](../testing/commands-02-14-windows.md)),
and polylines, whose points marked *to confirm* wait for the Windows
comparison (cases J3–J7 below).

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
2. With a source: `Select lines to join to source:`, `Select arcs to
   join to source [Close(L)]:` for a source arc, `Select elliptical arcs
   to join to source [Close(L)]:` for a source elliptical arc (since
   2026-11-22, ADR 0041; its rules in
   [ELLIPSE's design](ellipse-command.md)), or `Select objects to join
   to source:` for a source polyline or spline (a spline's since
   2026-11-24, when it took other curves; from 2026-11-23 `Select
   splines to join to source:`, ADR 0043); any selection method, Enter ending it.
   Close turns the source arc into its circle, an elliptical arc into its
   ellipse.

### With a source object

The result is the source, which keeps its id and every property (layer,
color, linetype, lineweight, XDATA…); the objects joined to it are
erased.

| Source | Joins | Rule |
|---|---|---|
| Line | lines | Collinear, gaps allowed: the result spans the farthest ends. |
| Arc | arcs | Same center and radius, gaps allowed, joined counterclockwise from the source. A full turn gives a circle. `cLose` alone turns the source arc into its circle. |
| Open spline | open splines, and since 2026-11-24 lines, arcs, elliptical arcs and open polylines | Contiguous, as a polyline: one spline, each piece an exact spline raised to the highest degree, turned round as needed, closed when the chain comes back to its start; by its vertices (since 2026-11-23, ADR 0043 D4; its rules in [SPLINE's design](spline-command.md#the-spline-as-a-curve-adr-0043)). A line, an arc or a polyline as source leaves a spline. |
| Open polyline | lines, arcs, open polylines | Contiguous (ends touching within the model tolerance, no gap filled) and at the polyline's height: one polyline, segments in path order, arcs as bulges. Pieces attach at either end, turned round when they run the other way. |

A circle, a closed polyline or a closed spline cannot be a source, nor
join one.

### Polylines: what each segment keeps

As AutoCAD, the source polyline keeps its id and every property, its
plane, thickness and linetype generation included; in a polyline made many
at once from a line or an arc, those of that first object (its thickness
too), in the World plane.

- **Widths** *(to confirm)*: each segment keeps its own. Segments that come
  from lines and arcs have none. A constant width stays only while every
  segment still has it; otherwise the polyline keeps per-segment widths.
- **A chain that comes back to its start** *(to confirm)* closes the
  polyline, and nothing more joins it.
- **A branch** *(to confirm)*: where several pieces touch an end, the first
  selected joins; the others are discarded.
- **Height:** what is not at the source's height (its elevation in the
  world) is discarded; a tilted polyline cannot join.

### Many at once

The objects are grouped as AutoCAD does, level by level (one height
each):

1. **Chains of touching ends,** each from its earliest object selected:
   collinear lines alone stay a line, arcs of one circle alone an arc (the
   circle when they close it), and any other chain becomes a polyline.
2. **What is left,** lines and arcs, as with a source: collinear lines into
   one line across gaps, arcs of one circle into one arc.

Each result takes the properties of its first object selected. When that
object is a polyline, it is the result; when it is a line or an arc, a
new polyline is made with its properties and the line or arc is erased.

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
2. **Polylines** (done, 2026-11-17): an open polyline as source; chains of
   touching lines, arcs and polylines many at once. The rules for widths,
   thickness and height follow AutoCAD (maintainer, 2026-11-16), those
   marked *to confirm* until the Windows comparison.

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
join_groups, JoinPath, chain_paths}`; `CadDocument::join_entities` and
`close_arc` (arq-core), with `Document::{arc_to_circle, join_path,
write_joined_polyline}`; the `join_entities` and `close_arc` bindings;
`arqstudiocad.commands.joining`.

Validation on LMDE 7 is internal: the results, a polyline's per-segment
widths included, are written to DWG and DXF and read back by the own codec.

## Windows comparison of the polylines

Five cases are in both comparison tools, `tools/progecad_command_compare.lsp`
(`arq:compare-commands`) and `tools/progecad_02_14_ours.py`, as the other
02.14 cases ([method](../testing/commands-02-14-windows.md)). What ours
gives, read back from our DWG and DXF:

| Case | Sources | Ours |
|---|---|---|
| J3 | open polyline (0,0)–(4,0), constant width 0.5, and line (4,0)–(4,3) | 3 vertices, open; since 2026-11-22 every segment 0.5 wide, written as the constant width 0.5 (before: 0.5/0.5 then 0/0, no constant width); ProgeCAD writes 0.5 at each vertex, drawn the same, as in M2 |
| J4 | polylines (0,0)–(4,0), width 0.2, and (8,0)–(4,0), width 0.8 | 3 vertices, open; widths 0.2/0.2, then 0.8/0.8 |
| J5 | four lines of the rectangle (0,0)–(4,3), many at once | a closed polyline of 4 vertices |
| J6 | open polyline (0,0)–(1,0), lines (1,0)–(2,1) and (1,0)–(2,−1) | the first line joins; the second is left |
| J7 | line (0,0)–(4,0) and an arc from (4,0) round to (4,4), many at once | an open polyline of 3 vertices, with the line's properties |

Each difference found is the maintainer's to decide, as the earlier ones
were.

**Run on 2026-11-20** ([results](../testing/commands-02-14-windows.md#join-on-polylines-matchprop-and-ellipse-2026-11-20)): J4, J6 and J7 the same as
ProgeCAD. J3 differed: ProgeCAD gives the joined line the polyline's width
0.5, which the maintainer chose to match, in a pull request of its own:
since 2026-11-22 the lines and arcs chained onto a polyline of one width
all along take it, from a source or as a chain's first selected; other
polylines keep their own widths (J4), and a source of varying width gives
none. J5
is the same rectangle starting at another vertex, kept.
