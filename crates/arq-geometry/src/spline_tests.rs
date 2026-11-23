use std::f64::consts::FRAC_1_SQRT_2;

use super::*;
use crate::vector::Vector2;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.distance_to(b) < 1e-9
}

/// The quadratic Bézier (0,0), (1,2), (2,0) as a spline.
fn bezier() -> Spline {
    let points = vec![p(0.0, 0.0), p(1.0, 2.0), p(2.0, 0.0)];
    Spline::new(2, points, vec![], vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], false).unwrap()
}

/// A quarter of the unit circle, from (1,0) to (0,1), as a rational
/// quadratic.
fn quarter() -> Spline {
    let points = vec![p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
    let weights = vec![1.0, FRAC_1_SQRT_2, 1.0];
    Spline::new(
        2,
        points,
        weights,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        false,
    )
    .unwrap()
}

#[test]
fn its_points_are_de_boors() {
    let b = bezier();
    // (1-t)²·P0 + 2t(1-t)·P1 + t²·P2.
    for t in [0.0, 0.25, 0.5, 0.8, 1.0] {
        let q = b.point_at(t);
        let expected = p(2.0 * t, 4.0 * t * (1.0 - t));
        assert!(near(q, expected), "{t}: {q:?}");
    }
    assert_eq!(b.ends(), (p(0.0, 0.0), p(2.0, 0.0)));
    assert!(b.weights().is_empty(), "weights all one: not rational");
    // A rational one draws a true circle.
    let q = quarter();
    for t in [0.1, 0.3, 0.5, 0.9] {
        assert!((q.point_at(t).distance_to(Point2::ORIGIN) - 1.0).abs() < 1e-12);
    }
    // Several spans: a cubic with an inner knot is continuous across it.
    let points = vec![
        p(0.0, 0.0),
        p(1.0, 3.0),
        p(3.0, 3.0),
        p(4.0, 0.0),
        p(6.0, 1.0),
    ];
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0];
    let c = Spline::new(3, points, vec![], knots, false).unwrap();
    assert!(near(c.point_at(0.4 - 1e-13), c.point_at(0.4)));
    assert_eq!(c.ends(), (p(0.0, 0.0), p(6.0, 1.0)));
}

#[test]
fn it_is_drawn_by_sixteen_segments_a_span() {
    let b = bezier();
    assert_eq!(b.flatten().len(), 17);
    let points = vec![
        p(0.0, 0.0),
        p(1.0, 3.0),
        p(3.0, 3.0),
        p(4.0, 0.0),
        p(6.0, 1.0),
    ];
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0];
    let c = Spline::new(3, points, vec![], knots, false).unwrap();
    assert_eq!(c.flatten().len(), 33, "two spans, the shared point once");
    // Closed: back to its start.
    let closed = Spline::new(
        2,
        vec![p(0.0, 0.0), p(1.0, 2.0), p(2.0, 0.0)],
        vec![],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        true,
    )
    .unwrap();
    let drawn = closed.flatten();
    assert_eq!((drawn.len(), drawn[drawn.len() - 1]), (18, p(0.0, 0.0)));
}

#[test]
fn its_nearest_point_box_and_image() {
    let q = quarter();
    let nearest = q.closest_point(p(3.0, 3.0));
    assert!(
        near(nearest, p(FRAC_1_SQRT_2, FRAC_1_SQRT_2)),
        "{nearest:?}"
    );
    assert!((q.distance_to(p(2.0, 0.0)) - 1.0).abs() < 1e-9);
    assert!(q.contains_point(p(0.6, 0.8), Tolerance::DEFAULT));
    // The Bézier peaks at (1,1), above its points' box? No: below its
    // middle control point, exactly at y = 1.
    let b = bezier().bounds();
    assert!(
        (b.max().y() - 1.0).abs() < 1e-12 && b.min().y() == 0.0,
        "{b:?}"
    );
    assert_eq!((b.min().x(), b.max().x()), (0.0, 2.0));
    // A peak between two drawn points, at t = 4/7 of (0,0), (1,4), (5,1):
    // y = 8t - 7t², 16/7 at most; and x = 2t(1-t) + 5t², least at 0.
    let peak = Spline::new(
        2,
        vec![p(0.0, 0.0), p(1.0, 4.0), p(5.0, 1.0)],
        vec![],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        false,
    )
    .unwrap()
    .bounds();
    assert!((peak.max().y() - 16.0 / 7.0).abs() < 1e-12, "{peak:?}");
    // A dip inside: (0,3), (2,-3), (5,1): y' = 20t - 12, least at t = 0.6,
    // between two drawn points.
    let dip = Spline::new(
        2,
        vec![p(0.0, 3.0), p(2.0, -3.0), p(5.0, 1.0)],
        vec![],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        false,
    )
    .unwrap();
    let low = (0..=100_000)
        .map(|i| dip.point_at(f64::from(i) / 100_000.0).y())
        .fold(f64::INFINITY, f64::min);
    assert!((dip.bounds().min().y() - low).abs() < 1e-9, "{low}");
    // Two peaks whose drawn heights are the other way round from their own:
    // the higher drawn, 1.31994, is not the higher curve, 1.32264.
    let twin = Spline::new(
        2,
        vec![
            p(0.0, 0.0),
            p(1.0, 2.0),
            p(2.0, 0.0),
            p(2.4569675167470146, 2.0169428452163323),
            p(4.0, 0.0),
        ],
        vec![],
        vec![0.0, 0.0, 0.0, 1.0 / 3.0, 0.6500396889414803, 1.0, 1.0, 1.0],
        false,
    )
    .unwrap();
    let top = (0..=200_000)
        .map(|i| twin.point_at(f64::from(i) / 200_000.0).y())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((twin.bounds().max().y() - top).abs() < 1e-9, "{top}");
    // A turn about the origin carries every point.
    let t = Transform2::rotation(Point2::ORIGIN, 0.7).unwrap();
    let turned = q.transformed(t).unwrap();
    for u in [0.0, 0.3, 1.0] {
        assert!(near(turned.point_at(u), t.apply_point(q.point_at(u))));
    }
    let moved = bezier().transformed(Transform2::translation(Vector2::new(5.0, 1.0).unwrap()));
    assert_eq!(moved.unwrap().ends(), (p(5.0, 1.0), p(7.0, 1.0)));
}

