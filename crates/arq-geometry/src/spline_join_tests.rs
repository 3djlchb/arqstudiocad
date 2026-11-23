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

fn cases() -> Vec<Spline> {
    vec![
        Spline::by_vertices(3, wave(), false).unwrap(),
        Spline::by_vertices(1, wave(), false).unwrap(),
        Spline::by_vertices(2, wave(), false).unwrap(),
        SplineFit::new(wave(), SplineKnots::Chord, None, None, false)
            .unwrap()
            .spline()
            .unwrap(),
        // Rational, non-uniform, a double inner knot.
        Spline::new(
            3,
            wave(),
            vec![1.0, 2.0, 0.5, 1.0, 3.0, 1.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.4, 0.4, 1.0, 1.0, 1.0, 1.0],
            false,
        )
        .unwrap(),
        // Not clamped at its ends.
        Spline::new(
            2,
            wave(),
            Vec::new(),
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            false,
        )
        .unwrap(),
    ]
}

/// Whether `a` and `b` are one curve: the same point at each fraction of
/// their domains, `b` run the other way when `back`.
fn same_curve(a: &Spline, b: &Spline, back: bool) -> bool {
    let ((a0, a1), (b0, b1)) = (a.domain(), b.domain());
    (0..=200).all(|k| {
        let f = f64::from(k) / 200.0;
        let g = if back { 1.0 - f } else { f };
        a.point_at(a0 + f * (a1 - a0))
            .distance_to(b.point_at(b0 + g * (b1 - b0)))
            < 1e-9
    })
}

fn multiplicities(s: &Spline) -> Vec<usize> {
    let mut out: Vec<(f64, usize)> = Vec::new();
    for &k in s.knots() {
        match out.last_mut() {
            Some((v, m)) if *v == k => *m += 1,
            _ => out.push((k, 1)),
        }
    }
    out.into_iter().map(|(_, m)| m).collect()
}

#[test]
fn a_spline_reversed_is_the_same_curve_the_other_way() {
    for s in cases() {
        let r = s.reversed();
        assert!(same_curve(&s, &r, true), "{s:?}");
        assert_eq!(r.domain(), s.domain());
        let back = r.reversed();
        assert_eq!(back.points(), s.points());
        assert_eq!(back.weights(), s.weights());
        // Mirrored twice, a knot may move by a rounding.
        for (a, b) in back.knots().iter().zip(s.knots()) {
            assert!((a - b).abs() < 1e-12);
        }
    }
}

#[test]
fn a_spline_elevated_is_the_same_curve_each_inner_knot_raised() {
    for s in cases() {
        for t in 1..=3 {
            let degree = s.degree() + t;
            let e = s.elevated(degree).unwrap();
            assert_eq!(e.degree(), degree);
            assert!(same_curve(&s, &e, false), "{s:?} by {t}");
            assert_eq!(e.domain(), s.domain());
            // Clamped, each inner knot `t` more times, so as smooth.
            let clamped = s.between(s.domain().0, s.domain().1).unwrap();
            let before = multiplicities(&clamped);
            let after = multiplicities(&e);
            assert_eq!(after.len(), before.len(), "{s:?} by {t}");
            assert_eq!(after[0], degree + 1);
            assert_eq!(*after.last().unwrap(), degree + 1);
            for (m, n) in before[1..before.len() - 1]
                .iter()
                .zip(&after[1..after.len() - 1])
            {
                assert_eq!(*n, m + t, "{s:?} by {t}");
            }
        }
    }
}

#[test]
fn a_spline_at_its_own_degree_is_itself_clamped_and_below_it_refused() {
    let s = &cases()[5];
    let same = s.elevated(2).unwrap();
    assert_eq!(same, s.between(s.domain().0, s.domain().1).unwrap());
    assert!(s.elevated(1).is_err());
    assert!(s.elevated(MAX_DEGREE + 1).is_err());
}

fn cubic(points: &[(f64, f64)]) -> Spline {
    let points: Vec<Point2> = points.iter().map(|&(x, y)| p(x, y)).collect();
    let degree = 3.min(points.len() - 1);
    Spline::by_vertices(degree, points, false).unwrap()
}

/// Whether `joined` runs through each of `pieces` in turn: each piece's
/// point at each fraction lies on it.
fn runs_through(joined: &Spline, pieces: &[Spline]) -> bool {
    pieces.iter().all(|s| {
        let (a, b) = s.domain();
        (0..=50).all(|k| {
            let q = s.point_at(a + (b - a) * f64::from(k) / 50.0);
            joined.distance_to(q) < 1e-7
        })
    })
}

