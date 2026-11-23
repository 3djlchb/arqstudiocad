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

fn path(points: &[(f64, f64)]) -> JoinPath {
    JoinPath {
        vertices: points.iter().map(|&(x, y)| (p(x, y), 0.0)).collect(),
        widths: vec![(0.0, 0.0); points.len() - 1],
    }
}

fn points(path: &JoinPath) -> Vec<(f64, f64)> {
    path.vertices.iter().map(|(q, _)| (q.x(), q.y())).collect()
}

#[test]
fn paths_chain_at_either_end_turned_round_when_needed() {
    let first = path(&[(0.0, 0.0), (1.0, 0.0)]);
    let others = [
        path(&[(1.0, 0.0), (1.0, 1.0)]),    // starts at its end
        path(&[(-1.0, 0.0), (0.0, 0.0)]),   // ends at its start
        path(&[(2.0, 1.0), (1.0, 1.0)]),    // ends at the new end: turned round
        path(&[(-1.0, -1.0), (-1.0, 0.0)]), // ends at the new start
        path(&[(9.0, 9.0), (8.0, 8.0)]),    // touches nothing
    ];
    let chain = chain_paths(&first, &others, TOL);
    assert_eq!(chain.taken, vec![0, 1, 2, 3]);
    assert!(!chain.closed);
    assert_eq!(
        points(&chain.path),
        [
            (-1.0, -1.0),
            (-1.0, 0.0),
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (2.0, 1.0)
        ]
    );
    assert_eq!(chain.path.widths.len(), 5);
}

