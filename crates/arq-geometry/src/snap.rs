//! Characteristic points of curves: the geometry behind object snaps.
//!
//! Pure geometry only. Choosing which snaps are active, the pick aperture
//! and the markers drawn on screen belong to the application (milestone
//! 02.5).

use crate::ellipse::Ellipse;
use crate::geometry::Geometry2;
use crate::intersect::intersect;
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// The kind of a characteristic point, named as AutoCAD's object snaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SnapKind {
    /// End of a line or arc.
    Endpoint,
    /// Middle of a line or arc.
    Midpoint,
    /// Center of a circle or arc.
    Center,
    /// 0°, 90°, 180° or 270° point of a circle or arc.
    Quadrant,
    /// Where two curves meet.
    Intersection,
    /// Foot of the perpendicular from a given point.
    Perpendicular,
    /// Point of tangency from a given point.
    Tangent,
    /// Point of the curve closest to the cursor.
    Nearest,
}

impl SnapKind {
    /// AutoCAD's short name, as typed on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Endpoint => "END",
            Self::Midpoint => "MID",
            Self::Center => "CEN",
            Self::Quadrant => "QUA",
            Self::Intersection => "INT",
            Self::Perpendicular => "PER",
            Self::Tangent => "TAN",
            Self::Nearest => "NEA",
        }
    }
}

/// A characteristic point and what makes it characteristic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapPoint {
    /// Which snap produced it.
    pub kind: SnapKind,
    /// Where it is.
    pub point: Point2,
}

impl SnapPoint {
    fn new(kind: SnapKind, point: Point2) -> Self {
        Self { kind, point }
    }
}

/// Points that do not depend on the cursor: end points, midpoint, center and
/// quadrants (for an arc, only the quadrants it passes through).
pub fn key_points(g: &Geometry2) -> Vec<SnapPoint> {
    let mut out = Vec::new();
    match *g {
        Geometry2::Line(l) => {
            out.push(SnapPoint::new(SnapKind::Endpoint, l.start()));
            out.push(SnapPoint::new(SnapKind::Endpoint, l.end()));
            out.push(SnapPoint::new(SnapKind::Midpoint, l.midpoint()));
        }
        Geometry2::Circle(c) => {
            out.push(SnapPoint::new(SnapKind::Center, c.center()));
            for i in 0..4 {
                let q = c.point_at_angle(f64::from(i) * std::f64::consts::FRAC_PI_2);
                out.push(SnapPoint::new(SnapKind::Quadrant, q));
            }
        }
        Geometry2::Arc(a) => {
            out.push(SnapPoint::new(SnapKind::Endpoint, a.start_point()));
            out.push(SnapPoint::new(SnapKind::Endpoint, a.end_point()));
            out.push(SnapPoint::new(SnapKind::Midpoint, a.midpoint()));
            out.push(SnapPoint::new(SnapKind::Center, a.center()));
            let exact = Tolerance::new(f64::MIN_POSITIVE, f64::MIN_POSITIVE).expect("positive");
            for i in 0..4 {
                let angle = f64::from(i) * std::f64::consts::FRAC_PI_2;
                if a.contains_angle(angle, exact) {
                    let q = a.circle().point_at_angle(angle);
                    out.push(SnapPoint::new(SnapKind::Quadrant, q));
                }
            }
        }
        Geometry2::Ellipse(e) => {
            // Its center, its arc's ends and middle, the ends of its axes
            // on it (ADR 0040).
            if let Some((a, b)) = e.ends() {
                out.push(SnapPoint::new(SnapKind::Endpoint, a));
                out.push(SnapPoint::new(SnapKind::Endpoint, b));
                out.push(SnapPoint::new(SnapKind::Midpoint, e.point_at(0.5)));
            }
            out.push(SnapPoint::new(SnapKind::Center, e.center()));
            for q in e.quadrants() {
                out.push(SnapPoint::new(SnapKind::Quadrant, q));
            }
        }
        // An open spline's ends (ADR 0042 D2).
        Geometry2::Spline(ref s) if !s.is_closed() => {
            let (a, b) = s.ends();
            out.push(SnapPoint::new(SnapKind::Endpoint, a));
            out.push(SnapPoint::new(SnapKind::Endpoint, b));
        }
        Geometry2::Spline(_) => {}
    }
    out
}

