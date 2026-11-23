use super::*;
use crate::transform::Transform2;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

fn wave() -> Vec<Point2> {
    vec![
        p(0.0, 0.0),
        p(3.0, 4.0),
        p(7.0, 1.0),
        p(9.0, 6.0),
        p(14.0, 5.0),
    ]
}

fn fit(points: Vec<Point2>, knots: SplineKnots) -> Spline {
    SplineFit::new(points, knots, None, None, false)
        .unwrap()
        .spline()
        .unwrap()
}

/// Gaussian elimination on a small dense system, for the reference.
fn solve_dense(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for c in 0..n {
        let r = (c..n)
            .max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))
            .unwrap();
        a.swap(c, r);
        b.swap(c, r);
        for i in c + 1..n {
            let f = a[i][c] / a[c][c];
            let pivot = a[c].clone();
            for (x, y) in a[i].iter_mut().zip(pivot).skip(c) {
                *x -= f * y;
            }
            b[i] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s: f64 = (i + 1..n).map(|k| a[i][k] * x[k]).sum();
        x[i] = (b[i] - s) / a[i][i];
    }
    x
}

/// How the reference cubic spline ends.
#[derive(Clone, Copy)]
enum End {
    Natural,
    /// Its derivative at the start and the end, either natural if `None`.
    Clamped(Option<f64>, Option<f64>),
    Periodic,
}

/// The classic cubic spline of values `y` at parameters `t` (by its
/// moments, not B-splines), evaluated at `u`. Periodic: `y` holds one
/// value fewer than `t`, the last wrapping to the first.
fn cubic(t: &[f64], y: &[f64], end: End, u: f64) -> f64 {
    let n = t.len() - 1;
    let y: Vec<f64> = (0..=n).map(|i| y[i % y.len()]).collect();
    let h: Vec<f64> = (0..n).map(|i| t[i + 1] - t[i]).collect();
    let slope = |i: usize| (y[i + 1] - y[i]) / h[i];
    let mut a = vec![vec![0.0; n + 1]; n + 1];
    let mut b = vec![0.0; n + 1];
    for i in 1..n {
        a[i][i - 1] = h[i - 1];
        a[i][i] = 2.0 * (h[i - 1] + h[i]);
        a[i][i + 1] = h[i];
        b[i] = 6.0 * (slope(i) - slope(i - 1));
    }
    match end {
        End::Natural | End::Clamped(None, None) => {
            a[0][0] = 1.0;
            a[n][n] = 1.0;
        }
        End::Clamped(start, finish) => {
            match start {
                Some(d) => {
                    a[0][0] = 2.0 * h[0];
                    a[0][1] = h[0];
                    b[0] = 6.0 * (slope(0) - d);
                }
                None => a[0][0] = 1.0,
            }
            match finish {
                Some(d) => {
                    a[n][n - 1] = h[n - 1];
                    a[n][n] = 2.0 * h[n - 1];
                    b[n] = 6.0 * (d - slope(n - 1));
                }
                None => a[n][n] = 1.0,
            }
        }
        End::Periodic => {
            // M_n = M_0, and the first point's equation wraps.
            a[n][0] = 1.0;
            a[n][n] = -1.0;
            a[0][n - 1] = h[n - 1];
            a[0][0] = 2.0 * (h[n - 1] + h[0]);
            a[0][1] = h[0];
            b[0] = 6.0 * (slope(0) - slope(n - 1));
        }
    }
    let m = solve_dense(a, b);
    let i = (0..n).find(|&i| u <= t[i + 1]).unwrap_or(n - 1);
    let (l, r) = (t[i + 1] - u, u - t[i]);
    m[i] * l.powi(3) / (6.0 * h[i])
        + m[i + 1] * r.powi(3) / (6.0 * h[i])
        + (y[i] / h[i] - m[i] * h[i] / 6.0) * l
        + (y[i + 1] / h[i] - m[i + 1] * h[i] / 6.0) * r
}

/// The spline matches the reference cubic in x and y everywhere along
/// its parameters.
fn assert_is_cubic(s: &Spline, t: &[f64], pts: &[Point2], ex: End, ey: End) {
    let xs: Vec<f64> = pts.iter().map(|q| q.x()).collect();
    let ys: Vec<f64> = pts.iter().map(|q| q.y()).collect();
    let (a, b) = s.domain();
    assert_eq!((a, b), (t[0], t[t.len() - 1]));
    for k in 0..=200 {
        let u = a + (b - a) * k as f64 / 200.0;
        let q = s.point_at(u);
        let (x, y) = (cubic(t, &xs, ex, u), cubic(t, &ys, ey, u));
        assert!(
            (q.x() - x).abs() < 1e-9 && (q.y() - y).abs() < 1e-9,
            "at {u}: {q:?} against ({x}, {y})"
        );
    }
}

