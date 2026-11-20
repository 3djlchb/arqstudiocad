//! Intersections with ellipses (ADR 0041): a submodule of `intersect`.

use std::f64::consts::{FRAC_PI_2, PI};

use super::*;
use crate::vector::Vector2;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry2 {
    Line::new(p(x0, y0), p(x1, y1)).unwrap().into()
}

/// About (0,0), semi-axes 5 along x and 3 along y, from `start` to `end`.
fn ellipse(start: f64, end: f64) -> Ellipse {
    Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, start, end).unwrap()
}

fn whole() -> Geometry2 {
    ellipse(0.0, 0.0).into()
}

/// Whether `a` and `b`, in either order, meet exactly at the points
/// `expected`, and each found lies on both curves.
fn assert_points(a: Geometry2, b: Geometry2, expected: &[Point2]) {
    for (x, y) in [(a, b), (b, a)] {
        let found = intersect(x, y, TOL);
        assert!(found.overlaps().is_empty(), "{:?}", found.overlaps());
        assert_eq!(found.points().len(), expected.len(), "{:?}", found.points());
        for e in expected {
            assert!(
                found.points().iter().any(|q| q.distance_to(*e) < 1e-9),
                "missing {e:?} in {:?}",
                found.points()
            );
        }
        for q in found.points() {
            assert!(
                x.distance_to(*q) < 1e-9 && y.distance_to(*q) < 1e-9,
                "{q:?}"
            );
        }
    }
}

#[test]
fn a_line_crosses_touches_or_misses_an_ellipse() {
    // y = 1.8: x² / 25 = 1 - 0.36, x = ±4.
    assert_points(
        line(-10.0, 1.8, 10.0, 1.8),
        whole(),
        &[p(-4.0, 1.8), p(4.0, 1.8)],
    );
    // Along the major axis, through the center.
    assert_points(
        line(-6.0, 0.0, 6.0, 0.0),
        whole(),
        &[p(-5.0, 0.0), p(5.0, 0.0)],
    );
    // Touching at the top, and missing just above it.
    assert_points(line(-10.0, 3.0, 10.0, 3.0), whole(), &[p(0.0, 3.0)]);
    assert_points(line(-10.0, 3.001, 10.0, 3.001), whole(), &[]);
    // A segment ending short of the curve, and one starting on it.
    assert_points(line(0.0, 0.0, 3.0, 1.0), whole(), &[]);
    assert_points(line(4.0, 1.8, 10.0, 1.8), whole(), &[p(4.0, 1.8)]);
    // A slanted line, its crossings checked by the equation.
    let found = intersect(line(-8.0, -6.0, 8.0, 7.0), whole(), TOL);
    assert_eq!(found.points().len(), 2);
    for q in found.points() {
        let (x, y) = (q.x(), q.y());
        assert!((x * x / 25.0 + y * y / 9.0 - 1.0).abs() < 1e-12, "{q:?}");
    }
}

#[test]
fn a_line_meets_an_elliptical_arc_only_on_its_sweep() {
    // The upper half: y = 1.8 meets it twice, y = -1.8 not at all.
    let upper: Geometry2 = ellipse(0.0, PI).into();
    assert_points(
        line(-10.0, 1.8, 10.0, 1.8),
        upper,
        &[p(-4.0, 1.8), p(4.0, 1.8)],
    );
    assert_points(line(-10.0, -1.8, 10.0, -1.8), upper, &[]);
    // Its ends count: the major axis meets it at (5,0) and (-5,0).
    assert_points(
        line(-6.0, 0.0, 6.0, 0.0),
        upper,
        &[p(-5.0, 0.0), p(5.0, 0.0)],
    );
    // From π/2 to π, only (-4,1.8).
    assert_points(
        line(-10.0, 1.8, 10.0, 1.8),
        ellipse(FRAC_PI_2, PI).into(),
        &[p(-4.0, 1.8)],
    );
}

#[test]
fn a_turned_ellipse_is_met_where_its_own_frame_says() {
    // Turned a quarter about (10,10): semi-axes 5 along y and 3 along x.
    let turned: Geometry2 = Ellipse::new(p(10.0, 10.0), v(0.0, 5.0), 0.6, 0.0, 0.0)
        .unwrap()
        .into();
    assert_points(
        line(11.8, 0.0, 11.8, 20.0),
        turned,
        &[p(11.8, 6.0), p(11.8, 14.0)],
    );
}

