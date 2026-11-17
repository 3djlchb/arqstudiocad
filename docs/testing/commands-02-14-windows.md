# The 2D commands of 02.14: Windows comparison

Milestone 02.14. The commands delivered on Linux (OFFSET, TRIM, EXTEND,
FILLET, CHAMFER, STRETCH, BREAK, JOIN and RECTANG's options) were validated
internally only. This page compares them with ProgeCAD 2026 on Windows
(2026-11-15), with the maintainer's permission. Two earlier pages settle the
cases that needed a rule first: [STRETCH on arcs](stretch-arcs-windows.md)
and [BREAK on closed polylines](break-closed-polylines-windows.md). ARRAY has
[its own page](array-rectangular-windows.md).

## Method

**Environment:** ProgeCAD 2026 Professional ESN (`ACADVER` 19.0i), a blank drawing from its own `pcadISO (Metric).dwt`, World UCS, Top view, object snaps, ortho and snap off for the run.

**Both sides, the same 26 cases:**
- **ProgeCAD:** `tools/progecad_command_compare.lsp` (`arq:compare-commands`) builds synthetic sources and runs ProgeCAD's own command at fixed points. It saves each result as DWG 2018.
- **Ours:** `tools/progecad_02_14_ours.py` runs the same cases through our core, on the same sources and points. It saves each result as DWG and as DXF.

**Comparing.** ProgeCAD reopened all 78 files, one per instance, from its command line, and described every entity with layer and color. Ours were compared with ProgeCAD's as multisets of entities: without handles or order, and with coordinates to 10⁻⁶.

**Driving ProgeCAD by script, details that matter for repeating the run:**
- **TRIM** runs in its Quick mode, as ours. The object must be picked by a point; a picked entity with its point trims the wrong side.
- **EXTEND** runs in its standard mode, with the boundary given, as our call does. `TRIMEXTENDMODE` is set back afterwards.
- **STRETCH's crossing window** on a polyline is given from right to left.
- **STRETCH runs before FILLET and CHAMFER:** after a polyline CHAMFER, ProgeCAD's next STRETCH selects nothing.
- **RECTANG** remembers its options from one rectangle to the next, and does not take several options in one command from a script. Each option is set by a rectangle of its own, deleted at once, and every option is restored to zero at the end.

## Results

Every case reads back the same from our DWG and from our DXF.

| Commands | Cases | Result |
|---|---|---|
| OFFSET | line, circle outward, arc inward, closed polyline outward, open polyline with an arc | same as ProgeCAD, but for the open polyline's last bulge (below) |
| TRIM, Quick | a line's end, a circle's top half, a line's middle between two edges | same |
| EXTEND | a line to a boundary line, an arc to a boundary line | same |
| STRETCH | a closed polyline's side, a line's end | same |
| FILLET, CHAMFER | two lines (radius 2; distances 2 and 3), a closed polyline (radius 1; distances 1 and 1) | same, but for the chamfered polyline's first vertex (below) |
| BREAK | a line, a circle, an arc, an open polyline | same |
| JOIN | two collinear lines with a gap, two arcs of one circle | same, but for the joined line's direction (below) |
| RECTANG | plain, fillet 1, chamfer 1 and 2, width 0.5 at elevation 2 | same, but **the chamfer** (below) |

**22 of the 26 cases are the same as ProgeCAD's**, entity for entity. Four differ:

**RECTANG with two chamfer distances (R3).**
- **The difference.** ProgeCAD applies them the other way round from ours. From (0,0) to (10,6) with distances 1 and 2, its corner at (10,0) is cut from (8,0) to (10,1); ours from (9,0) to (10,2).
- **ProgeCAD's rule.** Drawn from each of the four corners, ProgeCAD walks the rectangle from its first corner, along X first (ours runs counterclockwise from the first corner, the same walk when ProgeCAD's is counterclockwise). At every corner it puts the first distance on the side the walk leaves by, and the second on the side it arrives by. Ours does the reverse.
- **Decision.** The maintainer chose to make ours match ProgeCAD's (2026-11-15), in a pull request of its own.
- **Done** (2026-11-15). Our rectangle from each of the four corners, reopened in ProgeCAD, now has the shape of ProgeCAD's.
  - Drawn from (0,0) or from (10,6), it also starts at ProgeCAD's first vertex.
  - From (0,6) or from (10,0), ProgeCAD walks clockwise while ours keeps its counterclockwise order, so the first vertex differs; the shape does not.
- **Not affected.** CHAMFER between two lines (C1) already matches.

**JOIN of collinear lines (J1).**
- **The difference.** The same line, but ProgeCAD leaves it running the other way, from (10,0) to (0,0). Ours keeps the source line's direction, as it keeps the source's id and properties.
- **Decision.** The maintainer chose to keep ours (2026-11-15).

**CHAMFER on a closed polyline (C2).** The same polygon starting at another vertex: ProgeCAD starts at (1,0), ours at (0,1).
- ProgeCAD is not consistent here: its FILLET on the same square (F2) starts at (0,1), as ours does for both.
- The shape is the same. Only operations that depend on vertex 0, such as BREAK on a closed polyline, would see it.

**OFFSET of an open polyline (O5).** ProgeCAD leaves a bulge on the open polyline's last vertex, which no segment uses; ours writes 0. The geometry is the same.

**The drawings are not committed:** the two tools reproduce them.

## To run: JOIN on polylines (J3–J7)

Added to both tools on 2026-11-17 with JOIN's second delivery: a constant
width meeting a line, two widths, a rectangle of lines many at once, a
branch, and a line with an arc. What ours gives, and what to confirm, is in
[JOIN's design](../design/join-command.md#windows-comparison-of-the-polylines-to-run).

## To run: MATCHPROP (M1–M3)

Added on 2026-11-17: a line's layer and color given to a circle, a
polyline's width and thickness, and a varying width that is not given. The
dump now also writes each entity's linetype, linetype scale, lineweight,
thickness and transparency. What ours gives is in
[MATCHPROP's design](../design/matchprop-command.md#windows-comparison-to-run).

Text cases M4–M6 added the same day: a text's height given to a text and
to an MTEXT, and, in ProgeCAD only, a text with style, width factor,
oblique angle and rotation, to see which ProgeCAD gives. The dump now
describes texts and MTEXTs.

## To run: ELLIPSE (EL1–EL4)

Added on 2026-11-17: by an axis and a distance, by its Center, by a
Rotation, and an elliptical Arc whose first axis points west. The dump now
describes ellipses (center, major axis, ratio, parameters). What ours
gives, and what to confirm, is in
[ELLIPSE's design](../design/ellipse-command.md#windows-comparison-to-run).
