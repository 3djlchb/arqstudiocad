use super::*;

const TOL: Tolerance = Tolerance::DEFAULT;
/// tan(π/8), a quarter circle's bulge.
const Q: f64 = 0.414_213_562_373_095_03;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn path(points: &[(f64, f64, f64)]) -> Vec<(Point2, f64)> {
    points.iter().map(|&(x, y, b)| (p(x, y), b)).collect()
}

fn same(got: &[(Point2, f64)], expected: &[(f64, f64, f64)]) {
    assert_eq!(got.len(), expected.len(), "{got:?}");
    for (&(q, b), &(x, y, e)) in got.iter().zip(expected) {
        assert!(
            q.approx_eq(p(x, y), Tolerance::new(1e-9, 1e-12).unwrap()) && (b - e).abs() < 1e-9,
            "{got:?} != {expected:?}"
        );
    }
}

/// The signed area of a closed polyline with bulges.
fn enclosed(v: &[(Point2, f64)]) -> f64 {
    let path = segments(v, true, TOL).unwrap();
    area(&path)
}

const ELL: [(f64, f64, f64); 3] = [(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)];
const BOX: [(f64, f64, f64); 4] = [
    (0.0, 0.0, 0.0),
    (4.0, 0.0, 0.0),
    (4.0, 2.0, 0.0),
    (0.0, 2.0, 0.0),
];

#[test]
fn an_open_corner_is_trimmed_inside_and_extended_outside() {
    let v = path(&ELL);
    let inside = offset_polyline(&v, false, 1.0, p(5.0, 4.0), TOL).unwrap();
    same(
        &inside,
        &[(0.0, 1.0, 0.0), (9.0, 1.0, 0.0), (9.0, 10.0, 0.0)],
    );
    let outside = offset_polyline(&v, false, 1.0, p(5.0, -3.0), TOL).unwrap();
    same(
        &outside,
        &[(0.0, -1.0, 0.0), (11.0, -1.0, 0.0), (11.0, 10.0, 0.0)],
    );
}

#[test]
fn a_closed_rectangle_grows_outside_and_shrinks_inside() {
    let v = path(&BOX);
    let out = offset_polyline(&v, true, 0.5, p(2.0, -3.0), TOL).unwrap();
    same(
        &out,
        &[
            (-0.5, -0.5, 0.0),
            (4.5, -0.5, 0.0),
            (4.5, 2.5, 0.0),
            (-0.5, 2.5, 0.0),
        ],
    );
    let inner = offset_polyline(&v, true, 0.5, p(2.0, 1.0), TOL).unwrap();
    same(
        &inner,
        &[
            (0.5, 0.5, 0.0),
            (3.5, 0.5, 0.0),
            (3.5, 1.5, 0.0),
            (0.5, 1.5, 0.0),
        ],
    );
    // Clockwise, the inside is to its right: the same rectangle, reversed.
    let clockwise: Vec<_> = v.iter().rev().copied().collect();
    let inner = offset_polyline(&clockwise, true, 0.5, p(2.0, 1.0), TOL).unwrap();
    assert!(
        (enclosed(&inner) + 3.0).abs() < 1e-9,
        "3 × 1, still clockwise"
    );
}

#[test]
fn inward_beyond_its_middle_nothing_is_left() {
    let v = path(&BOX);
    for d in [1.0, 1.5, 3.0] {
        assert_eq!(
            offset_polyline(&v, true, d, p(2.0, 1.0), TOL),
            Err(GeometryError::OutOfRange {
                what: "offset distance"
            }),
            "{d}"
        );
    }
}

#[test]
fn rounded_corners_stay_round_outside_and_go_sharp_past_their_radius() {
    // A 10 × 6 rectangle with fillets of radius 2, counterclockwise.
    let v = path(&[
        (2.0, 0.0, 0.0),
        (8.0, 0.0, Q),
        (10.0, 2.0, 0.0),
        (10.0, 4.0, Q),
        (8.0, 6.0, 0.0),
        (2.0, 6.0, Q),
        (0.0, 4.0, 0.0),
        (0.0, 2.0, Q),
    ]);
    let out = offset_polyline(&v, true, 1.0, p(5.0, -2.0), TOL).unwrap();
    same(
        &out,
        &[
            (2.0, -1.0, 0.0),
            (8.0, -1.0, Q),
            (11.0, 2.0, 0.0),
            (11.0, 4.0, Q),
            (8.0, 7.0, 0.0),
            (2.0, 7.0, Q),
            (-1.0, 4.0, 0.0),
            (-1.0, 2.0, Q),
        ],
    );
    // Inward by 1: the fillets narrow to radius 1.
    let inner = offset_polyline(&v, true, 1.0, p(5.0, 3.0), TOL).unwrap();
    assert_eq!(inner.len(), 8);
    // Inward by 2.5, past the fillets: they vanish and the sides meet.
    let sharp = offset_polyline(&v, true, 2.5, p(5.0, 3.0), TOL).unwrap();
    same(
        &sharp,
        &[
            (2.5, 2.5, 0.0),
            (7.5, 2.5, 0.0),
            (7.5, 3.5, 0.0),
            (2.5, 3.5, 0.0),
        ],
    );
}

