//! Intersections between curves: the base of TRIM, EXTEND, the INTERSECTION
//! snap and exact crossing selection.
//!
//! Every pair is solved in a local frame (relative to a line's start or a
//! circle's center, ADR 0010); tangency, parallelism and coincidence are
//! decided with the explicit [`Tolerance`], never by exact equality.
//!
//! Precision limit: near-tangent crossings are ill-conditioned by nature.
//! Coordinates are only known to about ε (≈ 1e-9 at survey magnitudes ~1e6),
//! and an ε shift sideways moves a near-tangent meeting point by up to
//! √(r·ε) along the curves (≈ 4e-4 for r = 80). The points returned always
//! lie on both curves within tolerance; *where* along them is uncertain to
//! that extent. Clearly transversal crossings are accurate to about ε.

use std::f64::consts::TAU;

use crate::angle;
use crate::arc::Arc;
use crate::circle::Circle;
use crate::ellipse::Ellipse;
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::point::Point2;
use crate::roots::{TrigPoly, Zero};
use crate::tolerance::Tolerance;

/// Where two curves meet: isolated points (crossings and tangencies) and
/// shared pieces (two overlapping segments, arcs of the same circle, a
/// circle with itself). Both can occur together, e.g. two arcs of one
/// circle that share a piece and touch at another end.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Intersections {
    points: Vec<Point2>,
    overlaps: Vec<Geometry2>,
}

impl Intersections {
    /// Isolated meeting points (at most two for lines, circles and arcs).
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    /// Shared pieces of curve.
    pub fn overlaps(&self) -> &[Geometry2] {
        &self.overlaps
    }

    /// Whether the curves do not meet at all.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.overlaps.is_empty()
    }

    fn push_point(&mut self, p: Point2, tol: Tolerance) {
        if !self.points.iter().any(|q| q.approx_eq(p, tol)) {
            self.points.push(p);
        }
    }
}

/// Where `a` and `b` meet.
pub fn intersect(a: Geometry2, b: Geometry2, tol: Tolerance) -> Intersections {
    let mut out = Intersections::default();
    match (a, b) {
        (Geometry2::Line(l1), Geometry2::Line(l2)) => line_line(l1, l2, tol, &mut out),
        (Geometry2::Line(l), Geometry2::Ellipse(e))
        | (Geometry2::Ellipse(e), Geometry2::Line(l)) => line_ellipse(l, e, tol, &mut out),
        (Geometry2::Ellipse(e), other) | (other, Geometry2::Ellipse(e)) => {
            ellipse_ellipse(as_ellipse(other), e, tol, &mut out)
        }
        (Geometry2::Line(l), round) | (round, Geometry2::Line(l)) => {
            line_round(l, Round::of(round), tol, &mut out)
        }
        (r1, r2) => round_round(Round::of(r1), Round::of(r2), tol, &mut out),
    }
    out
}

/// A circle, optionally restricted to the sweep of an arc.
#[derive(Clone, Copy)]
struct Round {
    circle: Circle,
    arc: Option<Arc>,
}

impl Round {
    fn of(g: Geometry2) -> Self {
        match g {
            Geometry2::Circle(c) => Self {
                circle: c,
                arc: None,
            },
            Geometry2::Arc(a) => Self {
                circle: a.circle(),
                arc: Some(a),
            },
            Geometry2::Line(_) | Geometry2::Ellipse(_) => {
                unreachable!("lines and ellipses are handled separately")
            }
        }
    }

    /// Whether a point of the circle belongs to the arc's sweep.
    fn admits(self, p: Point2, tol: Tolerance) -> bool {
        self.arc.is_none_or(|a| {
            a.contains_angle((p - a.center()).angle(), angular_tolerance(a.radius(), tol))
        })
    }
}

/// Angular slack equivalent to the linear tolerance on a circle of radius `r`.
fn angular_tolerance(r: f64, tol: Tolerance) -> Tolerance {
    Tolerance::new(tol.linear(), tol.angular().max(tol.linear() / r)).expect("positive")
}

