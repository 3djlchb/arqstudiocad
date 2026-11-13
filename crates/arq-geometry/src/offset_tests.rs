use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.approx_eq(b, Tolerance::new(1e-9, 1e-12).unwrap())
}

fn line(a: (f64, f64), b: (f64, f64)) -> Geometry2 {
    Geometry2::Line(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap())
}

#[test]
fn a_line_moves_square_to_itself_to_the_side_shown() {
    let l = line((0.0, 0.0), (10.0, 0.0));
    let Geometry2::Line(up) = l.offset(2.0, p(3.0, 7.0), TOL).unwrap() else {
        panic!("a line")
    };
    assert!(near(up.start(), p(0.0, 2.0)) && near(up.end(), p(10.0, 2.0)));
    let Geometry2::Line(down) = l.offset(2.0, p(-50.0, -0.1), TOL).unwrap() else {
        panic!("a line")
    };
    assert!(near(down.start(), p(0.0, -2.0)) && near(down.end(), p(10.0, -2.0)));
    // Slanted: as long, parallel, 2 away.
    let l = line((0.0, 0.0), (3.0, 4.0));
    let Geometry2::Line(o) = l.offset(2.0, p(0.0, 5.0), TOL).unwrap() else {
        panic!("a line")
    };
    assert!(
        near(o.start(), p(-1.6, 1.2)) && near(o.end(), p(1.4, 5.2)),
        "{o:?}"
    );
}

#[test]
fn circles_and_arcs_widen_outside_and_narrow_inside() {
    let c = Geometry2::Circle(Circle::new(p(5.0, 5.0), 3.0).unwrap());
    let Geometry2::Circle(out) = c.offset(1.0, p(20.0, 5.0), TOL).unwrap() else {
        panic!()
    };
    assert_eq!((out.center(), out.radius()), (p(5.0, 5.0), 4.0));
    let Geometry2::Circle(inner) = c.offset(1.0, p(5.0, 6.0), TOL).unwrap() else {
        panic!()
    };
    assert_eq!(inner.radius(), 2.0);
    assert_eq!(
        c.offset(3.0, p(5.0, 6.0), TOL),
        Err(GeometryError::InvalidRadius),
        "nothing would be left"
    );
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 2.0, 0.0, FRAC_PI_2).unwrap());
    let Geometry2::Arc(wide) = a.offset(0.5, p(-5.0, -5.0), TOL).unwrap() else {
        panic!()
    };
    assert_eq!(
        (wide.radius(), wide.start_angle(), wide.end_angle()),
        (2.5, 0.0, FRAC_PI_2)
    );
    // Inside its circle, where the arc does not run: the circle decides.
    let Geometry2::Arc(narrow) = a.offset(0.5, p(-0.5, -0.5), TOL).unwrap() else {
        panic!()
    };
    assert_eq!(narrow.radius(), 1.5);
}

#[test]
fn through_a_point_measures_from_the_line_or_circle() {
    // Beyond the line's end: square to its line, not to its end.
    let l = line((0.0, 0.0), (10.0, 0.0));
    let Geometry2::Line(o) = l.offset_through(p(25.0, -3.0), TOL).unwrap() else {
        panic!()
    };
    assert!(near(o.start(), p(0.0, -3.0)) && near(o.end(), p(10.0, -3.0)));
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 2.0, 0.0, PI).unwrap());
    let Geometry2::Arc(o) = a.offset_through(p(0.0, -5.0), TOL).unwrap() else {
        panic!()
    };
    assert!((o.radius() - 5.0).abs() < 1e-12);
    assert_eq!(
        l.offset_through(p(4.0, 0.0), TOL),
        Err(GeometryError::OutOfRange { what: "side" }),
        "on it, no distance"
    );
}

#[test]
fn no_side_and_bad_distances_are_refused() {
    let l = line((0.0, 0.0), (10.0, 0.0));
    assert_eq!(
        l.offset(1.0, p(20.0, 0.0), TOL),
        Err(GeometryError::OutOfRange { what: "side" }),
        "on its line's extension"
    );
    let c = Geometry2::Circle(Circle::new(p(0.0, 0.0), 1.0).unwrap());
    assert_eq!(
        c.offset(1.0, p(0.0, 1.0), TOL),
        Err(GeometryError::OutOfRange { what: "side" })
    );
    for d in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            l.offset(d, p(0.0, 1.0), TOL),
            Err(GeometryError::NegativeOrNonFinite {
                what: "offset distance"
            }),
            "{d}"
        );
    }
}

#[test]
fn survey_coordinates_keep_the_distance() {
    let l = line((500_000.0, 9_800_000.0), (500_010.0, 9_800_000.0));
    let Geometry2::Line(o) = l.offset(0.15, p(500_005.0, 9_800_001.0), TOL).unwrap() else {
        panic!()
    };
    assert!((o.start().y() - 9_800_000.15).abs() < 1e-8, "{o:?}");
}