#[test]
fn an_arc_widens_outside_and_narrows_inside() {
    // A half circle about (1, 0), radius 1, through (1, -1).
    let v = path(&[(0.0, 0.0, 1.0), (2.0, 0.0, 0.0)]);
    let out = offset_polyline(&v, false, 1.0, p(1.0, -5.0), TOL).unwrap();
    same(&out, &[(-1.0, 0.0, 1.0), (3.0, 0.0, 0.0)]);
    let inner = offset_polyline(&v, false, 0.5, p(1.0, -0.2), TOL).unwrap();
    same(&inner, &[(0.5, 0.0, 1.0), (1.5, 0.0, 0.0)]);
    assert_eq!(
        offset_polyline(&v, false, 1.0, p(1.0, -0.2), TOL),
        Err(GeometryError::OutOfRange {
            what: "offset distance"
        }),
        "it would shrink to nothing"
    );
}

#[test]
fn a_line_meets_an_arc_that_is_not_tangent_at_their_crossing() {
    // Along x to (4, 0), then a quarter circle up and left about (4, 2)…
    // whose start is not tangent: about (2, 0) instead, radius 2.
    let v = path(&[(0.0, 0.0, 0.0), (4.0, 0.0, Q), (2.0, 2.0, 0.0)]);
    let out = offset_polyline(&v, false, 0.5, p(2.0, 1.0), TOL).unwrap();
    // The line, moved up, meets the narrower circle (radius 1.5 about
    // (2, 0)) where y = 0.5.
    let x = 2.0 + (1.5f64.powi(2) - 0.25).sqrt();
    same(
        &out[..2]
            .iter()
            .map(|&(q, b)| (q, if b == 0.0 { 0.0 } else { 1.0 }))
            .collect::<Vec<_>>(),
        &[(0.0, 0.5, 0.0), (x, 0.5, 1.0)],
    );
}

#[test]
fn through_a_point_takes_its_distance_from_the_nearest_segment() {
    let v = path(&ELL);
    let through = offset_polyline_through(&v, false, p(5.0, 3.0), TOL).unwrap();
    same(
        &through,
        &[(0.0, 3.0, 0.0), (7.0, 3.0, 0.0), (7.0, 10.0, 0.0)],
    );
    assert_eq!(
        offset_polyline_through(&v, false, p(5.0, 0.0), TOL),
        Err(GeometryError::OutOfRange { what: "side" })
    );
}

#[test]
fn no_side_and_bad_input_are_refused() {
    let v = path(&ELL);
    assert_eq!(
        offset_polyline(&v, false, 1.0, p(10.0, 5.0), TOL),
        Err(GeometryError::OutOfRange { what: "side" }),
        "on the path"
    );
    for d in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            offset_polyline(&v, false, d, p(5.0, 5.0), TOL),
            Err(GeometryError::NegativeOrNonFinite {
                what: "offset distance"
            })
        );
    }
    let dot = path(&[(1.0, 1.0, 0.0), (1.0, 1.0, 0.0)]);
    assert_eq!(
        offset_polyline(&dot, false, 1.0, p(5.0, 5.0), TOL),
        Err(GeometryError::ZeroLength)
    );
}

#[test]
fn survey_coordinates_keep_the_distance() {
    let (x, y) = (500_000.0, 9_800_000.0);
    let v = path(&[(x, y, 0.0), (x + 10.0, y, 0.0), (x + 10.0, y + 10.0, 0.0)]);
    let out = offset_polyline(&v, false, 0.15, p(x + 5.0, y + 5.0), TOL).unwrap();
    assert!((out[1].0.x() - (x + 9.85)).abs() < 1e-8 && (out[1].0.y() - (y + 0.15)).abs() < 1e-8);
}
