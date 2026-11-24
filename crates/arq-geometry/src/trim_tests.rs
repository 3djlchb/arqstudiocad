use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use crate::circle::Circle;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(a: (f64, f64), b: (f64, f64)) -> Geometry2 {
    Geometry2::Line(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap())
}

fn ends(g: &Geometry2) -> ((f64, f64), (f64, f64)) {
    let r = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
    let (a, b) = (g.start_point().unwrap(), g.end_point().unwrap());
    ((r(a.x()), r(a.y())), (r(b.x()), r(b.y())))
}

#[test]
fn a_line_loses_the_part_between_the_cuts_round_the_pick() {
    let l = line((0.0, 0.0), (10.0, 0.0));
    let edges = [line((3.0, -1.0), (3.0, 1.0)), line((7.0, -1.0), (7.0, 1.0))];
    let cuts = cutting_points(&l, &edges, TOL);
    let middle = trim(&l, &cuts, p(5.0, 0.2), TOL).unwrap().unwrap();
    assert_eq!(
        middle.iter().map(ends).collect::<Vec<_>>(),
        [((0.0, 0.0), (3.0, 0.0)), ((7.0, 0.0), (10.0, 0.0))],
        "split in two"
    );
    let start = trim(&l, &cuts, p(1.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(
        start.iter().map(ends).collect::<Vec<_>>(),
        [((3.0, 0.0), (10.0, 0.0))]
    );
    let end = trim(&l, &cuts, p(9.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(
        end.iter().map(ends).collect::<Vec<_>>(),
        [((0.0, 0.0), (7.0, 0.0))]
    );
}

#[test]
fn no_cut_or_cuts_at_its_ends_leave_nothing_to_trim() {
    let l = line((0.0, 0.0), (10.0, 0.0));
    assert_eq!(trim(&l, &[], p(5.0, 0.0), TOL), Ok(None));
    let at_end = cutting_points(&l, &[line((10.0, -1.0), (10.0, 1.0))], TOL);
    assert_eq!(at_end.len(), 1);
    assert_eq!(trim(&l, &at_end, p(5.0, 0.0), TOL), Ok(None));
}

#[test]
fn a_circle_cut_twice_becomes_the_arc_left() {
    let c = Geometry2::Circle(Circle::new(p(0.0, 0.0), 5.0).unwrap());
    let cuts = cutting_points(&c, &[line((-10.0, 0.0), (10.0, 0.0))], TOL);
    let left = trim(&c, &cuts, p(0.0, 5.0), TOL).unwrap().unwrap();
    let [Geometry2::Arc(a)] = left.as_slice() else {
        panic!("{left:?}")
    };
    assert!(
        (a.start_angle() - PI).abs() < 1e-12 && a.end_angle().abs() < 1e-12,
        "{a:?}"
    );
    assert!((a.sweep() - PI).abs() < 1e-12, "the lower half");
    // Picked below: the upper half is left.
    let left = trim(&c, &cuts, p(0.0, -5.0), TOL).unwrap().unwrap();
    let [Geometry2::Arc(a)] = left.as_slice() else {
        panic!()
    };
    assert!(a.start_angle().abs() < 1e-12 && (a.end_angle() - PI).abs() < 1e-12);
    // One cut cannot trim a circle.
    let one = cutting_points(&c, &[line((5.0, -1.0), (5.0, 1.0))], TOL);
    assert_eq!(trim(&c, &one, p(0.0, 5.0), TOL), Ok(None));
}

#[test]
fn an_arc_is_trimmed_by_its_turn() {
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 5.0, 0.0, PI).unwrap());
    let cuts = cutting_points(&a, &[line((0.0, 0.0), (0.0, 10.0))], TOL);
    let left = trim(&a, &cuts, p(3.0, 4.0), TOL).unwrap().unwrap();
    let [Geometry2::Arc(kept)] = left.as_slice() else {
        panic!("{left:?}")
    };
    assert!(
        (kept.start_angle() - FRAC_PI_2).abs() < 1e-12 && (kept.end_angle() - PI).abs() < 1e-12
    );
}

#[test]
fn a_line_grows_at_the_picked_end_to_the_nearest_boundary() {
    let l = line((0.0, 0.0), (5.0, 0.0));
    let edges = [
        line((8.0, -1.0), (8.0, 1.0)),
        line((12.0, -1.0), (12.0, 1.0)),
        line((-2.0, -1.0), (-2.0, 1.0)),
    ];
    let grown = extend(&l, &edges, p(4.0, 0.1), TOL).unwrap().unwrap();
    assert_eq!(
        ends(&grown),
        ((0.0, 0.0), (8.0, 0.0)),
        "the nearest beyond its end"
    );
    let grown = extend(&l, &edges, p(1.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(ends(&grown), ((-2.0, 0.0), (5.0, 0.0)), "at its start");
    // A boundary its line misses, or none that way: nothing.
    let missed = [line((8.0, 1.0), (8.0, 2.0))];
    assert_eq!(extend(&l, &missed, p(4.0, 0.0), TOL), Ok(None));
    let behind = [line((-2.0, -1.0), (-2.0, 1.0))];
    assert_eq!(extend(&l, &behind, p(4.0, 0.0), TOL), Ok(None));
    // To a circle: where its line first meets it.
    let ring = [Geometry2::Circle(Circle::new(p(10.0, 0.0), 3.0).unwrap())];
    let grown = extend(&l, &ring, p(4.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(ends(&grown), ((0.0, 0.0), (7.0, 0.0)));
}

#[test]
fn an_arc_grows_round_its_circle_and_a_circle_cannot() {
    // A quarter circle of radius 5, to a vertical line at x = -3 (y = ±4).
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 5.0, 0.0, FRAC_PI_2).unwrap());
    let edges = [line((-3.0, -10.0), (-3.0, 10.0))];
    let grown = extend(&a, &edges, p(0.5, 4.9), TOL).unwrap().unwrap();
    let Geometry2::Arc(g) = grown else { panic!() };
    assert!((g.end_angle() - 4.0f64.atan2(-3.0)).abs() < 1e-12, "{g:?}");
    assert_eq!(g.start_angle(), 0.0);
    // At its start, clockwise round: down to (-3, -4)'s angle.
    let grown = extend(&a, &edges, p(4.9, 0.5), TOL).unwrap().unwrap();
    let Geometry2::Arc(g) = grown else { panic!() };
    assert!(
        (g.start_angle() - (-4.0f64).atan2(-3.0).rem_euclid(TAU)).abs() < 1e-12,
        "{g:?}"
    );
    let c = Geometry2::Circle(Circle::new(p(0.0, 0.0), 1.0).unwrap());
    assert!(extend(&c, &edges, p(1.0, 0.0), TOL).is_err());
}

#[test]
fn survey_coordinates_trim_and_extend_exactly() {
    let (x, y) = (500_000.0, 9_800_000.0);
    let l = line((x, y), (x + 10.0, y));
    let edges = [line((x + 4.0, y - 1.0), (x + 4.0, y + 1.0))];
    let cuts = cutting_points(&l, &edges, TOL);
    let left = trim(&l, &cuts, p(x + 8.0, y), TOL).unwrap().unwrap();
    let end = left[0].end_point().unwrap();
    assert!((end.x() - (x + 4.0)).abs() < 1e-8);
    let short = line((x, y), (x + 2.0, y));
    let grown = extend(&short, &edges, p(x + 1.9, y), TOL).unwrap().unwrap();
    assert!((grown.end_point().unwrap().x() - (x + 4.0)).abs() < 1e-8);
}

#[test]
fn break_open_curves_project_order_split_and_clamp() {
    let l = line((1e6, 0.0), (1e6 + 10.0, 0.0));
    for (a, b) in [(3.0, 7.0), (7.0, 3.0)] {
        let pieces = break_curve(&l, p(1e6 + a, 2.0), p(1e6 + b, -2.0), TOL).unwrap();
        assert_eq!(
            pieces.iter().map(ends).collect::<Vec<_>>(),
            [
                ((1e6, 0.0), (1e6 + 3.0, 0.0)),
                ((1e6 + 7.0, 0.0), (1e6 + 10.0, 0.0)),
            ]
        );
    }
    let split = break_curve(&l, p(1e6 + 4.0, 0.0), p(1e6 + 4.0, 0.0), TOL).unwrap();
    assert_eq!(split.len(), 2);
    assert_eq!(ends(&split[0]).1, (1e6 + 4.0, 0.0));
    assert_eq!(ends(&split[1]).0, (1e6 + 4.0, 0.0));
    let clipped = break_curve(&l, p(1e6 - 5.0, 0.0), p(1e6 + 7.0, 0.0), TOL).unwrap();
    assert_eq!(
        clipped.iter().map(ends).collect::<Vec<_>>(),
        [((1e6 + 7.0, 0.0), (1e6 + 10.0, 0.0))]
    );
    assert!(break_curve(&l, p(1e6, 0.0), p(1e6, 0.0), TOL).is_err());
    assert!(
        break_curve(&l, p(1e6 - 1.0, 0.0), p(1e6 + 11.0, 0.0), TOL)
            .unwrap()
            .is_empty()
    );
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 5.0, 3.0 * FRAC_PI_2, PI).unwrap());
    let pieces = break_curve(&a, p(9.0, 0.0), p(0.0, 8.0), TOL).unwrap();
    assert_eq!(
        pieces.iter().map(ends).collect::<Vec<_>>(),
        [((0.0, -5.0), (5.0, 0.0)), ((0.0, 5.0), (-5.0, 0.0))]
    );
}

#[test]
fn break_circle_removes_the_directed_counterclockwise_gap() {
    let circle = Geometry2::Circle(Circle::new(p(0.0, 0.0), 5.0).unwrap());
    for (first, second, expected) in [
        (p(8.0, 0.0), p(0.0, 9.0), ((0.0, 5.0), (5.0, 0.0))),
        (p(0.0, 9.0), p(8.0, 0.0), ((5.0, 0.0), (0.0, 5.0))),
        (p(0.0, -9.0), p(8.0, 0.0), ((5.0, 0.0), (0.0, -5.0))),
    ] {
        let pieces = break_curve(&circle, first, second, TOL).unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(ends(&pieces[0]), expected);
    }
    assert!(break_curve(&circle, p(5.0, 0.0), p(5.0, 0.0), TOL).is_err());
    assert!(break_curve(&circle, p(5.0, 0.0), p(5.0, 1e-12), TOL).is_err());
}

/// About (0,0), semi-axes 5 along x and 3 along y, from `start` to `end`
/// (equal: the whole ellipse).
fn ellipse(start: f64, end: f64) -> crate::Ellipse {
    crate::Ellipse::new(
        p(0.0, 0.0),
        crate::Vector2::new(5.0, 0.0).unwrap(),
        0.6,
        start,
        end,
    )
    .unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.distance_to(b) < 1e-9
}

/// An elliptical piece's ends and sweep.
fn elliptical(g: &Geometry2) -> (Point2, Point2, f64) {
    let Geometry2::Ellipse(e) = g else {
        panic!("{g:?}")
    };
    let (a, b) = e.ends().expect("an arc");
    (a, b, e.sweep())
}

#[test]
fn a_whole_ellipse_cut_twice_becomes_the_arc_left() {
    // x = 4 cuts it at (4, ±1.8); picked on its right, that side goes.
    let tol = Tolerance::DEFAULT;
    let e: Geometry2 = ellipse(0.0, 0.0).into();
    let cuts = cutting_points(&e, &[line((4.0, -9.0), (4.0, 9.0))], tol);
    let left = trim(&e, &cuts, p(5.1, 0.2), tol).unwrap().unwrap();
    let [piece] = &left[..] else {
        panic!("{left:?}")
    };
    let (a, b, sweep) = elliptical(piece);
    assert!(near(a, p(4.0, 1.8)) && near(b, p(4.0, -1.8)), "{a:?} {b:?}");
    assert!(sweep > PI, "the long way round: {sweep}");
    // Cut once only, it has no part to take away.
    let one = cutting_points(&e, &[line((0.0, 0.0), (0.0, 9.0))], tol);
    assert!(trim(&e, &one, p(0.0, 3.0), tol).unwrap().is_none());
}

#[test]
fn an_elliptical_arc_cut_in_the_middle_becomes_two() {
    // The upper half, cut at its top (0,3) and at x = -4, picked between.
    let tol = Tolerance::DEFAULT;
    let upper: Geometry2 = ellipse(0.0, PI).into();
    let cuts = cutting_points(
        &upper,
        &[line((0.0, 0.0), (0.0, 9.0)), line((-4.0, 0.0), (-4.0, 9.0))],
        tol,
    );
    let left = trim(&upper, &cuts, p(-2.5, 2.8), tol).unwrap().unwrap();
    let [first, second] = &left[..] else {
        panic!("{left:?}")
    };
    let (a, b, _) = elliptical(first);
    assert!(near(a, p(5.0, 0.0)) && near(b, p(0.0, 3.0)), "{a:?} {b:?}");
    let (a, b, _) = elliptical(second);
    assert!(
        near(a, p(-4.0, 1.8)) && near(b, p(-5.0, 0.0)),
        "{a:?} {b:?}"
    );
    // Picked beyond the last cut: only the part to its end goes.
    let left = trim(&upper, &cuts, p(-4.8, 0.5), tol).unwrap().unwrap();
    let [piece] = &left[..] else {
        panic!("{left:?}")
    };
    let (a, b, _) = elliptical(piece);
    assert!(near(a, p(5.0, 0.0)) && near(b, p(-4.0, 1.8)), "{a:?} {b:?}");
    // A cut at its very start cuts nothing off there.
    let at_start = cutting_points(&upper, &[line((5.0, -1.0), (5.0, 1.0))], tol);
    assert_eq!(at_start.len(), 1);
    assert!(trim(&upper, &at_start, p(4.0, 1.8), tol).unwrap().is_none());
}

#[test]
fn a_break_takes_a_span_out_of_an_ellipse() {
    let tol = Tolerance::DEFAULT;
    // Whole: the counterclockwise span from the first point to the second
    // goes, as a circle's.
    let whole = ellipse(0.0, 0.0);
    let (q1, q2) = (whole.point_at_param(0.5), whole.point_at_param(1.5));
    let left = break_curve(&whole.into(), q1, q2, tol).unwrap();
    let [piece] = &left[..] else {
        panic!("{left:?}")
    };
    let (a, b, sweep) = elliptical(piece);
    assert!(near(a, q2) && near(b, q1), "{a:?} {b:?}");
    assert!((sweep - (TAU - 1.0)).abs() < 1e-9, "{sweep}");
    // An arc broken in the middle: two arcs, whatever the points' order.
    let upper = ellipse(0.0, PI);
    let (q1, q2) = (upper.point_at_param(1.0), upper.point_at_param(2.0));
    for (first, second) in [(q1, q2), (q2, q1)] {
        let left = break_curve(&upper.into(), first, second, tol).unwrap();
        let [a, b] = &left[..] else {
            panic!("{left:?}")
        };
        let (s0, e0, _) = elliptical(a);
        let (s1, e1, _) = elliptical(b);
        assert!(near(s0, p(5.0, 0.0)) && near(e0, q1), "{s0:?} {e0:?}");
        assert!(near(s1, q2) && near(e1, p(-5.0, 0.0)), "{s1:?} {e1:?}");
    }
    // From an arc's very start, wherever it starts: only the span to the
    // second point goes, not the rest round to its end.
    for start in [0.3, 0.7, 1.1, 2.9, 4.4, 5.9] {
        let arc = ellipse(start, start + 2.0);
        let (from, to) = arc.ends().unwrap();
        let second = arc.point_at_param(start + 1.0);
        let left = break_curve(&arc.into(), from, second, tol).unwrap();
        let [piece] = &left[..] else {
            panic!("{start}: {left:?}")
        };
        let (a, b, _) = elliptical(piece);
        assert!(near(a, second) && near(b, to), "{start}: {a:?} {b:?}");
    }
    // The same point twice on a whole ellipse breaks nothing.
    assert!(break_curve(&whole.into(), q1, q1, tol).is_err());
}

#[test]
fn an_elliptical_arc_grows_round_its_ellipse() {
    let tol = Tolerance::DEFAULT;
    // The first quarter, (5,0) to (0,3), grown at its end to x = -4: to
    // (-4,1.8).
    let quarter: Geometry2 = ellipse(0.0, FRAC_PI_2).into();
    let wall = line((-4.0, -9.0), (-4.0, 9.0));
    let grown = extend(&quarter, std::slice::from_ref(&wall), p(0.2, 3.0), tol)
        .unwrap()
        .unwrap();
    let (a, b, _) = elliptical(&grown);
    assert!(near(a, p(5.0, 0.0)) && near(b, p(-4.0, 1.8)), "{a:?} {b:?}");
    // At its start, back to y = -1.8 on x from 0 to 9: to (4,-1.8).
    let floor = line((0.0, -1.8), (9.0, -1.8));
    let grown = extend(&quarter, &[floor], p(5.0, 0.1), tol)
        .unwrap()
        .unwrap();
    let (a, b, _) = elliptical(&grown);
    assert!(near(a, p(4.0, -1.8)) && near(b, p(0.0, 3.0)), "{a:?} {b:?}");
    // No edge that way, and a whole ellipse cannot grow.
    assert!(
        extend(
            &quarter,
            &[line((20.0, 0.0), (21.0, 0.0))],
            p(0.2, 3.0),
            tol
        )
        .unwrap()
        .is_none()
    );
    assert!(
        extend(
            &ellipse(0.0, 0.0).into(),
            std::slice::from_ref(&wall),
            p(5.0, 0.0),
            tol
        )
        .is_err()
    );
    // An edge crossing only the arc itself is not reached by going round
    // onto it.
    let across = line((4.0, 0.0), (4.0, 9.0));
    assert!(
        extend(&quarter, &[across], p(0.2, 3.0), tol)
            .unwrap()
            .is_none()
    );
    // An edge only beyond its other end, round the far side: not reached
    // the short way, so the nearest wins.
    let both = [wall.clone(), line((4.0, -9.0), (4.0, -1.0))];
    let grown = extend(&quarter, &both, p(0.2, 3.0), tol).unwrap().unwrap();
    assert!(near(elliptical(&grown).1, p(-4.0, 1.8)));
}

#[test]
fn an_ellipse_bounds_lines_and_arcs_extended_to_it() {
    let tol = Tolerance::DEFAULT;
    let edge: Geometry2 = ellipse(0.0, 0.0).into();
    // A line along y = 1.8 from x = -10, grown at its end: to (-4,1.8).
    let grown = extend(
        &line((-10.0, 1.8), (-8.0, 1.8)),
        std::slice::from_ref(&edge),
        p(-8.0, 1.8),
        tol,
    )
    .unwrap()
    .unwrap();
    assert!(near(grown.end_point().unwrap(), p(-4.0, 1.8)), "{grown:?}");
    // An arc of radius 4 about the center, 0 to 45°, grown at its end:
    // round to where the circle next meets the ellipse, x < 0.
    let arc: Geometry2 = crate::Arc::new(p(0.0, 0.0), 4.0, 0.0, PI / 4.0)
        .unwrap()
        .into();
    let tip = arc.end_point().unwrap();
    let grown = extend(&arc, std::slice::from_ref(&edge), tip, tol)
        .unwrap()
        .unwrap();
    let end = grown.end_point().unwrap();
    let (x, y) = ((25.0f64 * 7.0 / 16.0).sqrt(), (81.0f64 / 16.0).sqrt());
    assert!(near(end, p(-x, y)), "{end:?}");
    // A line passing above it, along y = 4, reaches nothing.
    assert!(
        extend(&line((-10.0, 4.0), (-8.0, 4.0)), &[edge], p(-8.0, 4.0), tol)
            .unwrap()
            .is_none()
    );
    // Only the upper half as an edge: a line along y = -1.8 reaches none.
    let upper: Geometry2 = ellipse(0.0, PI).into();
    assert!(
        extend(
            &line((-10.0, -1.8), (-8.0, -1.8)),
            &[upper],
            p(-8.0, -1.8),
            tol
        )
        .unwrap()
        .is_none()
    );
}

fn bezier() -> crate::spline::Spline {
    crate::spline::Spline::by_vertices(
        3,
        vec![p(0.0, 0.0), p(2.0, 3.0), p(4.0, 3.0), p(6.0, 0.0)],
        false,
    )
    .unwrap()
}

#[test]
fn a_spline_grows_straight_along_its_end_tangent_as_progecad_grows_it() {
    // Case SC4: from (6,0) along (2,-3) to x = 8, at (8,-3); the piece a
    // cubic of its own, its knots as long as it, sqrt(13).
    let tol = Tolerance::DEFAULT;
    let wall: Geometry2 = Line::new(p(8.0, -9.0), p(8.0, 9.0)).unwrap().into();
    let s = bezier();
    let Some(Geometry2::Spline(grown)) = extend(
        &Geometry2::Spline(s.clone().into()),
        &[wall],
        p(5.9, 0.2),
        tol,
    )
    .unwrap() else {
        panic!("grown")
    };
    let third = |a: f64, b: f64| (a * 1e9).round() == (b * 1e9).round();
    let expected = [
        p(0.0, 0.0),
        p(2.0, 3.0),
        p(4.0, 3.0),
        p(6.0, 0.0),
        p(20.0 / 3.0, -1.0),
        p(22.0 / 3.0, -2.0),
        p(8.0, -3.0),
    ];
    assert_eq!(grown.points().len(), expected.len());
    for (q, e) in grown.points().iter().zip(expected) {
        assert!(third(q.x(), e.x()) && third(q.y(), e.y()), "{q:?} {e:?}");
    }
    let root13 = 13f64.sqrt();
    let knots = grown.knots();
    assert_eq!(&knots[..7], &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    for &k in &knots[7..] {
        assert!((k - (1.0 + root13)).abs() < 1e-12, "{knots:?}");
    }
    // The old curve is kept, to the bit.
    for k in 0..=20 {
        let u = f64::from(k) / 20.0;
        assert!(grown.point_at(u).distance_to(s.point_at(u)) < 1e-12);
    }
}

#[test]
fn a_spline_grows_at_its_start_to_a_circle_and_not_when_closed_or_nothing_is_there() {
    let tol = Tolerance::DEFAULT;
    // From (0,0) back along (-2,-3) to the circle about (-2,-3), radius 1.
    let ring: Geometry2 = crate::circle::Circle::new(p(-2.0, -3.0), 1.0)
        .unwrap()
        .into();
    let Some(Geometry2::Spline(grown)) = extend(
        &Geometry2::Spline(bezier().into()),
        std::slice::from_ref(&ring),
        p(0.1, 0.2),
        tol,
    )
    .unwrap() else {
        panic!("grown at its start")
    };
    let out = crate::vector::Vector2::new(-2.0, -3.0)
        .unwrap()
        .normalized()
        .unwrap();
    let meets = p(-2.0, -3.0) - out * 1.0;
    assert!(
        grown.ends().0.distance_to(meets) < 1e-9,
        "{:?}",
        grown.ends()
    );
    assert!(grown.ends().1.distance_to(p(6.0, 0.0)) < 1e-12);
    // Nothing that way: none.
    let far: Geometry2 = Line::new(p(-8.0, 5.0), p(-8.0, 9.0)).unwrap().into();
    assert_eq!(
        extend(
            &Geometry2::Spline(bezier().into()),
            &[far],
            p(5.9, 0.2),
            tol
        )
        .unwrap(),
        None
    );
    // A closed spline has no end to grow.
    let closed = crate::spline::Spline::by_vertices(
        3,
        vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        true,
    )
    .unwrap();
    assert!(
        extend(
            &Geometry2::Spline(closed.into()),
            std::slice::from_ref(&ring),
            p(0.0, 0.0),
            tol
        )
        .is_err()
    );
}
