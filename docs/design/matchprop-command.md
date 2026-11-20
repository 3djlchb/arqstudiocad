# MATCHPROP command

Milestone 02.14, group 4, with AutoCAD 2021 as the behavioral baseline.
Scope chosen by the maintainer (2026-11-17): the basic properties the
drawing model holds and the polyline's special ones first (#206), then the
text's (option A: style, height, width factor and oblique angle); Settings
as a Property Settings window; one undo step per selection of
destinations.

## Behavior

`MATCHPROP`, alias `MA`, Spanish command `IGUALARPROP`: first in the
Modify menu, in a Properties group of the Ribbon's Home tab, and on the
Layers toolbar, through one shared action.

1. `Select source object:` one object is picked. What was selected before
   is let go.
2. It says the settings in force: `Current active settings: Color Layer
   Linetype Linetype scale Lineweight Transparency Thickness Text
   Polyline`.
3. `Select destination object(s) [Settings(S)]:` the viewport selects
   (picks, windows, crossings); Enter gives the properties to what is
   selected, in one `MATCHPROP` undo step, and asks again. Enter with
   nothing selected ends the command.
4. **Settings** opens Property Settings: a check box per property, under
   Basic Properties and Special Properties. What is chosen is remembered
   for the session, and said again.

## The properties

| Setting | Name in the API | Given |
|---|---|---|
| Color | `color` | the color, a color book's name and handle with it |
| Layer | `layer` | the layer |
| Linetype | `linetype` | the linetype, its record's handle with it |
| Linetype scale | `ltscale` | the linetype scale |
| Lineweight | `lineweight` | the lineweight |
| Transparency | `transparency` | the transparency |
| Thickness | `thickness` | between entities that have one: lines, arcs, circles, polylines, texts |
| Text | `text` | between texts (TEXT, MTEXT, attribute definitions and attributes): the text style and height, and where both have them (not an MTEXT) the width factor and oblique angle *(to confirm)*; never the justification, the rotation or the words |
| Polyline | `polyline` | between lightweight polylines: the linetype generation, and the width when the source's is uniform (a varying width is not given, as AutoCAD documents) |

- **What already matches is left alone:** a target written only when a
  value differs, so a selection that changes nothing makes no undo step.
  Reaching an entity to write it records a change, so the target is
  worked on a copy first.
- **Locked layers:** the source may be on one, as it is only read. Targets
  on one are left and counted: `{count} were on a locked layer.`
- **Text, what AutoCAD documents:** its help says only that the text
  style is given. Height, width factor and oblique angle follow the
  maintainer's option A and wait for the Windows comparison (M4–M6): if
  ProgeCAD gives less, a short pull request takes it out. The
  justification is never given, as it would move the text about its
  point.
- **Not offered yet:** plot style, and the special properties of
  dimensions, hatches, tables, viewports, materials and multileaders. They
  come with the entities and milestones that hold them.

## Reference and validation

Behavioral reference: AutoCAD 2021 Help, "MATCHPROP (Command)" and
"Property Settings Dialog Box". The implementation is original; no
Autodesk source or artwork is copied.

Where the code lives: `arq_document::MatchSettings` and
`Document::match_properties` (`matching.rs`); `CadDocument::match_properties`
and `Matched` (arq-core); the `match_properties` binding, which takes the
settings by name; `arqstudiocad.commands.matchprop` and
`arqstudiocad.ui.match_settings_dialog`.

Tests belong to the document (every property and the thickness given,
only those chosen, a polyline's uniform width and linetype generation, a
varying width not given, nothing written when matching, every property
through DWG and DXF), the core (one step per call, locked targets left and
counted, a locked source, no step when nothing changes, undo), the binding
(names, unknown names refused) and Python/Qt (the prompts and the loop,
Settings remembered, cancelling, the window, the shared action, Spanish).
Each was verified by sabotage.

Validation on LMDE 7 is internal.

## Windows comparison

Three cases are in both comparison tools, as 02.14's others
([method](../testing/commands-02-14-windows.md)). The LISP dump now also
writes each entity's linetype, linetype scale, lineweight, thickness and
transparency, where it has them. Our tool can only make what the Python
API sets (layers, colors, a polyline's width and thickness), so the other
properties are left to the Rust tests.

| Case | Source and target | Ours |
|---|---|---|
| M1 | a line on A-WALLS, red, and a circle | the circle on A-WALLS, red |
| M2 | an open polyline of width 0.5 and thickness 2, and a plain one | the target with width 0.5 and thickness 2 |
| M3 | a polyline of widths 0.2 then 0.8, and one of width 0.25 | the target keeps 0.25 |
| M4 | a text 2.5 high and one 1.0 high | the target 2.5 high, its words kept |
| M5 | a text 2.5 high and an MTEXT 1.0 high | the MTEXT 2.5 high |
| M6 | ProgeCAD only (ours cannot make the source yet): a text in style ROTULO, width factor 0.8, oblique 15°, rotated 30° | to see which of style, width and oblique ProgeCAD gives, and that it keeps the rotation |

The LISP dump also describes texts and MTEXTs (insertion point, words,
height, style, and a text's width factor, oblique angle and rotation).

**Run on 2026-11-20** ([results](../testing/commands-02-14-windows.md#join-on-polylines-matchprop-and-ellipse-2026-11-20)): M1, M4 and M5 the same as
ProgeCAD. Kept by the maintainer's choice: M2, the same widths written per
vertex by ProgeCAD; M3, where ProgeCAD tapers the target's first segment
and ours, as AutoCAD documents, gives no varying width; M6, where ProgeCAD
also gives the rotation and ours never does.
