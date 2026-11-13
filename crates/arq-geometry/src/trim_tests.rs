use std::f64::consts::{FRAC_PI_2, PI};

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
    let cuts = cutting_points(l, &edges, TOL);
    let middle = trim(l, &cuts, p(5.0, 0.2), TOL).unwrap().unwrap();
    assert_eq!(
        middle.iter().map(ends).collect::<Vec<_>>(),
        [((0.0, 0.0), (3.0, 0.0)), ((7.0, 0.0), (10.0, 0.0))],
        "split in two"
    );
    let start = trim(l, &cuts, p(1.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(
        start.iter().map(ends).collect::<Vec<_>>(),
        [((3.0, 0.0), (10.0, 0.0))]
    );
    let end = trim(l, &cuts, p(9.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(
        end.iter().map(ends).collect::<Vec<_>>(),
        [((0.0, 0.0), (7.0, 0.0))]
    );
}

#[test]
fn no_cut_or_cuts_at_its_ends_leave_nothing_to_trim() {
    let l = line((0.0, 0.0), (10.0, 0.0));
    assert_eq!(trim(l, &[], p(5.0, 0.0), TOL), Ok(None));
    let at_end = cutting_points(l, &[line((10.0, -1.0), (10.0, 1.0))], TOL);
    assert_eq!(at_end.len(), 1);
    assert_eq!(trim(l, &at_end, p(5.0, 0.0), TOL), Ok(None));
}

#[test]
fn a_circle_cut_twice_becomes_the_arc_left() {
    let c = Geometry2::Circle(Circle::new(p(0.0, 0.0), 5.0).unwrap());
    let cuts = cutting_points(c, &[line((-10.0, 0.0), (10.0, 0.0))], TOL);
    let left = trim(c, &cuts, p(0.0, 5.0), TOL).unwrap().unwrap();
    let [Geometry2::Arc(a)] = left.as_slice() else {
        panic!("{left:?}")
    };
    assert!(
        (a.start_angle() - PI).abs() < 1e-12 && a.end_angle().abs() < 1e-12,
        "{a:?}"
    );
    assert!((a.sweep() - PI).abs() < 1e-12, "the lower half");
    // Picked below: the upper half is left.
    let left = trim(c, &cuts, p(0.0, -5.0), TOL).unwrap().unwrap();
    let [Geometry2::Arc(a)] = left.as_slice() else {
        panic!()
    };
    assert!(a.start_angle().abs() < 1e-12 && (a.end_angle() - PI).abs() < 1e-12);
    // One cut cannot trim a circle.
    let one = cutting_points(c, &[line((5.0, -1.0), (5.0, 1.0))], TOL);
    assert_eq!(trim(c, &one, p(0.0, 5.0), TOL), Ok(None));
}

#[test]
fn an_arc_is_trimmed_by_its_turn() {
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 5.0, 0.0, PI).unwrap());
    let cuts = cutting_points(a, &[line((0.0, 0.0), (0.0, 10.0))], TOL);
    let left = trim(a, &cuts, p(3.0, 4.0), TOL).unwrap().unwrap();
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
    let grown = extend(l, &edges, p(4.0, 0.1), TOL).unwrap().unwrap();
    assert_eq!(
        ends(&grown),
        ((0.0, 0.0), (8.0, 0.0)),
        "the nearest beyond its end"
    );
    let grown = extend(l, &edges, p(1.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(ends(&grown), ((-2.0, 0.0), (5.0, 0.0)), "at its start");
    // A boundary its line misses, or none that way: nothing.
    let missed = [line((8.0, 1.0), (8.0, 2.0))];
    assert_eq!(extend(l, &missed, p(4.0, 0.0), TOL), Ok(None));
    let behind = [line((-2.0, -1.0), (-2.0, 1.0))];
    assert_eq!(extend(l, &behind, p(4.0, 0.0), TOL), Ok(None));
    // To a circle: where its line first meets it.
    let ring = [Geometry2::Circle(Circle::new(p(10.0, 0.0), 3.0).unwrap())];
    let grown = extend(l, &ring, p(4.0, 0.0), TOL).unwrap().unwrap();
    assert_eq!(ends(&grown), ((0.0, 0.0), (7.0, 0.0)));
}

#[test]
fn an_arc_grows_round_its_circle_and_a_circle_cannot() {
    // A quarter circle of radius 5, to a vertical line at x = -3 (y = ±4).
    let a = Geometry2::Arc(Arc::new(p(0.0, 0.0), 5.0, 0.0, FRAC_PI_2).unwrap());
    let edges = [line((-3.0, -10.0), (-3.0, 10.0))];
    let grown = extend(a, &edges, p(0.5, 4.9), TOL).unwrap().unwrap();
    let Geometry2::Arc(g) = grown else { panic!() };
    assert!((g.end_angle() - 4.0f64.atan2(-3.0)).abs() < 1e-12, "{g:?}");
    assert_eq!(g.start_angle(), 0.0);
    // At its start, clockwise round: down to (-3, -4)'s angle.
    let grown = extend(a, &edges, p(4.9, 0.5), TOL).unwrap().unwrap();
    let Geometry2::Arc(g) = grown else { panic!() };
    assert!(
        (g.start_angle() - (-4.0f64).atan2(-3.0).rem_euclid(TAU)).abs() < 1e-12,
        "{g:?}"
    );
    let c = Geometry2::Circle(Circle::new(p(0.0, 0.0), 1.0).unwrap());
    assert!(extend(c, &edges, p(1.0, 0.0), TOL).is_err());
}

#[test]
fn survey_coordinates_trim_and_extend_exactly() {
    let (x, y) = (500_000.0, 9_800_000.0);
    let l = line((x, y), (x + 10.0, y));
    let edges = [line((x + 4.0, y - 1.0), (x + 4.0, y + 1.0))];
    let cuts = cutting_points(l, &edges, TOL);
    let left = trim(l, &cuts, p(x + 8.0, y), TOL).unwrap().unwrap();
    let end = left[0].end_point().unwrap();
    assert!((end.x() - (x + 4.0)).abs() < 1e-8);
    let short = line((x, y), (x + 2.0, y));
    let grown = extend(short, &edges, p(x + 1.9, y), TOL).unwrap().unwrap();
    assert!((grown.end_point().unwrap().x() - (x + 4.0)).abs() < 1e-8);
}
