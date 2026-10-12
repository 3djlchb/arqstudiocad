//! Property tests: laws that must hold for any finite input, checked on
//! random values up to survey-coordinate magnitudes (~1e7).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use arq_geometry::snap::{self, SnapKind};
use arq_geometry::{
    Arc, Bounds2, Circle, Geometry2, Intersections, Line, Point2, Tolerance, Transform2, Vector2,
    angle, intersect,
};
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

/// A local scene: offsets within ±100 around a base point that may sit at
/// survey coordinates, so random curves actually meet.
fn near(base: Point2) -> impl Strategy<Value = Point2> {
    (-100.0..100.0f64, -100.0..100.0f64)
        .prop_map(move |(dx, dy)| base + Vector2::new(dx, dy).unwrap())
}

fn near_geometry(base: Point2) -> impl Strategy<Value = Geometry2> {
    prop_oneof![
        (near(base), near(base))
            .prop_filter("non-zero length", |(a, b)| a.distance_to(*b) > 1e-3)
            .prop_map(|(a, b)| Line::new(a, b).unwrap().into()),
        (near(base), 1.0..100.0f64).prop_map(|(c, r)| Circle::new(c, r).unwrap().into()),
        (near(base), 1.0..100.0f64, any_angle(), 0.05..(TAU - 0.05))
            .prop_map(|(c, r, s, w)| Arc::new(c, r, s, s + w).unwrap().into()),
    ]
}

/// Two curves near the same base point.
fn scene() -> impl Strategy<Value = (Point2, Geometry2, Geometry2)> {
    point().prop_flat_map(|base| (Just(base), near_geometry(base), near_geometry(base)))
}

fn same_points(a: &Intersections, b: &Intersections, tol: Tolerance) -> bool {
    a.points().len() == b.points().len()
        && a.points()
            .iter()
            .all(|p| b.points().iter().any(|q| p.approx_eq(*q, tol)))
}

/// One elementary transformation, as the editing commands create them.
fn step() -> impl Strategy<Value = Transform2> {
    prop_oneof![
        vector().prop_map(Transform2::translation),
        (point(), any_angle()).prop_map(|(c, a)| Transform2::rotation(c, a).unwrap()),
        (point(), 0.5..2.0f64).prop_map(|(c, f)| Transform2::scaling(c, f).unwrap()),
        line().prop_map(Transform2::mirror),
    ]
}

/// A chain of 1 to 4 steps composed with `then`, plus the steps themselves.
fn transform() -> impl Strategy<Value = (Transform2, Vec<Transform2>)> {
    prop::collection::vec(step(), 1..=4).prop_map(|steps| {
        let t = steps.iter().skip(1).fold(steps[0], |acc, s| acc.then(*s));
        (t, steps)
    })
}

