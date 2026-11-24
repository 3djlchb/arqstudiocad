use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

/// Every point of `s` at 400 fractions of its domain, and the fraction.
fn along(s: &Spline) -> Vec<Point2> {
    let (a, b) = s.domain();
    (0..=400)
        .map(|k| s.point_at(a + (b - a) * f64::from(k) / 400.0))
        .collect()
}

#[test]
fn a_line_is_a_spline_of_degree_one_between_its_ends() {
    let line = Line::new(p(1.0, 2.0), p(7.0, -1.0)).unwrap();
    let s = Spline::from_line(line).unwrap();
    assert_eq!(s.degree(), 1);
    assert_eq!(s.ends(), (p(1.0, 2.0), p(7.0, -1.0)));
    assert!(along(&s).iter().all(|q| line.distance_to(*q) < 1e-12));
}

#[test]
fn an_arc_is_a_rational_quadratic_on_its_circle_from_end_to_end() {
    // A quarter, three quarters and nearly a whole turn, across 0.
    for (start, end) in [(0.3, 0.3 + FRAC_PI_2), (1.0, 1.0 + 1.5 * PI), (5.0, 4.9)] {
        let arc = Arc::new(p(2.0, -1.0), 3.0, start, end).unwrap();
        let s = Spline::from_arc(arc).unwrap();
        assert_eq!(s.degree(), 2);
        assert!(!s.weights().is_empty(), "rational");
        let (a, b) = s.ends();
        assert!(a.distance_to(arc.start_point()) < 1e-12);
        assert!(b.distance_to(arc.end_point()) < 1e-12);
        for q in along(&s) {
            assert!((q.distance_to(arc.center()) - 3.0).abs() < 1e-12, "{q:?}");
            assert!(arc.distance_to(q) < 1e-12, "within the arc's sweep");
        }
        // A piece a quarter turn at most.
        let pieces = s.span_breaks().len() - 1;
        assert_eq!(pieces, (arc.sweep() / FRAC_PI_2).ceil() as usize);
    }
}

#[test]
fn an_elliptical_arc_is_exact_and_a_whole_ellipse_refused() {
    let e = Ellipse::new(p(1.0, 1.0), v(3.0, 4.0), 0.4, 0.5, 4.0).unwrap();
    let s = Spline::from_elliptical_arc(e).unwrap();
    let (a, b) = s.ends();
    let (sa, sb) = e.ends().unwrap();
    assert!(a.distance_to(sa) < 1e-12 && b.distance_to(sb) < 1e-12);
    for q in along(&s) {
        assert!(e.distance_to(q) < 1e-9, "{q:?}");
    }
    let whole = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.0, TAU).unwrap();
    assert!(Spline::from_elliptical_arc(whole).is_err());
    assert!(Spline::from_curve(&Geometry2::Ellipse(whole)).is_none());
    let circle = crate::circle::Circle::new(p(0.0, 0.0), 1.0).unwrap();
    assert!(Spline::from_curve(&Geometry2::Circle(circle)).is_none());
}

#[test]
fn a_polyline_path_is_one_spline_through_its_segments_and_arcs() {
    // Straight, then a bulged arc either way round.
    let path = [
        (p(0.0, 0.0), 0.0),
        (p(4.0, 0.0), 0.5),
        (p(4.0, 4.0), -1.0),
        (p(8.0, 4.0), 0.0),
    ];
    let s = Spline::from_path(&path).unwrap();
    assert_eq!(s.degree(), 2);
    assert_eq!(s.ends().0, p(0.0, 0.0));
    assert!(s.ends().1.distance_to(p(8.0, 4.0)) < 1e-12);
    // Each segment's own points lie on the spline.
    for w in path.windows(2) {
        let ((a, bulge), (b, _)) = (w[0], w[1]);
        let segment = if bulge == 0.0 {
            Geometry2::Line(Line::new(a, b).unwrap())
        } else {
            Geometry2::Arc(Arc::from_bulge(a, b, bulge).unwrap().0)
        };
        for k in 0..=20 {
            let q = segment.point_at(f64::from(k) / 20.0);
            assert!(s.distance_to(q) < 1e-9, "{q:?}");
        }
    }
    // And the spline goes nowhere else.
    for q in along(&s) {
        let near = path.windows(2).any(|w| {
            let ((a, bulge), (b, _)) = (w[0], w[1]);
            let g = if bulge == 0.0 {
                Geometry2::Line(Line::new(a, b).unwrap())
            } else {
                Geometry2::Arc(Arc::from_bulge(a, b, bulge).unwrap().0)
            };
            g.distance_to(q) < 1e-9
        });
        assert!(near, "{q:?}");
    }
}
