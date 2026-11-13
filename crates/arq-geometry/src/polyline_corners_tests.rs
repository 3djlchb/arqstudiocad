use std::f64::consts::FRAC_PI_4;

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

fn square() -> Vec<(Point2, f64)> {
    path(&[
        (0.0, 0.0, 0.0),
        (10.0, 0.0, 0.0),
        (10.0, 10.0, 0.0),
        (0.0, 10.0, 0.0),
    ])
}

#[test]
fn every_corner_of_a_closed_polyline_is_rounded() {
    let cut = cut_polyline_corners(&square(), true, CornerCut::Fillet(2.0), None, TOL).unwrap();
    assert_eq!((cut.done, cut.too_short), (4, 0));
    same(
        &cut.vertices,
        &[
            (0.0, 2.0, Q),
            (2.0, 0.0, 0.0),
            (8.0, 0.0, Q),
            (10.0, 2.0, 0.0),
            (10.0, 8.0, Q),
            (8.0, 10.0, 0.0),
            (2.0, 10.0, Q),
            (0.0, 8.0, 0.0),
        ],
    );
    // Clockwise, the arcs turn the other way.
    let clockwise: Vec<_> = square().into_iter().rev().collect();
    let cut = cut_polyline_corners(&clockwise, true, CornerCut::Fillet(2.0), None, TOL).unwrap();
    assert!(cut.vertices.iter().all(|&(_, b)| b <= 0.0) && cut.done == 4);
}

#[test]
fn an_open_polyline_has_corners_only_between_its_ends() {
    let ell = path(&[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)]);
    let cut = cut_polyline_corners(&ell, false, CornerCut::Chamfer(1.0, 2.0), None, TOL).unwrap();
    assert_eq!(cut.done, 1);
    same(
        &cut.vertices,
        &[
            (0.0, 0.0, 0.0),
            (9.0, 0.0, 0.0),
            (10.0, 2.0, 0.0),
            (10.0, 10.0, 0.0),
        ],
    );
    assert!(cut_polyline_corners(&ell, false, CornerCut::Fillet(1.0), Some(0), TOL).is_err());
}

#[test]
fn corners_too_large_for_their_segments_are_left_and_counted() {
    let cut = cut_polyline_corners(&square(), true, CornerCut::Fillet(6.0), None, TOL).unwrap();
    assert_eq!(
        (cut.done, cut.too_short),
        (2, 2),
        "each takes 6 of a side 10 long"
    );
    assert_eq!(cut.vertices.len(), 6);
}

#[test]
fn one_corner_alone_and_the_angle_method() {
    let cut = cut_polyline_corners(&square(), true, CornerCut::Fillet(2.0), Some(1), TOL).unwrap();
    assert_eq!(cut.done, 1);
    same(
        &cut.vertices,
        &[
            (0.0, 0.0, 0.0),
            (8.0, 0.0, Q),
            (10.0, 2.0, 0.0),
            (10.0, 10.0, 0.0),
            (0.0, 10.0, 0.0),
        ],
    );
    let cut = cut_polyline_corners(
        &square(),
        true,
        CornerCut::ChamferAngle(2.0, FRAC_PI_4),
        Some(1),
        TOL,
    )
    .unwrap();
    same(
        &cut.vertices,
        &[
            (0.0, 0.0, 0.0),
            (8.0, 0.0, 0.0),
            (10.0, 2.0, 0.0),
            (10.0, 10.0, 0.0),
            (0.0, 10.0, 0.0),
        ],
    );
}

#[test]
fn arcs_and_straight_runs_are_left_alone() {
    // A corner next to an arc, and one in line.
    let v = path(&[
        (0.0, 0.0, 0.0),
        (10.0, 0.0, Q),
        (12.0, 2.0, 0.0),
        (12.0, 6.0, 0.0),
        (12.0, 10.0, 0.0),
    ]);
    let cut = cut_polyline_corners(&v, false, CornerCut::Fillet(1.0), None, TOL).unwrap();
    assert_eq!((cut.done, cut.too_short), (0, 0));
    assert_eq!(cut.vertices, v);
    assert!(cut_polyline_corners(&v, false, CornerCut::Fillet(-1.0), None, TOL).is_err());
}

#[test]
fn the_vertex_two_segments_share() {
    assert_eq!(shared_vertex(0, 1, 4, true), Some(1));
    assert_eq!(shared_vertex(2, 1, 4, true), Some(2));
    assert_eq!(shared_vertex(3, 0, 4, true), Some(0), "round the start");
    assert_eq!(shared_vertex(3, 0, 4, false), None);
    assert_eq!(shared_vertex(0, 2, 4, true), None);
}
