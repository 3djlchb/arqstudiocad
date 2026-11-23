# 0042. The spline: an exact shape in the drawing, made by SPLINE

Date: 2026-11-22 · Status: Accepted (maintainer, 2026-11-22)

## Context

Milestone 02.14's group 5 is ELLIPSE and SPLINE. The ellipse is done
(ADRs 0040 and 0041). Until now, a SPLINE read from a DWG or DXF was kept
by the codec and drawn as the segments the codec breaks it into: sixteen
in each span between distinct knots. So a spline:

- snapped to an end at every pair of its segments;
- could not be moved, turned, scaled or mirrored at all;
- had the codec's box;
- could not be made.

The maintainer chose the following on 2026-11-22:

- the ellipse's path: the command, an exact shape, snaps and whole
  transforms first, and the spline as an editable curve later;
- both of AutoCAD's methods, by fit points and by control vertices;
- for fitting, a standard global interpolation, writing the control points
  it computes as well as the fit points.

## Decision

### D1 — `arq_geometry::Spline`

A NURBS curve in the plan, as DWG keeps it:

- its degree, a nondecreasing knot vector, its control points and, when
  rational, their weights;
- DWG's closed flag.

It evaluates by de Boor's algorithm in homogeneous coordinates. It
provides its ends, its nearest point (refined by Newton's method), an
exact box computed once, and its image by a similarity, which carries the
control points and keeps the knots and weights.

It is not a `Geometry2` yet (D5). No library is added: the arithmetic is
small and our own (master plan §16).

### D2 — `Shape::Spline` in the drawing

A spline whose control points all lie at one height is drawn as one exact
shape. It is held in an `Arc`, so `Shape` and `DrawItem` stop being
`Copy`: cloning one is cloning a count, and the renderers take shapes by
reference.

Any other spline keeps the codec's segments:
- one out of the plan;
- one of fit points only;
- one our geometry refuses.

Splines inside blocks and in hatch boundaries come to it the same way.

Picks, windows and crossings see the spline:
- a pick, by its curve;
- a window or a polygon, by its chords, as the ellipse was in ADR 0040,
  until D5.

It snaps to the ends of an open spline and to its nearest point, no longer
to an end between two chords.

### D3 — Nothing drawn changes

The renderers, CPU and GPU, draw the shape chord by chord from
`Spline::flatten`. It is the codec's own sampling, sixteen segments a
span, back to the start when closed, replicated operation for operation.

Tests compare, bit for bit:
- the chords with the codec's segments, for a non-uniform, a closed, a
  rational and a downward spline;
- the CPU's image with its chords' as lines;
- the GPU's vertices, dots and ends likewise.

### D4 — Made and moved

- **Moved:** a spline in the plan is moved, turned, scaled and mirrored as
  a whole by our geometry. Its control and fit points and its tangents are
  carried, each at its height, and its knots and weights are kept.
- **Boxed** (maintainer, 2026-11-22):
  - its drawn shape's box is exact, so windows select it by its curve;
  - the entity's box, which the spatial index and Zoom Extents use, is its
    control points' box. It holds the curve and costs nothing.

  An exact box for every entity read made a drawing of 20,000 splines
  open 1.65 times and save 4 times slower, since it is computed again
  whenever the entity is read and checked.
- **Made:** SPLINE, its own pull request, makes one by AutoCAD's two
  methods:
  - **Fit:** through the points, with Knots (Chord, Square root,
    Uniform), optional start and end tangents, Close and Undo. It uses
    the global cubic interpolation of Piegl and Tiller's *The NURBS Book*
    with that parameterization, free ends when no tangent is given, and
    writes both the fit points and the control points.
  - **CV:** by control vertices, with Degree 1 to 10, Close and Undo.

  Tolerance and Object come later. How close the fit is to AutoCAD's, the
  Windows comparison will tell.

### D5 — Later, each in a pull request

The spline as a `Geometry2` curve, as the ellipse became (ADR 0041):

- intersections, and the Intersection, Perpendicular and Tangent snaps;
- TRIM, EXTEND and BREAK;
- grips and the Properties panel;
- JOIN.

With it comes the ellipse's OFFSET, which AutoCAD makes a spline.

## Consequences

- Imported splines gain correct snaps and transforms; until now they could
  not be transformed at all. Their drawn box is exact, their entity's box
  the control points'.
- `Shape` has one more variant, held by reference count. Code that copied
  shapes clones or borrows them instead.
- What AutoCAD and ProgeCAD make from fit points, and with Close, is to be
  confirmed in the Windows comparison.
