//! Tests of the edges' curves (a submodule of `curve`). Distances are
//! worked out by hand.

use std::f64::consts::FRAC_PI_2;

use arq_geometry::{GeometryError, Point3, Transform3, Vector3};

use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn distance_to_a_line() {
    let line = LineCurve::new(p(1.0, 0.0, 0.0), v(0.0, 0.0, 5.0)).unwrap();
    assert_eq!(line.direction(), v(0.0, 0.0, 1.0), "unit");
    assert!(
        close(line.distance(p(4.0, 4.0, 7.0)), 5.0),
        "3-4-5 from the axis"
    );
    assert!(close(line.distance(p(1.0, 0.0, -3.0)), 0.0));
    assert_eq!(
        LineCurve::new(p(0.0, 0.0, 0.0), Vector3::ZERO),
        Err(GeometryError::ZeroVector)
    );
}

#[test]
fn distance_to_a_circle_and_an_ellipse() {
    // A circle of radius 2 in the plane z = 1.
    let circle =
        EllipseCurve::new(p(0.0, 0.0, 1.0), v(0.0, 0.0, 1.0), v(2.0, 0.0, 0.0), 1.0).unwrap();
    assert!(close(circle.distance(p(0.0, 5.0, 1.0)), 3.0));
    assert!(
        close(circle.distance(p(0.0, 2.0, 5.0)), 4.0),
        "above the curve"
    );
    assert!(close(circle.distance(p(0.6, 0.8, 1.0)), 1.0), "inside");
    // An ellipse of semi-axes 4 (x) and 2 (y): its vertices and a point
    // near its end along x.
    let ellipse =
        EllipseCurve::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), v(4.0, 0.0, 0.0), 0.5).unwrap();
    assert!(close(ellipse.distance(p(0.0, 5.0, 0.0)), 3.0));
    assert!(close(ellipse.distance(p(7.0, 0.0, 0.0)), 3.0));
    assert!(close(ellipse.distance(ellipse.point_at(1.0)), 0.0), "on it");
    let on = ellipse.point_at(0.7);
    let n = v(on.x() / 16.0, on.y() / 4.0, 0.0).normalized().unwrap();
    assert!(
        close(ellipse.distance(on + n * 0.25), 0.25),
        "out along the normal"
    );
    assert!(
        close(ellipse.point_at(FRAC_PI_2).y(), 2.0),
        "minor = normal × major"
    );
}

#[test]
fn ellipses_are_checked() {
    let (c, n) = (p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
    assert!(
        EllipseCurve::new(c, n, v(1.0, 0.0, 0.0), 0.0).is_err(),
        "zero ratio"
    );
    assert!(
        EllipseCurve::new(c, n, Vector3::ZERO, 1.0).is_err(),
        "no radius"
    );
    assert!(
        EllipseCurve::new(c, n, v(1.0, 0.0, 1.0), 1.0).is_err(),
        "axis off its plane"
    );
}

#[test]
fn curves_move_with_a_transform() {
    let t = Transform3::translation(v(0.0, 0.0, 3.0));
    let circle = Curve::Ellipse(
        EllipseCurve::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), v(1.0, 0.0, 0.0), 1.0).unwrap(),
    );
    let moved = circle.transformed(t).unwrap();
    assert!(close(moved.distance(p(1.0, 0.0, 3.0)), 0.0));
    let line = Curve::Line(LineCurve::new(p(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)).unwrap());
    assert!(close(
        line.transformed(t).unwrap().distance(p(9.0, 0.0, 3.0)),
        0.0
    ));
}