fn line_line(a: Line, b: Line, tol: Tolerance, out: &mut Intersections) {
    // Local frame at a's start.
    let (r, s) = (a.vector(), b.vector());
    let (rl, sl) = (r.length(), s.length());
    let qp = b.start() - a.start();
    let denom = r.cross(s);
    let (slack_a, slack_b) = (tol.linear() / rl, tol.linear() / sl);

    if denom.abs() <= tol.angular() * rl * sl {
        // Parallel: they meet only if collinear.
        if (qp.cross(r) / rl).abs() > tol.linear() {
            return;
        }
        let rr = r.length_squared();
        let (t0, t1) = (qp.dot(r) / rr, (qp + s).dot(r) / rr);
        let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
        if hi - lo > slack_a {
            let piece = Line::new(a.point_at(lo), a.point_at(hi)).expect("distinct ends");
            out.overlaps.push(piece.into());
        } else if hi - lo >= -slack_a {
            out.push_point(a.point_at(lo.clamp(0.0, 1.0)), tol);
        }
        return;
    }

    let t = qp.cross(s) / denom;
    let u = qp.cross(r) / denom;
    if (-slack_a..=1.0 + slack_a).contains(&t) && (-slack_b..=1.0 + slack_b).contains(&u) {
        out.push_point(a.point_at(t.clamp(0.0, 1.0)), tol);
    }
}

fn line_round(l: Line, round: Round, tol: Tolerance, out: &mut Intersections) {
    // Local frame at the circle's center.
    let c = round.circle.center();
    let radius = round.circle.radius();
    let r = l.vector();
    let rl = r.length();
    let f = l.start() - c;
    let t0 = -f.dot(r) / r.length_squared();
    let h = (f + r * t0).length(); // distance from the center to the line
    if h > radius + tol.linear() {
        return;
    }
    // Half chord, written to avoid cancellation when h is close to radius.
    let half = ((radius - h).max(0.0) * (radius + h)).sqrt();
    let params: &[f64] = if half <= tol.linear() {
        &[t0]
    } else {
        &[t0 - half / rl, t0 + half / rl]
    };
    let slack = tol.linear() / rl;
    for &t in params {
        if (-slack..=1.0 + slack).contains(&t) {
            let p = l.start() + r * t;
            if round.admits(p, tol) {
                out.push_point(p, tol);
            }
        }
    }
}

fn round_round(a: Round, b: Round, tol: Tolerance, out: &mut Intersections) {
    let (c1, r1) = (a.circle.center(), a.circle.radius());
    let (c2, r2) = (b.circle.center(), b.circle.radius());
    // Local frame at the first center.
    let d = c2 - c1;
    let dist = d.length();

    if dist <= tol.linear() {
        if (r1 - r2).abs() <= tol.linear() {
            same_circle(a, b, tol, out);
        }
        return; // concentric, different radii: never meet
    }
    if dist > r1 + r2 + tol.linear() || dist < (r1 - r2).abs() - tol.linear() {
        return; // apart, or one inside the other
    }
    let along = (dist * dist + r1 * r1 - r2 * r2) / (2.0 * dist);
    let h = (r1 * r1 - along * along).max(0.0).sqrt();
    let u = d / dist;
    let base = c1 + u * along;
    let candidates: &[_] = if h <= tol.linear() {
        &[base]
    } else {
        &[base + u.perp() * h, base - u.perp() * h]
    };
    for &p in candidates {
        if a.admits(p, tol) && b.admits(p, tol) {
            out.push_point(p, tol);
        }
    }
}

/// Two rounds on the same circle: the shared part of their sweeps.
fn same_circle(a: Round, b: Round, tol: Tolerance, out: &mut Intersections) {
    match (a.arc, b.arc) {
        (None, None) => out.overlaps.push(a.circle.into()),
        (Some(arc), None) | (None, Some(arc)) => out.overlaps.push(arc.into()),
        (Some(x), Some(y)) => arc_arc_overlap(x, y, tol, out),
    }
}

