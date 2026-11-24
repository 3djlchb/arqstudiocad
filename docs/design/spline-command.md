# SPLINE command

Milestone 02.14, group 5, with AutoCAD 2021 as the behavioral baseline
([ADR 0042](../architecture/adr/0042-spline.md)). The maintainer chose the
scope:
- **2026-11-22:** the ellipse's path; both of AutoCAD's methods; a
  standard global interpolation.
- **2026-11-23:** fit-only splines computed, Close fitted periodic, two
  pull requests.

The making is in part B1 (`CadSpline`); this page is the command.

## Behavior

`SPLINE`, alias `SPL`, the same name in Spanish, in the Draw menu, the
Ribbon's Draw group and the Draw toolbar. Top view only.

It first says the settings, remembered for the session as AutoCAD's
SPLMETHOD, SPLKNOTS and SPLDEGREE: `Current settings: Method=Fit
Knots=Chord`, or `Method=CV  Degree=3`.

1. `Specify first point [Method(M)/Knots(K)]:` (Fit) or `[Method(M)/
   Degree(D)]:` (CV).
   - **Method:** `Enter spline creation method [Fit(F)/CV] <F>:`.
   - **Knots:** `Enter knot parameterization [Chord(C)/Square root(S)/
     Uniform(U)] <C>:`.
   - **Degree:** `Enter degree of spline <3>:`, a whole number from 1 to
     10.
   The first prompt comes back after each of them.
2. **Fit:** `Enter next point`, with:
   - **start Tangency** (after the first point): `Specify start tangent:`,
     the direction from the first point to the point given;
   - **end Tangency** and **Undo** (from the second point): `Specify end
     tangent:`, the direction from the last point, ends the command; Undo
     takes back the last point;
   - **Close** (from the third point): the spline closes, periodic, and
     the command ends; a start tangent given is not used.
3. **CV:** `Enter next point`, with Undo from the second point and Close
   from the third. The degree is lowered to one fewer than the points when
   they are fewer.
4. **Enter** ends it with two points or more; with one, nothing is drawn.

A point on the last one is refused ("The two points must differ."), and a
tangent shown by its own base point too. Points that make no spline draw
nothing ("Those points define no spline.").

While points are asked, the spline follows the cursor: through it as the
next point, or ending along it at the end tangent. One `SPLINE` undo step;
cancelling leaves the drawing unchanged.

What is drawn is ADR 0042's:
- **Fit:** the cubic through the points, keeping its fit points, knots
  method and tangents with its control points.
- **CV:** the spline of those vertices, its knots uniform.
- **Snaps:** it snaps to its ends and nearest point.
- **Transforms:** it moves, turns, scales and mirrors as a whole.

Object and toLerance are not offered yet (ADR 0042 D4).

## Reference and validation

Behavioral reference: AutoCAD 2021 Help, "SPLINE (Command)". The
implementation is original; no Autodesk source or artwork is copied.

Tests:
- **Command:** each prompt and its options in turn, Knots and the method
  and degree remembered, start and end Tangency, Undo, Close dropping a
  start tangent, CV with a degree refused out of range, a point on the
  last one or a tangent on its base refused, nothing drawn when
  cancelled.
- **Preview:** the spline it shows is the one the command draws, through
  the cursor or ending along it.
- **UI:** the Draw menu, ribbon and toolbar, and the Spanish label and
  name.
- **Snaps:** an open spline drawn snaps to its end; a closed one has none.

Each was verified by sabotage.

## Windows comparison

Cases SP1–SP6 are in both comparison tools, as 02.14's others
([method](../testing/commands-02-14-windows.md)). The LISP dump describes
each spline: its degree, closed and periodic flags, knot
parameterization, tangents, fit points, control points and knots.

The points are (0,0), (3,4), (7,1) and (9,6), but for SP3:

| Case | Command | Ours |
|---|---|---|
| SP1 | Fit, Chord | cubic, knots 0×4, 5, 10, 15.3852×4; control (0,0), (0.8656,2.1171), (2.5968,6.3514), (7.7418,−1.605), (8.5703,3.4031), (9,6) |
| SP2 | Fit, start tangent toward (0,5), end toward (12,6) | control (0,0), (0,1.6667), (2.8364,6.8714), (8.2071,−2.7919), (7.2049,6), (9,6) |
| SP2b | the same, the tangents asked after the points (older prompts) | as SP2 |
| SP3 | Fit, Close, through (0,0), (6,1), (8,5), (3,7) | periodic, 7 control points, the last three the first three |
| SP4 | Fit, Knots Square root | knots 0×4, 2.2361, 4.4721, 6.7927×4 |
| SP5 | CV, degree 3 | the four points, knots 0×4, 1×4 |
| SP6 | CV, degree 2, Close | periodic, the first two points repeated, knots −2 to 6 |