/// The point of `g` closest to `cursor`.
pub fn nearest_point(g: &Geometry2, cursor: Point2) -> SnapPoint {
    SnapPoint::new(SnapKind::Nearest, g.closest_point(cursor))
}

/// Where `a` and `b` meet, including the ends of any shared pieces.
pub fn intersection_points(a: &Geometry2, b: &Geometry2, tol: Tolerance) -> Vec<SnapPoint> {
    let found = intersect(a, b, tol);
    let mut points: Vec<Point2> = found.points().to_vec();
    for piece in found.overlaps() {
        points.extend(piece.start_point());
        points.extend(piece.end_point());
    }
    points
        .into_iter()
        .map(|p| SnapPoint::new(SnapKind::Intersection, p))
        .collect()
}

/// Points of `g` where a segment from `from` meets it at a right angle.
///
/// A line yields the foot of the perpendicular when it falls on the segment;
/// a circle the two points on the radius through `from`; an arc those of
/// them within its sweep. From the center of a circle every point qualifies,
/// so none is returned. An ellipse yields up to four: where the segment from
/// `from` meets its tangent at a right angle (ADR 0041).
pub fn perpendicular_points(g: &Geometry2, from: Point2, tol: Tolerance) -> Vec<SnapPoint> {
    let points: Vec<Point2> = match *g {
        Geometry2::Ellipse(e) => ellipse_perpendiculars(e, from, tol),
        Geometry2::Spline(ref s) => crate::intersect_spline::spline_perpendiculars(s, from),
        Geometry2::Line(l) => {
            let v = l.vector();
            let t = (from - l.start()).dot(v) / v.length_squared();
            let slack = tol.linear() / l.length();
            if (-slack..=1.0 + slack).contains(&t) {
                vec![l.point_at(t.clamp(0.0, 1.0))]
            } else {
                vec![]
            }
        }
        Geometry2::Circle(_) | Geometry2::Arc(_) => {
            let (center, radius) = round(g);
            match (from - center).normalized() {
                Ok(dir) if !(from - center).is_zero(tol) => {
                    let candidates = [center + dir * radius, center - dir * radius];
                    candidates
                        .into_iter()
                        .filter(|q| on_curve(g, *q, tol))
                        .collect()
                }
                _ => vec![],
            }
        }
    };
    points
        .into_iter()
        .map(|p| SnapPoint::new(SnapKind::Perpendicular, p))
        .collect()
}

/// Points of a circle or arc where a line from `from` touches it tangentially.
///
/// Outside the circle there are two; on it, `from` itself; inside, none.
/// Lines have no tangent points.
pub fn tangent_points(g: &Geometry2, from: Point2, tol: Tolerance) -> Vec<SnapPoint> {
    match *g {
        Geometry2::Line(_) => return vec![],
        Geometry2::Ellipse(e) => {
            return ellipse_tangents(e, from, tol)
                .into_iter()
                .map(|p| SnapPoint::new(SnapKind::Tangent, p))
                .collect();
        }
        Geometry2::Spline(ref s) => {
            return crate::intersect_spline::spline_tangents(s, from, tol)
                .into_iter()
                .map(|p| SnapPoint::new(SnapKind::Tangent, p))
                .collect();
        }
        Geometry2::Circle(_) | Geometry2::Arc(_) => {}
    }
    let (center, radius) = round(g);
    let to_from = from - center; // local frame at the center
    let distance = to_from.length();
    let points: Vec<Point2> = if (distance - radius).abs() <= tol.linear() {
        vec![from]
    } else if distance < radius {
        vec![]
    } else {
        // Tangent points sit at ±acos(r/d) from the direction towards `from`.
        let spread = (radius / distance).acos();
        let dir = to_from / distance;
        [spread, -spread]
            .into_iter()
            .map(|a| center + dir.rotated(a) * radius)
            .collect()
    };
    points
        .into_iter()
        .filter(|q| on_curve(g, *q, tol))
        .map(|p| SnapPoint::new(SnapKind::Tangent, p))
        .collect()
}

fn round(g: &Geometry2) -> (Point2, f64) {
    match *g {
        Geometry2::Circle(c) => (c.center(), c.radius()),
        Geometry2::Arc(a) => (a.center(), a.radius()),
        Geometry2::Line(_) | Geometry2::Ellipse(_) | Geometry2::Spline(_) => {
            unreachable!("a circle or an arc")
        }
    }
}