fn arc_arc_overlap(x: Arc, y: Arc, tol: Tolerance, out: &mut Intersections) {
    let slack = angular_tolerance(x.radius(), tol).angular();
    let (sx, wx) = (x.start_angle(), x.sweep());
    // y's sweep relative to x's start, also shifted one turn back so a piece
    // wrapping past x's start is found too.
    let offset = angle::ccw_sweep(sx, y.start_angle());
    for shift in [0.0, -TAU] {
        let (lo, hi) = (
            (offset + shift).max(0.0),
            (offset + shift + y.sweep()).min(wx),
        );
        if hi - lo > slack {
            let piece = Arc::new(x.center(), x.radius(), sx + lo, sx + hi).expect("non-zero sweep");
            out.overlaps.push(piece.into());
        } else if hi - lo >= -slack {
            out.push_point(x.circle().point_at_angle(sx + lo.clamp(0.0, wx)), tol);
        }
    }
}

/// A circle or an arc as the ellipse it is, of ratio one; an ellipse as
/// itself.
fn as_ellipse(g: Geometry2) -> Ellipse {
    let x = |r: f64| crate::vector::Vector2::new(r, 0.0).expect("a radius is finite");
    match g {
        Geometry2::Ellipse(e) => Ok(e),
        Geometry2::Circle(c) => Ellipse::new(c.center(), x(c.radius()), 1.0, 0.0, 0.0),
        Geometry2::Arc(a) => Ellipse::new(
            a.center(),
            x(a.radius()),
            1.0,
            a.start_angle(),
            a.end_angle(),
        ),
        Geometry2::Line(_) => unreachable!("lines are handled separately"),
    }
    .expect("a circle's ellipse is valid")
}

/// A line and an ellipse: in the ellipse's own frame, where it is the unit
/// circle and the line still a line, a quadratic. A line passing just
/// outside it touches it when the ellipse's point nearest the line is
/// within the tolerance of it, judged in the world.
fn line_ellipse(l: Line, e: Ellipse, tol: Tolerance, out: &mut Intersections) {
    let (x0, y0) = e.local(l.start());
    let (x1, y1) = e.local(l.end());
    let (dx, dy) = (x1 - x0, y1 - y0);
    let dd = dx * dx + dy * dy;
    // The line's point nearest the center, in the local frame.
    let t0 = -(x0 * dx + y0 * dy) / dd;
    let (fx, fy) = (x0 + dx * t0, y0 + dy * t0);
    let h = fx.hypot(fy);
    let slack = tol.linear() / l.length();
    let mut on_line = |t: f64, p: Point2| {
        if (-slack..=1.0 + slack).contains(&t) && e.admits(p, tol) {
            out.push_point(p, tol);
        }
    };
    if h >= 1.0 {
        // Apart, or touching: the ellipse's point toward the line.
        let touch = e.at_local(fx / h, fy / h);
        let infinite_distance = (touch - l.start()).cross(l.vector()).abs() / l.length();
        if infinite_distance <= tol.linear() {
            let t = (touch - l.start()).dot(l.vector()) / l.vector().length_squared();
            on_line(t, touch);
        }
        return;
    }
    // Half chord, written to avoid cancellation when h is close to one.
    let half = ((1.0 - h) * (1.0 + h)).sqrt() / dd.sqrt();
    for t in [t0 - half, t0 + half] {
        on_line(t, l.start() + l.vector() * t);
    }
}

