use super::*;
use crate::intersect::intersect;
use crate::snap::{self, SnapKind};
use crate::{Arc, Circle, GeometryError, Line, Transform2};

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// The exact circle about `c` of radius `r`, as a closed rational
/// quadratic spline of four quarters, from (c.x + r, c.y) round.
fn circle(c: Point2, r: f64) -> Geometry2 {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let at = |x: f64, y: f64| p(c.x() + x * r, c.y() + y * r);
    let points = vec![
        at(1.0, 0.0),
        at(1.0, 1.0),
        at(0.0, 1.0),
        at(-1.0, 1.0),
        at(-1.0, 0.0),
        at(-1.0, -1.0),
        at(0.0, -1.0),
        at(1.0, -1.0),
        at(1.0, 0.0),
    ];
    let weights = vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0];
    let knots = vec![
        0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
    ];
    Spline::new(2, points, weights, knots, true).unwrap().into()
}

fn line(a: (f64, f64), b: (f64, f64)) -> Geometry2 {
    Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap().into()
}

/// The points where `a` and `b` meet, either order, each on both curves,
/// matched against `expected` in any order, crossings to 1e-9.
fn assert_meet(a: &Geometry2, b: &Geometry2, expected: &[Point2]) {
    assert_meet_within(a, b, expected, 1e-9);
}

/// As [`assert_meet`], each found within `along` of where expected: a
/// tangency is known only to about √ε of the curves' size along them.
fn assert_meet_within(a: &Geometry2, b: &Geometry2, expected: &[Point2], along: f64) {
    for (x, y) in [(a, b), (b, a)] {
        let found = intersect(x, y, TOL);
        assert!(found.overlaps().is_empty());
        let points = found.points();
        assert_eq!(
            points.len(),
            expected.len(),
            "{points:?} against {expected:?}"
        );
        for e in expected {
            assert!(
                points.iter().any(|q| q.distance_to(*e) < along),
                "{e:?} not in {points:?}"
            );
        }
        for q in points {
            assert!(
                x.distance_to(*q) < 1e-9 && y.distance_to(*q) < 1e-9,
                "{q:?}"
            );
        }
    }
}

#[test]
fn a_line_crosses_a_spline_where_the_circle_it_draws_would() {
    let c = circle(p(0.0, 0.0), 5.0);
    assert_meet(
        &c,
        &line((-9.0, 3.0), (9.0, 3.0)),
        &[p(4.0, 3.0), p(-4.0, 3.0)],
    );
    // A segment reaching only one of them.
    assert_meet(&c, &line((0.0, 3.0), (9.0, 3.0)), &[p(4.0, 3.0)]);
    // Through the start of its closed run, on a sample.
    assert_meet(&c, &line((0.0, 0.0), (9.0, 0.0)), &[p(5.0, 0.0)]);
    assert_meet(&c, &line((6.0, -9.0), (6.0, 9.0)), &[]);
    // An open spline ending within the tolerance of a line, not on it,
    // meets it there.
    let end = p(6.0, 0.3 * TOL.linear());
    let short: Geometry2 =
        Spline::by_vertices(3, vec![p(0.0, 0.0), p(2.0, 3.0), p(4.0, 3.0), end], false)
            .unwrap()
            .into();
    assert_meet(&short, &line((5.0, 0.0), (9.0, 0.0)), &[end]);
}

#[test]
fn a_tangent_line_touches_once_and_a_near_one_crosses_twice() {
    let c = circle(p(1.0, 2.0), 5.0);
    assert_meet(&c, &line((-9.0, 7.0), (9.0, 7.0)), &[p(1.0, 7.0)]);
    // Within the tolerance of touching: one point still.
    let inside = 7.0 + 0.5 * TOL.linear();
    let found = intersect(&c, &line((-9.0, inside), (9.0, inside)), TOL);
    assert_eq!(found.points().len(), 1, "{:?}", found.points());
    // Clearly past it, none; just short of it, two very close crossings.
    assert_meet(&c, &line((-9.0, 7.001), (9.0, 7.001)), &[]);
    let dy: f64 = 1e-4;
    let x = (25.0 - (5.0 - dy).powi(2)).sqrt();
    assert_meet(
        &c,
        &line((-9.0, 7.0 - dy), (9.0, 7.0 - dy)),
        &[p(1.0 + x, 7.0 - dy), p(1.0 - x, 7.0 - dy)],
    );
}

