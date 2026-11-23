# 0043. The spline as a curve: `Geometry2::Spline`, in five deliveries

Date: 2026-11-23 · Status: Accepted (maintainer, 2026-11-23) · Carries out ADR 0042 D5

## Context

ADR 0042 made the spline an exact shape in the drawing, made by SPLINE,
but not a `Geometry2`. The rest of the core works on `Geometry2`:
- intersections and their snaps;
- crossing selection;
- TRIM, EXTEND, BREAK, OFFSET and JOIN;
- grips and the Properties panel.

So a spline:
- gives no Intersection, Perpendicular or Tangent snap;
- is crossed by its chords, not its curve;
- does not cut a TRIM;
- cannot be trimmed, broken, joined, offset or edited by grips.

The ellipse waits on it too: its OFFSET is a spline in AutoCAD (ADR 0041).

The maintainer chose the following on 2026-11-23:
- `Geometry2` holds the spline itself, and stops being `Copy`;
- EXTEND refuses a spline until the Windows comparison shows AutoCAD's
  rule;
- JOIN joins splines to splines only;
- OFFSET approximates the exact offset to a tolerance relative to the
  curve's size.

## Decision

### D1 — `Geometry2::Spline`

A spline lying in the plan (ADR 0042 D1) is a curve, of kind `SPLINE`.
It is held in an `Arc`, as `Shape::Spline` is.

- **Copying:** `Geometry2` and the plan geometry that holds it are `Clone`
  and no longer `Copy`. Cloning one clones a reference count. Code that
  copied curves clones or borrows them; the compiler finds every place.
- **Closed:** when DWG says so (ADR 0042 D1); open otherwise.
- **Its points:** `point_at` counts along its own parameter, over its
  domain.
- **Its length:** the integral of its speed, by Gauss–Legendre quadrature
  over each span, its panels halved until two estimates agree to about
  1e-12 of the length.
- **The point at a distance:** found by Newton's method on that length,
  kept within a bracket, as the ellipse's.
- **Moved as a whole:** a `Transform2` carries it as ADR 0042 D4 does.

### D2 — Intersections

Splines have no closed form: every meeting point is found numerically,
with no new library.

- **Candidates by the chords:** the spline's drawn chords (sixteen a span)
  meet the other curve, or the other spline's chords. Each meeting is a
  candidate.
- **Refined on the curves:** each candidate is refined by Newton's method
  on the curves themselves, kept within its chord's parameters:
  - against a line, on the signed distance to the line;
  - against a circle or an arc, on the distance to its center less its
    radius;
  - against an ellipse, on its implicit equation;
  - between two splines, in both parameters at once.
- **Tangencies:** where the curves come within the tolerance without
  crossing, the chords may miss them. So the closest approaches between
  neighbouring chords are refined too, and kept as one touching point when
  they lie within the tolerance, judged in the world, as the ellipse's are
  (ADR 0041 D2).
- **One spline with itself:** a spline crossing itself is not needed by
  any command here and is not looked for.

### D3 — Snaps and selection

- **Intersection** with every curve.
- **Perpendicular** from a point: the zeros of `(P(u) − from) · P′(u)`,
  bracketed by the chords and refined by Newton.
- **Tangent** from a point: the zeros of `(P(u) − from) × P′(u)`,
  likewise.
- **The rest:** its ends and nearest point (ADR 0042 D2), from the same
  `key_points` as every other curve.
- **Crossings:** a crossing window, polygon or fence meets the curve
  itself, no longer its chords.
- **As an edge:** a spline is an edge that TRIM cuts at and EXTEND reaches.

### D4 — Editing

- **TRIM and BREAK:** the pieces left are exact, cut by inserting knots.
  - A closed spline cut twice becomes the open piece left.
  - An open one cut in the middle becomes two, the second a copy with
    every property.
  - A trimmed or broken spline keeps its control points and loses its fit
    points, which no longer describe it. Whether AutoCAD keeps them, the
    Windows comparison will tell.
- **EXTEND** refuses a spline ("Cannot extend that object."), nothing
  changed. A case for Windows shows what AutoCAD does first (maintainer,
  2026-11-23).
- **Plan geometry:** `Entity::plan` gives a spline lying in the plan, so
  the own model owns it (ADR 0018 stage 1 widened again) and a save checks
  it by its data, as the ellipse's (ADR 0041 D4).
- **STRETCH:** moves the control points and fit points inside the crossing
  area, refitting through the fit points when it has them.
- **Grips,** as AutoCAD:
  - a spline through fit points shows its fit points;
  - one by vertices shows its control vertices;
  - a fit point moved refits the spline through it;
  - a vertex moved moves it alone.
- **Properties:** its degree, closed, the knots method, and the
  coordinates of a fit point or vertex chosen by its number, editable as
  the grips are; its length, read only.
- **JOIN** (maintainer, 2026-11-23): a source spline takes the splines
  touching its ends, end to end, turned round as needed, making one
  spline. It closes when it comes back to its start. Each spline is
  brought to the highest degree among them and their knots run on one
  after the other. Lines, arcs, ellipses and polylines are not joined to a
  spline yet.
- **OFFSET** of an ellipse or a spline (maintainer, 2026-11-23):
  - **What it makes:** a cubic spline through points of the exact offset,
    its tangents at the ends those of the exact offset.
  - **Its precision:** points are added where it strays from the exact
    offset by more than a millionth of the curve's size, the diagonal of
    its box. So it is as precise in millimetres as in metres, and is
    usually a few dozen points.
  - **Refused**, nothing changed, where the offset curls back on itself:
    inward past the smallest radius of curvature.

### D5 — Deliveries, each in a pull request

1. D1–D3: the curve, its length, intersections, snaps and crossings, TRIM
   cutting at splines. `Geometry2` and `PlanGeometry` stop being `Copy`.
2. TRIM and BREAK of splines, and EXTEND's refusal. Done (2026-11-23):
   they reach a spline through a curve of its own,
   `Document::spline_curve` and `set_spline_curve`, as the ellipse's
   second delivery did, until the third folds it into `Entity::plan`; a
   piece across where a closed spline closes is its two pieces joined; a
   closed curve's span goes round by its own period, a spline's no longer
   `TAU`.
3. Plan geometry, STRETCH, grips and Properties, in two pull requests, as
   the ellipse's was:
   - **3a, plan geometry and STRETCH** (2026-11-23):
     - `Entity::plan` gives a spline lying in the plan, so the own model
       owns it and a save checks it by its data, as an ellipse;
       `spline_curve` folds into it.
     - Written as plan geometry, a spline is by its vertices. Moved,
       turned, scaled or mirrored whole, it keeps its fit points and
       tangents, as before.
     - Its entity box stays its control points'. What it is drawn by and
       its exact box are computed when first asked for, so reading,
       checking and comparing splines costs little.
     - STRETCH as D4 says.
   - **3b, grips and Properties.**
4. JOIN of splines.
5. OFFSET of ellipses and splines.

Each delivery adds its cases to the Windows comparison tools.

## Consequences

- `Geometry2` and `PlanGeometry` are no longer `Copy`. Code that copied
  them clones or borrows them; this costs nothing measurable for the other
  curves, which is to be measured in delivery 1.
- Every match on `Geometry2` takes the spline: as itself where the meaning
  is clear, and refused where its delivery is still to come.
- An imported spline gains the snaps, crossings and edits of a curve, as
  each delivery arrives.
- Until delivery 2, TRIM and BREAK refuse a spline; EXTEND refuses one
  after it too, until the Windows comparison.
