use super::*;
use crate::geometry::Geometry2;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

fn tol() -> Tolerance {
    Tolerance::DEFAULT
}

/// The ellipse about (1,2), semi-axes 5 along (3,4)/5 and 3.
fn ellipse() -> Ellipse {
    Ellipse::new(p(1.0, 2.0), v(3.0, 4.0), 0.6, 0.0, std::f64::consts::TAU).unwrap()
}

fn wave() -> Spline {
    Spline::by_vertices(
        3,
        vec![
            p(0.0, 0.0),
            p(3.0, 4.0),
            p(7.0, 1.0),
            p(9.0, 6.0),
            p(14.0, 5.0),
        ],
        false,
    )
    .unwrap()
}

fn spline_of(g: Geometry2) -> Spline {
    match g {
        Geometry2::Spline(s) => s.as_ref().clone(),
        other => panic!("{other:?}"),
    }
}

/// Whether `made` is curve `c`'s offset `shift` away (left when positive):
/// each of its points that far from `c` on that side, and each point of the
/// exact offset that near it, within `precision`.
fn is_offset(c: Smooth, made: &Spline, shift: f64, precision: f64) -> bool {
    let distance = shift.abs();
    let (a, b) = made.domain();
    let on = (0..=1000).all(|k| {
        let q = made.point_at(a + (b - a) * f64::from(k) / 1000.0);
        let t = c.closest_param(q);
        let d = c.derivative(t);
        let left = d.cross(q - c.point(t)) > 0.0;
        (c.distance_to(q) - distance).abs() <= precision && left == (shift > 0.0)
    });
    let (s, e) = c.domain();
    let covered = (0..=1000).all(|k| {
        let t = s + (e - s) * f64::from(k) / 1000.0;
        let d = c.derivative(t);
        let q = c.point(t) + d.perp() * (shift / d.length());
        made.distance_to(q) <= precision
    });
    on && covered
}

#[test]
fn an_ellipse_offset_is_a_closed_cubic_within_a_millionth_of_its_size() {
    let e = ellipse();
    let size = Smooth::Ellipse(e).size();
    // Outside (right of it, counterclockwise), then inside short of its
    // tightest bend, b²/a = 1.8.
    let cases: [(Point2, f64); 2] = [(p(20.0, 0.0), -1.5), (p(1.0, 2.0), 1.7)];
    for (toward, shift) in cases {
        let made = spline_of(
            Geometry2::Ellipse(e)
                .offset(shift.abs(), toward, tol())
                .unwrap(),
        );
        assert!(made.is_closed() && made.is_periodic());
        assert_eq!(made.degree(), 3);
        assert!(made.weights().is_empty());
        assert!(is_offset(Smooth::Ellipse(e), &made, shift, 1e-6 * size));
        assert!(made.points().len() < 200, "{}", made.points().len());
    }
}

#[test]
fn an_ellipse_offset_inward_past_its_tightest_bend_is_refused() {
    let e = ellipse();
    let inside = p(1.0, 2.0);
    assert!(Geometry2::Ellipse(e).offset(1.79, inside, tol()).is_ok());
    for d in [1.8, 1.81, 4.0] {
        assert!(
            Geometry2::Ellipse(e).offset(d, inside, tol()).is_err(),
            "{d}"
        );
    }
    assert!(
        Geometry2::Ellipse(e)
            .offset(50.0, p(30.0, 30.0), tol())
            .is_ok()
    );
}

#[test]
fn an_elliptical_arc_offset_ends_at_its_ends_offset_along_their_tangents() {
    let arc = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.6, 0.3, 2.0).unwrap();
    let made = spline_of(
        Geometry2::Ellipse(arc)
            .offset(0.5, p(0.0, 9.0), tol())
            .unwrap(),
    );
    assert!(!made.is_closed());
    let c = Smooth::Ellipse(arc);
    let (a, b) = c.domain();
    for (t, end) in [(a, made.ends().0), (b, made.ends().1)] {
        let d = c.derivative(t);
        let exact = c.point(t) + d.perp() * (-0.5 / d.length());
        assert!(end.distance_to(exact) < 1e-12, "{end:?} {exact:?}");
    }
    let (u0, u1) = made.domain();
    for (t, u) in [(a, u0), (b, u1)] {
        let (d, m) = (c.derivative(t), made.derivative_at(u));
        assert!(d.cross(m).abs() / (d.length() * m.length()) < 1e-12);
        assert!(d.dot(m) > 0.0, "the same way");
    }
    assert!(is_offset(c, &made, -0.5, 1e-6 * c.size()));
}

