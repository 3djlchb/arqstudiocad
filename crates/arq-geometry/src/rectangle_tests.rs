use std::f64::consts::{FRAC_PI_2, FRAC_PI_6, PI};

use super::*;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.approx_eq(b, Tolerance::new(1e-9, 1e-10).unwrap())
}

fn same(corners: [Point2; 4], expected: [(f64, f64); 4]) {
    for (got, (x, y)) in corners.iter().zip(expected) {
        assert!(near(*got, p(x, y)), "{corners:?} != {expected:?}");
    }
}

/// Twice the signed area: positive when the corners run counterclockwise.
fn signed_area2(c: [Point2; 4]) -> f64 {
    (0..4)
        .map(|i| {
            let (a, b) = (c[i], c[(i + 1) % 4]);
            a.x() * b.y() - b.x() * a.y()
        })
        .sum()
}

#[test]
fn two_corners_in_every_quarter_run_counterclockwise_from_the_first() {
    let r = Rectangle::from_corners(p(1.0, 2.0), p(5.0, 4.0), 0.0, TOL).unwrap();
    same(
        r.corners(),
        [(1.0, 2.0), (5.0, 2.0), (5.0, 4.0), (1.0, 4.0)],
    );
    assert_eq!((r.length(), r.width(), r.area()), (4.0, 2.0, 8.0));
    assert!(near(r.other(), p(5.0, 4.0)));

    let r = Rectangle::from_corners(p(1.0, 2.0), p(-3.0, 4.0), 0.0, TOL).unwrap();
    same(
        r.corners(),
        [(1.0, 2.0), (1.0, 4.0), (-3.0, 4.0), (-3.0, 2.0)],
    );

    for other in [p(5.0, 4.0), p(-3.0, 4.0), p(-3.0, 0.0), p(5.0, 0.0)] {
        let r = Rectangle::from_corners(p(1.0, 2.0), other, 0.0, TOL).unwrap();
        assert_eq!(r.corners()[0], p(1.0, 2.0));
        assert!(near(r.other(), other));
        assert!((signed_area2(r.corners()) - 2.0 * r.area()).abs() < 1e-9);
    }
}

#[test]
fn rotation_sees_the_other_corner_in_the_turned_frame() {
    // Turned a right angle: the length runs up, the width to the left.
    let r = Rectangle::from_corners(p(0.0, 0.0), p(-2.0, 4.0), FRAC_PI_2, TOL).unwrap();
    assert!((r.length() - 4.0).abs() < 1e-12 && (r.width() - 2.0).abs() < 1e-12);
    same(
        r.corners(),
        [(0.0, 0.0), (0.0, 4.0), (-2.0, 4.0), (-2.0, 0.0)],
    );

    // At 30°, a point along the rotation and one square to it.
    let (s, c) = FRAC_PI_6.sin_cos();
    let other = p(3.0 * c - 1.0 * s, 3.0 * s + 1.0 * c);
    let r = Rectangle::from_corners(p(0.0, 0.0), other, FRAC_PI_6, TOL).unwrap();
    assert!((r.length() - 3.0).abs() < 1e-12 && (r.width() - 1.0).abs() < 1e-12);
    assert!(near(r.corners()[1], p(3.0 * c, 3.0 * s)));
    assert!(near(r.other(), other));
}

#[test]
fn survey_coordinates_keep_their_size() {
    let first = p(500_000.25, 9_800_000.5);
    let r = Rectangle::from_corners(first, p(500_004.25, 9_800_003.5), 0.0, TOL).unwrap();
    assert_eq!((r.length(), r.width()), (4.0, 3.0));
}

#[test]
fn a_corner_on_a_side_line_or_the_first_leaves_no_rectangle() {
    for other in [p(1.0, 2.0), p(5.0, 2.0), p(1.0, 9.0)] {
        assert_eq!(
            Rectangle::from_corners(p(1.0, 2.0), other, 0.0, TOL),
            Err(GeometryError::ZeroLength)
        );
    }
    // On the turned frame's side line, though not on an axis.
    assert_eq!(
        Rectangle::from_corners(p(0.0, 0.0), p(1.0, 1.0), PI / 4.0, TOL),
        Err(GeometryError::ZeroLength)
    );
}

