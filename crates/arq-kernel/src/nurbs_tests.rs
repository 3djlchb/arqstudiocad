//! Tests of NURBS curves and surfaces (a submodule of `nurbs`). Every
//! expected value is worked out by hand.

use std::f64::consts::FRAC_1_SQRT_2;

use arq_geometry::{GeometryError, Point3, Transform3, Vector3};

use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn near(a: Point3, b: Point3) -> bool {
    a.distance_to(b) < 1e-9
}

/// A quarter of the unit circle in the XY plane, from (1,0) to (0,1): the
/// rational quadratic with the middle weight √2/2.
fn quarter() -> NurbsCurve {
    NurbsCurve::new(
        2,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0)],
        vec![1.0, FRAC_1_SQRT_2, 1.0],
    )
    .unwrap()
}

#[test]
fn a_degree_one_curve_is_a_segment() {
    let line = NurbsCurve::new(
        1,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![p(0.0, 0.0, 0.0), p(4.0, 0.0, 0.0)],
        vec![1.0, 1.0],
    )
    .unwrap();
    assert_eq!(line.domain(), (0.0, 1.0));
    assert!(near(line.point_at(0.25), p(1.0, 0.0, 0.0)));
    assert!(close(line.distance(p(2.0, 3.0, 0.0)), 3.0));
    assert!(
        close(line.distance(p(6.0, 0.0, 0.0)), 2.0),
        "clamped to its end"
    );
}

#[test]
fn a_curve_of_several_spans_finds_the_right_one() {
    // Degree 1 through (0,0), (1,0), (1,1), knots 0 0 1 2 2: two spans,
    // the second running up from (1,0).
    let bend = NurbsCurve::new(
        1,
        vec![0.0, 0.0, 1.0, 2.0, 2.0],
        vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0)],
        vec![1.0; 3],
    )
    .unwrap();
    assert!(near(bend.point_at(0.5), p(0.5, 0.0, 0.0)));
    assert!(near(bend.point_at(1.5), p(1.0, 0.5, 0.0)));
    assert!(
        near(bend.point_at(2.0), p(1.0, 1.0, 0.0)),
        "the domain's end"
    );
}

#[test]
fn a_rational_quarter_is_exactly_circular() {
    let q = quarter();
    assert!(near(q.point_at(0.0), p(1.0, 0.0, 0.0)));
    assert!(near(q.point_at(1.0), p(0.0, 1.0, 0.0)));
    assert!(near(q.point_at(0.5), p(FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0)));
    for i in 0..=20 {
        let at = q.point_at(f64::from(i) / 20.0);
        assert!(close(at.to_vector().length(), 1.0), "{at:?} off the circle");
    }
    // From (2,2) the closest point is the middle, 2√2 - 1 away.
    assert!(close(q.distance(p(2.0, 2.0, 0.0)), 2.0 * 2f64.sqrt() - 1.0));
    assert!(close(q.distance(p(0.0, 0.0, 0.0)), 1.0));
}

#[test]
fn bad_curves_are_refused() {
    let points = vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)];
    let one = vec![1.0, 1.0];
    let refused = |degree: usize, knots: Vec<f64>, weights: Vec<f64>| {
        NurbsCurve::new(degree, knots, points.clone(), weights).is_err()
    };
    assert!(refused(0, vec![0.0, 0.0, 1.0], one.clone()), "degree 0");
    assert!(
        refused(2, vec![0.0, 0.0, 0.0, 1.0, 1.0], one.clone()),
        "too few points"
    );
    assert!(refused(1, vec![0.0, 0.0, 1.0], one.clone()), "knot count");
    assert!(
        refused(1, vec![0.5, 0.0, 1.0, 1.0], one.clone()),
        "knot order (the domain 0..1 itself fine)"
    );
    assert!(
        refused(1, vec![1.0, 1.0, 1.0, 1.0], one.clone()),
        "empty domain"
    );
    assert!(
        refused(1, vec![0.0, 0.0, 1.0, 1.0], vec![1.0, 0.0]),
        "zero weight"
    );
    assert_eq!(
        NurbsCurve::new(1, vec![0.0, 0.0, f64::NAN, 1.0], points.clone(), one),
        Err(GeometryError::NonFinite { what: "knot" })
    );
}

