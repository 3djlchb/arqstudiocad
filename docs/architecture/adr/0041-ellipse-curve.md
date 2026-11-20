# 0041. The ellipse as a curve: `Geometry2::Ellipse`, in four deliveries

Date: 2026-11-20 · Status: Accepted (maintainer, 2026-11-20) · Carries out
ADR 0040 D5

## Context

ADR 0040 made the ellipse an exact shape in the drawing, but not a
`Geometry2`. The rest of the core works on `Geometry2`: intersections and
their snaps, crossing selection, TRIM, EXTEND, BREAK, OFFSET, JOIN, grips
and the Properties panel. So an ellipse:

- gave no Intersection, Perpendicular or Tangent snap;
- was crossed by its chords, not its curve;
- since ADR 0040, no longer cut a TRIM (its chords had);
- could not be trimmed, extended, broken, joined or edited by grips.

The maintainer chose the following on 2026-11-20:

- four deliveries;
- OFFSET waits for SPLINE;
- JOIN takes only arcs of the same ellipse;
- intersections are computed exactly where closed forms are robust, and
  numerically elsewhere, with no new library.

## Decision

### D1 — `Geometry2::Ellipse`

An ellipse or elliptical arc (ADR 0040 D1) is a curve, of kind
`ELLIPSE`:

- **Closed** when it is whole, an arc otherwise.
- **Its points:** `point_at` counts along its parameter, so the middle of
  an arc is the point at its middle parameter, as its Midpoint snap since
  ADR 0040.
- **Its length:** the integral of its speed, by adaptive Gauss–Legendre
  quadrature in panels of an eighth of a turn, to about 1e-12 of the
  length.
- **The point at a distance:** found by Newton's method on that length,
  kept within a bracket.
- **Moved as a whole:** a `Transform2` carries it as ADR 0040 D4 does.

### D2 — Intersections

- **A line and an ellipse:** in the ellipse's own frame, the ellipse is the
  unit circle and the line is still a line, so the meeting points are a
  quadratic's roots. A line passing just outside touches the ellipse when
  the ellipse's point nearest to it is within the tolerance, judged in the
  world.
- **A circle, an arc or an ellipse with an ellipse:** a circle counts as an
  ellipse of ratio one. The first curve's points are put into the second's
  implicit equation, which gives a trigonometric polynomial of degree two
  in the first's parameter: at most four zeros and four turns.
  `roots::periodic_zeros` finds them:
  - it samples the polynomial 256 times a turn and bisects every crossing
    to the parameter's precision;
  - it finds the turning points where the analytic derivative crosses
    zero;
  - a turning point past zero between two samples holds the two crossings
    they missed;
  - two crossings nearer than a sample, with a turning point between them,
    are a pair.

  The caller judges each turning point and each pair by distance in the
  world, so a tangency is one point, whether or not rounding split it.
- **Arcs of one ellipse:** two arcs on the same ellipse share the common
  part of their sweeps, as two arcs of one circle do.

The precision near tangencies is that of the circles (`intersect`'s
module note).

### D3 — Snaps and selection

The ellipse gives the following snaps:

- **Intersection** with every curve.
- **Perpendicular** from a point: up to four feet, the zeros of
  `(P(t) − from) · P′(t)`. From the center of a circle-like ellipse every
  point is a foot, so there is none.
- **Tangent:** tangency survives the ellipse's own frame, so the tangent
  points are the unit circle's, brought back.

Its center, ends, middle, quadrants and nearest point (ADR 0040 D2) come
from the same `key_points` as every other curve. A crossing window or
polygon meets the curve itself, as AutoCAD does, not the chords drawn
(ADR 0040 D3), which lie within a thousandth or so of the curve. An
ellipse is again an edge that TRIM cuts at, now by its curve. Fences do
not see it yet (D4).

### D4 — Deliveries, each in a pull request

1. This one: D1–D3.
2. TRIM, EXTEND and BREAK of ellipses, as AutoCAD:
   - a whole ellipse cut twice becomes the arc left;
   - an arc cut in the middle becomes two arcs;
   - an arc grows round its ellipse;
   - the ellipse also bounds EXTEND and is seen by fences.

   They reach the ellipse through a curve of its own, kept apart from the
   entity's plan geometry: `Document::curve_plan` gives a line's, circle's
   or arc's plan geometry, or an ellipse lying in the plan, and
   `Document::set_curve_plan` writes it back, in the ellipse's own sense
   seen from below (maintainer, 2026-11-20). `Entity::plan`, the own model
   (ADR 0018), the check after saving, grips, STRETCH and Properties are
   left as they were until delivery 3, which opens `Entity::plan` to
   ellipses and folds the two together.
3. Grips and the Properties panel: center, axes, ratio, and the start and
   end angles. To be compared with ProgeCAD.
4. JOIN: a source elliptical arc takes the arcs of its own ellipse,
   making an arc or the whole ellipse; Close completes it. Joining with
   lines or arcs, which AutoCAD turns into a SPLINE, waits for SPLINE.

OFFSET of an ellipse is a SPLINE in AutoCAD, so it is refused with nothing
changed until SPLINE exists. FILLET and CHAMFER with ellipses are not
planned here.

## Consequences

- Until delivery 2, TRIM, EXTEND, BREAK and OFFSET refused an ellipse;
  since it, only OFFSET does (`OutOfRange { what: "curve" }`).
- The entity's plan geometry (`Entity::plan`) gives no ellipse yet, so
  its grips and STRETCH are unchanged; delivery 3 opens it (D4).
- `Geometry2` has one more variant. Every match on it takes the ellipse:
  as itself where the meaning is clear, and refused where its delivery is
  still to come.