#[test]
fn circles_arcs_and_ellipses_cross_a_spline() {
    let c = circle(p(0.0, 0.0), 5.0);
    // Two circles of radius 5, centers 5 apart.
    let h = (25.0f64 - 6.25).sqrt();
    let other: Geometry2 = Circle::new(p(5.0, 0.0), 5.0).unwrap().into();
    assert_meet(&c, &other, &[p(2.5, h), p(2.5, -h)]);
    // Its upper half only.
    let upper: Geometry2 = Arc::new(p(5.0, 0.0), 5.0, 0.0, std::f64::consts::PI)
        .unwrap()
        .into();
    assert_meet(&c, &upper, &[p(2.5, h)]);
    // An ellipse of semi-axes 6 and 4: four crossings.
    let e: Geometry2 = Ellipse::new(
        p(0.0, 0.0),
        Vector2::new(6.0, 0.0).unwrap(),
        4.0 / 6.0,
        0.0,
        0.0,
    )
    .unwrap()
    .into();
    let (x, y) = (16.2f64.sqrt(), 8.8f64.sqrt());
    assert_meet(&c, &e, &[p(x, y), p(-x, y), p(x, -y), p(-x, -y)]);
}

#[test]
fn two_splines_cross_and_touch() {
    let a = circle(p(0.0, 0.0), 5.0);
    assert_meet(&a, &circle(p(6.0, 0.0), 5.0), &[p(3.0, 4.0), p(3.0, -4.0)]);
    // Touching from outside, at (5,0), and from inside at (0,5).
    assert_meet_within(&a, &circle(p(10.0, 0.0), 5.0), &[p(5.0, 0.0)], 1e-7);
    assert_meet_within(&a, &circle(p(0.0, 3.0), 2.0), &[p(0.0, 5.0)], 1e-7);
    assert_meet(&a, &circle(p(20.0, 0.0), 5.0), &[]);
    // Touching between samples, at 0.3 radians round.
    let (c, s) = (0.3f64.cos(), 0.3f64.sin());
    assert_meet_within(
        &a,
        &circle(p(10.0 * c, 10.0 * s), 5.0),
        &[p(5.0 * c, 5.0 * s)],
        1e-7,
    );
    // Polylines of degree one, crossing once.
    let zig: Geometry2 = Spline::by_vertices(1, vec![p(0.0, 0.0), p(4.0, 4.0), p(8.0, 0.0)], false)
        .unwrap()
        .into();
    let flat: Geometry2 = Spline::by_vertices(1, vec![p(-1.0, 1.0), p(3.0, 1.0)], false)
        .unwrap()
        .into();
    assert_meet(&zig, &flat, &[p(1.0, 1.0)]);
}

#[test]
fn a_spline_shares_itself() {
    let a = circle(p(0.0, 0.0), 5.0);
    let found = intersect(&a, &a.clone(), TOL);
    assert_eq!(found.overlaps(), [a]);
}

#[test]
fn far_from_the_origin_the_points_still_lie_on_both_curves() {
    let shift = Transform2::translation(Vector2::new(5e5, -3e5).unwrap());
    let c = shift.apply(&circle(p(0.0, 0.0), 5.0)).unwrap();
    let l = shift.apply(&line((-9.0, 3.0), (9.0, 3.0))).unwrap();
    let found = intersect(&c, &l, TOL);
    assert_eq!(found.points().len(), 2);
    for q in found.points() {
        assert!(c.distance_to(*q) < 1e-7 && l.distance_to(*q) < 1e-7);
    }
}

#[test]
fn snaps_on_a_spline() {
    let c = circle(p(0.0, 0.0), 5.0);
    let near =
        |found: &[snap::SnapPoint], q: Point2| found.iter().any(|s| s.point.distance_to(q) < 1e-7);
    // Perpendicular from (10,0): the near and far points on its radius.
    let feet = snap::perpendicular_points(&c, p(10.0, 0.0), TOL);
    assert_eq!(feet.len(), 2, "{feet:?}");
    assert!(near(&feet, p(5.0, 0.0)) && near(&feet, p(-5.0, 0.0)));
    assert!(feet.iter().all(|s| s.kind == SnapKind::Perpendicular));
    // Tangent from (10,0): at ±60° about the center.
    let touch = snap::tangent_points(&c, p(10.0, 0.0), TOL);
    let y = 5.0 * 3f64.sqrt() / 2.0;
    assert_eq!(touch.len(), 2, "{touch:?}");
    assert!(near(&touch, p(2.5, y)) && near(&touch, p(2.5, -y)));
    assert!(
        snap::tangent_points(&c, p(1.0, 0.0), TOL).is_empty(),
        "none from inside"
    );
    // From a point on it, that point itself, once, as a circle gives.
    let on = snap::tangent_points(&c, p(3.0, 4.0), TOL);
    assert_eq!(on.len(), 1, "{on:?}");
    assert!(on[0].point.distance_to(p(3.0, 4.0)) < 1e-9, "{on:?}");
    let round: Geometry2 = Circle::new(p(0.0, 0.0), 5.0).unwrap().into();
    assert_eq!(snap::tangent_points(&round, p(3.0, 4.0), TOL).len(), 1);
    // Intersection with a line.
    let at = snap::intersection_points(&c, &line((-9.0, 3.0), (9.0, 3.0)), TOL);
    assert!(near(&at, p(4.0, 3.0)) && at.iter().all(|s| s.kind == SnapKind::Intersection));
    // A closed spline has no ends; an open one has two.
    assert!(snap::key_points(&c).is_empty());
    let open: Geometry2 = Spline::by_vertices(
        3,
        vec![p(0.0, 0.0), p(1.0, 2.0), p(3.0, 2.0), p(4.0, 0.0)],
        false,
    )
    .unwrap()
    .into();
    let ends = snap::key_points(&open);
    assert_eq!(ends.len(), 2);
    assert!(ends.iter().all(|s| s.kind == SnapKind::Endpoint));
    assert!(near(&ends, p(0.0, 0.0)) && near(&ends, p(4.0, 0.0)));
    // Its direction at a point: the circle's tangent.
    let d = snap::tangent_direction(&c, p(0.0, 5.0));
    assert!((d.x() + 1.0).abs() < 1e-9 && d.y().abs() < 1e-9, "{d:?}");
}

