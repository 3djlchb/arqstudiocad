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

**To run.**

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
- **Refused for now:** the spline itself is not yet trimmed, broken or
  extended (deliveries 2 and later).

Cases SC1–SC4, the Bézier from (0,0) by (2,3) and (4,3) to (6,0):

| Case | Command | Ours |
|---|---|---|
| SC1 | TRIM (Quick) the line (−1,1)–(7,1) at (3,1) | two lines, to x = 3 − √5 and from x = 3 + √5 |
| SC2 | EXTEND the line (−5,1)–(−2,1) to the spline | to (3 − √5, 1) |
| SC3 | TRIM (Quick) the spline at its top, cut by x = 2 and x = 4 | refused, unchanged: what ProgeCAD makes decides delivery 2 |
| SC4 | EXTEND the spline at its end (6,0) to x = 8 | refused, unchanged: what ProgeCAD makes decides EXTEND's rule |

**To run.**
