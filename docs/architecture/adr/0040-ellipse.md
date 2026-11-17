# 0040. The ellipse: an exact shape in the drawing, made by ELLIPSE

Date: 2026-11-17 · Status: Accepted (maintainer, 2026-11-17)

## Context

Milestone 02.14's group 5 is ELLIPSE and SPLINE, "new curves, new
entities in the own model". Until now, an ELLIPSE read from a DWG or DXF
was kept by the codec and drawn as segments between points on it
(`ellipse_points`, sixteen a radian). Our geometry (`Geometry2`) knew
lines, circles and arcs only, so an ellipse:

- snapped to an end at every pair of its segments, and to their middles;
- could not be moved, turned, scaled or mirrored as a whole;
- had the codec's box, its whole ellipse's even for an arc;
- could not be made.

Putting it into `Geometry2` reaches everything that walks those curves:
intersections, TRIM, EXTEND, BREAK, OFFSET, JOIN, grips and the
Properties panel. The maintainer chose to deliver in steps (2026-11-17,
option A), this one first, and the drawing's shape with it (option A
again).

## Decision

### D1 — `arq_geometry::Ellipse`

An ellipse or elliptical arc in the plan, as DWG keeps it: center, major
semi-axis as a vector, minor to major ratio in (0, 1], start parameter and
sweep in (0, 2π], counterclockwise. It carries ELLIPSE's constructions:
by an axis and the other semi-axis (the longer the major, as AutoCAD), by a
rotation (up to 89.4°, as AutoCAD), and an arc by angles or parameters
measured from the first axis drawn. Its points, ends, quadrants, exact
bounds, nearest point and its image by a similarity (a mirror keeping it
counterclockwise) are its own. It is not a `Geometry2` yet (D5).

### D2 — `Shape::Ellipse` in the drawing

An ellipse whose normal lies along ±Z is drawn as one exact shape, in
world plan coordinates; seen from below, its parameters change sign. A
tilted one keeps its segments. Picks, windows and crossings see the
ellipse, and it snaps to its center, its arc's ends and middle, its
quadrants and its nearest point; no longer to an end between two chords.

### D3 — Nothing drawn changes

The renderers, CPU and GPU, draw the shape chord by chord from
`Ellipse::flatten`: sixteen points a radian, eight at least, 128 at most,
the count and the points the segments always had. Tests compare the
chords with the old segments', the CPU's image of an ellipse with its
chords' as lines (equal pixel for pixel), and the GPU's vertices, dots
and ends likewise. The cost per ellipse is that of its chords, as before.

### D4 — Made and moved

`Document::add_ellipse` writes an ELLIPSE in the World plane on the
current layer; ELLIPSE (alias `EL`, Spanish `ELIPSE`) makes it, its shape
following the cursor. An ellipse in the plan is moved, turned, scaled and
mirrored as a whole by our geometry, not the codec's transform, so its
sense survives a mirror from above or below. Its box is computed exactly.

### D5 — Later, each in a pull request

The ellipse as a `Geometry2` curve: intersections with other curves (and
the Intersection, Perpendicular and Tangent snaps), TRIM, EXTEND, BREAK,
OFFSET, JOIN, grips and the Properties panel. SPLINE follows the same
path.

## Consequences

- Imported ellipses gain correct snaps, transforms and boxes.
- `Shape` has one more variant; every renderer draws it from the same
  points (D3).
- A prompt can measure an angle shown by a point from a direction of its
  own (`Prompt.angle_base`), as a typed angle is: ELLIPSE's arc angles
  count from the first axis drawn, typed or shown.
- What AutoCAD and ProgeCAD do with arc angles measured from a first axis
  that is not horizontal, and with parameters, is to be confirmed in the
  Windows comparison.
