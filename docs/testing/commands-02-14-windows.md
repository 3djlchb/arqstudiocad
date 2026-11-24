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

## JOIN on polylines, MATCHPROP and ELLIPSE (2026-11-20)

The cases added with JOIN's second delivery, MATCHPROP and ELLIPSE, run in
ProgeCAD 2026 on Windows with the maintainer's go-ahead, by the same method
and tools as above. Every earlier case ran again with them.

**41 cases** (M6 in ProgeCAD only): ProgeCAD's 41 results and our 40, each
in DWG and DXF, all 121 files reopened by ProgeCAD from its command line and
described with `arq:dump-full`, layer, color, linetype, linetype scale,
lineweight, thickness and transparency included.

**33 of the 41 are the same as ProgeCAD's**, entity for entity, and every
case reads back the same from our DWG and from our DXF.

| Commands | Cases | Result |
|---|---|---|
| The earlier 26 | O, T, E, S, F, C, B, J1–J2, R | as before: the same, but C2, J1 and O5 (above); R3 now the same, since the rule changed |
| JOIN on polylines | J3 a width meeting a line, J4 two widths, J5 a rectangle many at once, J6 a branch, J7 a line and an arc | J4, J6, J7 the same; **J3** and J5 differ (below) |
| MATCHPROP | M1 layer and color, M2 a width and thickness, M3 a varying width, M4 a text's height, M5 a text's height to an MTEXT | M1, M4, M5 the same; M2 and M3 differ (below) |
| MATCHPROP, ProgeCAD only | M6 a text in style ROTULO, width factor 0.8, oblique 15°, rotated 30° | ProgeCAD gives all four (below) |
| ELLIPSE | EL1 axis and distance, EL2 Center, EL3 Rotation, EL4 Arc | **all the same**, the arc's angles counted from the first axis point, as ours |

**JOIN of a constant width and a line (J3).**
- **The difference.** ProgeCAD gives the joined line the polyline's width: vertices (0,0), (4,0) and (4,3) with widths 0.5/0.5, 0.5/0.5 and 0/0. Ours keeps the line's 0.
- **Decision.** The maintainer chose to match ProgeCAD (2026-11-20), in a pull request of its own; done on 2026-11-22, ours now as ProgeCAD's.

**JOIN of four lines many at once (J5).** The same closed rectangle, counterclockwise in both; ProgeCAD starts at (0,3), ours at (0,0). The maintainer chose to keep ours (2026-11-20).

**MATCHPROP of a constant width (M2).** The same widths drawn: ProgeCAD writes 0.5 at each vertex with no constant width, ours sets the constant width 0.5. The maintainer chose to keep ours (2026-11-20).

**MATCHPROP from a varying width (M3).**
- **ProgeCAD** leaves the target's first segment tapering from 0.2 to 0.8, the first source segment's start and the second's width.
- **Ours** gives no width, so the target keeps 0.25. AutoCAD's documentation says the same: a source polyline of varying width gives no width.
- **Decision.** The maintainer chose to keep ours (2026-11-20).

**MATCHPROP of a text's look (M6).** ProgeCAD gives the style, the width factor, the oblique angle and the rotation. Ours, by its design (option A), gives the first three and never the rotation. The maintainer chose to keep ours (2026-11-20): matching properties does not turn a drawing's texts. Our tool cannot build this source yet; the rule is covered by the Rust tests.

## ELLIPSE edits (2026-11-20)

The cases added with the ellipse as a curve (ADR 0041), run the same day
after merging them, with TRIM, EXTEND, BREAK and STRETCH's earlier cases
again: 23 cases, 67 files reopened.

| Commands | Cases | Result |
|---|---|---|
| TRIM, EXTEND, BREAK | EL5 a whole ellipse trimmed, EL6 an arc cut in the middle, EL7 an arc extended, EL8 a whole ellipse broken, EL9 a line extended to an ellipse | **all the same** |
| STRETCH | EL10 a whole ellipse, EL11 an arc with both ends in the window | the same |
| STRETCH | EL12 an arc with one end in the window | differs (below) |
| The earlier TRIM, EXTEND, BREAK, STRETCH | T1–T3, E1–E2, B1–B4, S1–S2 | the same, as before |

