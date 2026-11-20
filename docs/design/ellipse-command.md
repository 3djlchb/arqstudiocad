# ELLIPSE command

Milestone 02.14, group 5, with AutoCAD 2021 as the behavioral baseline
([ADR 0040](../architecture/adr/0040-ellipse.md)). Scope chosen by the
maintainer (2026-11-17, option A): the command, the exact shape, snaps and
whole transforms in this delivery; the ellipse as an editable curve later.

## Behavior

`ELLIPSE`, alias `EL`, Spanish command `ELIPSE`, in the Draw menu, the
Ribbon's Draw group and the Draw toolbar. Top view only.

1. `Specify axis endpoint of ellipse [Arc(A)/Center(C)]:` a point, the
   first end of an axis.
   - `Specify other endpoint of axis:` its other end; the center is
     between them.
   - **Center:** `Specify center of ellipse:` then `Specify endpoint of
     axis:`.
2. `Specify distance to other axis [Rotation(R)]:` the other semi-axis,
   typed or as the distance from the center to a point. The longer axis
   becomes the major one.
   - **Rotation:** `Specify rotation around major axis:` the ellipse seen
     as a circle turned about the first axis; the other semi-axis is the
     cosine of the angle times the first. Up to 89.4°, as AutoCAD.
3. **Arc** (chosen at the first prompt, which then asks for the elliptical
   arc's axis endpoint):
   - `Specify start angle [Parameter(P)]:` then `Specify end angle
     [Parameter(P)/Included angle(I)]:`.
   - Angles count counterclockwise from the first axis drawn (from the
     center toward its first end), typed or shown by a point.
   - **Parameter** takes the ellipse's own parameters instead of angles
     about the center; **Included angle** gives the end as the start plus
     the angle typed.

While points are asked, the ellipse follows the cursor: the whole one
once the axis is known, then the arc. One `ELLIPSE` undo step;
cancelling, or points and distances that make no ellipse ("Those points
and distances define no ellipse."), leave the drawing unchanged.

The ellipse snaps to its center, its arc's ends and middle, its quadrants
and its nearest point, and is moved, copied, turned, scaled and mirrored
as a whole (ADR 0040). Since ADR 0041 it is a curve: it gives the
Intersection, Perpendicular and Tangent snaps, a crossing meets its curve,
and TRIM cuts at it. Since its second delivery it is trimmed, extended and
broken as AutoCAD does:

- **TRIM:** a whole ellipse cut twice becomes the arc left; an arc cut in
  the middle becomes two, the second a copy with every property.
- **EXTEND:** an arc grows round its ellipse, never round onto itself; a
  whole ellipse cannot ("Cannot extend that object.").
- **BREAK:** a whole ellipse loses the counterclockwise span from the
  first point to the second, as a circle; an arc, the span between them.
- **As an edge:** it bounds lines, arcs and polylines extended to it, and
  a fence meets it.

Each is one undo step, and an ellipse seen from below keeps its normal.

STRETCH moves a whole ellipse when its center is inside the crossing
area, as a circle, and an elliptical arc when both its ends are, as an
arc; an arc with one end inside is refused, nothing changed.

**Grips,** as AutoCAD (maintainer, 2026-11-20):

- the center moves it;
- an axis end sets that semi-axis to its distance from the center, its
  direction and the other semi-axis kept; the longer becomes the major,
  an arc keeping its place;
- an elliptical arc's ends slide along the ellipse toward the cursor;
- an arc shows only the axis ends on it, and none at its own ends.

**Properties,** as AutoCAD:

- editable: Center X and Y, Major radius, Minor radius (past the major,
  they swap), Radius ratio (up to 1), and an arc's Start and End angle, in
  degrees about the center from the major axis;
- shown: the major axis's X and Y, an arc's start and end points, its
  length and its area (an arc's, the sector from the center).

To confirm in ProgeCAD: the angles (the LISP dump now writes its
`StartAngle` and `EndAngle`), and the grips, by hand (see the Windows
comparison).

## Reference and validation

Behavioral reference: AutoCAD 2021 Help, "ELLIPSE (Command)". The
implementation is original; no Autodesk source or artwork is copied.

Tests:
- **Geometry:** constructions and the swap of axes, the rotation limits,
  arcs by angles and parameters from a first axis that is the minor one,
  exact bounds against dense sampling, the nearest point, the flattening,
  turns and mirrors point by point.
- **Drawing:** the exact shape's chords equal the old segments' (normals
  ±Z, whole, half and wrapping arcs); snaps and picks on an arc, no snap
  between chords; turns and mirrors of an ellipse seen from below.
- **Renderers:** the CPU's image and the GPU's vertices equal its chords'
  as lines.
- **Core:** one undo step; move, turn, scale and mirror carry every point.
- **Command and UI:** each prompt path, angles shown by points, cancelling,
  the preview, the shared action in Spanish.

Each was verified by sabotage.

## Windows comparison

Cases EL1–EL9 are in both comparison tools, as 02.14's others
([method](../testing/commands-02-14-windows.md)); the LISP dump describes
ellipses (center, major axis, ratio, parameters).

| Case | Command | Ours |
|---|---|---|
| EL1 | axis (0,0)–(10,0), distance 3 | center (5,0), major (−5,0) (toward the first point), ratio 0.6, whole |
| EL2 | Center (5,0), axis end (7,0), distance 4 | major (0,4), ratio 0.5 |
| EL3 | axis (0,0)–(10,0), Rotation 60° | ratio 0.5 |
| EL4 | Arc: axis (0,0)–(10,0), distance 3, start 30°, end 120° | from the first axis toward (0,0): to confirm |

Cases EL5–EL9 are about (0,0), with semi-axes 5 and 3:

| Case | Command | Ours |
|---|---|---|
| EL5 | TRIM (Quick) a whole ellipse at (5,0), cut by x = 4 | the arc from (4,1.8) round to (4,−1.8) |
| EL6 | TRIM (Quick) the upper half at its top-left, cut by x = 0 and x = −4 | two arcs: (5,0)–(0,3) and (−4,1.8)–(−5,0) |
| EL7 | EXTEND the first quarter at (0,3) to x = −4 | the arc from (5,0) to (−4,1.8) |
| EL8 | BREAK a whole ellipse, First (5,0), second (0,3) | the arc from (0,3) round to (5,0) |
| EL9 | EXTEND a line along y = 1.8 from x = −10 to the ellipse | to (−4,1.8) |
| EL10 | STRETCH a whole ellipse, crossing window from (6,1) to (−1,−1), holding its center, by (2,3) | moved to center (2,3) |
| EL11 | STRETCH the first quarter, window round both ends, by (1,0) | moved, ends (6,0) and (1,3) |
| EL12 | STRETCH the first quarter, window round (5,0) only, by (1,0) | refused, unchanged: to confirm |

To confirm: that angles count from the first axis drawn when it is not
horizontal (EL4 draws one whose first end is (0,0), the axis pointing
west), and what Parameter gives.

**Run on 2026-11-20** ([results](../testing/commands-02-14-windows.md#join-on-polylines-matchprop-and-ellipse-2026-11-20)): EL1–EL11 the same as ProgeCAD,
the major axis toward the first point and the arc's angles counted from
it; a whole ellipse moves when the crossing window holds its center and
stays when it does not. EL12 differs: ProgeCAD reshapes the arc into
another ellipse, which ours refuses, kept by the maintainer's choice.
Parameter was not among the cases.