#[test]
fn splines_touching_a_source_join_end_to_end_turned_round_as_needed() {
    let tol = Tolerance::DEFAULT;
    let source = cubic(&[(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)]);
    // After its end, the other way round; before its start; one apart.
    let after = cubic(&[(8.0, 0.0), (7.0, -2.0), (5.0, -1.0), (4.0, 0.0)]);
    let before = cubic(&[(-4.0, 1.0), (-3.0, 3.0), (-1.0, 1.0), (0.0, 0.0)]);
    let apart = cubic(&[(20.0, 0.0), (21.0, 1.0), (22.0, 0.0), (23.0, 1.0)]);
    let others = [apart.clone(), after.clone(), before.clone()];
    let (joined, taken) = join_splines(&source, &others, tol).unwrap().unwrap();
    assert_eq!(taken, vec![1, 2]);
    assert!(!joined.is_closed());
    assert_eq!(joined.degree(), 3);
    assert_eq!(joined.ends(), (p(-4.0, 1.0), p(8.0, 0.0)));
    assert!(runs_through(&joined, &[source, after, before]));
    assert!(joined.distance_to(p(21.0, 1.0)) > 1.0);
    // Where two meet, a knot `degree` times: one control point there.
    let m = multiplicities(&joined);
    assert_eq!(m, vec![4, 3, 3, 4]);
    assert_eq!(joined.points().len(), 10);
}

#[test]
fn splines_of_lower_degree_rise_to_the_highest_and_weights_carry_on() {
    let tol = Tolerance::DEFAULT;
    let line = Spline::by_vertices(1, vec![p(0.0, 0.0), p(4.0, 0.0)], false).unwrap();
    // Rational, its last weight not its first, joined after the line.
    let arc = Spline::new(
        2,
        vec![p(4.0, 0.0), p(6.0, 0.0), p(6.0, 2.0)],
        vec![3.0, 3.0 * std::f64::consts::FRAC_1_SQRT_2, 3.0],
        Spline::clamped_knots(2, 3),
        false,
    )
    .unwrap();
    let quartic = Spline::by_vertices(
        4,
        vec![
            p(6.0, 2.0),
            p(6.0, 4.0),
            p(8.0, 5.0),
            p(9.0, 3.0),
            p(10.0, 4.0),
        ],
        false,
    )
    .unwrap();
    let pieces = [arc.clone(), quartic.clone()];
    let (joined, taken) = join_splines(&line, &pieces, tol).unwrap().unwrap();
    assert_eq!(taken, vec![0, 1]);
    assert_eq!(joined.degree(), 4);
    assert!(runs_through(&joined, &[line, arc.clone(), quartic]));
    // The arc stays a circle's quarter, exactly: weights carried on.
    let (a, b) = arc.domain();
    for k in 0..=20 {
        let q = arc.point_at(a + (b - a) * f64::from(k) / 20.0);
        assert!((q.distance_to(p(4.0, 2.0)) - 2.0).abs() < 1e-12);
    }
    assert!(!joined.weights().is_empty());
}

#[test]
fn a_chain_back_to_its_start_closes_and_nothing_more_joins() {
    let tol = Tolerance::DEFAULT;
    let a = cubic(&[(0.0, 0.0), (1.0, -1.0), (3.0, -1.0), (4.0, 0.0)]);
    let b = cubic(&[(4.0, 0.0), (5.0, 1.0), (5.0, 3.0), (4.0, 4.0)]);
    // Its end a rounding off the source's start.
    let c = cubic(&[(4.0, 4.0), (2.0, 6.0), (-1.0, 2.0), (0.0, 1e-12)]);
    let spur = cubic(&[(0.0, 0.0), (-1.0, -1.0), (-2.0, -1.0), (-3.0, 0.0)]);
    let (joined, taken) = join_splines(&a, &[b, c, spur], tol).unwrap().unwrap();
    assert_eq!(taken, vec![0, 1]);
    assert!(joined.is_closed());
    let n = joined.points().len();
    assert_eq!(joined.points()[n - 1], joined.points()[0]);
    // Its own vertices leave out the copy at its end.
    assert_eq!(joined.own_vertices().len(), n - 1);
}

#[test]
fn a_branch_takes_the_earliest_and_a_closed_spline_joins_nothing() {
    let tol = Tolerance::DEFAULT;
    let source = cubic(&[(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)]);
    let up = cubic(&[(4.0, 0.0), (5.0, 2.0), (6.0, 3.0), (7.0, 5.0)]);
    let down = cubic(&[(4.0, 0.0), (5.0, -2.0), (6.0, -3.0), (7.0, -5.0)]);
    let (_, taken) = join_splines(&source, &[down.clone(), up.clone()], tol)
        .unwrap()
        .unwrap();
    assert_eq!(taken, vec![0]);
    // A closed spline, as source or other, though an end touches.
    let ring = Spline::by_vertices(3, wave(), true).unwrap();
    let (at, _) = ring.ends();
    let from_ring = cubic(&[(at.x(), at.y()), (30.0, 1.0), (31.0, 2.0), (32.0, 0.0)]);
    assert!(
        join_splines(&ring, std::slice::from_ref(&from_ring), tol)
            .unwrap()
            .is_none()
    );
    assert!(join_splines(&from_ring, &[ring], tol).unwrap().is_none());
    assert!(join_splines(&source, &[], tol).unwrap().is_none());
}
