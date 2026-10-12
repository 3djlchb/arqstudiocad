//! Property tests: laws that must hold for any finite input, checked on
//! random values up to survey-coordinate magnitudes (~1e7).

use std::f64::consts::{PI, TAU};

use arq_geometry::{Bounds2, Point2, Tolerance, Vector2, angle};
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
    fn lerp_hits_endpoints_exactly(a in point(), b in point()) {
        prop_assert_eq!(a.lerp(b, 0.0), a);
        prop_assert_eq!(a.lerp(b, 1.0), b);
        let mid = a.midpoint(b);
        let m = a.to_vector().length() + b.to_vector().length();
        prop_assert!(close(mid.distance_to(a), mid.distance_to(b), m));
    }
}