ProgeCAD may lack AutoCAD 2021's prompts: a case it does not take logs its
command error. To confirm:
- the fit: knots in chord lengths and the control points;
- the tangents' length;
- what Close makes of a fit;
- the CV knots.

**Run on 2026-11-23** ([results](../testing/commands-02-14-windows.md#elliptical-arcs-joined-spline-and-the-spline-as-a-curve-2026-11-23)): SP1, SP4 and SP5 the same as
ProgeCAD. SP3 is the same curve, which ProgeCAD reads as open from our
DXF: to be written as ProgeCAD does, its first fit point repeated at the
end. SP2b's start tangent points the other way in ProgeCAD; ours kept.
SP2 and SP6 cannot be compared: ProgeCAD has no tangent options among the
points and makes every spline cubic.

## The spline as a curve (ADR 0043)

Delivery 1, on 2026-11-23: a spline lying in the plan is a `Geometry2`
curve.
- **Snaps:** it gives the Intersection snap with every curve, the
  Perpendicular snap and the Tangent snap. Its ends and nearest point snap
  as before.
- **Selection:** a crossing window or polygon meets the curve itself, no
  longer its chords.
- **As an edge:** TRIM cuts lines, arcs, circles, ellipses and polylines
  at it, and EXTEND grows them to it.
- **TRIM and BREAK** (delivery 2, 2026-11-23) cut the spline itself
  exactly, by knot insertion, as AutoCAD cuts a curve:
  - a closed spline cut twice becomes the open piece left;
  - an open one cut in the middle becomes two, the second a copy with
    every property;
  - BREAK takes away the span between its points, a closed spline's
    counterclockwise from the first.

  A piece keeps its control points and loses its fit points, which no
  longer describe it. Fences and Quick TRIM see a spline as they see any
  curve.
- **EXTEND** (2026-11-24, ProgeCAD's rule from case SC4): an open
  spline grows from the end picked straight along its tangent to the
  nearest edge that way; the straight piece becomes part of the spline,
  raised to its degree, its knots as long as the piece. Quick mode looks
  for edges along that ray. A closed spline cannot be extended.
- **STRETCH** (delivery 3a, 2026-11-23):
  - A spline through fit points moves its fit points inside the crossing
    area and is fitted again through them, its knots method and tangents
    kept.
  - One by its vertices moves its vertices inside; a closed one's
    repeated vertices follow their own, so it stays closed.
  - With none inside it stays; with all inside it moves whole.
- **Grips** (delivery 3b, 2026-11-23), as AutoCAD:
  - a spline through fit points shows a grip at each fit point; one
    stretched moves that point, and the spline is fitted again, its knots
    method and tangents kept;
  - one by its vertices shows its own vertices, those a closed one repeats
    left out; one stretched moves that vertex and its copies;
  - Move, Rotate, Scale and Mirror move the spline whole;
  - a point that would make no spline (onto its neighbor) is refused,
    nothing changed.
- **Properties:**
  - shown: its method (Fit or CV), degree, closing, knots (Fit), length
    and elevation;
  - for one spline, a fit point or control vertex chosen by its number,
    its X and Y editable.
- **JOIN** (delivery 4, 2026-11-23):
  - a source open spline takes the open splines touching its ends,
    end to end, each turned round when it runs the other way; where
    several touch an end, the first selected joins;
  - each is brought exactly to the highest degree among them, and their
    knots run on one after the other, so where two meet the curve is one
    control point;
  - many at once, touching splines chain into one spline from the first
    selected;
  - a chain that comes back to its start closes: clamped round to its
    start, written closed but not periodic;
  - the result keeps the source's id and properties, by its vertices,
    its fit points dropped, which no longer describe it *(to confirm)*;
  - a closed spline is no source and joins nothing;
  - since 2026-11-24, as AutoCAD: a source spline also takes the lines,
    arcs, elliptical arcs and open polylines touching its ends, each as
    an exact spline (an arc a rational quadratic); many at once, a chain
    with a spline in it is a spline, kept by that spline; a line, an arc
    or a polyline as source leaves a spline, discarded.
- **OFFSET** (delivery 5, 2026-11-23), of an ellipse or a spline:
  - a cubic spline by its vertices through points of the exact offset,
    within a millionth of the curve's size of it, its end tangents the
    curve's; closed and periodic for a whole ellipse or a closed spline;
  - with every property of the source, which stays (unless Erase);
    Through, Multiple, Undo, Erase and Layer as for any curve;
  - refused where the offset would curl back on itself: inward past the
    tightest bend (for an ellipse b²/a, at the ends of its major axis);
  - a spline with a corner, where its tangent breaks, is refused for now.

Cases SC1–SC4, the Bézier from (0,0) by (2,3) and (4,3) to (6,0):

| Case | Command | Ours |
|---|---|---|
| SC1 | TRIM (Quick) the line (−1,1)–(7,1) at (3,1) | two lines, to x = 3 − √5 and from x = 3 + √5 |
| SC2 | EXTEND the line (−5,1)–(−2,1) to the spline | to (3 − √5, 1) |
| SC3 | TRIM (Quick) the spline at its top, cut by x = 2 and x = 4 | two splines, (0,0)–(2,2) and (4,2)–(6,0), by their vertices; whether ProgeCAD keeps fit points to confirm |
| SC4 | EXTEND the spline at its end (6,0) to x = 8 | since 2026-11-24 as ProgeCAD: to (8,−3) along its end tangent, control points (6,0), (6⅔,−1), (7⅓,−2), (8,−3) added, knots 1 ×3 then 1 + √13 ×4 |
| SC5 | BREAK the spline, First (2,2), second (4,2) | the same two splines as SC3 |
| SC6 | STRETCH the Bézier, crossing window (3.5,2)–(4.5,3.5) round its vertex (4,3), by (1,1) | that vertex at (5,4), the others kept |
| SC7 | STRETCH a spline made through (0,0), (3,4), (7,1), (9,6), window (6,0)–(8,2), by (0,−2) | fitted again through (7,−1), its fit points kept |
| SC8 | JOIN from the Bézier the one from (12,0) by (10,−3) and (8,−3) to (6,0) | one cubic by (0,0), (2,3), (4,3), (6,0), (8,−3), (10,−3), (12,0), knots 0 ×4, 1 ×3, 2 ×4 |
| SC9 | JOIN from the Bézier the quadratic by (6,0), (8,−3), (10,0) | one cubic, the quadratic raised: (6,0), (7⅓,−2), (8⅔,−2), (10,0) after the Bézier's points, knots as SC8 |
| SC11 | OFFSET the ellipse about (0,0), semi-axes 5 and 3, by 1 toward (9,0) | a closed cubic of 79 control points through (6,0) and (0,4) |
| SC12 | OFFSET its upper half, parameters 0 to π, by 1 toward (0,1) | an open cubic of 37 control points from (4,0) to (−4,0) |
| SC14 | JOIN from the Bézier the line (6,0)–(8,0) and the half circle about (9,0), radius 1, below it | one cubic from (0,0) to (10,0), rational, of 13 control points |
| SC15 | JOIN many at once the lines (−2,0)–(0,0) and (6,0)–(8,0) and the Bézier | one cubic from (−2,0) to (8,0), of 10 control points, the Bézier's id kept |
| SC13 | OFFSET the Bézier by 0.5 toward (3,5) | an open cubic of 35 control points from (−0.416,0.277) to (6.416,0.277) |
| SC10 | JOIN many at once three cubic Béziers in a ring: (0,0) (1,−1) (3,−1) (4,0); (4,0) (5,1) (5,3) (4,4); (4,4) (2,6) (−1,2) (0,0) | one closed cubic of 10 control points, its last its first, knots 0 ×4, 1 ×3, 2 ×3, 3 ×4; DXF flag 70 = 9, closed and not periodic |

**Run on 2026-11-23** ([results](../testing/commands-02-14-windows.md#elliptical-arcs-joined-spline-and-the-spline-as-a-curve-2026-11-23)): SC1–SC3, SC5, SC7 and SC10
the same as ProgeCAD. SC4: ProgeCAD extends a spline by a straight segment
along its end tangent, the rule adopted on 2026-11-24. SC6, SC8 and SC9 kept as ours.
SC11–SC13: ProgeCAD's offsets are quadratics or cubics of fewer control
points, ours 10 to over 100 times closer.
