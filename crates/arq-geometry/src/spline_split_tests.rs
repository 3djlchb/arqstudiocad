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

fn open_cases() -> Vec<Spline> {
    vec![
        Spline::by_vertices(3, wave(), false).unwrap(),
        Spline::by_vertices(1, wave(), false).unwrap(),
        Spline::by_vertices(5, wave(), false).unwrap(),
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
        // Not clamped at its ends, as some drawings keep one.
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

fn closed_cases() -> Vec<Spline> {
    // Rational, periodic: its weights differ where it closes.
    let mut wrapped = wave();
    wrapped.extend_from_within(..2);
    let weights = vec![1.0, 2.0, 0.5, 1.5, 3.0, 0.8, 1.0, 2.0];
    let knots = (0..11).map(|j| f64::from(j) - 2.0).collect();
    let rational = Spline::new(2, wrapped, weights, knots, true).unwrap();
    // Closed but clamped, its ends one point of different weights, as a
    // drawing may keep one.
    let clamped = Spline::new(
        2,
        vec![
            p(0.0, 0.0),
            p(5.0, 3.0),
            p(9.0, -2.0),
            p(4.0, -6.0),
            p(0.0, 0.0),
        ],
        vec![2.0, 1.0, 1.5, 1.0, 0.5],
        vec![0.0, 0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0, 1.0],
        true,
    )
    .unwrap();
    vec![
        rational,
        clamped,
        Spline::by_vertices(2, wave(), true).unwrap(),
        Spline::by_vertices(3, wave(), true).unwrap(),
        SplineFit::new(wave(), SplineKnots::SquareRoot, None, None, true)
            .unwrap()
            .spline()
            .unwrap(),
    ]
}

fn scale(s: &Spline) -> f64 {
    let b = s.bounds();
    b.width().hypot(b.height())
}

/// `piece` is `whole` from `u0` to `u1`, parameter for parameter, clamped
/// at both ends and open.
fn assert_piece(whole: &Spline, piece: &Spline, u0: f64, u1: f64) {
    assert_eq!(piece.domain(), (u0, u1));
    assert!(!piece.is_closed());
    let p = piece.degree();
    let k = piece.knots();
    assert!(k[..=p].iter().all(|&t| t == u0) && k[k.len() - p - 1..].iter().all(|&t| t == u1));
    let near = 1e-11 * scale(whole);
    for i in 0..=100 {
        let u = u0 + (u1 - u0) * i as f64 / 100.0;
        let (a, b) = (piece.point_at(u), whole.point_at(u));
        assert!(a.distance_to(b) < near, "{u}: {a:?} against {b:?}");
    }
    let (start, end) = piece.ends();
    assert!(
        start.distance_to(whole.point_at(u0)) < near && end.distance_to(whole.point_at(u1)) < near
    );
}

#[test]
fn a_piece_is_the_same_curve_between_its_parameters() {
    for whole in open_cases().iter().chain(&closed_cases()) {
        let (a, b) = whole.domain();
        let at = |f: f64| a + (b - a) * f;
        for (f0, f1) in [
            (0.0, 1.0),
            (0.0, 0.37),
            (0.21, 1.0),
            (0.13, 0.71),
            (0.5, 0.5000001),
        ] {
            let piece = whole.between(at(f0), at(f1)).unwrap();
            assert_piece(whole, &piece, at(f0), at(f1));
        }
        // At inner knots, already knots there.
        let inner: Vec<f64> = whole
            .knots()
            .iter()
            .copied()
            .filter(|&k| k > a && k < b)
            .collect();
        if let (Some(&k0), Some(&k1)) = (inner.first(), inner.last())
            && k0 < k1
        {
            assert_piece(whole, &whole.between(k0, k1).unwrap(), k0, k1);
        }
    }
}

#[test]
fn the_two_pieces_either_side_of_a_cut_make_the_whole() {
    for whole in open_cases() {
        let (a, b) = whole.domain();
        let cut = a + 0.42 * (b - a);
        let (left, right) = (
            whole.between(a, cut).unwrap(),
            whole.between(cut, b).unwrap(),
        );
        assert!((left.length() + right.length() - whole.length()).abs() < 1e-10 * whole.length());
        assert_eq!(left.ends().1, right.ends().0, "they meet");
    }
}

#[test]
fn a_closed_spline_s_piece_round_where_it_closes() {
    for whole in closed_cases() {
        let (a, b) = whole.domain();
        let period = b - a;
        let (u0, u1) = (a + 0.7 * period, a + 1.3 * period);
        let piece = whole.between_round(u0, u1).unwrap();
        assert_eq!(piece.domain(), (u0, u1));
        assert!(!piece.is_closed());
        let near = 1e-10 * scale(&whole);
        for i in 0..=100 {
            let u = u0 + (u1 - u0) * i as f64 / 100.0;
            let back = if u > b { u - period } else { u };
            let (x, y) = (piece.point_at(u), whole.point_at(back));
            assert!(x.distance_to(y) < near, "{u}: {x:?} against {y:?}");
        }
        // With the rest of the loop, its whole length.
        let rest = whole.between(a + 0.3 * period, u0).unwrap();
        assert!((piece.length() + rest.length() - whole.length()).abs() < 1e-9 * whole.length());
    }
}

#[test]
fn a_cut_a_hair_from_a_knot_is_at_the_knot() {
    // No span of nearly no length is made.
    let s = Spline::by_vertices(3, wave(), false).unwrap();
    let (a, b) = s.domain();
    let inner = s.knots()[4];
    let piece = s.between(inner + 1e-15 * (b - a), b).unwrap();
    assert_eq!(piece.domain().0, inner);
    let distinct = piece.span_breaks();
    assert!(
        distinct.windows(2).all(|w| w[1] - w[0] > 1e-9),
        "{distinct:?}"
    );
}

#[test]
fn pieces_it_refuses() {
    let s = Spline::by_vertices(3, wave(), false).unwrap();
    let (a, b) = s.domain();
    let refused = GeometryError::OutOfRange { what: "parameter" };
    assert_eq!(s.between(b, a), Err(refused.clone()));
    assert_eq!(s.between(a, a), Err(refused.clone()));
    assert_eq!(s.between(a - 1.0, b), Err(refused.clone()));
    assert_eq!(s.between(a, f64::NAN), Err(refused.clone()));
    // An open spline does not go round; a closed one, by a period at most.
    assert_eq!(s.between_round(a + 0.5, b + 0.1), Err(refused.clone()));
    let c = Spline::by_vertices(3, wave(), true).unwrap();
    let (a, b) = c.domain();
    assert_eq!(c.between_round(a + 0.5, b + (b - a)), Err(refused));
}

mod trimmed {
    use super::*;
    use crate::{Geometry2, Line, Tolerance, break_curve, cutting_points, trim};

    const TOL: Tolerance = Tolerance::DEFAULT;

    /// The exact circle about the origin of radius 5 as a closed rational
    /// spline of four quarters, from (5,0) round.
    fn circle() -> Geometry2 {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let at = |x: f64, y: f64| p(5.0 * x, 5.0 * y);
        let points = vec![
            at(1.0, 0.0),
            at(1.0, 1.0),
            at(0.0, 1.0),
            at(-1.0, 1.0),
            at(-1.0, 0.0),
            at(-1.0, -1.0),
            at(0.0, -1.0),
            at(1.0, -1.0),
            at(1.0, 0.0),
        ];
        let weights = vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0];
        let knots = vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ];
        Spline::new(2, points, weights, knots, true).unwrap().into()
    }

    /// The Bézier from (0,0) by (2,3) and (4,3) to (6,0): x = 6u.
    fn bezier() -> Geometry2 {
        Spline::by_vertices(
            3,
            vec![p(0.0, 0.0), p(2.0, 3.0), p(4.0, 3.0), p(6.0, 0.0)],
            false,
        )
        .unwrap()
        .into()
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> Geometry2 {
        Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap().into()
    }

    fn ends(g: &Geometry2) -> (Point2, Point2) {
        (g.start_point().unwrap(), g.end_point().unwrap())
    }

    fn near(a: Point2, b: Point2) -> bool {
        a.distance_to(b) < 1e-9
    }

    /// Every point of `g` lies on the circle of radius 5.
    fn on_circle(g: &Geometry2) {
        for i in 0..=50 {
            let q = g.point_at(i as f64 / 50.0);
            assert!(((q - Point2::ORIGIN).length() - 5.0).abs() < 1e-12, "{q:?}");
        }
    }

    #[test]
    fn a_closed_spline_trimmed_twice_is_the_open_piece_left() {
        let c = circle();
        let cuts = cutting_points(&c, &[line((-9.0, 3.0), (9.0, 3.0))], TOL);
        // Picked at the top: the long part below is left, round from (−4,3).
        let left = trim(&c, &cuts, p(0.0, 5.0), TOL).unwrap().unwrap();
        let [piece] = &left[..] else {
            panic!("{left:?}")
        };
        let (a, b) = ends(piece);
        assert!(near(a, p(-4.0, 3.0)) && near(b, p(4.0, 3.0)), "{a:?} {b:?}");
        assert!(piece.contains_point(p(0.0, -5.0), TOL) && !piece.contains_point(p(0.0, 5.0), TOL));
        let sweep = std::f64::consts::PI + 2.0 * 0.6f64.asin();
        assert!(
            (piece.length() - 5.0 * sweep).abs() < 1e-9,
            "{}",
            piece.length()
        );
        on_circle(piece);
        // Picked at the bottom: the short part above, not round.
        let left = trim(&c, &cuts, p(0.0, -5.0), TOL).unwrap().unwrap();
        let [piece] = &left[..] else {
            panic!("{left:?}")
        };
        let (a, b) = ends(piece);
        assert!(near(a, p(4.0, 3.0)) && near(b, p(-4.0, 3.0)), "{a:?} {b:?}");
        assert!(piece.contains_point(p(0.0, 5.0), TOL));
        on_circle(piece);
    }

    #[test]
    fn an_open_spline_trimmed_in_the_middle_is_two() {
        let s = bezier();
        let cuts = cutting_points(
            &s,
            &[line((2.0, -9.0), (2.0, 9.0)), line((4.0, -9.0), (4.0, 9.0))],
            TOL,
        );
        let left = trim(&s, &cuts, p(3.0, 2.25), TOL).unwrap().unwrap();
        let [first, second] = &left[..] else {
            panic!("{left:?}")
        };
        assert!(near(ends(first).0, p(0.0, 0.0)) && near(ends(first).1, p(2.0, 2.0)));
        assert!(near(ends(second).0, p(4.0, 2.0)) && near(ends(second).1, p(6.0, 0.0)));
        // Its end trimmed off.
        let left = trim(&s, &cuts, p(5.5, 0.6), TOL).unwrap().unwrap();
        let [piece] = &left[..] else {
            panic!("{left:?}")
        };
        assert!(near(ends(piece).0, p(0.0, 0.0)) && near(ends(piece).1, p(4.0, 2.0)));
    }

    #[test]
    fn a_spline_is_broken_as_a_curve_is() {
        let s = bezier();
        let pieces = break_curve(&s, p(2.0, 2.0), p(4.0, 2.0), TOL).unwrap();
        let [first, second] = &pieces[..] else {
            panic!("{pieces:?}")
        };
        assert!(near(ends(first).1, p(2.0, 2.0)) && near(ends(second).0, p(4.0, 2.0)));
        // At one point: split without a gap.
        let pieces = break_curve(&s, p(3.0, 2.25), p(3.0, 2.25), TOL).unwrap();
        let [first, second] = &pieces[..] else {
            panic!("{pieces:?}")
        };
        assert!(near(ends(first).1, p(3.0, 2.25)) && near(ends(second).0, p(3.0, 2.25)));
        assert!((first.length() + second.length() - s.length()).abs() < 1e-10);
        // A closed one loses the span from the first point round to the
        // second, as a circle does.
        let c = circle();
        let pieces = break_curve(&c, p(5.0, 0.0), p(0.0, 5.0), TOL).unwrap();
        let [piece] = &pieces[..] else {
            panic!("{pieces:?}")
        };
        let (a, b) = ends(piece);
        assert!(near(a, p(0.0, 5.0)) && near(b, p(5.0, 0.0)), "{a:?} {b:?}");
        assert!(piece.contains_point(p(-5.0, 0.0), TOL));
        assert!((piece.length() - 7.5 * std::f64::consts::PI).abs() < 1e-9);
        on_circle(piece);
        assert!(
            break_curve(&c, p(5.0, 0.0), p(5.0, 0.0), TOL).is_err(),
            "no span"
        );
        // The other way round: from (0,5) round to (5,0) is taken, the
        // quarter between them left.
        let pieces = break_curve(&c, p(0.0, 5.0), p(5.0, 0.0), TOL).unwrap();
        let [piece] = &pieces[..] else {
            panic!("{pieces:?}")
        };
        let (a, b) = ends(piece);
        assert!(near(a, p(5.0, 0.0)) && near(b, p(0.0, 5.0)), "{a:?} {b:?}");
        assert!((piece.length() - 2.5 * std::f64::consts::PI).abs() < 1e-9);
    }
}