**A whole ellipse stretched (EL10).**
- **The case was wrong at first.** Its crossing window lay round the center and touched no part of the ellipse, so ProgeCAD selected nothing. Ours, given the ellipse directly, moved it.
- **Probed again,** with a window crossing the ellipse: ProgeCAD moves a whole ellipse when the window holds its center, and leaves it when it does not. Ours does the same in both.
- **The case** now uses the crossing window, in both tools.

**An arc stretched by one end (EL12).**
- **ProgeCAD** reshapes it into another ellipse: center (0.4378,−0.0950), major axis (5.5782,−0.2137), ratio 0.5538.
- **Ours** refuses it, by its design, and changes nothing.
- **Decision.** The maintainer chose to keep the refusal (2026-11-20). ProgeCAD's rule is not evident, and changes the whole ellipse.

## Elliptical arcs joined, SPLINE and the spline as a curve (2026-11-23)

The automated cases added with JOIN of elliptical arcs (ADR 0041), SPLINE
(ADR 0042) and the spline as a curve (ADR 0043), run in ProgeCAD 2026 on
Windows (Codex paused; the maintainer's go-ahead), by the method above:
23 cases, 67 files reopened.

**Splines read from the DXF.** ProgeCAD's ActiveX gives neither a spline's
knots nor whether it is periodic, which these cases are about. So each of
ProgeCAD's results was also saved by ProgeCAD as DXF, and the splines were
compared by their group codes: degree, flags, knots, control points, fit
points, tangents and weights. Shapes were compared by sampling both curves.

| Commands | Cases | Result |
|---|---|---|
| JOIN of elliptical arcs | EL13 across a gap, EL14 many at once round to the whole ellipse, EL15 Close | **all the same** |
| SPLINE | SP1 fit, Chord; SP4 Square root knots; SP5 by vertices | the same; in SP4 ProgeCAD keeps no fit points, ours does |
| SPLINE, closed by fit points | SP3 | **the same curve, 4×10⁻⁷ apart**; our DXF read by ProgeCAD as open (below) |
| SPLINE, tangents | SP2b, asked after the points | the start tangent differs (below) |
| SPLINE, not comparable | SP2 tangents among the points (AutoCAD 2021's prompts); SP6 degree 2 | ProgeCAD has neither: it takes no tangent options there and makes every spline cubic, whatever the degree asked |
| TRIM, EXTEND, BREAK with splines | SC1 a line trimmed at a spline, SC2 a line extended to one, SC3 the spline trimmed, SC5 broken | **the same** |
| EXTEND a spline | SC4 | ProgeCAD extends it; ours refused (below) |
| STRETCH | SC7 a fit point | the same |
| STRETCH | SC6 a control vertex | ProgeCAD moves nothing; ours moves the vertex |
| JOIN of splines | SC10 three in a ring at once | the same: closed, not periodic |
| JOIN of splines | SC8 two cubics; SC9 a cubic and a quadratic | differ (below) |
| OFFSET | SC11 an ellipse, SC12 an elliptical arc, SC13 a spline | different approximations, ours closer (below) |

**A spline closed by its fit points, saved in DXF (SP3).**
- **The same curve.** ProgeCAD keeps it clamped: its last control point is its first, its first fit point repeated at the end (5 fit points). Ours keeps it periodic: its last three control points are its first three, with 4 fit points. Sampled, the two curves lie within 4×10⁻⁷ of each other.
- **What goes wrong.** ProgeCAD reads our DWG of it as closed, and our DXF as open, though both DXFs carry the same flags (70 = 1067). Closed splines by vertices (SP6, SC10, SC11) read back closed from our DXF.
- **Decision.** The maintainer chose to write it as ProgeCAD does, the first fit point repeated at the end, in a pull request of its own (2026-11-23).

**A spline extended (SC4).**
- **ProgeCAD** extends the Bézier from its end (6,0) to x = 8 by a straight segment along its end tangent, to (8,−3). The segment becomes part of the spline: control points (6,0), (6.667,−1), (7.333,−2), (8,−3), a span of knots as long as the segment, √13.
- **Ours** refused it, by its design, until ProgeCAD showed its rule.
- **Decision.** The maintainer chose to adopt ProgeCAD's rule (2026-11-23), in a pull request of its own.

**The start tangent asked after the points (SP2b).** Given toward (0,5), ProgeCAD's spline leaves (0,0) downward, away from it; ours leaves toward (0,5), by its design. The end tangent is the same. The maintainer chose to keep ours (2026-11-23).

**Splines joined (SC8, SC9).**
- **SC8.** The same curve. ProgeCAD runs it from (12,0), the joined spline's far end; ours keeps the source's direction, as for lines (J1).
- **SC9.** ProgeCAD gives a quadratic that starts at (2,3), losing the cubic's first span from (0,0). Ours raises the quadratic to a cubic, exactly.
- **Decision.** Ours kept for both (the maintainer, 2026-11-23).

**Curves offset (SC11–SC13).** Both are approximations, of different kinds. The error is the largest difference, along the curve, between its distance from the source and the offset distance:

| Case | ProgeCAD | Ours |
|---|---|---|
| SC11 the whole ellipse, by 1 | quadratic, 53 control points, uniform knots every 7.5°; error 1.1×10⁻⁴ | cubic, 79 control points, periodic; error 1.1×10⁻⁵ |
| SC12 its upper half, by 1 | quadratic, 27 control points; error 1.5×10⁻⁴ | cubic, 37 control points; error 9.7×10⁻⁶ |
| SC13 the Bézier, by 0.5 | cubic, 27 control points, uniform knots; error 3.2×10⁻⁴ | cubic, 35 control points; error under 2×10⁻⁶, the measurement's resolution |

Ours is 10 to more than 100 times closer, with a few more control points. ProgeCAD's quadratics are written as rational with every weight 1.

**Other differences kept by the maintainer (2026-11-23):** SC6, where ProgeCAD's STRETCH moves fit points (SC7) but not control vertices; SP4's fit points.

## To run: JOIN onto a spline (SC14–SC15)

Added on 2026-11-24, when JOIN took lines, arcs, elliptical arcs and open
polylines onto a spline, as AutoCAD (the maintainer's choice):
- SC14, from the Bézier, a line after it and a half circle after that;
- SC15, lines either side of it, many at once.

To see: whether ProgeCAD makes one spline of each, its degree, whether
it is rational, and which object's id and direction it keeps. What ours
gives is in [SPLINE's design](../design/spline-command.md#the-spline-as-a-curve-adr-0043).

## To run by hand: ELLIPSE grips and Properties

Added on 2026-11-20 with the ellipse's grips and Properties (ADR 0041). By
hand, on an ellipse about (0,0) with semi-axes 5 and 3, and on its first
quarter, compared with [ELLIPSE's design](../design/ellipse-command.md):
which grips show; the center grip moved; the grip at (5,0) taken to (8,1);
the grip at (0,3) taken to (0,7), past the major; the quarter's end at
(0,3) taken toward (−10,0). And the Properties panel's Start and End angle
of the quarter, against the dump's `start-angle` and `end-angle`.

## To run by hand: spline grips and Properties

Added on 2026-11-23 with the spline's grips and Properties (ADR 0043). By
hand, on a spline SPLINE makes through (0,0), (3,4), (7,1), (9,6) and on
one by those vertices, compared with [SPLINE's design](../design/spline-command.md):
- which grips show;
- the fit point (7,1) taken to (7,−1), and the vertex (9,6) to (9,8);
- the Properties panel's rows, and a point's X and Y edited.