/// The feet of the perpendiculars from `from` to an ellipse: where
/// `(P(t) - from) · P'(t)` is zero, a trigonometric polynomial of degree
/// two. From the center of a circle-like ellipse every point qualifies, so
/// none is returned.
fn ellipse_perpendiculars(e: Ellipse, from: Point2, tol: Tolerance) -> Vec<Point2> {
    // With P(t) = c + u cos t + w sin t and d = c - from:
    // (P - from)·P' = d·w cos t - d·u sin t + u·w cos 2t + (|w|² - |u|²)/2 sin 2t.
    let (u, w, d) = (e.major_axis(), e.minor_axis(), e.center() - from);
    let poly = crate::roots::TrigPoly {
        cos2: u.dot(w),
        sin2: (w.length_squared() - u.length_squared()) / 2.0,
        cos: d.dot(w),
        sin: -d.dot(u),
        constant: 0.0,
    };
    let normal = |t: f64| poly.value(t);
    if (0..16).all(|i| {
        normal(std::f64::consts::TAU * f64::from(i) / 16.0).abs() <= tol.linear() * u.length()
    }) {
        return vec![];
    }
    let square = |t: f64| normal(t).abs() <= tol.linear() * e.derivative_at(t).length();
    let mut found = Vec::new();
    for zero in crate::roots::periodic_zeros(poly) {
        match zero {
            crate::roots::Zero::Crossing(t) => found.push(t),
            crate::roots::Zero::Touch(t) if square(t) => found.push(t),
            crate::roots::Zero::Touch(_) => {}
            crate::roots::Zero::Pair { turn, .. } if square(turn) => found.push(turn),
            crate::roots::Zero::Pair { first, second, .. } => found.extend([first, second]),
        }
    }
    let mut points: Vec<Point2> = Vec::new();
    for t in found {
        let at = e.point_at_param(t);
        let speed = e.derivative_at(t).length();
        if e.admits_param(t, tol.angular().max(tol.linear() / speed))
            && !points.iter().any(|q| q.approx_eq(at, tol))
        {
            points.push(at);
        }
    }
    points
}

/// The points of an ellipse where a line from `from` touches it. Tangency
/// survives the ellipse's own frame, where it is the unit circle, so they
/// are the unit circle's tangent points brought back: two outside it,
/// `from` itself on it, none inside.
fn ellipse_tangents(e: Ellipse, from: Point2, tol: Tolerance) -> Vec<Point2> {
    let whole = Ellipse::new(e.center(), e.major_axis(), e.ratio(), 0.0, 0.0).expect("its axes");
    // Off by more than the tolerance in its own frame, it is off it.
    let reach = tol.linear() / e.minor_axis().length();
    let (x, y) = e.local(from);
    let near = (x.hypot(y) - 1.0).abs() <= reach * (1.0 + 1e-9);
    let points = if near && whole.distance_to(from) <= tol.linear() {
        vec![from]
    } else {
        let d = x.hypot(y);
        if d < 1.0 {
            vec![]
        } else {
            let (toward, spread) = (y.atan2(x), (1.0 / d).acos());
            [toward + spread, toward - spread]
                .into_iter()
                .map(|a| e.at_local(a.cos(), a.sin()))
                .collect()
        }
    };
    points.into_iter().filter(|&p| e.admits(p, tol)).collect()
}

/// Whether a point of the supporting circle belongs to `g` (the arc's sweep).
fn on_curve(g: &Geometry2, q: Point2, tol: Tolerance) -> bool {
    match *g {
        Geometry2::Arc(a) => {
            let slack = tol.angular().max(tol.linear() / a.radius());
            let angular = Tolerance::new(tol.linear(), slack).expect("positive");
            a.contains_angle((q - a.center()).angle(), angular)
        }
        _ => true,
    }
}