/// No corner where a closed spline closes: its derivative at the end, from
/// before, is its derivative at the start, from after (one-sided
/// differences of second order).
fn assert_smooth_where_it_closes(s: &Spline) {
    let (a, b) = s.domain();
    let h = 1e-4 * (b - a);
    let c = |u: f64| s.point_at(u) - Point2::ORIGIN;
    let before = (c(b) * 3.0 - c(b - h) * 4.0 + c(b - 2.0 * h)) * (0.5 / h);
    let after = (c(a + h) * 4.0 - c(a) * 3.0 - c(a + 2.0 * h)) * (0.5 / h);
    assert!(
        (before - after).length() < 1e-5 * before.length(),
        "{before:?} against {after:?}"
    );
}

fn chords(pts: &[Point2], step: impl Fn(f64) -> f64) -> Vec<f64> {
    let mut t = vec![0.0];
    for w in pts.windows(2) {
        t.push(t[t.len() - 1] + step(w[0].distance_to(w[1])));
    }
    t
}

#[test]
fn through_fit_points_it_is_the_natural_cubic_with_the_points_parameters() {
    let pts = wave();
    for (knots, step) in [
        (SplineKnots::Chord, (|d| d) as fn(f64) -> f64),
        (SplineKnots::SquareRoot, f64::sqrt),
        (SplineKnots::Uniform, |_| 1.0),
    ] {
        let s = fit(pts.clone(), knots);
        let t = chords(&pts, step);
        assert_eq!(s.degree(), 3);
        assert_eq!(s.points().len(), pts.len() + 2);
        let mut expected = vec![0.0; 4];
        expected.extend_from_slice(&t[1..t.len() - 1]);
        expected.extend([t[t.len() - 1]; 4]);
        assert_eq!(s.knots(), expected.as_slice(), "{knots:?}");
        assert!(!s.is_closed());
        assert_is_cubic(&s, &t, &pts, End::Natural, End::Natural);
    }
}

#[test]
fn two_fit_points_make_a_straight_spline() {
    let s = fit(vec![p(1.0, 1.0), p(5.0, 4.0)], SplineKnots::Chord);
    assert_eq!(s.ends(), (p(1.0, 1.0), p(5.0, 4.0)));
    for q in s.flatten() {
        let along = (*q - p(1.0, 1.0)).cross(v(4.0, 3.0));
        assert!(along.abs() < 1e-9, "{q:?}");
    }
}

#[test]
fn a_tangent_sets_its_end_s_direction_the_chord_length_its_speed() {
    let pts = wave();
    let t = chords(&pts, |d| d);
    let (length, run) = (t[t.len() - 1], t[t.len() - 1]);
    let speed = length / run;
    let (start, end) = (v(1.0, 0.0), v(0.0, -2.0));
    let both = SplineFit::new(
        pts.clone(),
        SplineKnots::Chord,
        Some(start),
        Some(end),
        false,
    )
    .unwrap()
    .spline()
    .unwrap();
    // x' = speed·(1, 0) at the start, speed·(0, -1) at the end.
    assert_is_cubic(
        &both,
        &t,
        &pts,
        End::Clamped(Some(speed), Some(0.0)),
        End::Clamped(Some(0.0), Some(-speed)),
    );
    let first = SplineFit::new(pts.clone(), SplineKnots::Chord, Some(start), None, false)
        .unwrap()
        .spline()
        .unwrap();
    assert_is_cubic(
        &first,
        &t,
        &pts,
        End::Clamped(Some(speed), None),
        End::Clamped(Some(0.0), None),
    );
    let last = SplineFit::new(pts.clone(), SplineKnots::Chord, None, Some(end), false)
        .unwrap()
        .spline()
        .unwrap();
    assert_is_cubic(
        &last,
        &t,
        &pts,
        End::Clamped(None, Some(0.0)),
        End::Clamped(None, Some(-speed)),
    );
}

#[test]
fn a_tangent_s_speed_is_the_chord_length_over_the_parameters_run() {
    // Square-root knots run over less than the chord length.
    let pts = wave();
    let t = chords(&pts, f64::sqrt);
    let length: f64 = pts.windows(2).map(|w| w[0].distance_to(w[1])).sum();
    let speed = length / t[t.len() - 1];
    let s = SplineFit::new(
        pts.clone(),
        SplineKnots::SquareRoot,
        Some(v(0.0, 3.0)),
        None,
        false,
    )
    .unwrap()
    .spline()
    .unwrap();
    assert_is_cubic(
        &s,
        &t,
        &pts,
        End::Clamped(Some(0.0), None),
        End::Clamped(Some(speed), None),
    );
}

