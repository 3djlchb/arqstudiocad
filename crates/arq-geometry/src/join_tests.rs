use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use crate::point::Point2;

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(a: (f64, f64), b: (f64, f64)) -> Line {
    Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap()
}

fn arc(start: f64, end: f64) -> Arc {
    Arc::new(p(2.0, 3.0), 5.0, start, end).unwrap()
}

fn ends(l: Line) -> ((f64, f64), (f64, f64)) {
    ((l.start().x(), l.start().y()), (l.end().x(), l.end().y()))
}

fn angles(g: Geometry2) -> (f64, f64) {
    let Geometry2::Arc(a) = g else {
        panic!("not an arc: {g:?}")
    };
    (a.start_angle(), a.end_angle())
}

#[test]
fn collinear_lines_join_across_their_gaps_in_the_source_direction() {
    let source = line((2.0, 0.0), (4.0, 0.0));
    let others = [
        line((9.0, 0.0), (6.0, 0.0)),  // beyond its end, reversed
        line((-3.0, 0.0), (0.0, 0.0)), // behind its start
        line((0.0, 1.0), (5.0, 1.0)),  // parallel: not collinear
        line((3.0, 0.0), (3.0, 2.0)),  // touching, across: not collinear
        line((2.5, 0.0), (3.5, 0.0)),  // inside it
    ];
    let (joined, taken) = join_lines(source, &others, TOL);
    assert_eq!(ends(joined), ((-3.0, 0.0), (9.0, 0.0)));
    assert_eq!(taken, vec![0, 1, 4]);
}

#[test]
fn a_line_joined_with_nothing_is_unchanged() {
    let source = line((0.1, 0.2), (0.3, 0.7));
    let (joined, taken) = join_lines(source, &[line((5.0, 5.0), (6.0, 7.0))], TOL);
    assert_eq!(joined, source);
    assert!(taken.is_empty());
}

#[test]
fn diagonal_lines_at_survey_coordinates_keep_their_own_ends() {
    let (x, y) = (712_345.678_9, 9_876_543.21);
    let source = line((x, y), (x + 10.0, y + 10.0));
    let other = line((x + 30.0, y + 30.0), (x + 20.0, y + 20.0));
    let (joined, taken) = join_lines(source, &[other], TOL);
    assert_eq!(taken, vec![0]);
    assert_eq!(joined.start(), source.start());
    assert_eq!(joined.end(), other.start(), "the farthest end itself");
}

#[test]
fn arcs_of_one_circle_join_counterclockwise_from_the_source() {
    let source = arc(0.0, FRAC_PI_2);
    // Ahead of it, across a gap.
    let (g, taken) = join_arcs(source, &[arc(PI, 1.5 * PI)], TOL);
    assert_eq!(taken, vec![0]);
    assert_eq!(angles(g), (0.0, 1.5 * PI));
    // Clockwise behind it: reached counterclockwise, the long way round.
    let (g, _) = join_arcs(source, &[arc(1.6 * PI, 1.9 * PI)], TOL);
    assert_eq!(angles(g), (0.0, 1.9 * PI));
    // Inside it: unchanged.
    let (g, taken) = join_arcs(source, &[arc(0.2, 0.4)], TOL);
    assert_eq!(taken, vec![0]);
    assert_eq!(g, Geometry2::Arc(source));
}

#[test]
fn arcs_that_go_all_the_way_round_make_the_circle() {
    let source = arc(0.0, FRAC_PI_2);
    let (g, _) = join_arcs(source, &[arc(PI, 0.1)], TOL); // past the source's start
    assert_eq!(g, Geometry2::Circle(source.circle()));
    let (g, _) = join_arcs(source, &[arc(FRAC_PI_2, TAU)], TOL); // ending at its start
    assert_eq!(g, Geometry2::Circle(source.circle()));
}

#[test]
fn an_arc_starting_a_rounding_before_the_source_is_no_full_turn() {
    let source = arc(1.0, 2.0);
    let (g, _) = join_arcs(source, &[arc(1.0 - 1e-12, 1.5)], TOL);
    assert_eq!(g, Geometry2::Arc(source), "it starts with the source");
}

#[test]
fn arcs_of_other_circles_are_left_out() {
    let source = arc(0.0, 1.0);
    let other_radius = Arc::new(p(2.0, 3.0), 5.1, 2.0, 3.0).unwrap();
    let other_center = Arc::new(p(2.1, 3.0), 5.0, 2.0, 3.0).unwrap();
    let (g, taken) = join_arcs(source, &[other_radius, other_center], TOL);
    assert!(taken.is_empty());
    assert_eq!(g, Geometry2::Arc(source));
}

#[test]
fn many_at_once_groups_collinear_lines_and_arcs_of_one_circle() {
    let circle = arc(0.0, 1.0).circle();
    let curves = [
        Geometry2::Line(line((0.0, 0.0), (1.0, 0.0))), // 0: with 3
        Geometry2::Arc(arc(0.0, 1.0)),                 // 1: with 4
        Geometry2::Line(line((0.0, 5.0), (1.0, 5.0))), // 2: alone
        Geometry2::Line(line((3.0, 0.0), (4.0, 0.0))), // 3
        Geometry2::Arc(arc(2.0, 3.0)),                 // 4
        Geometry2::Circle(circle),                     // 5: never
        Geometry2::Line(line((0.0, 0.0), (0.0, 1.0))), // 6: alone
    ];
    assert_eq!(join_groups(&curves, TOL), vec![vec![0, 3], vec![1, 4]]);
}
