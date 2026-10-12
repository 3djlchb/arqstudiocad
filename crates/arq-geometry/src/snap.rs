//! Characteristic points of curves: the geometry behind object snaps.
//!
//! Pure geometry only. Choosing which snaps are active, the pick aperture
//! and the markers drawn on screen belong to the application (phase 6).

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
pub fn key_points(g: Geometry2) -> Vec<SnapPoint> {
    let mut out = Vec::new();
    match g {
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
    }
    out
}

/// The point of `g` closest to `cursor`.
pub fn nearest_point(g: Geometry2, cursor: Point2) -> SnapPoint {
    SnapPoint::new(SnapKind::Nearest, g.closest_point(cursor))
}

/// Where `a` and `b` meet, including the ends of any shared pieces.
pub fn intersection_points(a: Geometry2, b: Geometry2, tol: Tolerance) -> Vec<SnapPoint> {
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
/// so none is returned.
pub fn perpendicular_points(g: Geometry2, from: Point2, tol: Tolerance) -> Vec<SnapPoint> {
    let points: Vec<Point2> = match g {
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
pub fn tangent_points(g: Geometry2, from: Point2, tol: Tolerance) -> Vec<SnapPoint> {
    if let Geometry2::Line(_) = g {
        return vec![];
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

fn round(g: Geometry2) -> (Point2, f64) {
    match g {
        Geometry2::Circle(c) => (c.center(), c.radius()),
        Geometry2::Arc(a) => (a.center(), a.radius()),
        Geometry2::Line(_) => unreachable!("lines have no center"),
    }
}

/// Whether a point of the supporting circle belongs to `g` (the arc's sweep).
fn on_curve(g: Geometry2, q: Point2, tol: Tolerance) -> bool {
    match g {
        Geometry2::Arc(a) => {
            let slack = tol.angular().max(tol.linear() / a.radius());
            let angular = Tolerance::new(tol.linear(), slack).expect("positive");
            a.contains_angle((q - a.center()).angle(), angular)
        }
        _ => true,
    }
}

/// Unit direction of `g` at a point on it (for tests and future snaps).
pub fn tangent_direction(g: Geometry2, at: Point2) -> Vector2 {
    match g {
        Geometry2::Line(l) => l.direction(),
        Geometry2::Circle(_) | Geometry2::Arc(_) => {
            let (center, _) = round(g);
            (at - center).normalized().map_or(Vector2::X, Vector2::perp)
        }
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
        let l = key_points(Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into());
        assert_eq!(kinds(&l), ["END", "END", "MID"]);
        assert!(has(&l, p(5.0, 0.0)));
        let c = key_points(Circle::new(p(0.0, 0.0), 2.0).unwrap().into());
        assert_eq!(kinds(&c), ["CEN", "QUA", "QUA", "QUA", "QUA"]);
        assert!(has(&c, p(0.0, -2.0)));
        // 45° → 135° crosses only the North quadrant.
        let a = key_points(
            Arc::new(p(0.0, 0.0), 1.0, 45f64.to_radians(), 135f64.to_radians())
                .unwrap()
                .into(),
        );
        assert_eq!(kinds(&a), ["END", "END", "MID", "CEN", "QUA"]);
        assert!(has(&a, p(0.0, 1.0)));
    }

    #[test]
    fn perpendicular_on_lines_and_circles() {
        let l: Geometry2 = Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into();
        assert!(has(&perpendicular_points(l, p(4.0, 7.0), TOL), p(4.0, 0.0)));
        assert!(
            perpendicular_points(l, p(14.0, 7.0), TOL).is_empty(),
            "foot beyond the segment"
        );
        let c: Geometry2 = Circle::new(p(0.0, 0.0), 5.0).unwrap().into();
        let per = perpendicular_points(c, p(10.0, 0.0), TOL);
        assert_eq!(per.len(), 2);
        assert!(has(&per, p(5.0, 0.0)) && has(&per, p(-5.0, 0.0)));
        assert!(
            perpendicular_points(c, p(0.0, 0.0), TOL).is_empty(),
            "from the center"
        );
        let upper: Geometry2 = Arc::new(p(0.0, 0.0), 5.0, 0.0, std::f64::consts::PI)
            .unwrap()
            .into();
        let per = perpendicular_points(upper, p(0.0, 10.0), TOL);
        assert_eq!(per.len(), 1, "only the upper point is on the arc");
        assert!(has(&per, p(0.0, 5.0)));
    }

    #[test]
    fn tangents_from_outside_on_and_inside() {
        let c: Geometry2 = Circle::new(p(0.0, 0.0), 5.0).unwrap().into();
        // From (0, 10): tangent points at ±60° from North, i.e. (±4.33, 2.5).
        let t = tangent_points(c, p(0.0, 10.0), TOL);
        assert_eq!(t.len(), 2);
        let x = 5.0 * 30f64.to_radians().cos();
        assert!(has(&t, p(x, 2.5)) && has(&t, p(-x, 2.5)));
        assert_eq!(kinds(&t), ["TAN", "TAN"]);
        assert!(
            has(&tangent_points(c, p(5.0, 0.0), TOL), p(5.0, 0.0)),
            "on the circle"
        );
        assert!(tangent_points(c, p(1.0, 0.0), TOL).is_empty(), "inside");
        let l: Geometry2 = Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap().into();
        assert!(tangent_points(l, p(0.0, 5.0), TOL).is_empty());
    }

    #[test]
    fn intersections_include_overlap_ends() {
        let a: Geometry2 = Line::new(p(0.0, 0.0), p(5.0, 0.0)).unwrap().into();
        let b: Geometry2 = Line::new(p(2.0, 0.0), p(9.0, 0.0)).unwrap().into();
        let i = intersection_points(a, b, TOL);
        assert!(has(&i, p(2.0, 0.0)) && has(&i, p(5.0, 0.0)));
        assert!(i.iter().all(|s| s.kind == SnapKind::Intersection));
    }

    #[test]
    fn nearest_and_names() {
        let l: Geometry2 = Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into();
        let n = nearest_point(l, p(3.0, 4.0));
        assert_eq!((n.kind.name(), n.point), ("NEA", p(3.0, 0.0)));
    }
}