/// Unit direction of `g` at a point on it (for tests and future snaps).
pub fn tangent_direction(g: &Geometry2, at: Point2) -> Vector2 {
    match *g {
        Geometry2::Line(l) => l.direction(),
        Geometry2::Circle(_) | Geometry2::Arc(_) => {
            let (center, _) = round(g);
            (at - center).normalized().map_or(Vector2::X, Vector2::perp)
        }
        Geometry2::Ellipse(e) => e
            .derivative_at(e.param_of(at))
            .normalized()
            .unwrap_or(Vector2::X),
        Geometry2::Spline(ref s) => s
            .derivative_at(s.closest_param(at))
            .normalized()
            .unwrap_or(Vector2::X),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Arc, Circle, Line};

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn kinds(points: &[SnapPoint]) -> Vec<&'static str> {
        points.iter().map(|s| s.kind.name()).collect()
    }

    fn has(points: &[SnapPoint], q: Point2) -> bool {
        points.iter().any(|s| s.point.approx_eq(q, TOL))
    }

    #[test]
    fn key_points_of_each_curve() {
        let l = key_points(&Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into());
        assert_eq!(kinds(&l), ["END", "END", "MID"]);
        assert!(has(&l, p(5.0, 0.0)));
        let c = key_points(&Circle::new(p(0.0, 0.0), 2.0).unwrap().into());
        assert_eq!(kinds(&c), ["CEN", "QUA", "QUA", "QUA", "QUA"]);
        assert!(has(&c, p(0.0, -2.0)));
        // 45° → 135° crosses only the North quadrant.
        let a = key_points(
            &Arc::new(p(0.0, 0.0), 1.0, 45f64.to_radians(), 135f64.to_radians())
                .unwrap()
                .into(),
        );
        assert_eq!(kinds(&a), ["END", "END", "MID", "CEN", "QUA"]);
        assert!(has(&a, p(0.0, 1.0)));
    }

    #[test]
    fn perpendicular_on_lines_and_circles() {
        let l: Geometry2 = Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into();
        assert!(has(
            &perpendicular_points(&l, p(4.0, 7.0), TOL),
            p(4.0, 0.0)
        ));
        assert!(
            perpendicular_points(&l, p(14.0, 7.0), TOL).is_empty(),
            "foot beyond the segment"
        );
        let c: Geometry2 = Circle::new(p(0.0, 0.0), 5.0).unwrap().into();
        let per = perpendicular_points(&c, p(10.0, 0.0), TOL);
        assert_eq!(per.len(), 2);
        assert!(has(&per, p(5.0, 0.0)) && has(&per, p(-5.0, 0.0)));
        assert!(
            perpendicular_points(&c, p(0.0, 0.0), TOL).is_empty(),
            "from the center"
        );
        let upper: Geometry2 = Arc::new(p(0.0, 0.0), 5.0, 0.0, std::f64::consts::PI)
            .unwrap()
            .into();
        let per = perpendicular_points(&upper, p(0.0, 10.0), TOL);
        assert_eq!(per.len(), 1, "only the upper point is on the arc");
        assert!(has(&per, p(0.0, 5.0)));
    }

    #[test]
    fn tangents_from_outside_on_and_inside() {
        let c: Geometry2 = Circle::new(p(0.0, 0.0), 5.0).unwrap().into();
        // From (0, 10): tangent points at ±60° from North, i.e. (±4.33, 2.5).
        let t = tangent_points(&c, p(0.0, 10.0), TOL);
        assert_eq!(t.len(), 2);
        let x = 5.0 * 30f64.to_radians().cos();
        assert!(has(&t, p(x, 2.5)) && has(&t, p(-x, 2.5)));
        assert_eq!(kinds(&t), ["TAN", "TAN"]);
        assert!(
            has(&tangent_points(&c, p(5.0, 0.0), TOL), p(5.0, 0.0)),
            "on the circle"
        );
        assert!(tangent_points(&c, p(1.0, 0.0), TOL).is_empty(), "inside");
        let l: Geometry2 = Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap().into();
        assert!(tangent_points(&l, p(0.0, 5.0), TOL).is_empty());
    }

    #[test]
    fn an_ellipses_own_points_perpendiculars_and_tangents() {
        use crate::{Ellipse, Vector2};
        let e = |start: f64, end: f64| -> Geometry2 {
            Ellipse::new(
                p(0.0, 0.0),
                Vector2::new(5.0, 0.0).unwrap(),
                0.6,
                start,
                end,
            )
            .unwrap()
            .into()
        };
        let whole = e(0.0, 0.0);
        let k = key_points(&whole);
        assert_eq!(kinds(&k), ["CEN", "QUA", "QUA", "QUA", "QUA"]);
        assert!(has(&k, p(0.0, -3.0)) && has(&k, p(-5.0, 0.0)));
        let half = key_points(&e(0.0, std::f64::consts::PI));
        assert_eq!(
            kinds(&half),
            ["END", "END", "MID", "CEN", "QUA", "QUA", "QUA"]
        );
        assert!(has(&half, p(0.0, 3.0)));
        // From above the top, the feet are the top and the bottom; from a
        // point inside near the end of the major axis, four.
        let feet = perpendicular_points(&whole, p(0.0, 7.0), TOL);
        assert_eq!(feet.len(), 2, "{feet:?}");
        assert!(has(&feet, p(0.0, 3.0)) && has(&feet, p(0.0, -3.0)));
        let feet = perpendicular_points(&whole, p(1.0, 0.5), TOL);
        assert_eq!(feet.len(), 4, "{feet:?}");
        for f in &feet {
            let (q, at) = (f.point, Geometry2::distance_to(&whole, f.point));
            assert!(at < 1e-9);
            let along = tangent_direction(&whole, q);
            assert!((q - p(1.0, 0.5)).dot(along).abs() < 1e-9, "{f:?}");
        }
        // From its centers of curvature, on its evolute, the foot there is
        // double: one point, not two split by rounding.
        for k in 1..40 {
            let (sin, cos) = (f64::from(k) * 0.15).sin_cos();
            let evolute = p(16.0 / 5.0 * cos.powi(3), -16.0 / 3.0 * sin.powi(3));
            let feet = perpendicular_points(&whole, evolute, TOL);
            let double = p(5.0 * cos, 3.0 * sin);
            let near_double: Vec<_> = feet
                .iter()
                .filter(|f| f.point.distance_to(double) < 1e-6)
                .collect();
            assert_eq!(near_double.len(), 1, "{k}: {feet:?}");
        }
        // The upper half keeps the feet on it; a circle-like one from its
        // center has none.
        let feet = perpendicular_points(&e(0.0, std::f64::consts::PI), p(0.0, 7.0), TOL);
        assert_eq!(feet.len(), 1);
        let round: Geometry2 =
            Ellipse::new(p(0.0, 0.0), Vector2::new(2.0, 0.0).unwrap(), 1.0, 0.0, 0.0)
                .unwrap()
                .into();
        assert!(perpendicular_points(&round, p(0.0, 0.0), TOL).is_empty());
        // Tangents from (10,0): the touching points satisfy x·10/25 = 1.
        let touch = tangent_points(&whole, p(10.0, 0.0), TOL);
        assert_eq!(touch.len(), 2);
        for t in &touch {
            assert!((t.point.x() - 2.5).abs() < 1e-9, "{t:?}");
            let along = tangent_direction(&whole, t.point);
            assert!((t.point - p(10.0, 0.0)).cross(along).abs() < 1e-9);
        }
        assert_eq!(tangent_points(&whole, p(5.0, 0.0), TOL).len(), 1, "on it");
        let Geometry2::Ellipse(curve) = whole else {
            unreachable!()
        };
        // A hair outside, within the tolerance: from itself.
        let on = curve.point_at_param(0.7)
            + Vector2::new(0.7f64.cos() / 5.0, 0.7f64.sin() / 3.0).unwrap() * 1e-12;
        let touch = tangent_points(&whole, on, TOL);
        assert_eq!(touch.len(), 1, "on it, off a quadrant: {touch:?}");
        assert!(touch[0].point.approx_eq(on, TOL));
        assert!(
            tangent_points(&whole, p(1.0, 1.0), TOL).is_empty(),
            "inside"
        );
        // The upper half keeps only its own.
        assert_eq!(
            tangent_points(&e(0.0, std::f64::consts::PI), p(10.0, 0.0), TOL).len(),
            1
        );
    }

    #[test]
    fn intersections_include_overlap_ends() {
        let a: Geometry2 = Line::new(p(0.0, 0.0), p(5.0, 0.0)).unwrap().into();
        let b: Geometry2 = Line::new(p(2.0, 0.0), p(9.0, 0.0)).unwrap().into();
        let i = intersection_points(&a, &b, TOL);
        assert!(has(&i, p(2.0, 0.0)) && has(&i, p(5.0, 0.0)));
        assert!(i.iter().all(|s| s.kind == SnapKind::Intersection));
    }

    #[test]
    fn nearest_and_names() {
        let l: Geometry2 = Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into();
        let n = nearest_point(&l, p(3.0, 4.0));
        assert_eq!((n.kind.name(), n.point), ("NEA", p(3.0, 0.0)));
    }
}