#[test]
fn closed_it_is_the_periodic_cubic_smooth_where_it_closes() {
    let pts = vec![
        p(0.0, 0.0),
        p(6.0, 1.0),
        p(8.0, 5.0),
        p(3.0, 7.0),
        p(-1.0, 4.0),
    ];
    for (knots, step) in [
        (SplineKnots::Chord, (|d| d) as fn(f64) -> f64),
        (SplineKnots::SquareRoot, f64::sqrt),
        (SplineKnots::Uniform, |_| 1.0),
    ] {
        let s = SplineFit::new(pts.clone(), knots, None, None, true)
            .unwrap()
            .spline()
            .unwrap();
        let mut round = pts.clone();
        round.push(pts[0]);
        let t = chords(&round, step);
        assert!(s.is_closed());
        assert_eq!(s.degree(), 3);
        // Wrapped: the last three control points are the first three.
        let c = s.points();
        assert_eq!(c.len(), pts.len() + 3);
        assert_eq!(c[pts.len()..], c[..3]);
        assert_is_cubic(&s, &t, &pts, End::Periodic, End::Periodic);
        let (a, b) = s.domain();
        assert!(
            s.point_at(a)
                .approx_eq(s.point_at(b), crate::Tolerance::DEFAULT)
        );
        assert_smooth_where_it_closes(&s);
    }
}

#[test]
fn a_closed_fit_s_last_point_on_its_first_is_dropped() {
    let square = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
    let mut back = square.to_vec();
    back.push(square[0]);
    let a = SplineFit::new(square.to_vec(), SplineKnots::Chord, None, None, true).unwrap();
    let b = SplineFit::new(back, SplineKnots::Chord, None, None, true).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.points(), square.as_slice());
}

#[test]
fn a_fit_s_image_is_the_fit_of_the_images() {
    // So a spline moved by its control points and one made again from its
    // moved fit points, as DWG reads one back, are the same curve.
    let turn = Transform2::rotation(p(2.0, 1.0), 0.7)
        .unwrap()
        .then(Transform2::scaling(p(0.0, 0.0), 2.5).unwrap());
    for knots in [
        SplineKnots::Chord,
        SplineKnots::SquareRoot,
        SplineKnots::Uniform,
    ] {
        for closed in [false, true] {
            let tangent = (!closed).then(|| v(1.0, 1.0));
            let made = |pts: Vec<Point2>, d: Option<Vector2>| {
                SplineFit::new(pts, knots, d, None, closed)
                    .unwrap()
                    .spline()
                    .unwrap()
            };
            let moved = made(wave(), tangent).transformed(turn).unwrap();
            let pts = wave().into_iter().map(|q| turn.apply_point(q)).collect();
            let again = made(pts, tangent.map(|d| turn.apply_vector(d)));
            for (a, b) in moved.points().iter().zip(again.points()) {
                assert!(a.distance_to(*b) < 1e-9, "{knots:?} {closed}: {a:?} {b:?}");
            }
        }
    }
}

#[test]
fn fit_points_it_refuses() {
    let refused = |pts: Vec<Point2>, d: Option<Vector2>, closed: bool| {
        SplineFit::new(pts, SplineKnots::Chord, d, None, closed).unwrap_err()
    };
    let reason = GeometryError::OutOfRange {
        what: "spline fit points",
    };
    assert_eq!(refused(vec![p(0.0, 0.0)], None, false), reason);
    assert_eq!(refused(vec![p(0.0, 0.0), p(1.0, 0.0)], None, true), reason);
    assert_eq!(
        refused(
            vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0), p(2.0, 1.0)],
            None,
            false
        ),
        reason
    );
    // Closed on its first point with too few left.
    assert_eq!(
        refused(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)], None, true),
        reason
    );
    assert_eq!(
        refused(wave(), Some(v(1.0, 0.0)), true),
        GeometryError::OutOfRange {
            what: "spline tangent"
        }
    );
    let zero = Vector2::new(0.0, 0.0).unwrap();
    assert_eq!(
        refused(wave(), Some(zero), false),
        GeometryError::ZeroVector
    );
    let end = SplineFit::new(wave(), SplineKnots::Chord, None, Some(zero), false);
    assert_eq!(end.unwrap_err(), GeometryError::ZeroVector);
}

#[test]
fn knots_codes_are_dwg_s() {
    for k in [
        SplineKnots::Chord,
        SplineKnots::SquareRoot,
        SplineKnots::Uniform,
    ] {
        assert_eq!(SplineKnots::from_code(k.code()), k);
    }
    assert_eq!(SplineKnots::Chord.code(), 0);
    assert_eq!(SplineKnots::SquareRoot.code(), 1);
    assert_eq!(SplineKnots::Uniform.code(), 2);
    assert_eq!(SplineKnots::from_code(15), SplineKnots::Chord);
}

