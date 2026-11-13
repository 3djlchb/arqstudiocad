use std::f64::consts::PI;

use super::*;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64, bulge: f64, w0: f64, w1: f64) -> BandVertex {
    BandVertex {
        point: p(x, y),
        bulge,
        start_width: w0,
        end_width: w1,
    }
}

/// The area the triangles cover, counting each once (they never overlap
/// in these shapes).
fn area(triangles: &[[Point2; 3]]) -> f64 {
    triangles.iter().map(|&t| area2(t).abs() / 2.0).sum()
}

fn inside(triangles: &[[Point2; 3]], q: Point2) -> bool {
    triangles.iter().any(|&[a, b, c]| {
        let s = |u: Point2, w: Point2| (w - u).cross(q - u);
        let (d1, d2, d3) = (s(a, b), s(b, c), s(c, a));
        (d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0) || (d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0)
    })
}

#[test]
fn a_straight_segment_is_a_rectangle_its_width_across() {
    let t = band(
        &[v(0.0, 0.0, 0.0, 1.0, 1.0), v(4.0, 0.0, 0.0, 0.0, 0.0)],
        false,
        TOL,
    );
    assert_eq!(t.len(), 2);
    assert!((area(&t) - 4.0).abs() < 1e-12);
    assert!(inside(&t, p(2.0, 0.49)) && inside(&t, p(0.01, -0.49)));
    assert!(
        !inside(&t, p(2.0, 0.51)) && !inside(&t, p(-0.01, 0.0)),
        "butt ends"
    );
    assert!(t.iter().all(|&tri| area2(tri) > 0.0), "counterclockwise");
}

#[test]
fn a_tapered_segment_grows_from_its_start_width_to_its_end_width() {
    let t = band(
        &[v(0.0, 0.0, 0.0, 0.0, 2.0), v(4.0, 0.0, 0.0, 0.0, 0.0)],
        false,
        TOL,
    );
    assert!(
        (area(&t) - 4.0).abs() < 1e-12,
        "a triangle 4 long, 2 wide at its end"
    );
    assert!(inside(&t, p(3.9, 0.9)) && !inside(&t, p(1.0, 0.5)));
}

#[test]
fn a_wide_rectangle_is_a_frame_its_corners_mitered() {
    let w = 0.5;
    let ring = [(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)];
    let vertices: Vec<_> = ring.iter().map(|&(x, y)| v(x, y, 0.0, w, w)).collect();
    let t = band(&vertices, true, TOL);
    // The frame between 4.5 x 2.5 and 3.5 x 1.5: no gap, no overlap.
    assert!(
        (area(&t) - (4.5 * 2.5 - 3.5 * 1.5)).abs() < 1e-12,
        "{}",
        area(&t)
    );
    for corner in [
        p(-0.24, -0.24),
        p(4.24, -0.24),
        p(4.24, 2.24),
        p(-0.24, 2.24),
    ] {
        assert!(inside(&t, corner), "the outer corner {corner:?} is filled");
    }
    assert!(!inside(&t, p(2.0, 1.0)), "the middle stays empty");
    // Open, the first and last segments end square at the first vertex.
    let open = band(&vertices, false, TOL);
    assert!(!inside(&open, p(-0.24, -0.24)));
}

#[test]
fn an_arc_is_a_ring_counterclockwise_or_not() {
    // A half circle of radius 1 about (1, 0), 0.5 wide: π/2 (1.25² − 0.75²).
    let expected = PI / 2.0 * (1.25f64.powi(2) - 0.75f64.powi(2));
    for bulge in [1.0, -1.0] {
        let t = band(
            &[v(0.0, 0.0, bulge, 0.5, 0.5), v(2.0, 0.0, 0.0, 0.0, 0.0)],
            false,
            TOL,
        );
        assert!(
            (area(&t) - expected).abs() < 1e-3 * expected,
            "{bulge}: {}",
            area(&t)
        );
        // Bulge 1 runs counterclockwise from (0,0): below the chord.
        let below = bulge > 0.0;
        let mid = p(1.0, if below { -1.0 } else { 1.0 });
        assert!(inside(&t, mid), "{bulge}: the ring passes the arc's middle");
        assert!(!inside(&t, p(1.0, 0.0)), "{bulge}: not its center");
    }
}

#[test]
fn a_line_and_a_tangent_arc_meet_without_a_gap() {
    // A slot: straight, half circle, straight back, half circle.
    let w = 0.2;
    let path = [
        v(0.0, 0.0, 0.0, w, w),
        v(4.0, 0.0, 1.0, w, w),
        v(4.0, 2.0, 0.0, w, w),
        v(0.0, 2.0, 1.0, w, w),
    ];
    let t = band(&path, true, TOL);
    for q in [p(4.0, 0.09), p(4.0, -0.09), p(4.0, 2.09), p(0.0, 1.91)] {
        assert!(inside(&t, q), "{q:?} at a joint is filled");
    }
}

#[test]
fn nothing_is_covered_without_width() {
    let line = [v(0.0, 0.0, 0.0, 0.0, 0.0), v(4.0, 0.0, 0.0, 0.0, 0.0)];
    assert!(band(&line, false, TOL).is_empty());
    let bad = [v(0.0, 0.0, 0.0, f64::NAN, 1.0), v(4.0, 0.0, 0.0, 0.0, 0.0)];
    assert!(band(&bad, false, TOL).is_empty());
    let point = [v(1.0, 1.0, 0.0, 1.0, 1.0), v(1.0, 1.0, 0.0, 1.0, 1.0)];
    assert!(band(&point, false, TOL).is_empty(), "no length");
    assert!(band(&[], true, TOL).is_empty());
}

#[test]
fn a_sharp_turn_is_not_mitered_beyond_the_limit() {
    // Back almost on itself: a miter would reach far beyond the vertex.
    let path = [
        v(0.0, 0.0, 0.0, 0.2, 0.2),
        v(4.0, 0.0, 0.0, 0.2, 0.2),
        v(0.0, 0.05, 0.0, 0.0, 0.0),
    ];
    let t = band(&path, false, TOL);
    let far = t
        .iter()
        .flatten()
        .map(|q| q.distance_to(p(4.0, 0.0)))
        .fold(0.0, f64::max);
    assert!(far < 4.1, "nothing reaches out past the vertex: {far}");
}

#[test]
fn survey_coordinates_keep_the_width() {
    let (x, y) = (500_000.0, 9_800_000.0);
    let t = band(
        &[v(x, y, 0.0, 0.1, 0.1), v(x + 3.0, y, 0.0, 0.0, 0.0)],
        false,
        TOL,
    );
    assert!((area(&t) - 0.3).abs() < 1e-6, "{}", area(&t));
}
