use super::*;
use crate::spline_fit::{SplineFit, SplineKnots};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn wave() -> Vec<Point2> {
    vec![
        p(0.0, 0.0),
        p(3.0, 4.0),
        p(7.0, 1.0),
        p(9.0, 6.0),
        p(14.0, 5.0),
        p(16.0, -2.0),
    ]
}

/// A rational quadratic quarter of the circle of radius `r` about the
/// origin, from (r,0) to (0,r).
fn quarter(r: f64) -> Spline {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    Spline::new(
        2,
        vec![p(r, 0.0), p(r, r), p(0.0, r)],
        vec![1.0, w, 1.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        false,
    )
    .unwrap()
}

fn cases() -> Vec<Spline> {
    vec![
        Spline::by_vertices(3, wave(), false).unwrap(),
        Spline::by_vertices(2, wave(), true).unwrap(),
        Spline::by_vertices(5, wave(), false).unwrap(),
        Spline::by_vertices(1, wave(), false).unwrap(),
        SplineFit::new(wave(), SplineKnots::Chord, None, None, false)
            .unwrap()
            .spline()
            .unwrap(),
        quarter(3.0),
        // Non-uniform and rational, with a double inner knot.
        Spline::new(
            3,
            wave(),
            vec![1.0, 2.0, 0.5, 1.0, 3.0, 1.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.4, 0.4, 1.0, 1.0, 1.0, 1.0],
            false,
        )
        .unwrap(),
    ]
}

#[test]
fn the_derivative_is_the_curve_s_slope() {
    for s in cases() {
        let (a, b) = s.domain();
        let h = 1e-6 * (b - a);
        for k in 1..200 {
            let u = a + (b - a) * k as f64 / 200.0;
            // Away from inner knots, where a curve of low continuity bends.
            if s.knots().iter().any(|&t| (t - u).abs() < 2.0 * h) {
                continue;
            }
            let numeric = (s.point_at(u + h) - s.point_at(u - h)) / (2.0 * h);
            let (at, exact) = s.point_and_derivative(u);
            assert!(at.distance_to(s.point_at(u)) < 1e-12 * (1.0 + at.x().abs() + at.y().abs()));
            let error = (exact - numeric).length();
            assert!(
                error < 1e-5 * (1.0 + exact.length()),
                "{u}: {exact:?} against {numeric:?}"
            );
        }
    }
}

#[test]
fn the_derivative_at_the_ends_is_along_the_end_legs() {
    // A clamped curve leaves along its first leg, at its degree over the
    // knot gap times it.
    let s = Spline::by_vertices(3, wave(), false).unwrap();
    let k = s.knots();
    let first = (wave()[1] - wave()[0]) * (3.0 / (k[4] - k[1]));
    let last = (wave()[5] - wave()[4]) * (3.0 / (k[k.len() - 2] - k[5]));
    let (a, b) = s.domain();
    assert!((s.derivative_at(a) - first).length() < 1e-12);
    assert!((s.derivative_at(b) - last).length() < 1e-12);
}

#[test]
fn a_rational_quarter_circle_is_a_quarter_of_its_circumference() {
    for r in [0.5, 3.0, 1000.0] {
        let s = quarter(r);
        let expected = std::f64::consts::FRAC_PI_2 * r;
        assert!((s.length() - expected).abs() < 1e-11 * expected, "{r}");
        // Its speed may vary but its direction is the circle's tangent.
        for k in 0..=10 {
            let u = k as f64 / 10.0;
            let (q, d) = s.point_and_derivative(u);
            assert!((q - Point2::ORIGIN).dot(d).abs() < 1e-9 * r * d.length());
        }
    }
}

#[test]
fn a_spline_of_degree_one_is_as_long_as_its_legs() {
    let s = Spline::by_vertices(1, wave(), false).unwrap();
    let legs: f64 = wave().windows(2).map(|w| w[0].distance_to(w[1])).sum();
    assert!((s.length() - legs).abs() < 1e-12 * legs);
}

#[test]
fn its_length_is_the_limit_of_its_chords() {
    for s in cases() {
        let (a, b) = s.domain();
        let n = 200_000;
        let chords: f64 = (0..n)
            .map(|i| {
                let (u, v) = (
                    a + (b - a) * i as f64 / n as f64,
                    a + (b - a) * (i + 1) as f64 / n as f64,
                );
                s.point_at(u).distance_to(s.point_at(v))
            })
            .sum();
        let length = s.length();
        // Chords fall short of the curve by its curvature, to 1/n².
        assert!(
            length >= chords - 1e-10 * length && length - chords < 1e-7 * length,
            "{length} {chords}"
        );
        let half = (a + b) / 2.0;
        assert!(
            (s.length_between(a, half) + s.length_between(half, b) - length).abs() < 1e-11 * length
        );
        assert_eq!(s.length_between(b, a), 0.0);
    }
}

#[test]
fn the_point_at_a_distance_is_that_far_along() {
    for s in cases() {
        let length = s.length();
        let (a, _) = s.domain();
        for k in 0..=20 {
            let d = length * k as f64 / 20.0;
            let u = s.param_at_distance(d).unwrap();
            assert!((s.length_between(a, u) - d).abs() < 1e-10 * length, "{d}");
            assert_eq!(s.point_at_distance(d).unwrap(), s.point_at(u));
        }
        let (start, end) = s.ends();
        assert!(s.point_at_distance(0.0).unwrap().distance_to(start) < 1e-12 * length);
        assert!(s.point_at_distance(length).unwrap().distance_to(end) < 1e-9 * length);
        let past = GeometryError::OutOfRange { what: "distance" };
        assert_eq!(s.param_at_distance(length * 1.001), Err(past.clone()));
        assert_eq!(s.param_at_distance(-1.0), Err(past.clone()));
        assert_eq!(s.param_at_distance(f64::NAN), Err(past));
    }
}

#[test]
fn its_spans_break_at_its_distinct_inner_knots() {
    let s = Spline::new(
        3,
        wave(),
        Vec::new(),
        vec![0.0, 0.0, 0.0, 0.0, 0.4, 0.4, 1.0, 1.0, 1.0, 1.0],
        false,
    )
    .unwrap();
    assert_eq!(s.span_breaks(), [0.0, 0.4, 1.0]);
    let closed = Spline::by_vertices(2, wave(), true).unwrap();
    assert_eq!(closed.span_breaks(), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
}
