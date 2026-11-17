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