/// Two ellipses, a circle being one of ratio one: where the first's
/// points fall on the second's implicit equation, a trigonometric
/// polynomial of degree two in the first's parameter, its zeros found by
/// bracketing (ADR 0041). The same ellipse twice shares the arcs' common
/// part.
fn ellipse_ellipse(a: Ellipse, b: Ellipse, tol: Tolerance, out: &mut Intersections) {
    let whole_b =
        Ellipse::new(b.center(), b.major_axis(), b.ratio(), 0.0, 0.0).expect("b's own axes");
    // A point whose implicit value is beyond this is farther than the
    // tolerance: in b's frame, a distance d moves its radius by d / minor
    // at most, and its implicit value by (d / minor)·(2 + d / minor).
    let reach = tol.linear() / b.minor_axis().length();
    let near_b = |p: Point2| {
        let (x, y) = b.local(p);
        (x * x + y * y - 1.0).abs() <= reach * (2.0 + reach) * (1.0 + 1e-9)
    };
    let on_whole_b = |p: Point2| near_b(p) && whole_b.distance_to(p) <= tol.linear();
    let alike = |u: f64, w: f64| (u - w).abs() <= tol.linear();
    if a.center().distance_to(b.center()) <= tol.linear()
        && alike(a.major_axis().length(), b.major_axis().length())
        && alike(a.minor_axis().length(), b.minor_axis().length())
        && (0..8)
            .map(|i| a.point_at_param(TAU * f64::from(i) / 8.0 + 0.1))
            .all(on_whole_b)
    {
        same_ellipse(a, b, tol, out);
        return;
    }
    // a's points in b's frame, L(t) = l0 + lu cos t + lw sin t, put into
    // |L|² - 1: a trigonometric polynomial of degree two.
    let l0 = b.local(a.center());
    let lu = b.local_vector(a.major_axis());
    let lw = b.local_vector(a.minor_axis());
    let dot = |p: (f64, f64), q: (f64, f64)| p.0 * q.0 + p.1 * q.1;
    let implicit = TrigPoly {
        cos2: (dot(lu, lu) - dot(lw, lw)) / 2.0,
        sin2: dot(lu, lw),
        cos: 2.0 * dot(l0, lu),
        sin: 2.0 * dot(l0, lw),
        constant: dot(l0, l0) + (dot(lu, lu) + dot(lw, lw)) / 2.0 - 1.0,
    };
    let mut found = Vec::new();
    for zero in crate::roots::periodic_zeros(implicit) {
        match zero {
            Zero::Crossing(t) => found.push(t),
            Zero::Touch(t) if on_whole_b(a.point_at_param(t)) => found.push(t),
            Zero::Touch(_) => {}
            // A tangency the rounding split in two: one point.
            Zero::Pair { turn, .. } if on_whole_b(a.point_at_param(turn)) => found.push(turn),
            Zero::Pair { first, second, .. } => found.extend([first, second]),
        }
    }
    for t in found {
        let p = a.point_at_param(t);
        if a.admits(p, tol) && b.admits(p, tol) {
            out.push_point(p, tol);
        }
    }
}

