use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use crate::line::Line;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.distance_to(b) < 1e-9
}

#[test]
fn the_longer_axis_is_the_major_one_as_autocad_makes_it() {
    // From (0,0) to (10,0), the other semi-axis 3: major along x.
    let e = Ellipse::from_axis(p(5.0, 0.0), p(10.0, 0.0), 3.0).unwrap();
    assert_eq!((e.major_axis(), e.ratio()), (v(5.0, 0.0), 0.6));
    assert!(e.is_full() && e.ends().is_none());
    // The other semi-axis longer: the major turns a quarter from the first.
    let e = Ellipse::from_axis(p(0.0, 0.0), p(2.0, 0.0), 4.0).unwrap();
    assert!(near(p(0.0, 0.0) + e.major_axis(), p(0.0, 4.0)));
    assert_eq!(e.ratio(), 0.5);
    assert!(Ellipse::from_axis(p(0.0, 0.0), p(2.0, 0.0), 0.0).is_err());
    assert!(Ellipse::from_axis(p(0.0, 0.0), p(0.0, 0.0), 1.0).is_err());
}

#[test]
fn a_rotation_makes_the_other_axis_its_cosine_up_to_89_4_degrees() {
    let e = Ellipse::from_rotation(p(0.0, 0.0), p(4.0, 0.0), PI / 3.0).unwrap();
    assert!((e.ratio() - 0.5).abs() < 1e-12);
    assert!(Ellipse::from_rotation(p(0.0, 0.0), p(4.0, 0.0), MAX_ROTATION).is_ok());
    assert!(Ellipse::from_rotation(p(0.0, 0.0), p(4.0, 0.0), 1.57).is_err());
    assert!(Ellipse::from_rotation(p(0.0, 0.0), p(4.0, 0.0), -0.1).is_err());
}

#[test]
fn an_arc_by_angles_from_the_first_axis_ends_on_those_rays() {
    let whole = Ellipse::from_axis(p(0.0, 0.0), p(4.0, 0.0), 2.0).unwrap();
    let first = v(4.0, 0.0);
    let arc = whole.arc_from(first, 0.0, PI / 4.0, false).unwrap();
    let (a, b) = arc.ends().unwrap();
    assert!(near(a, p(4.0, 0.0)));
    assert!(
        (b.y() - b.x()).abs() < 1e-9 && b.x() > 0.0,
        "on the 45° ray: {b:?}"
    );
    assert!(arc.contains_point(b, Tolerance::DEFAULT));
    // By parameters: a quarter of the parameter is the end of the minor axis.
    let arc = whole.arc_from(first, 0.0, FRAC_PI_2, true).unwrap();
    assert!(near(arc.ends().unwrap().1, p(0.0, 2.0)));
    assert!(whole.arc_from(first, 1.0, 1.0, false).is_err());
}

#[test]
fn angles_count_from_the_first_axis_even_when_it_is_the_minor_one() {
    // First axis along x, 2 long; the other 4: the major is along y.
    let whole = Ellipse::from_axis(p(0.0, 0.0), p(2.0, 0.0), 4.0).unwrap();
    let arc = whole.arc_from(v(2.0, 0.0), 0.0, FRAC_PI_2, false).unwrap();
    let (a, b) = arc.ends().unwrap();
    assert!(near(a, p(2.0, 0.0)) && near(b, p(0.0, 4.0)));
}

#[test]
fn the_bounds_hold_a_turned_ellipse_and_an_arc_tightly() {
    let tilted = Ellipse::new(p(1.0, 2.0), v(3.0, 4.0), 0.4, 0.0, TAU).unwrap();
    let arc = Ellipse::new(p(1.0, 2.0), v(3.0, 4.0), 0.4, 0.3, 2.2).unwrap();
    for e in [tilted, arc] {
        let b = e.bounds();
        let samples: Vec<Point2> = (0..=20_000)
            .map(|i| e.point_at_param(e.start_param() + e.sweep() * f64::from(i) / 20_000.0))
            .collect();
        let dense = Bounds2::from_points(samples).unwrap();
        assert!(b.min().distance_to(dense.min()) < 1e-6, "{b:?} {dense:?}");
        assert!(b.max().distance_to(dense.max()) < 1e-6, "{b:?} {dense:?}");
    }
}