#[test]
fn surfaces_evaluate_their_grid_with_u_the_slower_index() {
    // A bilinear patch: rows along u are x, columns along v are y, so the
    // point at (u, v) is (2u, 3v).
    let patch = NurbsSurface::new(
        1,
        1,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        2,
        2,
        vec![
            p(0.0, 0.0, 0.0),
            p(0.0, 3.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 3.0, 0.0),
        ],
        vec![1.0; 4],
    )
    .unwrap();
    assert!(near(patch.point_at(0.5, 1.0 / 3.0), p(1.0, 1.0, 0.0)));
    assert!(close(patch.distance(p(1.0, 1.5, 5.0)), 5.0));
    // A quarter cylinder of radius 1 and height 2: the rational quarter
    // swept up z. Not symmetric in u and v, so the grid order shows.
    let w = FRAC_1_SQRT_2;
    let arc = [p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0)];
    let points = arc
        .iter()
        .flat_map(|&c| [c, c + Vector3::new(0.0, 0.0, 2.0).unwrap()])
        .collect();
    let cylinder = NurbsSurface::new(
        2,
        1,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        3,
        2,
        points,
        vec![1.0, 1.0, w, w, 1.0, 1.0],
    )
    .unwrap();
    assert!(near(cylinder.point_at(0.5, 0.5), p(w, w, 1.0)));
    assert!(close(cylinder.distance(p(2.0, 0.0, 0.5)), 1.0));
    assert!(
        close(cylinder.distance(p(1.5 * w, 1.5 * w, 1.5)), 0.5),
        "off its middle"
    );
    // At radius 2, 0.3 rad round and at height 0.37: no grid sample lies
    // there, so the Newton steps find it.
    let (s, c) = 0.3f64.sin_cos();
    let d = cylinder.distance(p(2.0 * c, 2.0 * s, 0.37));
    assert!(close(d, 1.0), "{d}");
    assert!(
        NurbsSurface::new(
            1,
            1,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            2,
            2,
            vec![],
            vec![]
        )
        .is_err(),
        "a grid of the wrong size"
    );
}

#[test]
fn nurbs_move_with_their_control_points() {
    let t = Transform3::translation(Vector3::new(0.0, 0.0, 3.0).unwrap());
    let moved = quarter().transformed(t);
    assert!(near(
        moved.point_at(0.5),
        p(FRAC_1_SQRT_2, FRAC_1_SQRT_2, 3.0)
    ));
    assert_eq!(moved.weights(), quarter().weights());
}

/// The derivative by central differences, for comparison.
fn numeric(f: impl Fn(f64) -> Point3, t: f64) -> Vector3 {
    let h = 1e-5;
    (f(t + h) - f(t - h)) / (2.0 * h)
}

#[test]
fn a_curves_derivative_is_exact() {
    let q = quarter();
    for t in [0.1, 0.37, 0.5, 0.9] {
        let d = q.derivative_at(t);
        assert!(
            (d - numeric(|s| q.point_at(s), t)).length() < 1e-8,
            "at {t}"
        );
        // On the circle, the tangent is perpendicular to the radius.
        assert!(d.dot(q.point_at(t).to_vector()).abs() < 1e-12, "at {t}");
    }
    // Degree one: each span's direction, scaled by its parameter length.
    let polyline = NurbsCurve::new(
        1,
        vec![0.0, 0.0, 2.0, 3.0, 3.0],
        vec![p(0.0, 0.0, 0.0), p(4.0, 0.0, 0.0), p(4.0, 3.0, 0.0)],
        vec![1.0; 3],
    )
    .unwrap();
    assert!((polyline.derivative_at(1.0) - Vector3::new(2.0, 0.0, 0.0).unwrap()).length() < 1e-12);
    assert!((polyline.derivative_at(2.5) - Vector3::new(0.0, 3.0, 0.0).unwrap()).length() < 1e-12);
}

#[test]
fn a_surfaces_derivatives_are_exact() {
    // The quarter circle swept up z, rational in u, linear in v.
    let w = FRAC_1_SQRT_2;
    let surface = NurbsSurface::new(
        2,
        1,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        3,
        2,
        vec![
            p(1.0, 0.0, 0.0),
            p(1.0, 0.0, 2.0),
            p(1.0, 1.0, 0.0),
            p(1.0, 1.0, 2.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 1.0, 2.0),
        ],
        vec![1.0, 1.0, w, w, 1.0, 1.0],
    )
    .unwrap();
    for (u, v) in [(0.2, 0.3), (0.5, 0.5), (0.85, 0.1)] {
        let (su, sv) = surface.derivatives(u, v);
        assert!((su - numeric(|s| surface.point_at(s, v), u)).length() < 1e-8);
        assert!((sv - numeric(|s| surface.point_at(u, s), v)).length() < 1e-8);
        assert!(
            (sv - Vector3::new(0.0, 0.0, 2.0).unwrap()).length() < 1e-12,
            "straight up"
        );
    }
}