#[test]
fn a_spline_offset_is_within_a_millionth_of_its_size_on_either_side() {
    let s = wave();
    let c = Smooth::Spline(&s);
    let cases: [(Point2, f64); 2] = [(p(5.0, 8.0), 0.4), (p(5.0, -3.0), -0.8)];
    for (toward, shift) in cases {
        let made = spline_of(
            Geometry2::Spline(s.clone().into())
                .offset(shift.abs(), toward, tol())
                .unwrap(),
        );
        assert!(!made.is_closed());
        assert!(is_offset(c, &made, shift, 1e-6 * c.size()), "{shift}");
    }
    // A rational quarter circle's offset stays on its circle.
    let h = std::f64::consts::FRAC_1_SQRT_2;
    let quarter = Spline::new(
        2,
        vec![p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
        vec![1.0, h, 1.0],
        Spline::clamped_knots(2, 3),
        false,
    )
    .unwrap();
    let made = spline_of(
        Geometry2::Spline(quarter.into())
            .offset(1.0, p(5.0, 5.0), tol())
            .unwrap(),
    );
    let (a, b) = made.domain();
    for k in 0..=200 {
        let q = made.point_at(a + (b - a) * f64::from(k) / 200.0);
        assert!((q.distance_to(p(0.0, 0.0)) - 3.0).abs() < 1e-6 * 8f64.sqrt());
    }
}

#[test]
fn a_closed_spline_offset_closes_and_one_through_a_point_passes_by_it() {
    let ring = Spline::by_vertices(
        3,
        vec![p(0.0, 0.0), p(6.0, 0.0), p(6.0, 4.0), p(0.0, 4.0)],
        true,
    )
    .unwrap();
    let c = Smooth::Spline(&ring);
    let made = spline_of(
        Geometry2::Spline(ring.clone().into())
            .offset(0.5, p(20.0, 2.0), tol())
            .unwrap(),
    );
    assert!(made.is_closed() && made.is_periodic());
    // Its points run counterclockwise, so outside is right.
    assert!(is_offset(c, &made, -0.5, 1e-6 * c.size()));
    // Through a point 0.7 outside: that offset, by it.
    let t = 0.3;
    let d = ring.derivative_at(t);
    let through = ring.point_at(t) + d.perp() * (-0.7 / d.length());
    let by = spline_of(
        Geometry2::Spline(ring.clone().into())
            .offset_through(through, tol())
            .unwrap(),
    );
    assert!(by.distance_to(through) < 1e-6 * c.size());
}

#[test]
fn a_spline_with_a_corner_or_a_point_on_the_curve_is_refused() {
    // Refused as a corner, not left to fail to fit.
    let corner = Err(GeometryError::OutOfRange { what: "corner" });
    let zig = Spline::by_vertices(1, vec![p(0.0, 0.0), p(4.0, 4.0), p(8.0, 0.0)], false).unwrap();
    assert_eq!(offset_spline(&zig, 0.5, p(4.0, 9.0), tol()), corner);
    // Two arcs of a cubic joined at a corner: knot 1 three times.
    let kinked = Spline::new(
        3,
        vec![
            p(0.0, 0.0),
            p(1.0, 2.0),
            p(3.0, 2.0),
            p(4.0, 0.0),
            p(5.0, 2.0),
            p(7.0, 2.0),
            p(8.0, 0.0),
        ],
        Vec::new(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0],
        false,
    )
    .unwrap();
    assert_eq!(offset_spline(&kinked, 0.2, p(4.0, 9.0), tol()), corner);
    // A knot three times where the tangent runs on is no corner.
    let smooth = Spline::new(
        3,
        vec![
            p(0.0, 0.0),
            p(1.0, 2.0),
            p(3.0, 2.0),
            p(4.0, 2.0),
            p(5.0, 2.0),
            p(7.0, 2.0),
            p(8.0, 0.0),
        ],
        Vec::new(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0],
        false,
    )
    .unwrap();
    assert!(
        Geometry2::Spline(smooth.into())
            .offset(0.2, p(4.0, 9.0), tol())
            .is_ok()
    );
    // Closed, clamped round to its start with a corner there.
    let drop = Spline::new(
        3,
        vec![p(0.0, 0.0), p(4.0, 4.0), p(-4.0, 4.0), p(0.0, 0.0)],
        Vec::new(),
        Spline::clamped_knots(3, 4),
        true,
    )
    .unwrap();
    assert_eq!(offset_spline(&drop, 0.1, p(0.0, 9.0), tol()), corner);
    // A cusp within a span, where it stops: refused either side.
    let cusp = Spline::by_vertices(
        3,
        vec![p(0.0, 0.0), p(1.0, 1.0), p(0.0, 1.0), p(1.0, 0.0)],
        false,
    )
    .unwrap();
    assert!(cusp.derivative_at(0.5).length() < 1e-12);
    for toward in [p(0.5, 2.0), p(0.5, -2.0)] {
        assert!(offset_spline(&cusp, 0.05, toward, tol()).is_err());
    }
    // On the curve: no side.
    let s = wave();
    let on = s.point_at(0.4);
    assert!(Geometry2::Spline(s.into()).offset(0.5, on, tol()).is_err());
    assert!(
        Geometry2::Ellipse(ellipse())
            .offset(0.5, ellipse().point_at_param(1.0), tol())
            .is_err()
    );
}

#[test]
fn a_spline_offset_inward_past_its_tightest_bend_is_refused() {
    // A rational quarter circle of radius 2: inward, by 2 or more, refused.
    let h = std::f64::consts::FRAC_1_SQRT_2;
    let quarter = Spline::new(
        2,
        vec![p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
        vec![1.0, h, 1.0],
        Spline::clamped_knots(2, 3),
        false,
    )
    .unwrap();
    let g = Geometry2::Spline(quarter.into());
    assert!(g.offset(1.99, p(0.1, 0.1), tol()).is_ok());
    assert!(g.offset(2.0, p(0.1, 0.1), tol()).is_err());
    assert!(g.offset(2.5, p(0.1, 0.1), tol()).is_err());
}

#[test]
fn the_tightest_bend_between_two_samples_is_found() {
    // A flat elliptical arc, its tightest bend b²/a = 0.05 at parameter 0,
    // between two of the samples it is checked at.
    let arc = Ellipse::new(p(0.0, 0.0), v(5.0, 0.0), 0.1, -0.5, 1.0).unwrap();
    let inside = p(0.0, 0.0);
    assert!(offset_ellipse(arc, 0.0498, inside, tol()).is_ok());
    assert!(offset_ellipse(arc, 0.0502, inside, tol()).is_err());
}