#[test]
fn a_circle_meets_an_ellipse_up_to_four_times() {
    // Radius 4 about the center: with x²/25 + y²/9 = 1, x² = 25·7/16 and
    // y² = 81/16.
    let (x, y) = ((25.0f64 * 7.0 / 16.0).sqrt(), (81.0f64 / 16.0).sqrt());
    let circle: Geometry2 = Circle::new(p(0.0, 0.0), 4.0).unwrap().into();
    assert_points(circle, whole(), &[p(x, y), p(-x, y), p(x, -y), p(-x, -y)]);
    // An arc of it, the first quadrant only.
    let arc: Geometry2 = Arc::new(p(0.0, 0.0), 4.0, 0.0, FRAC_PI_2).unwrap().into();
    assert_points(arc, whole(), &[p(x, y)]);
    // Touching inside at the minor axis's ends, and apart.
    let inner: Geometry2 = Circle::new(p(0.0, 0.0), 3.0).unwrap().into();
    assert_points(inner, whole(), &[p(0.0, 3.0), p(0.0, -3.0)]);
    let apart: Geometry2 = Circle::new(p(20.0, 0.0), 2.0).unwrap().into();
    assert_points(apart, whole(), &[]);
    // Touching outside at (5,0).
    let beside: Geometry2 = Circle::new(p(7.0, 0.0), 2.0).unwrap().into();
    assert_points(beside, whole(), &[p(5.0, 0.0)]);
    // Touching outside at parameter 0.3, and a hair further out, within
    // the tolerance, without crossing.
    let at = ellipse(0.0, 0.0).point_at_param(0.3);
    let normal = Vector2::new(0.3f64.cos() / 5.0, 0.3f64.sin() / 3.0)
        .unwrap()
        .normalized()
        .unwrap();
    for gap in [0.0, 1e-11] {
        let touching: Geometry2 = Circle::new(at + normal * (2.0 + gap), 2.0).unwrap().into();
        assert_points(touching, whole(), &[at]);
    }
}

#[test]
fn two_ellipses_cross_four_times_or_touch() {
    // The same ellipse turned a quarter: by symmetry x² = y², and
    // x²·(1/25 + 1/9) = 1.
    let turned: Geometry2 = Ellipse::new(p(0.0, 0.0), v(0.0, 5.0), 0.6, 0.0, 0.0)
        .unwrap()
        .into();
    let c = (1.0 / (1.0 / 25.0 + 1.0 / 9.0f64)).sqrt();
    assert_points(turned, whole(), &[p(c, c), p(-c, c), p(c, -c), p(-c, -c)]);
    // The upper half of one: only the crossings above.
    assert_points(turned, ellipse(0.0, PI).into(), &[p(c, c), p(-c, c)]);
    // Side by side, touching at (5,0).
    let beside: Geometry2 = Ellipse::new(p(10.0, 0.0), v(5.0, 0.0), 0.6, 0.0, 0.0)
        .unwrap()
        .into();
    assert_points(beside, whole(), &[p(5.0, 0.0)]);
    // A slanted, narrow one through it: every point found lies on both.
    let slanted: Geometry2 = Ellipse::new(p(2.0, 1.0), v(4.0, 3.0), 0.3, 0.0, 0.0)
        .unwrap()
        .into();
    let found = intersect(slanted, whole(), TOL);
    assert_eq!(found.points().len(), 2, "{:?}", found.points());
    for q in found.points() {
        assert!(slanted.distance_to(*q) < 1e-9 && whole().distance_to(*q) < 1e-9);
    }
}

#[test]
fn arcs_of_one_ellipse_share_their_common_part() {
    // From 0 to π and from π/2 to 3π/2 share π/2 to π.
    let found = intersect(
        ellipse(0.0, PI).into(),
        ellipse(FRAC_PI_2, 1.5 * PI).into(),
        TOL,
    );
    assert!(found.points().is_empty(), "{:?}", found.points());
    let [Geometry2::Ellipse(piece)] = found.overlaps() else {
        panic!("{:?}", found.overlaps())
    };
    assert!((piece.start_param() - FRAC_PI_2).abs() < 1e-9);
    assert!((piece.sweep() - FRAC_PI_2).abs() < 1e-9);
    // End to end they touch: 0 to π/2 and π/2 to π.
    let found = intersect(
        ellipse(0.0, FRAC_PI_2).into(),
        ellipse(FRAC_PI_2, PI).into(),
        TOL,
    );
    assert!(found.overlaps().is_empty());
    assert_eq!(found.points().len(), 1);
    assert!(found.points()[0].distance_to(p(0.0, 3.0)) < 1e-9);
    // One starting before the other: 0 to π and 3π/2 round to π/2 share
    // 0 to π/2.
    let found = intersect(
        ellipse(0.0, PI).into(),
        ellipse(1.5 * PI, FRAC_PI_2).into(),
        TOL,
    );
    let [Geometry2::Ellipse(piece)] = found.overlaps() else {
        panic!("{:?}", found.overlaps())
    };
    assert!(piece.start_param().abs() < 1e-9 && (piece.sweep() - FRAC_PI_2).abs() < 1e-9);
    // The same ellipse by the other end of its major axis: all of it.
    let same = Ellipse::new(p(0.0, 0.0), v(-5.0, 0.0), 0.6, 0.0, 0.0).unwrap();
    assert_eq!(intersect(same.into(), whole(), TOL).overlaps().len(), 1);
    // A circle is an ellipse of ratio one.
    let round = Ellipse::new(p(0.0, 0.0), v(2.0, 0.0), 1.0, 0.0, 0.0).unwrap();
    let circle: Geometry2 = Circle::new(p(0.0, 0.0), 2.0).unwrap().into();
    assert_eq!(intersect(circle, round.into(), TOL).overlaps().len(), 1);
}