#[test]
fn the_nearest_point_quadrants_and_flattening() {
    let e = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.0, TAU).unwrap();
    assert!(near(e.closest_point(p(9.0, 0.0)), p(5.0, 0.0)));
    assert!(near(e.closest_point(p(0.0, -7.0)), p(0.0, -3.0)));
    assert!((e.distance_to(p(0.0, 1.0)) - 2.0).abs() < 1e-9);
    assert_eq!(e.quadrants().len(), 4);
    let quarter = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.1, 1.4).unwrap();
    assert!(quarter.quadrants().is_empty());
    let points = e.flatten();
    assert_eq!(
        points.len(),
        101 + 1,
        "16 a radian over a turn: ceil(16·2π) = 101 steps"
    );
    assert!(near(points[0], p(5.0, 0.0)) && near(points[101], p(5.0, 0.0)));
    assert_eq!(quarter.flatten().len(), 21 + 1, "ceil(16·1.3) = 21 steps");
}

#[test]
fn a_turn_and_a_mirror_carry_every_point_with_them() {
    let arc = Ellipse::new(p(1.0, 2.0), v(3.0, 1.0), 0.5, 0.4, 2.6).unwrap();
    let turn = Transform2::rotation(p(0.0, 0.0), 0.7)
        .unwrap()
        .then(Transform2::scaling(p(5.0, 5.0), 2.0).unwrap());
    let mirror = Transform2::mirror(Line::new(p(0.0, 0.0), p(1.0, 3.0)).unwrap());
    for t in [turn, mirror] {
        let image = arc.transformed(t).unwrap();
        let (a, b) = arc.ends().unwrap();
        let (ia, ib) = image.ends().unwrap();
        let mapped = (t.apply_point(a), t.apply_point(b));
        if t.is_reflection() {
            assert!(near(ia, mapped.1) && near(ib, mapped.0), "ends swap");
        } else {
            assert!(near(ia, mapped.0) && near(ib, mapped.1));
        }
        for i in 0..=10 {
            let q = t.apply_point(arc.point_at_param(0.4 + 2.2 * f64::from(i) / 10.0));
            assert!(image.contains_point(q, Tolerance::new(1e-9, 1e-12).unwrap()));
        }
        assert!((image.sweep() - arc.sweep()).abs() < 1e-12);
    }
}

#[test]
fn its_length_is_the_integral_of_its_speed() {
    // Semi-axes 5 and 3: against ten million chords, which fall short by
    // about the length over the chords squared.
    let e = Ellipse::new(p(1.0, 2.0), v(4.0, 3.0), 0.6, 0.0, 0.0).unwrap();
    let n = 10_000_000;
    let chords: f64 = (0..n)
        .map(|i| {
            let (a, b) = (TAU * i as f64 / n as f64, TAU * (i + 1) as f64 / n as f64);
            e.point_at_param(a).distance_to(e.point_at_param(b))
        })
        .sum();
    assert!(
        (e.length() - chords).abs() < 1e-9,
        "{} {chords}",
        e.length()
    );
    // Ramanujan's second approximation is good to 1e-10 here.
    let (a, b) = (5.0f64, 3.0f64);
    let h = ((a - b) / (a + b)).powi(2);
    let ramanujan = PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()));
    assert!((e.length() - ramanujan).abs() < 1e-8);
    // A quarter is a quarter, by symmetry; a circle's is its arc.
    let quarter = Ellipse::new(p(1.0, 2.0), v(4.0, 3.0), 0.6, 0.0, FRAC_PI_2).unwrap();
    assert!((quarter.length() * 4.0 - e.length()).abs() < 1e-11);
    let round = Ellipse::new(p(0.0, 0.0), v(2.0, 0.0), 1.0, 0.0, 1.0).unwrap();
    assert!((round.length() - 2.0).abs() < 1e-13);
    // A very narrow one, from a parameter where it turns sharply inside
    // the quadrature's panels: four times its major axis, nearly.
    let thin = Ellipse::new(p(0.0, 0.0), v(1.0, 0.0), 1e-6, 0.1, 0.1).unwrap();
    assert!((thin.length() - 4.0).abs() < 1e-9, "{}", thin.length());
}