#[test]
fn a_spline_is_a_curve() {
    let c = circle(p(0.0, 0.0), 5.0);
    assert_eq!(c.kind().name(), "SPLINE");
    assert!(c.is_closed() && c.start_point().is_none() && c.end_point().is_none());
    assert!((c.length() - std::f64::consts::TAU * 5.0).abs() < 1e-10);
    assert!(c.point_at(0.25).distance_to(p(0.0, 5.0)) < 1e-12);
    assert!(c.contains_point(p(3.0, 4.0), TOL) && !c.contains_point(p(3.0, 4.1), TOL));
    assert!((c.distance_to(p(0.0, 7.0)) - 2.0).abs() < 1e-9);
    assert!(c.point_at_distance(c.length() * 2.0).is_err());
    // Its parameter in [0, 1] runs over its whole domain, here not [0, 1].
    let fit: Geometry2 = crate::SplineFit::new(
        vec![p(0.0, 0.0), p(3.0, 4.0), p(7.0, 1.0)],
        crate::SplineKnots::Chord,
        None,
        None,
        false,
    )
    .unwrap()
    .spline()
    .unwrap()
    .into();
    assert!(fit.point_at(1.0).distance_to(p(7.0, 1.0)) < 1e-12);
    assert!(fit.point_at(0.5).distance_to(p(0.0, 0.0)) > 1.0);
    let open: Geometry2 =
        Spline::by_vertices(2, vec![p(0.0, 0.0), p(2.0, 2.0), p(4.0, 0.0)], false)
            .unwrap()
            .into();
    assert_eq!(
        (open.start_point(), open.end_point()),
        (Some(p(0.0, 0.0)), Some(p(4.0, 0.0)))
    );
    let moved = Transform2::translation(Vector2::new(1.0, 1.0).unwrap())
        .apply(&open)
        .unwrap();
    assert_eq!(moved.start_point(), Some(p(1.0, 1.0)));
}

#[test]
fn extend_refuses_a_spline_and_reaches_one() {
    use crate::{extend, trim};
    let c = circle(p(0.0, 0.0), 5.0);
    let edge = line((-9.0, 3.0), (9.0, 3.0));
    let refused = GeometryError::OutOfRange { what: "curve" };
    assert_eq!(
        extend(&c, std::slice::from_ref(&edge), p(5.0, 0.0), TOL),
        Err(refused)
    );
    // A line along y = 3 from the left grows to the spline.
    let short = line((-10.0, 3.0), (-8.0, 3.0));
    let grown = extend(&short, std::slice::from_ref(&c), p(-8.0, 3.0), TOL)
        .unwrap()
        .unwrap();
    assert!(
        grown.end_point().unwrap().distance_to(p(-4.0, 3.0)) < 1e-9,
        "{grown:?}"
    );
    // An arc of radius 3 about (0,-3), grown round to the spline.
    let arc: Geometry2 = Arc::new(p(0.0, -3.0), 3.0, 0.0, 0.5).unwrap().into();
    let grown = extend(
        &arc,
        std::slice::from_ref(&c),
        arc.end_point().unwrap(),
        TOL,
    )
    .unwrap();
    assert!(grown.is_some_and(|g| c.distance_to(g.end_point().unwrap()) < 1e-9));
    // A line can be trimmed at a spline.
    let left = trim(
        &edge,
        &crate::cutting_points(&edge, std::slice::from_ref(&c), TOL),
        p(0.0, 3.0),
        TOL,
    )
    .unwrap()
    .unwrap();
    assert_eq!(left.len(), 2);
}
