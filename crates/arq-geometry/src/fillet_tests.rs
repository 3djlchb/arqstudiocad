use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use super::*;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(a: (f64, f64), b: (f64, f64)) -> Line {
    Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap()
}

fn ends(g: Option<Geometry2>) -> ((f64, f64), (f64, f64)) {
    let g = g.expect("a curve");
    let r = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
    let (a, b) = (g.start_point().unwrap(), g.end_point().unwrap());
    ((r(a.x()), r(a.y())), (r(b.x()), r(b.y())))
}

/// Two lines crossing at the origin, picked toward +x and +y.
fn cross() -> (Geometry2, Point2, Geometry2, Point2) {
    (
        Geometry2::Line(line((-10.0, 0.0), (10.0, 0.0))),
        p(5.0, 0.0),
        Geometry2::Line(line((0.0, -10.0), (0.0, 10.0))),
        p(0.0, 5.0),
    )
}

#[test]
fn a_fillet_rounds_the_corner_on_the_sides_picked() {
    let (a, pa, b, pb) = cross();
    let c = fillet(a, pa, b, pb, 2.0, true, TOL).unwrap();
    assert_eq!(ends(c.first), ((2.0, 0.0), (10.0, 0.0)));
    assert_eq!(ends(c.second), ((0.0, 2.0), (0.0, 10.0)));
    let Some(Geometry2::Arc(arc)) = c.joint else {
        panic!("{c:?}")
    };
    assert!(arc.center().approx_eq(p(2.0, 2.0), TOL) && (arc.radius() - 2.0).abs() < 1e-12);
    assert!((arc.sweep() - FRAC_PI_2).abs() < 1e-12, "a quarter circle");
    // Picked toward -x and -y: the other corner.
    let c = fillet(a, p(-5.0, 0.0), b, p(0.0, -5.0), 2.0, true, TOL).unwrap();
    assert_eq!(ends(c.first), ((-10.0, 0.0), (-2.0, 0.0)));
    let Some(Geometry2::Arc(arc)) = c.joint else {
        panic!()
    };
    assert!(arc.center().approx_eq(p(-2.0, -2.0), TOL));
}

#[test]
fn radius_zero_meets_the_lines_extending_or_trimming_them() {
    let a = Geometry2::Line(line((0.0, 0.0), (10.0, 0.0)));
    let b = Geometry2::Line(line((12.0, 2.0), (12.0, 10.0)));
    let c = fillet(a, p(5.0, 0.0), b, p(12.0, 5.0), 0.0, true, TOL).unwrap();
    assert_eq!(ends(c.first), ((0.0, 0.0), (12.0, 0.0)), "extended");
    assert_eq!(
        ends(c.second),
        ((12.0, 0.0), (12.0, 10.0)),
        "extended, its direction kept"
    );
    assert!(c.joint.is_none());
}

#[test]
fn without_trim_only_the_arc_is_made() {
    let (a, pa, b, pb) = cross();
    let c = fillet(a, pa, b, pb, 2.0, false, TOL).unwrap();
    assert!(c.first.is_none() && c.second.is_none() && c.joint.is_some());
}

#[test]
fn a_fillet_too_large_or_between_parallels_is_refused() {
    let (a, pa, b, pb) = cross();
    assert!(
        fillet(a, pa, b, pb, 20.0, true, TOL).is_err(),
        "longer than the lines"
    );
    let up = Geometry2::Line(line((0.0, 3.0), (10.0, 3.0)));
    let flat = Geometry2::Line(line((0.0, 0.0), (10.0, 0.0)));
    assert_eq!(
        fillet(flat, p(1.0, 0.0), up, p(1.0, 3.0), 1.0, true, TOL),
        Err(GeometryError::OutOfRange { what: "parallel" })
    );
    assert!(fillet(a, pa, b, pb, -1.0, true, TOL).is_err());
}

#[test]
fn a_line_and_an_arc_meet_by_an_arc_tangent_to_both() {
    // The line y = 0; the lower half of a circle of radius 3 about (0, 5).
    let l = Geometry2::Line(line((-10.0, 0.0), (10.0, 0.0)));
    let arc = Geometry2::Arc(Arc::new(p(0.0, 5.0), 3.0, PI, 2.0 * PI).unwrap());
    let c = fillet(l, p(5.0, 0.0), arc, p(2.6, 3.5), 2.0, true, TOL).unwrap();
    // Its center 2 above the line and 5 from the circle's: (4, 2).
    let Some(Geometry2::Arc(joint)) = c.joint else {
        panic!("{c:?}")
    };
    assert!(joint.center().approx_eq(p(4.0, 2.0), TOL), "{joint:?}");
    assert_eq!(ends(c.first), ((4.0, 0.0), (10.0, 0.0)));
    let Some(Geometry2::Arc(kept)) = c.second else {
        panic!()
    };
    let tangent = (p(2.4, 3.2) - p(0.0, 5.0)).angle().rem_euclid(2.0 * PI);
    assert!((kept.start_angle() - tangent).abs() < 1e-9, "{kept:?}");
    // Either order.
    let swapped = fillet(arc, p(2.6, 3.5), l, p(5.0, 0.0), 2.0, true, TOL).unwrap();
    assert_eq!(swapped.second, c.first);
}

#[test]
fn a_chamfer_cuts_the_corner_by_its_distances() {
    let a = line((-10.0, 0.0), (10.0, 0.0));
    let b = line((0.0, -10.0), (0.0, 10.0));
    let c = chamfer(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, 3.0, true, TOL).unwrap();
    assert_eq!(ends(c.joint), ((2.0, 0.0), (0.0, 3.0)));
    assert_eq!(ends(c.first), ((2.0, 0.0), (10.0, 0.0)));
    assert_eq!(ends(c.second), ((0.0, 3.0), (0.0, 10.0)));
    let corner = chamfer(a, p(5.0, 0.0), b, p(0.0, 5.0), 0.0, 0.0, true, TOL).unwrap();
    assert!(corner.joint.is_none());
    assert_eq!(ends(corner.first), ((0.0, 0.0), (10.0, 0.0)));
    assert!(chamfer(a, p(5.0, 0.0), b, p(0.0, 5.0), 20.0, 1.0, true, TOL).is_err());
}

#[test]
fn the_angle_method_gives_the_second_distance() {
    let a = line((-10.0, 0.0), (10.0, 0.0));
    let b = line((0.0, -10.0), (0.0, 10.0));
    let d = chamfer_second_distance(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, FRAC_PI_4).unwrap();
    assert!((d - 2.0).abs() < 1e-12, "45° on a right angle: as long");
    let d = chamfer_second_distance(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, PI / 3.0).unwrap();
    assert!((d - 2.0 * 3.0f64.sqrt()).abs() < 1e-9, "{d}");
    assert!(chamfer_second_distance(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, FRAC_PI_2).is_err());
}