/// Two arcs of one ellipse: the shared part of their sweeps, in the first's
/// parameter, as two arcs of one circle share theirs.
fn same_ellipse(a: Ellipse, b: Ellipse, tol: Tolerance, out: &mut Intersections) {
    match (a.is_full(), b.is_full()) {
        (true, true) => out.overlaps.push(a.into()),
        (true, false) => out.overlaps.push(b.into()),
        (false, true) => out.overlaps.push(a.into()),
        (false, false) => {
            let (b0, _) = b.ends().expect("an arc");
            let slack = tol.linear() / a.derivative_at(a.start_param()).length().max(tol.linear());
            let (sx, wx) = (a.start_param(), a.sweep());
            // b's sweep in a's parameter: its ends' parameters on a.
            let b_start = a.param_of(b0);
            let b_sweep = {
                let (_, b1) = b.ends().expect("an arc");
                match angle::ccw_sweep(b_start, a.param_of(b1)) {
                    0.0 => TAU,
                    w => w,
                }
            };
            let offset = angle::ccw_sweep(sx, b_start);
            for shift in [0.0, -TAU] {
                let (lo, hi) = (
                    (offset + shift).max(0.0),
                    (offset + shift + b_sweep).min(wx),
                );
                if hi - lo > slack {
                    let piece =
                        Ellipse::new(a.center(), a.major_axis(), a.ratio(), sx + lo, sx + hi)
                            .expect("a non-zero sweep");
                    out.overlaps.push(piece.into());
                } else if hi - lo >= -slack {
                    out.push_point(a.point_at_param(sx + lo.clamp(0.0, wx)), tol);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "intersect_ellipse_tests.rs"]
mod ellipse_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::GeometryKind;
    use std::f64::consts::FRAC_PI_2;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }
    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry2 {
        Line::new(p(x0, y0), p(x1, y1)).unwrap().into()
    }
    fn circle(x: f64, y: f64, r: f64) -> Geometry2 {
        Circle::new(p(x, y), r).unwrap().into()
    }
    fn arc(x: f64, y: f64, r: f64, start_deg: f64, end_deg: f64) -> Geometry2 {
        Arc::new(p(x, y), r, start_deg.to_radians(), end_deg.to_radians())
            .unwrap()
            .into()
    }

    /// Asserts the result has exactly `expected` points (in any order).
    fn assert_points(i: &Intersections, expected: &[Point2]) {
        assert_eq!(i.points().len(), expected.len(), "points: {:?}", i.points());
        for e in expected {
            assert!(
                i.points()
                    .iter()
                    .any(|q| q.approx_eq(*e, Tolerance::new(1e-9, 1e-9).unwrap())),
                "missing {e:?} in {:?}",
                i.points()
            );
        }
        assert!(
            i.overlaps().is_empty(),
            "unexpected overlaps {:?}",
            i.overlaps()
        );
    }

    #[test]
    fn lines_crossing_parallel_and_apart() {
        assert_points(
            &intersect(line(0.0, 0.0, 10.0, 10.0), line(0.0, 10.0, 10.0, 0.0), TOL),
            &[p(5.0, 5.0)],
        );
        assert!(
            intersect(line(0.0, 0.0, 1.0, 1.0), line(0.0, 10.0, 10.0, 0.0), TOL).is_empty(),
            "would cross beyond the segment"
        );
        assert!(
            intersect(line(0.0, 0.0, 10.0, 0.0), line(0.0, 1.0, 10.0, 1.0), TOL).is_empty(),
            "parallel"
        );
        assert_points(
            &intersect(line(0.0, 0.0, 10.0, 0.0), line(5.0, 0.0, 5.0, 8.0), TOL),
            &[p(5.0, 0.0)],
        );
    }

    #[test]
    fn collinear_lines_overlap_touch_or_miss() {
        let i = intersect(line(0.0, 0.0, 5.0, 0.0), line(2.0, 0.0, 9.0, 0.0), TOL);
        assert!(i.points().is_empty());
        let Geometry2::Line(piece) = i.overlaps()[0] else {
            panic!("expected a segment")
        };
        assert_eq!((piece.start(), piece.end()), (p(2.0, 0.0), p(5.0, 0.0)));
        assert_points(
            &intersect(line(0.0, 0.0, 5.0, 0.0), line(5.0, 0.0, 9.0, 0.0), TOL),
            &[p(5.0, 0.0)],
        );
        assert!(intersect(line(0.0, 0.0, 5.0, 0.0), line(6.0, 0.0, 9.0, 0.0), TOL).is_empty());
    }

    #[test]
    fn line_and_circle() {
        let c = circle(0.0, 0.0, 5.0);
        assert_points(
            &intersect(line(-10.0, 3.0, 10.0, 3.0), c, TOL),
            &[p(-4.0, 3.0), p(4.0, 3.0)],
        );
        assert_points(
            &intersect(line(-10.0, 5.0, 10.0, 5.0), c, TOL),
            &[p(0.0, 5.0)],
        );
        assert!(intersect(line(-10.0, 6.0, 10.0, 6.0), c, TOL).is_empty());
        assert_points(
            &intersect(line(0.0, 0.0, 10.0, 0.0), c, TOL),
            &[p(5.0, 0.0)],
        );
        assert!(
            intersect(line(-1.0, 0.0, 1.0, 0.0), c, TOL).is_empty(),
            "entirely inside"
        );
        assert_points(
            &intersect(c, line(-10.0, 3.0, 10.0, 3.0), TOL),
            &[p(-4.0, 3.0), p(4.0, 3.0)],
        );
    }

    #[test]
    fn line_and_arc_keep_points_within_the_sweep() {
        // Upper half arc: only the crossing with y = 3 survives, not y = -3.
        let upper = arc(0.0, 0.0, 5.0, 0.0, 180.0);
        assert_points(
            &intersect(line(-10.0, 3.0, 10.0, 3.0), upper, TOL),
            &[p(-4.0, 3.0), p(4.0, 3.0)],
        );
        assert!(intersect(line(-10.0, -3.0, 10.0, -3.0), upper, TOL).is_empty());
        assert_points(
            &intersect(line(3.0, -10.0, 3.0, 10.0), upper, TOL),
            &[p(3.0, 4.0)],
        );
    }

    #[test]
    fn circles_cross_touch_nest_and_coincide() {
        let c = circle(0.0, 0.0, 5.0);
        assert_points(
            &intersect(c, circle(8.0, 0.0, 5.0), TOL),
            &[p(4.0, 3.0), p(4.0, -3.0)],
        );
        assert_points(&intersect(c, circle(10.0, 0.0, 5.0), TOL), &[p(5.0, 0.0)]);
        assert_points(&intersect(c, circle(2.0, 0.0, 3.0), TOL), &[p(5.0, 0.0)]);
        assert!(
            intersect(c, circle(20.0, 0.0, 5.0), TOL).is_empty(),
            "apart"
        );
        assert!(
            intersect(c, circle(1.0, 0.0, 1.0), TOL).is_empty(),
            "nested"
        );
        assert!(
            intersect(c, circle(0.0, 0.0, 3.0), TOL).is_empty(),
            "concentric"
        );
        let same = intersect(c, circle(0.0, 0.0, 5.0), TOL);
        assert_eq!(same.overlaps(), &[c]);
    }

    #[test]
    fn arcs_of_the_same_circle() {
        // Shared piece 60°..90°.
        let i = intersect(
            arc(0.0, 0.0, 1.0, 0.0, 90.0),
            arc(0.0, 0.0, 1.0, 60.0, 180.0),
            TOL,
        );
        let Geometry2::Arc(piece) = i.overlaps()[0] else {
            panic!("expected an arc")
        };
        assert!((piece.start_angle() - 60f64.to_radians()).abs() < 1e-12);
        assert!((piece.end_angle() - FRAC_PI_2).abs() < 1e-12);
        // Two pieces: 0°..300° and 270°..30° share 270..300 and 0..30.
        let two = intersect(
            arc(0.0, 0.0, 1.0, 0.0, 300.0),
            arc(0.0, 0.0, 1.0, 270.0, 30.0),
            TOL,
        );
        assert_eq!(two.overlaps().len(), 2);
        // End to end: they only touch at 90°.
        assert_points(
            &intersect(
                arc(0.0, 0.0, 1.0, 0.0, 90.0),
                arc(0.0, 0.0, 1.0, 90.0, 180.0),
                TOL,
            ),
            &[p(0.0, 1.0)],
        );
        assert!(
            intersect(
                arc(0.0, 0.0, 1.0, 0.0, 45.0),
                arc(0.0, 0.0, 1.0, 90.0, 180.0),
                TOL
            )
            .is_empty()
        );
        // A piece plus a touching end: 0..270 and 180..360 share 180..270, touch at 0.
        let mixed = intersect(
            arc(0.0, 0.0, 1.0, 0.0, 270.0),
            arc(0.0, 0.0, 1.0, 180.0, 0.0),
            TOL,
        );
        assert_eq!(mixed.overlaps().len(), 1);
        assert_points(
            &Intersections {
                overlaps: vec![],
                ..mixed
            },
            &[p(1.0, 0.0)],
        );
    }

    #[test]
    fn circle_with_an_arc_on_it_overlaps_the_arc() {
        let a = arc(0.0, 0.0, 2.0, 30.0, 120.0);
        let i = intersect(circle(0.0, 0.0, 2.0), a, TOL);
        assert_eq!(i.overlaps(), &[a]);
        assert_eq!(i.overlaps()[0].kind(), GeometryKind::Arc);
    }

    #[test]
    fn arcs_on_different_circles() {
        let a = arc(0.0, 0.0, 5.0, 0.0, 90.0);
        let b = arc(8.0, 0.0, 5.0, 90.0, 270.0);
        assert_points(&intersect(a, b, TOL), &[p(4.0, 3.0)]);
        let half = arc(0.0, 0.0, 5.0, 180.0, 360.0);
        assert_points(&intersect(half, b, TOL), &[p(4.0, -3.0)]);
        assert!(intersect(arc(0.0, 0.0, 5.0, 90.0, 180.0), b, TOL).is_empty());
    }

    #[test]
    fn precise_at_survey_coordinates() {
        let (x, y) = (500_000.0, 9_800_000.0);
        let i = intersect(
            line(x, y, x + 10.0, y + 10.0),
            line(x, y + 10.0, x + 10.0, y),
            TOL,
        );
        assert_points(&i, &[p(x + 5.0, y + 5.0)]);
        let j = intersect(circle(x, y, 5.0), circle(x + 8.0, y, 5.0), TOL);
        assert_points(&j, &[p(x + 4.0, y + 3.0), p(x + 4.0, y - 3.0)]);
    }
}