/// Composed transforms of large coordinates accumulate rounding: scale the
/// tolerance to the largest magnitude seen, with headroom for the chain.
fn chain_tol(points: &[Point2]) -> Tolerance {
    tol_for(10.0 * points.iter().map(|p| magnitude(*p)).fold(0.0, f64::max))
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
    fn geometry2_delegates_exactly(
        l in line(), c in circle(), a in arc(), q in point(), t in 0.0..=1.0f64,
    ) {
        prop_assert_eq!(Geometry2::from(l).closest_point(q), l.closest_point(q));
        prop_assert_eq!(Geometry2::from(c).distance_to(q), c.distance_to(q));
        prop_assert_eq!(Geometry2::from(a).point_at(t), a.point_at(t));
        prop_assert_eq!(Geometry2::from(a).bounds(), a.bounds());
        prop_assert_eq!(Geometry2::from(l).length(), l.length());
        prop_assert_eq!(Geometry2::from(a).start_point(), Some(a.start_point()));
        prop_assert_eq!(Geometry2::from(c).end_point(), None);
    }

    #[test]
    fn composition_equals_applying_each_step((t, steps) in transform(), p in point()) {
        let stepwise = steps.iter().fold(p, |q, s| s.apply_point(q));
        let tol = chain_tol(&[p, stepwise, t.apply_point(p)]);
        prop_assert!(t.apply_point(p).approx_eq(stepwise, tol));
    }

    #[test]
    fn inverse_undoes((t, _) in transform(), p in point()) {
        let image = t.apply_point(p);
        let back = t.inverse().apply_point(image);
        prop_assert!(back.approx_eq(p, chain_tol(&[p, image])));
    }

    #[test]
    fn distances_scale_by_the_factor((t, _) in transform(), p in point(), q in point()) {
        let (tp, tq) = (t.apply_point(p), t.apply_point(q));
        let expected = p.distance_to(q) * t.scale_factor();
        prop_assert!((tp.distance_to(tq) - expected).abs() <= chain_tol(&[p, q, tp, tq]).linear());
    }

    #[test]
    fn transformed_curves_contain_the_images_of_their_points(
        (t, _) in transform(), l in line(), c in circle(), a in arc(), s in 0.0..=1.0f64,
    ) {
        for g in [Geometry2::from(l), c.into(), a.into()] {
            let image = t.apply(g).unwrap();
            prop_assert_eq!(image.kind(), g.kind());
            let q = t.apply_point(g.point_at(s));
            let tol = chain_tol(&[g.point_at(s), q, image.bounds().min(), image.bounds().max()]);
            prop_assert!(image.contains_point(q, tol), "{:?} lost a point under {:?}", g.kind(), t);
            let expected_length = g.length() * t.scale_factor();
            prop_assert!((image.length() - expected_length).abs() <= tol.linear() * 10.0);
        }
    }

    #[test]
    fn mirror_is_an_involution(axis in line(), p in point()) {
        let m = Transform2::mirror(axis);
        let back = m.apply_point(m.apply_point(p));
        prop_assert!(back.approx_eq(p, chain_tol(&[p, axis.start()])));
        prop_assert!(m.is_reflection());
    }

    #[test]
    fn intersection_points_lie_on_both_curves((base, a, b) in scene()) {
        let tol = tol_for(magnitude(base) + 300.0);
        let found = intersect(a, b, TOL);
        for p in found.points() {
            prop_assert!(a.contains_point(*p, tol), "{p:?} not on {a:?}");
            prop_assert!(b.contains_point(*p, tol), "{p:?} not on {b:?}");
        }
        for piece in found.overlaps() {
            let mid = piece.point_at(0.5);
            prop_assert!(a.contains_point(mid, tol) && b.contains_point(mid, tol));
        }
        if found.overlaps().is_empty() {
            prop_assert!(found.points().len() <= 2);
        }
    }

    #[test]
    fn intersection_is_symmetric((base, a, b) in scene()) {
        let tol = tol_for(magnitude(base) + 300.0);
        let (ab, ba) = (intersect(a, b, TOL), intersect(b, a, TOL));
        prop_assert!(same_points(&ab, &ba, tol), "{:?} vs {:?}", ab.points(), ba.points());
        prop_assert_eq!(ab.overlaps().len(), ba.overlaps().len());
    }

    #[test]
    fn crossing_lines_meet_where_they_were_built(
        base in point(), d1 in any_angle(), d2 in any_angle(),
        l1 in 1.0..100.0f64, l2 in 1.0..100.0f64, t1 in 0.0..=1.0f64, t2 in 0.0..=1.0f64,
    ) {
        // Two segments through `base`, at parameters t1 and t2.
        prop_assume!(angle::difference(d1, d2).abs() > 0.01 && (PI - angle::difference(d1, d2).abs()) > 0.01);
        let (u1, u2) = (Vector2::from_polar(l1, d1).unwrap(), Vector2::from_polar(l2, d2).unwrap());
        let a = Line::new(base - u1 * t1, base + u1 * (1.0 - t1)).unwrap();
        let b = Line::new(base - u2 * t2, base + u2 * (1.0 - t2)).unwrap();
        let found = intersect(a.into(), b.into(), TOL);
        let tol = tol_for(magnitude(base) + 200.0);
        prop_assert_eq!(found.points().len(), 1);
        prop_assert!(found.points()[0].approx_eq(base, tol));
    }

    #[test]
    fn curves_through_a_point_meet_there(
        base in point(), r1 in 1.0..100.0f64, r2 in 1.0..100.0f64,
        a1 in any_angle(), a2 in any_angle(), dir in any_angle(), half in 1.0..50.0f64,
    ) {
        // A circle, an arc and a line all passing through `base`.
        let c1 = Circle::new(base - Vector2::from_polar(r1, a1).unwrap(), r1).unwrap();
        let c2 = Circle::new(base - Vector2::from_polar(r2, a2).unwrap(), r2).unwrap();
        let arc = Arc::new(c2.center(), r2, a2 - 0.5, a2 + 0.5).unwrap();
        let u = Vector2::from_polar(half, dir).unwrap();
        let line = Line::new(base - u, base + u).unwrap();
        let tol = tol_for(magnitude(base) + 300.0);
        // Directions of the curves at `base` (circles: perpendicular to the radius).
        let (t_line, t_c1, t_c2) = (dir, a1 + FRAC_PI_2, a2 + FRAC_PI_2);
        for (x, y, crossing) in [
            (Geometry2::from(line), Geometry2::from(c1), t_line - t_c1),
            (line.into(), arc.into(), t_line - t_c2),
            (c1.into(), arc.into(), t_c1 - t_c2),
        ] {
            // Near-tangent crossings are ill-conditioned: an input error of ε
            // (about 1e-9 at survey coordinates) moves the meeting point by up
            // to √(r·ε) along the curves. Their points are still checked to lie
            // on both curves by `intersection_points_lie_on_both_curves`.
            if crossing.sin().abs() < 0.05 {
                continue;
            }
            let found = intersect(x, y, TOL);
            let hit = found.points().iter().any(|p| p.approx_eq(base, tol))
                || !found.overlaps().is_empty();
            prop_assert!(hit, "{:?} and {:?} miss {:?}: {:?}", x, y, base, found);
        }
    }

    #[test]
    fn key_points_lie_on_the_curve((base, g, _) in scene()) {
        let tol = tol_for(magnitude(base) + 300.0);
        for s in snap::key_points(g) {
            if s.kind == SnapKind::Center {
                continue;
            }
            prop_assert!(g.contains_point(s.point, tol), "{:?} {:?} not on {:?}", s.kind, s.point, g);
            prop_assert!(g.bounds().contains_point(s.point, tol));
        }
    }

    #[test]
    fn perpendicular_points_make_a_right_angle((base, g, _) in scene(), from in point()) {
        let from = base + (from - Point2::ORIGIN) * 1e-5; // near the scene
        let tol = tol_for(magnitude(base) + 300.0);
        for s in snap::perpendicular_points(g, from, TOL) {
            prop_assert!(g.contains_point(s.point, tol));
            let Ok(to_from) = (from - s.point).normalized() else { continue };
            let cos = to_from.dot(snap::tangent_direction(g, s.point));
            prop_assert!(cos.abs() <= 1e-6, "angle off 90° by cos = {cos}");
        }
    }

    #[test]
    fn tangent_points_touch_without_crossing(
        c in circle(), dir in any_angle(), extra in 1.001..10.0f64,
    ) {
        // A point outside the circle has exactly two tangent points.
        let from = c.center() + Vector2::from_polar(c.radius() * extra, dir).unwrap();
        let tangents = snap::tangent_points(c.into(), from, TOL);
        prop_assert_eq!(tangents.len(), 2);
        let tol = tol_for(magnitude(c.center()) + c.radius() * 10.0);
        for s in tangents {
            prop_assert!(c.contains_point(s.point, tol));
            let radial = (s.point - c.center()).normalized().unwrap();
            let towards = (s.point - from).normalized().unwrap();
            prop_assert!(radial.dot(towards).abs() <= 1e-6, "radius not perpendicular to the tangent");
        }
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
