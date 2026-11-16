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
    let r = Rectangle::from_area(p(1.0, 1.0), 12.0, 4.0, true, 0.0, Corners::Square, TOL).unwrap();
    same(
        r.corners(),
        [(1.0, 1.0), (5.0, 1.0), (5.0, 4.0), (1.0, 4.0)],
    );
    let r = Rectangle::from_area(p(1.0, 1.0), 12.0, 4.0, false, 0.0, Corners::Square, TOL).unwrap();
    same(
        r.corners(),
        [(1.0, 1.0), (4.0, 1.0), (4.0, 5.0), (1.0, 5.0)],
    );
    assert!((r.area() - 12.0).abs() < 1e-12);

    let r =
        Rectangle::from_area(p(0.0, 0.0), 6.0, 2.0, true, FRAC_PI_2, Corners::Square, TOL).unwrap();
    same(
        r.corners(),
        [(0.0, 0.0), (0.0, 2.0), (-3.0, 2.0), (-3.0, 0.0)],
    );
}

#[test]
fn area_refuses_what_leaves_no_rectangle() {
    let o = p(0.0, 0.0);
    assert_eq!(
        Rectangle::from_area(o, 12.0, 0.0, true, 0.0, Corners::Square, TOL),
        Err(GeometryError::ZeroLength)
    );
    assert_eq!(
        Rectangle::from_area(o, 0.0, 2.0, true, 0.0, Corners::Square, TOL),
        Err(GeometryError::ZeroLength)
    );
    assert_eq!(
        Rectangle::from_area(o, -4.0, 2.0, true, 0.0, Corners::Square, TOL),
        Err(GeometryError::NegativeOrNonFinite { what: "area" })
    );
    assert_eq!(
        Rectangle::from_area(o, 1e308, 1e-308, true, 0.0, Corners::Square, TOL),
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

/// The area a closed polyline encloses, its arcs' segments included:
/// positive counterclockwise.
fn polyline_area(vertices: &[(Point2, f64)]) -> f64 {
    let n = vertices.len();
    (0..n)
        .map(|i| {
            let ((a, bulge), b) = (vertices[i], vertices[(i + 1) % n].0);
            let chord = a.x() * b.y() - b.x() * a.y();
            // The segment between an arc and its chord, signed by the bulge.
            let theta = 4.0 * bulge.atan();
            let c = a.distance_to(b);
            let segment = if bulge == 0.0 {
                0.0
            } else {
                let r = c / (2.0 * (theta / 2.0).sin().abs());
                r * r / 2.0 * (theta.abs() - theta.abs().sin()) * bulge.signum()
            };
            chord / 2.0 + segment
        })
        .sum()
}

fn same_vertices(got: &[(Point2, f64)], expected: &[((f64, f64), f64)]) {
    assert_eq!(got.len(), expected.len(), "{got:?}");
    for ((p0, b0), ((x, y), b1)) in got.iter().zip(expected) {
        assert!(
            near(*p0, p(*x, *y)) && (b0 - b1).abs() < 1e-12,
            "{got:?} != {expected:?}"
        );
    }
}

const Q: f64 = 0.414_213_562_373_095_03; // tan(π/8), a quarter circle's bulge

#[test]
fn filleted_corners_are_quarter_circles_from_the_first_corner_on() {
    let r = Rectangle::from_corners(p(0.0, 0.0), p(4.0, 2.0), 0.0, TOL).unwrap();
    let v = r.vertices(Corners::Fillet(0.5), TOL).unwrap();
    same_vertices(
        &v,
        &[
            ((0.5, 0.0), 0.0),
            ((3.5, 0.0), Q),
            ((4.0, 0.5), 0.0),
            ((4.0, 1.5), Q),
            ((3.5, 2.0), 0.0),
            ((0.5, 2.0), Q),
            ((0.0, 1.5), 0.0),
            ((0.0, 0.5), Q),
        ],
    );
    let expected = 8.0 - 4.0 * 0.25 * (1.0 - PI / 4.0);
    assert!((polyline_area(&v) - expected).abs() < 1e-12);
    assert!((r.area_with(Corners::Fillet(0.5)) - expected).abs() < 1e-12);
}

/// The polyline's vertices as a set: a shape, whatever its first vertex.
fn shape(v: &[(Point2, f64)]) -> Vec<(i64, i64)> {
    let mut s: Vec<(i64, i64)> = v
        .iter()
        .map(|(q, _)| ((q.x() * 1e6).round() as i64, (q.y() * 1e6).round() as i64))
        .collect();
    s.sort_unstable();
    s
}

#[test]
fn chamfered_corners_are_cut_as_progecad_cuts_them() {
    // ProgeCAD 2026's RECTANG, chamfer 1 then 2, 10 by 6, drawn from each
    // of the four corners (docs/testing/commands-02-14-windows.md): walking
    // from the first corner along the length first, the first distance is
    // on the side the walk leaves each corner by, the second on the side it
    // arrives by.
    let from_bottom_left: [((f64, f64), f64); 8] = [
        ((1.0, 0.0), 0.0),
        ((8.0, 0.0), 0.0),
        ((10.0, 1.0), 0.0),
        ((10.0, 4.0), 0.0),
        ((9.0, 6.0), 0.0),
        ((2.0, 6.0), 0.0),
        ((0.0, 5.0), 0.0),
        ((0.0, 2.0), 0.0),
    ];
    let from_top_left: [((f64, f64), f64); 8] = [
        ((1.0, 6.0), 0.0),
        ((8.0, 6.0), 0.0),
        ((10.0, 5.0), 0.0),
        ((10.0, 2.0), 0.0),
        ((9.0, 0.0), 0.0),
        ((2.0, 0.0), 0.0),
        ((0.0, 1.0), 0.0),
        ((0.0, 4.0), 0.0),
    ];
    let as_points = |list: &[((f64, f64), f64)]| {
        list.iter()
            .map(|&((x, y), b)| (p(x, y), b))
            .collect::<Vec<_>>()
    };
    for (first, other, expected) in [
        ((0.0, 0.0), (10.0, 6.0), &from_bottom_left),
        ((10.0, 6.0), (0.0, 0.0), &from_bottom_left),
        ((0.0, 6.0), (10.0, 0.0), &from_top_left),
        ((10.0, 0.0), (0.0, 6.0), &from_top_left),
    ] {
        let r =
            Rectangle::from_corners(p(first.0, first.1), p(other.0, other.1), 0.0, TOL).unwrap();
        let v = r.vertices(Corners::Chamfer(1.0, 2.0), TOL).unwrap();
        assert_eq!(
            shape(&v),
            shape(&as_points(expected)),
            "from {first:?}: {v:?}"
        );
        assert!(
            (polyline_area(&v) - 56.0).abs() < 1e-12,
            "counterclockwise, 60 less 4"
        );
        assert_eq!(r.area_with(Corners::Chamfer(1.0, 2.0)), 56.0);
    }
    // From (0,0), vertex for vertex as ProgeCAD's: from where the first
    // corner's cut ends.
    let r = Rectangle::from_corners(p(0.0, 0.0), p(10.0, 6.0), 0.0, TOL).unwrap();
    same_vertices(
        &r.vertices(Corners::Chamfer(1.0, 2.0), TOL).unwrap(),
        &from_bottom_left,
    );
    // Turned a right angle, the walk leaves the first corner along the
    // length, now running up: the first distance is cut on it.
    let r = Rectangle::from_corners(p(0.0, 0.0), p(-2.0, 4.0), FRAC_PI_2, TOL).unwrap();
    let v = r.vertices(Corners::Chamfer(1.0, 0.5), TOL).unwrap();
    assert!(v.iter().any(|(q, _)| near(*q, p(0.0, 1.0))), "{v:?}");
    assert!(v.iter().any(|(q, _)| near(*q, p(-0.5, 0.0))), "{v:?}");
    assert!((polyline_area(&v) - 7.0).abs() < 1e-12);
}

#[test]
fn every_side_loses_one_chamfer_distance_of_each_kind() {
    // 10 by 6: each side loses one distance of each kind. 1 + 2 and 3 + 3
    // fit the width of 6; 4 + 3 does not, though twice 3 did before.
    let r = Rectangle::from_corners(p(0.0, 0.0), p(10.0, 6.0), 0.0, TOL).unwrap();
    assert!(r.fits(Corners::Chamfer(1.0, 2.0), TOL));
    assert!(r.fits(Corners::Chamfer(3.0, 3.0), TOL));
    assert!(!r.fits(Corners::Chamfer(4.0, 3.0), TOL));
    assert!(!r.fits(Corners::Chamfer(3.0, 4.0), TOL));
    assert!(r.vertices(Corners::Chamfer(4.0, 3.0), TOL).is_err());
}

#[test]
fn corners_that_take_a_whole_side_leave_no_segment_there() {
    // A square with fillets of half its side: a circle of four arcs.
    let r = Rectangle::from_corners(p(0.0, 0.0), p(2.0, 2.0), 0.0, TOL).unwrap();
    let v = r.vertices(Corners::Fillet(1.0), TOL).unwrap();
    same_vertices(
        &v,
        &[
            ((1.0, 0.0), Q),
            ((2.0, 1.0), Q),
            ((1.0, 2.0), Q),
            ((0.0, 1.0), Q),
        ],
    );
    assert!((polyline_area(&v) - PI).abs() < 1e-12);
    // The short sides only: a slot of two arcs and two straight sides.
    let r = Rectangle::from_corners(p(0.0, 0.0), p(4.0, 2.0), 0.0, TOL).unwrap();
    let v = r.vertices(Corners::Fillet(1.0), TOL).unwrap();
    assert_eq!(v.len(), 6, "{v:?}");
    assert!((polyline_area(&v) - (8.0 - (4.0 - PI))).abs() < 1e-12);
    // Chamfers meeting in the middle of every side: a diamond. Each side
    // loses one distance of each kind, so it takes a square and equal ones.
    let square = Rectangle::from_corners(p(0.0, 0.0), p(2.0, 2.0), 0.0, TOL).unwrap();
    let v = square.vertices(Corners::Chamfer(1.0, 1.0), TOL).unwrap();
    same_vertices(
        &v,
        &[
            ((1.0, 0.0), 0.0),
            ((2.0, 1.0), 0.0),
            ((1.0, 2.0), 0.0),
            ((0.0, 1.0), 0.0),
        ],
    );
    assert!(
        r.vertices(Corners::Chamfer(2.0, 1.0), TOL).is_err(),
        "3 does not fit a side of 2"
    );
}

#[test]
fn every_way_round_and_turned_the_corners_bulge_outward() {
    let rotations = [0.0, FRAC_PI_6, FRAC_PI_2, PI, -2.5];
    let others = [p(5.0, 3.0), p(-5.0, 3.0), p(-5.0, -3.0), p(5.0, -3.0)];
    for rotation in rotations {
        for other in others {
            let Ok(r) = Rectangle::from_corners(p(10.0, 20.0), other, rotation, TOL) else {
                continue;
            };
            for corners in [Corners::Fillet(0.3), Corners::Chamfer(0.4, 0.2)] {
                if !r.fits(corners, TOL) {
                    continue;
                }
                let v = r.vertices(corners, TOL).unwrap();
                let area = polyline_area(&v);
                assert!(
                    (area - r.area_with(corners)).abs() < 1e-9,
                    "{rotation} {other:?} {corners:?}: {area} != {}",
                    r.area_with(corners)
                );
                assert!(v.iter().all(|(_, b)| *b >= 0.0), "counterclockwise arcs");
            }
        }
    }
}

#[test]
fn corners_too_large_or_invalid_are_refused() {
    let r = Rectangle::from_corners(p(0.0, 0.0), p(4.0, 2.0), 0.0, TOL).unwrap();
    assert!(!r.fits(Corners::Fillet(1.01), TOL));
    assert_eq!(
        r.vertices(Corners::Fillet(1.01), TOL),
        Err(GeometryError::OutOfRange { what: "corner" })
    );
    assert_eq!(
        r.vertices(Corners::Chamfer(2.5, 0.5), TOL),
        Err(GeometryError::OutOfRange { what: "corner" })
    );
    assert_eq!(
        r.vertices(Corners::Fillet(-1.0), TOL),
        Err(GeometryError::NegativeOrNonFinite {
            what: "fillet radius"
        })
    );
    assert_eq!(
        r.vertices(Corners::Chamfer(1.0, f64::NAN), TOL),
        Err(GeometryError::NegativeOrNonFinite {
            what: "chamfer distance"
        })
    );
    // Zero cuts nothing: square corners, as Square draws them.
    let square = r.vertices(Corners::Square, TOL).unwrap();
    assert_eq!(r.vertices(Corners::Fillet(0.0), TOL).unwrap(), square);
    assert_eq!(r.vertices(Corners::Chamfer(1.0, 0.0), TOL).unwrap(), square);
    assert_eq!(square.len(), 4);
    assert_eq!(Corners::Chamfer(0.0, 3.0).area_lost(), 0.0);
}

#[test]
fn area_counts_what_the_corners_take() {
    for corners in [
        Corners::Square,
        Corners::Fillet(0.5),
        Corners::Chamfer(1.0, 0.5),
    ] {
        let r = Rectangle::from_area(p(0.0, 0.0), 12.0, 4.0, true, 0.0, corners, TOL).unwrap();
        assert_eq!(r.length(), 4.0);
        assert!((r.area_with(corners) - 12.0).abs() < 1e-12, "{corners:?}");
        let v = r.vertices(corners, TOL).unwrap();
        assert!((polyline_area(&v) - 12.0).abs() < 1e-12, "{corners:?}");
    }
    assert_eq!(
        Rectangle::from_area(
            p(0.0, 0.0),
            12.0,
            4.0,
            true,
            0.0,
            Corners::Fillet(-1.0),
            TOL
        ),
        Err(GeometryError::NegativeOrNonFinite {
            what: "fillet radius"
        })
    );
}