#[test]
fn by_vertices_open_it_is_clamped_its_degree_at_most_one_fewer_than_the_points() {
    let pts = wave();
    let s = Spline::by_vertices(3, pts.clone(), false).unwrap();
    assert_eq!(s.degree(), 3);
    assert_eq!(s.points(), pts.as_slice());
    assert_eq!(s.knots(), Spline::clamped_knots(3, 5).as_slice());
    assert_eq!(s.ends(), (pts[0], pts[4]));
    assert!(!s.is_closed());
    let low = Spline::by_vertices(10, pts[..3].to_vec(), false).unwrap();
    assert_eq!(low.degree(), 2);
    assert_eq!(
        Spline::by_vertices(1, pts.clone(), false)
            .unwrap()
            .flatten()
            .len(),
        4 * 16 + 1
    );
}

#[test]
fn by_vertices_closed_it_is_periodic_smooth_where_it_closes() {
    let pts = vec![
        p(0.0, 0.0),
        p(6.0, 1.0),
        p(8.0, 5.0),
        p(3.0, 7.0),
        p(-1.0, 4.0),
    ];
    for degree in [2, 3, 4] {
        let s = Spline::by_vertices(degree, pts.clone(), true).unwrap();
        assert!(s.is_closed());
        assert_eq!(s.degree(), degree);
        let c = s.points();
        assert_eq!(c.len(), pts.len() + degree);
        assert_eq!(c[..pts.len()], pts[..]);
        assert_eq!(c[pts.len()..], pts[..degree]);
        let (a, b) = s.domain();
        assert_eq!((a, b), (0.0, pts.len() as f64));
        assert!(
            s.point_at(a)
                .approx_eq(s.point_at(b), crate::Tolerance::DEFAULT)
        );
        assert_smooth_where_it_closes(&s);
    }
    // Three points closed at most of degree two.
    let small = Spline::by_vertices(3, pts[..3].to_vec(), true).unwrap();
    assert_eq!(small.degree(), 2);
}

#[test]
fn by_vertices_it_refuses() {
    let reason = GeometryError::OutOfRange {
        what: "spline degree",
    };
    assert_eq!(
        Spline::by_vertices(3, vec![p(0.0, 0.0)], false).unwrap_err(),
        reason
    );
    assert_eq!(
        Spline::by_vertices(3, vec![p(0.0, 0.0), p(1.0, 0.0)], true).unwrap_err(),
        reason
    );
    assert_eq!(Spline::by_vertices(0, wave(), false).unwrap_err(), reason);
}

#[test]
fn the_tridiagonal_solver_pivots_past_a_small_or_zero_diagonal() {
    // Its diagonal zero, or so small that eliminating by it loses every
    // digit: only exchanging rows solves it.
    for small in [0.0, 1e-15] {
        tridiagonal_case(small);
    }
}

fn tridiagonal_case(small: f64) {
    let n = 6;
    let rows: Vec<[f64; 3]> = (0..n)
        .map(|i| [1.0 + i as f64, small, 2.0 - 0.25 * i as f64])
        .collect();
    let rhs: Vec<[f64; 3]> = (0..n)
        .map(|i| [i as f64, 1.0, -(i as f64).powi(2)])
        .collect();
    let mut sys = Tridiagonal::new(n);
    for i in 0..n {
        sys.row(i, rows[i], rhs[i]);
    }
    let x = sys.solve().unwrap();
    for i in 0..n {
        for c in 0..3 {
            let mut s = rows[i][1] * x[i][c];
            if i > 0 {
                s += rows[i][0] * x[i - 1][c];
            }
            if i + 1 < n {
                s += rows[i][2] * x[i + 1][c];
            }
            assert!(
                (s - rhs[i][c]).abs() < 1e-12,
                "row {i}: {s} against {}",
                rhs[i][c]
            );
        }
    }
}

#[test]
fn a_unit_tangent_is_kept_to_the_bit() {
    // Written normalized and read back, a tangent must give the same
    // spline: normalizing it again may move it by a bit.
    let mut moved = 0;
    for k in 1..2000 {
        let a = k as f64 * 0.0031;
        let u = v(a.cos() * 3.0, a.sin() * 3.0).normalized().unwrap();
        moved += usize::from(u.normalized().unwrap() != u);
        let fit = SplineFit::new(wave(), SplineKnots::Chord, Some(u), Some(u), false).unwrap();
        assert_eq!(fit.start_tangent(), Some(u));
        assert_eq!(fit.end_tangent(), Some(u));
    }
    assert!(moved > 0, "some unit tangent moves when normalized again");
    let long = SplineFit::new(wave(), SplineKnots::Chord, Some(v(0.0, 2.0)), None, false).unwrap();
    assert_eq!(long.start_tangent(), Some(v(0.0, 1.0)));
}