#[test]
fn dimensions_go_toward_the_quarter_the_point_lies_in() {
    let first = p(10.0, 10.0);
    let cases = [
        (
            p(20.0, 20.0),
            [(10.0, 10.0), (14.0, 10.0), (14.0, 13.0), (10.0, 13.0)],
        ),
        (
            p(0.0, 20.0),
            [(10.0, 10.0), (10.0, 13.0), (6.0, 13.0), (6.0, 10.0)],
        ),
        (
            p(0.0, 0.0),
            [(10.0, 10.0), (6.0, 10.0), (6.0, 7.0), (10.0, 7.0)],
        ),
        (
            p(20.0, 0.0),
            [(10.0, 10.0), (10.0, 7.0), (14.0, 7.0), (14.0, 10.0)],
        ),
        // On a side's line: the positive way.
        (
            p(10.0, 10.0),
            [(10.0, 10.0), (14.0, 10.0), (14.0, 13.0), (10.0, 13.0)],
        ),
    ];
    for (toward, expected) in cases {
        let r = Rectangle::from_size(first, 4.0, 3.0, 0.0, toward, TOL).unwrap();
        same(r.corners(), expected);
        assert_eq!((r.length(), r.width()), (4.0, 3.0));
    }
    // Turned: the quarter is seen in the turned frame.
    let r = Rectangle::from_size(p(0.0, 0.0), 4.0, 3.0, FRAC_PI_2, p(-1.0, 1.0), TOL).unwrap();
    same(
        r.corners(),
        [(0.0, 0.0), (0.0, 4.0), (-3.0, 4.0), (-3.0, 0.0)],
    );
}

#[test]
fn dimensions_refuse_bad_sizes() {
    let o = p(0.0, 0.0);
    assert_eq!(
        Rectangle::from_size(o, 0.0, 3.0, 0.0, o, TOL),
        Err(GeometryError::ZeroLength)
    );
    assert_eq!(
        Rectangle::from_size(o, -1.0, 3.0, 0.0, o, TOL),
        Err(GeometryError::NegativeOrNonFinite { what: "length" })
    );
    assert_eq!(
        Rectangle::from_size(o, 1.0, f64::NAN, 0.0, o, TOL),
        Err(GeometryError::NegativeOrNonFinite { what: "width" })
    );
    assert_eq!(
        Rectangle::from_size(o, 1.0, 1.0, f64::INFINITY, o, TOL),
        Err(GeometryError::NonFinite { what: "rotation" })
    );
    // Far corners beyond f64 are refused, not built.
    assert_eq!(
        Rectangle::from_size(p(1e308, 0.0), 1e308, 1.0, 0.0, p(1.7e308, 0.0), TOL),
        Err(GeometryError::NonFinite { what: "Rectangle" })
    );
}

#[test]
fn area_with_the_length_or_the_width_given() {
    let r = Rectangle::from_area(p(1.0, 1.0), 12.0, 4.0, true, 0.0, TOL).unwrap();
    same(
        r.corners(),
        [(1.0, 1.0), (5.0, 1.0), (5.0, 4.0), (1.0, 4.0)],
    );
    let r = Rectangle::from_area(p(1.0, 1.0), 12.0, 4.0, false, 0.0, TOL).unwrap();
    same(
        r.corners(),
        [(1.0, 1.0), (4.0, 1.0), (4.0, 5.0), (1.0, 5.0)],
    );
    assert!((r.area() - 12.0).abs() < 1e-12);

    let r = Rectangle::from_area(p(0.0, 0.0), 6.0, 2.0, true, FRAC_PI_2, TOL).unwrap();
    same(
        r.corners(),
        [(0.0, 0.0), (0.0, 2.0), (-3.0, 2.0), (-3.0, 0.0)],
    );
}

#[test]
fn area_refuses_what_leaves_no_rectangle() {
    let o = p(0.0, 0.0);
    assert_eq!(
        Rectangle::from_area(o, 12.0, 0.0, true, 0.0, TOL),
        Err(GeometryError::ZeroLength)
    );
    assert_eq!(
        Rectangle::from_area(o, 0.0, 2.0, true, 0.0, TOL),
        Err(GeometryError::ZeroLength)
    );
    assert_eq!(
        Rectangle::from_area(o, -4.0, 2.0, true, 0.0, TOL),
        Err(GeometryError::NegativeOrNonFinite { what: "area" })
    );
    assert_eq!(
        Rectangle::from_area(o, 1e308, 1e-308, true, 0.0, TOL),
        Err(GeometryError::NonFinite { what: "Rectangle" })
    );
}

#[test]
fn non_finite_input_is_refused() {
    let o = p(0.0, 0.0);
    assert_eq!(
        Rectangle::from_corners(o, p(1.0, 1.0), f64::NAN, TOL),
        Err(GeometryError::NonFinite { what: "rotation" })
    );
    assert_eq!(
        Rectangle::from_corners(p(-1e308, 0.0), p(1e308, 1.0), 0.0, TOL),
        Err(GeometryError::NonFinite { what: "Rectangle" })
    );
}