#[test]
fn bad_splines_are_refused() {
    let three = vec![p(0.0, 0.0), p(1.0, 2.0), p(2.0, 0.0)];
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    assert!(Spline::new(0, three.clone(), vec![], knots.clone(), false).is_err());
    assert!(Spline::new(3, three.clone(), vec![], knots.clone(), false).is_err());
    let seven = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    assert_eq!(
        Spline::new(3, three.clone(), vec![], seven, false),
        Err(GeometryError::OutOfRange {
            what: "spline degree"
        }),
        "a degree of its count, refused as such"
    );
    assert!(Spline::new(2, three.clone(), vec![], knots[1..].to_vec(), false).is_err());
    let down = vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0];
    assert!(Spline::new(2, three.clone(), vec![], down, false).is_err());
    assert!(Spline::new(2, three.clone(), vec![1.0, -1.0, 1.0], knots.clone(), false).is_err());
    assert!(Spline::new(2, three.clone(), vec![1.0, 1.0], knots, false).is_err());
    assert!(Spline::new(1, vec![p(0.0, 0.0)], vec![], vec![0.0, 1.0, 2.0], false).is_err());
    assert_eq!(
        Spline::clamped_knots(3, 5),
        [0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]
    );
}

#[test]
fn a_closed_spline_s_nearest_point_is_found_across_where_it_closes() {
    // Just before its end, which is its start: the nearest sample is the
    // first, the nearest point on the other side.
    let s = Spline::by_vertices(
        2,
        vec![p(0.0, 0.0), p(6.0, 1.0), p(8.0, 5.0), p(3.0, 7.0)],
        true,
    )
    .unwrap();
    let (_, b) = s.domain();
    for back in [1e-9, 1e-6, 1e-3] {
        let q = s.point_at(b - back);
        assert!(s.distance_to(q) < 1e-12, "{back}: {}", s.distance_to(q));
    }
}

#[test]
fn two_splines_are_equal_by_their_data_not_by_what_they_computed() {
    let points = vec![p(0.0, 0.0), p(1.0, 3.0), p(3.5, 3.2), p(4.0, 0.5)];
    let knots = Spline::clamped_knots(3, 4);
    let make = |w: Vec<f64>, closed: bool| {
        Spline::new(3, points.clone(), w, knots.clone(), closed).unwrap()
    };
    let a = make(vec![1.0, 2.0, 1.0, 1.0], false);
    // One whose drawing and box were asked for is still equal.
    let b = make(vec![1.0, 2.0, 1.0, 1.0], false);
    let _ = (b.flatten(), b.bounds());
    assert_eq!(a, b);
    assert_ne!(a, make(vec![1.0, 2.5, 1.0, 1.0], false), "weights");
    assert_ne!(a, make(vec![1.0, 2.0, 1.0, 1.0], true), "closed");
    let other = Spline::new(
        3,
        points.clone(),
        vec![1.0, 2.0, 1.0, 1.0],
        vec![0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0],
        false,
    )
    .unwrap();
    assert_ne!(a, other, "knots");
}

#[test]
fn its_own_vertices_leave_out_what_a_closed_one_repeats() {
    let points = vec![
        p(0.0, 0.0),
        p(3.0, 4.0),
        p(7.0, 1.0),
        p(9.0, 6.0),
        p(14.0, 5.0),
    ];
    let open = Spline::by_vertices(3, points.clone(), false).unwrap();
    assert_eq!(open.own_vertices(), points.as_slice());
    let periodic = Spline::by_vertices(3, points.clone(), true).unwrap();
    assert_eq!(
        periodic.own_vertices(),
        points.as_slice(),
        "its three repeated left out"
    );
    // Clamped round to its first point: the last left out.
    let mut round = points.clone();
    round.push(points[0]);
    let clamped = Spline::new(3, round, Vec::new(), Spline::clamped_knots(3, 6), true).unwrap();
    assert_eq!(clamped.own_vertices(), points.as_slice());
    // A vertex moved: the copies with it; past its own, refused.
    let moved = periodic.with_vertex(1, p(3.0, 5.0)).unwrap();
    assert_eq!(moved.points()[1], p(3.0, 5.0));
    assert_eq!(moved.points()[6], p(3.0, 5.0), "its copy");
    assert_eq!(moved.knots(), periodic.knots());
    let moved = clamped.with_vertex(0, p(1.0, 1.0)).unwrap();
    assert_eq!(
        (moved.points()[0], moved.points()[5]),
        (p(1.0, 1.0), p(1.0, 1.0))
    );
    assert_eq!(
        open.with_vertex(5, p(0.0, 0.0)),
        Err(GeometryError::OutOfRange { what: "vertex" })
    );
}