#[test]
fn a_path_turned_round_turns_its_arcs_and_widths_too() {
    let arc_path = JoinPath::arc(arc(0.0, FRAC_PI_2));
    let bulge = arc_path.vertices[0].1;
    assert!((bulge - (FRAC_PI_2 / 4.0).tan()).abs() < 1e-15);
    let mut wide = path(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
    wide.vertices[1].1 = 0.5;
    wide.widths = vec![(0.1, 0.2), (0.3, 0.4)];
    let back = wide.reversed();
    assert_eq!(points(&back), [(2.0, 0.0), (1.0, 0.0), (0.0, 0.0)]);
    assert_eq!(
        back.vertices.iter().map(|v| v.1).collect::<Vec<_>>(),
        [-0.5, 0.0, 0.0]
    );
    assert_eq!(back.widths, [(0.4, 0.3), (0.2, 0.1)]);
    assert_eq!(back.reversed(), wide, "twice is the same path");
}

#[test]
fn a_chain_that_comes_back_to_its_start_closes() {
    let first = path(&[(0.0, 0.0), (4.0, 0.0)]);
    let others = [
        path(&[(4.0, 3.0), (4.0, 0.0)]),
        path(&[(0.0, 3.0), (4.0, 3.0)]),
        path(&[(0.0, 0.0), (0.0, 3.0)]),
        path(&[(0.0, 0.0), (-5.0, 0.0)]), // once closed, nothing more joins
    ];
    let chain = chain_paths(&first, &others, TOL);
    assert!(chain.closed);
    assert_eq!(chain.taken, vec![0, 1, 2]);
    assert_eq!(
        points(&chain.path),
        [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
    );
    assert_eq!(chain.path.widths.len(), 4, "the closing segment kept");
}

#[test]
fn at_a_branch_only_the_first_path_selected_joins() {
    let first = path(&[(0.0, 0.0), (1.0, 0.0)]);
    let others = [
        path(&[(1.0, 0.0), (2.0, 1.0)]),
        path(&[(1.0, 0.0), (2.0, -1.0)]),
    ];
    let chain = chain_paths(&first, &others, TOL);
    assert_eq!(chain.taken, vec![0]);
    assert_eq!(chain.path.end(), p(2.0, 1.0));
}

#[test]
fn nothing_touching_leaves_the_path_as_it_was() {
    let first = path(&[(0.0, 0.0), (1.0, 0.0)]);
    let chain = chain_paths(&first, &[path(&[(1.1, 0.0), (2.0, 0.0)])], TOL);
    assert!(chain.taken.is_empty() && !chain.closed);
    assert_eq!(chain.path, first);
}

/// About (0,0), semi-axes 5 along x and 3, from `start` to `end`.
fn elliptical(start: f64, end: f64) -> crate::Ellipse {
    crate::Ellipse::new(
        Point2::new(0.0, 0.0).unwrap(),
        crate::Vector2::new(5.0, 0.0).unwrap(),
        0.6,
        start,
        end,
    )
    .unwrap()
}

fn ellipse_of(g: Geometry2) -> crate::Ellipse {
    let Geometry2::Ellipse(e) = g else {
        panic!("{g:?}")
    };
    e
}

#[test]
fn elliptical_arcs_of_one_ellipse_join_counterclockwise_gaps_filled() {
    let tol = Tolerance::DEFAULT;
    // From 0 to 1, then 1.5 to 2.5 (a gap filled) and 2.5 to 3: 0 to 3.
    let (joined, taken) = join_elliptical_arcs(
        elliptical(0.0, 1.0),
        &[elliptical(1.5, 2.5), elliptical(2.5, 3.0)],
        tol,
    );
    let e = ellipse_of(joined);
    assert_eq!(taken, [0, 1]);
    assert!(
        e.start_param().abs() < 1e-12 && (e.sweep() - 3.0).abs() < 1e-12,
        "{e:?}"
    );
    // Going all the way round: the whole ellipse.
    let (whole, _) = join_elliptical_arcs(
        elliptical(0.0, 2.0),
        &[elliptical(2.0, 4.0), elliptical(4.0, 0.0)],
        tol,
    );
    assert!(ellipse_of(whole).is_full());
    // Short of the turn by a rounding: whole too.
    let (whole, _) = join_elliptical_arcs(
        elliptical(0.0, 2.0),
        &[elliptical(2.0, 4.0), elliptical(4.0, TAU - 1e-13)],
        tol,
    );
    assert!(ellipse_of(whole).is_full());
    // One written with its major axis the other way (parameters a half
    // turn on) is the same ellipse.
    let flipped = crate::Ellipse::new(
        Point2::new(0.0, 0.0).unwrap(),
        crate::Vector2::new(-5.0, 0.0).unwrap(),
        0.6,
        1.0 - std::f64::consts::PI,
        2.0 - std::f64::consts::PI,
    )
    .unwrap();
    let (joined, taken) = join_elliptical_arcs(elliptical(0.0, 1.0), &[flipped], tol);
    assert_eq!(taken, [0]);
    assert!((ellipse_of(joined).sweep() - 2.0).abs() < 1e-9);
}

#[test]
fn other_ellipses_and_whole_ones_do_not_join() {
    let tol = Tolerance::DEFAULT;
    let p = |x: f64, y: f64| Point2::new(x, y).unwrap();
    let v = |x: f64, y: f64| crate::Vector2::new(x, y).unwrap();
    let others = [
        crate::Ellipse::new(p(1.0, 0.0), v(5.0, 0.0), 0.6, 1.0, 2.0).unwrap(),
        crate::Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.5, 1.0, 2.0).unwrap(),
        crate::Ellipse::new(p(0.0, 0.0), v(0.0, 5.0), 0.6, 1.0, 2.0).unwrap(),
        crate::Ellipse::new(p(0.0, 0.0), v(6.0, 0.0), 0.5, 1.0, 2.0).unwrap(),
        elliptical(0.0, 0.0),
    ];
    let source = elliptical(0.0, 1.0);
    let (joined, taken) = join_elliptical_arcs(source, &others, tol);
    assert!(taken.is_empty(), "{taken:?}");
    assert_eq!(ellipse_of(joined), source);
    // A whole ellipse as the source takes nothing.
    let (_, taken) = join_elliptical_arcs(elliptical(0.0, 0.0), &[elliptical(1.0, 2.0)], tol);
    assert!(taken.is_empty());
    // Round ones, a circle-like ellipse, join whatever way their axis lies.
    let round =
        |m: crate::Vector2, s: f64, e: f64| crate::Ellipse::new(p(0.0, 0.0), m, 1.0, s, e).unwrap();
    let (_, taken) = join_elliptical_arcs(
        round(v(2.0, 0.0), 0.0, 1.0),
        &[round(v(0.0, 2.0), 0.0, 1.0)],
        tol,
    );
    assert_eq!(taken, [0]);
}

#[test]
fn many_at_once_groups_arcs_of_one_ellipse() {
    let tol = Tolerance::DEFAULT;
    let curves: Vec<Geometry2> = vec![
        elliptical(0.0, 1.0).into(),
        elliptical(0.0, 0.0).into(),
        elliptical(2.0, 3.0).into(),
    ];
    assert_eq!(
        join_groups(&curves, tol),
        [vec![0, 2]],
        "the whole one joins none"
    );
}
