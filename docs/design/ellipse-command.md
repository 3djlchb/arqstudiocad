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
and TRIM cuts at it.

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

## Windows comparison (to run)

Cases EL1–EL4 are in both comparison tools, as 02.14's others
([method](../testing/commands-02-14-windows.md)); the LISP dump describes
ellipses (center, major axis, ratio, parameters).

| Case | Command | Ours |
|---|---|---|
| EL1 | axis (0,0)–(10,0), distance 3 | center (5,0), major (5,0), ratio 0.6, whole |
| EL2 | Center (5,0), axis end (7,0), distance 4 | major (0,4), ratio 0.5 |
| EL3 | axis (0,0)–(10,0), Rotation 60° | ratio 0.5 |
| EL4 | Arc: axis (0,0)–(10,0), distance 3, start 30°, end 120° | from the first axis toward (0,0): to confirm |

To confirm: that angles count from the first axis drawn when it is not
horizontal (EL4 draws one whose first end is (0,0), the axis pointing
west), and what Parameter gives.
