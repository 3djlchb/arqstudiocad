//! Property tests: laws that must hold for any finite input, checked on
//! random values up to survey-coordinate magnitudes (~1e7).

use std::f64::consts::{PI, TAU};

use arq_geometry::{Arc, Bounds2, Circle, Line, Point2, Tolerance, Vector2, angle};
use proptest::prelude::*;

/// Coordinates from the origin up to UTM-like magnitudes.
const RANGE: std::ops::RangeInclusive<f64> = -1e7..=1e7;

fn coord() -> impl Strategy<Value = f64> {
    RANGE
}

fn point() -> impl Strategy<Value = Point2> {
    (coord(), coord()).prop_map(|(x, y)| Point2::new(x, y).unwrap())
}

fn vector() -> impl Strategy<Value = Vector2> {
    (coord(), coord()).prop_map(|(x, y)| Vector2::new(x, y).unwrap())
}

fn bounds() -> impl Strategy<Value = Bounds2> {
    (point(), point()).prop_map(|(a, b)| Bounds2::new(a, b))
}

const TOL: Tolerance = Tolerance::DEFAULT;

fn radius() -> impl Strategy<Value = f64> {
    1e-3..1e6f64
}

fn line() -> impl Strategy<Value = Line> {
    (point(), vector())
        .prop_filter("non-zero length", |(_, v)| v.length() > 1e-3)
        .prop_map(|(p, v)| Line::new(p, p + v).unwrap())
}

fn circle() -> impl Strategy<Value = Circle> {
    (point(), radius()).prop_map(|(c, r)| Circle::new(c, r).unwrap())
}

fn arc() -> impl Strategy<Value = Arc> {
    (point(), radius(), any_angle(), 1e-3..(TAU - 1e-3))
        .prop_map(|(c, r, start, sweep)| Arc::new(c, r, start, start + sweep).unwrap())
}

/// A tolerance scaled to the magnitude of the coordinates involved:
/// `f64` resolves about 1e-16 of a value, so allow a few thousand ulps.
fn tol_for(magnitude: f64) -> Tolerance {
    Tolerance::new(1e-12 * (1.0 + magnitude.abs()), 1e-9).unwrap()
}

fn magnitude(p: Point2) -> f64 {
    p.to_vector().length()
}

fn any_angle() -> impl Strategy<Value = f64> {
    -100.0 * TAU..=100.0 * TAU
}

/// Rounding error grows with magnitude: allow a few ulps of the largest value.
fn close(a: f64, b: f64, magnitude: f64) -> bool {
    (a - b).abs() <= 1e-13 * (1.0 + magnitude.abs())
}

fn close_vec(a: Vector2, b: Vector2, magnitude: f64) -> bool {
    close(a.x(), b.x(), magnitude) && close(a.y(), b.y(), magnitude)
}

/// Angles compared on the circle, so 0 and 2π - ε count as close.
fn close_angle(a: f64, b: f64) -> bool {
    angle::difference(a, b).abs() <= 1e-9
}