#[test]
fn a_point_at_a_distance_along_it_and_a_points_parameter() {
    let arc = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.3, 4.0).unwrap();
    let length = arc.length();
    for k in 0..=10 {
        let d = length * f64::from(k) / 10.0;
        let q = arc.point_at_distance(d).unwrap();
        let t = arc.param_of(q);
        let back = arc.length_between(
            arc.start_param(),
            arc.start_param() + angle::ccw_sweep(arc.start_param(), t),
        );
        assert!(
            (back - d).abs() < 1e-9 || (k == 0 && back.abs() < 1e-9),
            "{k}: {back} {d}"
        );
        assert!(arc.distance_to(q) < 1e-9);
    }
    assert!(arc.point_at_distance(length + 1e-6).is_err());
    assert!(arc.point_at_distance(-1e-6).is_err());
    assert!(arc.point_at_distance(f64::NAN).is_err());
    // The parameter of a point is its own, turned or not.
    let turned = Ellipse::new(p(3.0, -1.0), v(-2.0, 2.0), 0.4, 0.0, 0.0).unwrap();
    for t in [0.0, 1.0, 2.5, 5.9] {
        assert!((turned.param_of(turned.point_at_param(t)) - t).abs() < 1e-12);
    }
    // point_at counts along the parameter, its ends at 0 and 1.
    let (a, b) = arc.ends().unwrap();
    assert!(near(arc.point_at(0.0), a) && near(arc.point_at(1.0), b));
    assert!(near(arc.point_at(0.5), arc.point_at_param(0.3 + 1.85)));
}

#[test]
fn a_semi_axis_set_by_its_grip_keeps_the_other_and_swaps_when_longer() {
    // Semi-axes 5 along x and 3 along y, about (1,2).
    let e = Ellipse::new(p(1.0, 2.0), v(5.0, 0.0), 0.6, 0.0, 0.0).unwrap();
    let longer = e.with_semi_axis(true, 8.0).unwrap();
    assert_eq!(
        (longer.major_axis(), longer.ratio()),
        (v(8.0, 0.0), 3.0 / 8.0)
    );
    assert!(longer.is_full());
    let rounder = e.with_semi_axis(false, 4.0).unwrap();
    assert_eq!((rounder.major_axis(), rounder.ratio()), (v(5.0, 0.0), 0.8));
    // The minor grown past the major: the major turns to y, 7 long, the
    // other 5 along x.
    let swapped = e.with_semi_axis(false, 7.0).unwrap();
    assert!(near(p(1.0, 2.0) + swapped.major_axis(), p(1.0, 9.0)));
    assert!((swapped.ratio() - 5.0 / 7.0).abs() < 1e-15);
    // The major shrunk below the minor: the major is 3 along y now.
    let shrunk = e.with_semi_axis(true, 2.0).unwrap();
    assert!(near(p(1.0, 2.0) + shrunk.major_axis(), p(1.0, 5.0)));
    assert!((shrunk.ratio() - 2.0 / 3.0).abs() < 1e-15);
    assert!(e.with_semi_axis(true, 0.0).is_err());
    assert!(e.with_semi_axis(true, f64::NAN).is_err());
    // An arc keeps its place across a swap: its ends where they were, the
    // new semi-axes taking them along.
    let arc = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.3, 2.0).unwrap();
    let swapped = arc.with_semi_axis(false, 7.0).unwrap();
    let (a, b) = swapped.ends().unwrap();
    let (x, y) = (0.3f64.cos() * 5.0, 0.3f64.sin() * 7.0);
    assert!(near(a, p(x, y)), "{a:?}");
    let (x, y) = (2.0f64.cos() * 5.0, 2.0f64.sin() * 7.0);
    assert!(near(b, p(x, y)), "{b:?}");
}

#[test]
fn an_arc_end_grip_slides_along_the_ellipse() {
    let arc = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.0, FRAC_PI_2).unwrap();
    // Toward (-10,0): the end at (-5,0), the start kept.
    let longer = arc.with_end_at(false, p(-10.0, 0.0)).unwrap();
    let (a, b) = longer.ends().unwrap();
    assert!(near(a, p(5.0, 0.0)) && near(b, p(-5.0, 0.0)), "{a:?} {b:?}");
    // The start toward (4,-1.8), a point on it: there.
    let back = arc.with_end_at(true, p(4.0, -1.8)).unwrap();
    let (a, b) = back.ends().unwrap();
    assert!(near(a, p(4.0, -1.8)) && near(b, p(0.0, 3.0)), "{a:?} {b:?}");
    // Onto the other end, from the center, or on a whole one: refused.
    assert!(arc.with_end_at(false, p(10.0, 0.0)).is_err());
    assert!(arc.with_end_at(false, p(0.0, 0.0)).is_err());
    let whole = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.0, 0.0).unwrap();
    assert!(whole.with_end_at(true, p(1.0, 1.0)).is_err());
}