proptest! {
    #[test]
    fn point_plus_displacement_roundtrips(p in point(), v in vector()) {
        let m = p.to_vector().length() + v.length();
        prop_assert!(close_vec((p + v) - p, v, m));
        prop_assert!(close_vec(((p + v) - v).to_vector(), p.to_vector(), m));
    }

    #[test]
    fn point_difference_is_antisymmetric(a in point(), b in point()) {
        prop_assert_eq!(a - b, -(b - a));
    }

    #[test]
    fn rotation_undoes_and_keeps_length(v in vector(), a in any_angle()) {
        let r = v.rotated(a);
        prop_assert!(close(r.length(), v.length(), v.length()));
        prop_assert!(close_vec(r.rotated(-a), v, v.length()));
    }

    #[test]
    fn perp_is_orthogonal_and_ccw(v in vector()) {
        prop_assert_eq!(v.dot(v.perp()), 0.0);
        prop_assert!(v.cross(v.perp()) >= 0.0);
    }

    #[test]
    fn cross_is_antisymmetric(a in vector(), b in vector()) {
        prop_assert_eq!(a.cross(b), -b.cross(a));
    }

    #[test]
    fn normalized_has_unit_length(v in vector()) {
        prop_assume!(v.length() > 1e-9);
        prop_assert!((v.normalized().unwrap().length() - 1.0).abs() <= 1e-15 * 4.0);
    }

    #[test]
    fn polar_roundtrips(len in 1e-3..1e7f64, a in any_angle()) {
        let v = Vector2::from_polar(len, a).unwrap();
        prop_assert!(close(v.length(), len, len));
        prop_assert!(close_angle(v.angle(), angle::normalize(a)));
    }

    #[test]
    fn normalize_lands_in_range(a in any_angle()) {
        let n = angle::normalize(a);
        prop_assert!((0.0..TAU).contains(&n));
        prop_assert!(close_angle(n, a));
    }

    #[test]
    fn difference_is_short_and_exact(from in any_angle(), to in any_angle()) {
        let d = angle::difference(from, to);
        prop_assert!(d > -PI && d <= PI);
        prop_assert!(close_angle(from + d, to));
    }

    #[test]
    fn ccw_sweep_reaches_the_end(start in any_angle(), end in any_angle()) {
        let s = angle::ccw_sweep(start, end);
        prop_assert!((0.0..TAU).contains(&s));
        prop_assert!(close_angle(start + s, end));
    }

    #[test]
    fn distance_is_a_metric(a in point(), b in point(), c in point()) {
        let m = 3e7;
        prop_assert_eq!(a.distance_to(b), b.distance_to(a));
        prop_assert!(a.distance_to(c) <= a.distance_to(b) + b.distance_to(c) + 1e-13 * m);
    }

    #[test]
    fn bounds_contain_every_point(points in prop::collection::vec(point(), 1..50)) {
        let b = Bounds2::from_points(points.iter().copied()).unwrap();
        for p in &points {
            prop_assert!(b.contains_point(*p, TOL));
        }
        prop_assert!(b.width() >= 0.0 && b.height() >= 0.0);
        prop_assert!(b.contains_point(b.center(), TOL));
    }

    #[test]
    fn bounds_corner_order_does_not_matter(a in point(), b in point()) {
        prop_assert_eq!(Bounds2::new(a, b), Bounds2::new(b, a));
    }

    #[test]
    fn union_is_commutative_idempotent_and_covering(a in bounds(), b in bounds()) {
        let u = a.union(b);
        prop_assert_eq!(u, b.union(a));
        prop_assert_eq!(a.union(a), a);
        prop_assert!(u.contains(a, TOL) && u.contains(b, TOL));
    }

    #[test]
    fn box_intersection_is_symmetric_and_implied_by_containment(a in bounds(), b in bounds()) {
        prop_assert_eq!(a.intersects(b, TOL), b.intersects(a, TOL));
        if a.contains(b, TOL) {
            prop_assert!(a.intersects(b, TOL));
        }
        prop_assert!(a.intersects(a.union(b), TOL));
    }

    #[test]
    fn expanding_keeps_the_original(b in bounds(), margin in 0.0..1e6f64) {
        let e = b.expanded(margin).unwrap();
        prop_assert!(e.contains(b, TOL));
        prop_assert_eq!(b.expanded(0.0).unwrap(), b);
    }

    #[test]
    fn line_points_lie_on_the_line_and_in_its_bounds(l in line(), t in 0.0..=1.0f64) {
        let tol = tol_for(magnitude(l.start()) + l.length());
        let q = l.point_at(t);
        prop_assert!(l.contains_point(q, tol));
        prop_assert!(l.bounds().contains_point(q, tol));
        prop_assert_eq!(l.point_at(0.0), l.start());
        prop_assert_eq!(l.point_at(1.0), l.end());
    }

    #[test]
    fn line_closest_point_is_on_it_and_no_farther_than_the_ends(l in line(), q in point()) {
        let tol = tol_for(magnitude(l.start()) + l.length() + magnitude(q));
        let c = l.closest_point(q);
        prop_assert!(l.contains_point(c, tol));
        let d = l.distance_to(q);
        prop_assert!(d <= q.distance_to(l.start()) + tol.linear());
        prop_assert!(d <= q.distance_to(l.end()) + tol.linear());
    }

    #[test]
    fn circle_points_are_at_radius_and_in_bounds(c in circle(), a in any_angle()) {
        let tol = tol_for(magnitude(c.center()) + c.radius());
        let q = c.point_at_angle(a);
        prop_assert!(c.contains_point(q, tol));
        prop_assert!(c.bounds().contains_point(q, tol));
    }

    #[test]
    fn circle_closest_point_and_distance(c in circle(), q in point()) {
        prop_assume!(q.distance_to(c.center()) > 1e-6);
        let tol = tol_for(magnitude(c.center()) + c.radius() + magnitude(q));
        prop_assert!(c.contains_point(c.closest_point(q), tol));
        let expected = (q.distance_to(c.center()) - c.radius()).abs();
        prop_assert!((c.distance_to(q) - expected).abs() <= tol.linear());
    }

    #[test]
    fn arc_points_lie_on_it_and_in_its_bounds(a in arc(), t in 0.0..=1.0f64) {
        let tol = tol_for(magnitude(a.center()) + a.radius());
        let q = a.point_at(t);
        prop_assert!(a.contains_point(q, tol));
        prop_assert!(a.circle().contains_point(q, tol));
        prop_assert!(a.bounds().contains_point(q, tol));
        prop_assert!(a.contains_angle(a.start_angle() + t * a.sweep(), TOL));
        prop_assert!((a.length() - a.radius() * a.sweep()).abs() <= tol.linear());
    }

    #[test]
    fn arc_closest_point_is_on_it_and_no_farther_than_the_ends(a in arc(), q in point()) {
        prop_assume!(q.distance_to(a.center()) > 1e-6);
        let tol = tol_for(magnitude(a.center()) + a.radius() + magnitude(q));
        let c = a.closest_point(q);
        prop_assert!(a.contains_point(c, tol));
        prop_assert!(a.distance_to(q) <= q.distance_to(a.start_point()) + tol.linear());
        prop_assert!(a.distance_to(q) <= q.distance_to(a.end_point()) + tol.linear());
    }

    #[test]
    fn arc_from_three_points_passes_through_them(
        center in point(), r in 1.0..1e5f64, start in any_angle(),
        s1 in 0.1..1.5f64, s2 in 0.1..1.5f64, clockwise in any::<bool>(),
    ) {
        let c = Circle::new(center, r).unwrap();
        let sign = if clockwise { -1.0 } else { 1.0 };
        let (pa, pb, pc) = (
            c.point_at_angle(start),
            c.point_at_angle(start + sign * s1),
            c.point_at_angle(start + sign * (s1 + s2)),
        );
        let arc = Arc::from_three_points(pa, pb, pc, TOL).unwrap();
        // The center is computed in a local frame at the first point, so the
        // normal scaled tolerance holds even at survey coordinates.
        let tol = tol_for(magnitude(center) + r);
        for q in [pa, pb, pc] {
            prop_assert!(arc.contains_point(q, tol));
        }
        prop_assert!((arc.radius() - r).abs() <= tol.linear());
    }

    #[test]
    fn lerp_hits_endpoints_exactly(a in point(), b in point()) {
        prop_assert_eq!(a.lerp(b, 0.0), a);
        prop_assert_eq!(a.lerp(b, 1.0), b);
        let mid = a.midpoint(b);
        let m = a.to_vector().length() + b.to_vector().length();
        prop_assert!(close(mid.distance_to(a), mid.distance_to(b), m));
    }
}
