//! `Arc`: part of a circle, counterclockwise from `start_angle` to
//! `end_angle` (DWG convention, ADR 0010).

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::angle;
use crate::bounds::Bounds2;
use crate::circle::{Circle, check_radius};
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A circular arc running **counterclockwise** from `start_angle` to
/// `end_angle`. Both angles are normalized to `[0, 2π)`; the sweep is in
/// `(0, 2π)` (a full turn is a [`Circle`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    center: Point2,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
}

impl Arc {
    /// An arc around `center`, counterclockwise from `start_angle` to
    /// `end_angle` (radians, from East).
    pub fn new(
        center: Point2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> GeometryResult<Self> {
        check_radius(radius)?;
        if !(start_angle.is_finite() && end_angle.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Arc" });
        }
        let (start_angle, end_angle) = (angle::normalize(start_angle), angle::normalize(end_angle));
        if start_angle == end_angle {
            return Err(GeometryError::ZeroSweep);
        }
        Ok(Self {
            center,
            radius,
            start_angle,
            end_angle,
        })
    }

    /// The arc that passes through `a`, then `b`, then `c` (the ARC command's
    /// three-point mode). Stored counterclockwise: when `a → b → c` turns
    /// clockwise, the result runs from `c` to `a`.
    pub fn from_three_points(
        a: Point2,
        b: Point2,
        c: Point2,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        // Local frame at `a`: keeps survey coordinates precise.
        let ab = b - a;
        let ac = c - a;
        let cross = ab.cross(ac);
        let scale = ab.length() * ac.length();
        if scale == 0.0 || (cross / scale).abs() <= tol.angular() {
            return Err(GeometryError::Collinear);
        }
        let (ab2, ac2) = (ab.length_squared(), ac.length_squared());
        let d = 2.0 * cross;
        let offset = Vector2::new(
            (ac.y() * ab2 - ab.y() * ac2) / d,
            (ab.x() * ac2 - ac.x() * ab2) / d,
        )?;
        let center = a + offset;
        let angle_of = |p: Point2| (p - center).angle();
        let (start, end) = if cross > 0.0 { (a, c) } else { (c, a) };
        Self::new(center, offset.length(), angle_of(start), angle_of(end))
    }

    /// Center point.
    pub fn center(self) -> Point2 {
        self.center
    }

    /// Radius.
    pub fn radius(self) -> f64 {
        self.radius
    }

    /// Start angle in `[0, 2π)`.
    pub fn start_angle(self) -> f64 {
        self.start_angle
    }

    /// End angle in `[0, 2π)`.
    pub fn end_angle(self) -> f64 {
        self.end_angle
    }

    /// Counterclockwise angle from start to end, in `(0, 2π)`.
    pub fn sweep(self) -> f64 {
        angle::ccw_sweep(self.start_angle, self.end_angle)
    }

    /// The full circle this arc belongs to.
    pub fn circle(self) -> Circle {
        Circle::new(self.center, self.radius).expect("arc radius is valid")
    }

    /// Point at the start angle.
    pub fn start_point(self) -> Point2 {
        self.circle().point_at_angle(self.start_angle)
    }

    /// Point at the end angle.
    pub fn end_point(self) -> Point2 {
        self.circle().point_at_angle(self.end_angle)
    }

    /// Point halfway along the arc.
    pub fn midpoint(self) -> Point2 {
        self.point_at(0.5)
    }

    /// Arc length.
    pub fn length(self) -> f64 {
        self.radius * self.sweep()
    }

    /// Whether the direction `angle` (radians) falls within the arc's sweep,
    /// within `tol.angular()` beyond either end.
    pub fn contains_angle(self, angle: f64, tol: Tolerance) -> bool {
        let offset = angle::ccw_sweep(self.start_angle, angle);
        offset <= self.sweep() + tol.angular() || offset >= TAU - tol.angular()
    }

    /// Bounding box: the end points plus every axis extreme (East, North,
    /// West, South) the arc passes through.
    pub fn bounds(self) -> Bounds2 {
        let sweep = self.sweep();
        let quadrants = (0..4)
            .map(|i| f64::from(i) * FRAC_PI_2)
            .filter(|&q| angle::ccw_sweep(self.start_angle, q) <= sweep)
            .map(|q| self.circle().point_at_angle(q));
        Bounds2::from_points(
            [self.start_point(), self.end_point()]
                .into_iter()
                .chain(quadrants),
        )
        .expect("at least two points")
    }

    /// Point at normalized parameter `t`: start at 0, end at 1.
    pub fn point_at(self, t: f64) -> Point2 {
        if t == 1.0 {
            return self.end_point();
        }
        self.circle()
            .point_at_angle(self.start_angle + t * self.sweep())
    }

    /// Point at `distance` along the arc from its start.
    pub fn point_at_distance(self, distance: f64) -> GeometryResult<Point2> {
        if !(distance.is_finite() && (0.0..=self.length()).contains(&distance)) {
            return Err(GeometryError::OutOfRange { what: "distance" });
        }
        Ok(self.point_at(distance / self.length()))
    }

    /// Point of the arc closest to `p`: the radial projection when it falls
    /// within the sweep, otherwise the nearer end point. For the center
    /// itself (every point equally close) the start point is returned.
    pub fn closest_point(self, p: Point2) -> Point2 {
        let Ok(dir) = (p - self.center).normalized() else {
            return self.start_point();
        };
        if angle::ccw_sweep(self.start_angle, dir.angle()) <= self.sweep() {
            return self.center + dir * self.radius;
        }
        let (s, e) = (self.start_point(), self.end_point());
        if p.distance_squared_to(s) <= p.distance_squared_to(e) {
            s
        } else {
            e
        }
    }

    /// Shortest distance from `p` to the arc.
    pub fn distance_to(self, p: Point2) -> f64 {
        p.distance_to(self.closest_point(p))
    }

    /// Whether `p` lies on the arc, within `tol.linear()`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        self.distance_to(p) <= tol.linear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn deg(d: f64) -> f64 {
        d.to_radians()
    }

    #[test]
    fn rejects_invalid_arcs() {
        let c = p(0.0, 0.0);
        assert_eq!(
            Arc::new(c, 0.0, 0.0, 1.0),
            Err(GeometryError::InvalidRadius)
        );
        assert_eq!(
            Arc::new(c, 1.0, f64::NAN, 1.0),
            Err(GeometryError::NonFinite { what: "Arc" })
        );
        assert_eq!(Arc::new(c, 1.0, 1.0, 1.0), Err(GeometryError::ZeroSweep));
        assert_eq!(
            Arc::new(c, 1.0, 0.0, TAU),
            Err(GeometryError::ZeroSweep),
            "a full turn is a circle"
        );
    }

    #[test]
    fn angles_are_normalized() {
        let a = Arc::new(p(0.0, 0.0), 1.0, -FRAC_PI_2, 5.0 * PI).unwrap();
        assert!((a.start_angle() - 3.0 * FRAC_PI_2).abs() < 1e-12);
        assert!((a.end_angle() - PI).abs() < 1e-12);
    }

    #[test]
    fn sweep_and_points_follow_dwg() {
        // 0° to 90°: a quarter, counterclockwise through 45°.
        let a = Arc::new(p(0.0, 0.0), 10.0, 0.0, FRAC_PI_2).unwrap();
        assert!((a.sweep() - FRAC_PI_2).abs() < 1e-15);
        assert!(a.start_point().approx_eq(p(10.0, 0.0), TOL));
        assert!(a.end_point().approx_eq(p(0.0, 10.0), TOL));
        assert!((a.length() - 5.0 * PI).abs() < 1e-12);
        let m = a.midpoint();
        assert!(m.approx_eq(p(10.0 * deg(45.0).cos(), 10.0 * deg(45.0).sin()), TOL));
        // 90° to 0° is the other three quarters, through 180°.
        let b = Arc::new(p(0.0, 0.0), 10.0, FRAC_PI_2, 0.0).unwrap();
        assert!((b.sweep() - 3.0 * FRAC_PI_2).abs() < 1e-12);
        assert!(
            b.midpoint()
                .approx_eq(p(10.0 * deg(225.0).cos(), 10.0 * deg(225.0).sin()), TOL)
        );
    }

    #[test]
    fn crossing_east_wraps_around() {
        // 350° to 10° counterclockwise passes through East (0°).
        let a = Arc::new(p(0.0, 0.0), 1.0, deg(350.0), deg(10.0)).unwrap();
        assert!((a.sweep() - deg(20.0)).abs() < 1e-12);
        assert!(a.contains_angle(0.0, TOL));
        assert!(!a.contains_angle(PI, TOL));
        assert!(a.midpoint().approx_eq(p(1.0, 0.0), TOL));
    }

    #[test]
    fn bounds_include_crossed_quadrants_only() {
        let c = p(0.0, 0.0);
        // 45° → 135° crosses North only.
        let a = Arc::new(c, 1.0, deg(45.0), deg(135.0)).unwrap();
        let b = a.bounds();
        assert!((b.max().y() - 1.0).abs() < 1e-15, "reaches North");
        assert!((b.min().y() - deg(45.0).sin()).abs() < 1e-15);
        assert!((b.width() - 2.0 * deg(45.0).cos()).abs() < 1e-15);
        // 350° → 10° crosses East only.
        let e = Arc::new(c, 1.0, deg(350.0), deg(10.0)).unwrap().bounds();
        assert!((e.max().x() - 1.0).abs() < 1e-15);
        assert!(e.min().x() > 0.9);
    }

    #[test]
    fn points_along() {
        let a = Arc::new(p(0.0, 0.0), 2.0, 0.0, PI).unwrap();
        assert_eq!(a.point_at(0.0), a.start_point());
        assert_eq!(a.point_at(1.0), a.end_point());
        assert!(a.point_at_distance(PI).unwrap().approx_eq(p(0.0, 2.0), TOL));
        assert!(a.point_at_distance(a.length() + 0.1).is_err());
    }

    #[test]
    fn closest_point_inside_and_outside_the_sweep() {
        let a = Arc::new(p(0.0, 0.0), 5.0, 0.0, FRAC_PI_2).unwrap();
        assert!(
            a.closest_point(p(10.0, 10.0))
                .approx_eq(p(5.0 * deg(45.0).cos(), 5.0 * deg(45.0).sin()), TOL)
        );
        // Outside the sweep: the nearer end point.
        assert!(
            a.closest_point(p(3.0, -4.0)).approx_eq(p(5.0, 0.0), TOL),
            "start is nearer"
        );
        assert!(
            a.closest_point(p(-3.0, -1.0)).approx_eq(p(0.0, 5.0), TOL),
            "end is nearer"
        );
        assert!(a.closest_point(p(-1.0, 3.0)).approx_eq(p(0.0, 5.0), TOL));
        assert_eq!(a.closest_point(a.center()), a.start_point());
        assert!(a.contains_point(p(3.0, 4.0), TOL));
        assert!(
            !a.contains_point(p(-3.0, -4.0), TOL),
            "on the circle but not on the arc"
        );
    }

    #[test]
    fn three_points_ccw_and_cw() {
        let (a, b, c) = (p(1.0, 0.0), p(0.0, 1.0), p(-1.0, 0.0));
        let ccw = Arc::from_three_points(a, b, c, TOL).unwrap();
        assert!(ccw.center().approx_eq(p(0.0, 0.0), TOL));
        assert!((ccw.radius() - 1.0).abs() < 1e-12);
        assert!(ccw.start_point().approx_eq(a, TOL) && ccw.end_point().approx_eq(c, TOL));
        assert!(ccw.contains_point(b, TOL));
        // Same points through the South: clockwise, stored from c to a.
        let cw = Arc::from_three_points(a, p(0.0, -1.0), c, TOL).unwrap();
        assert!(cw.start_point().approx_eq(c, TOL) && cw.end_point().approx_eq(a, TOL));
        assert!(cw.contains_point(p(0.0, -1.0), TOL));
    }

    #[test]
    fn three_points_rejects_collinear() {
        let r = Arc::from_three_points(p(0.0, 0.0), p(1.0, 1.0), p(2.0, 2.0), TOL);
        assert_eq!(r, Err(GeometryError::Collinear));
        let r = Arc::from_three_points(p(0.0, 0.0), p(0.0, 0.0), p(2.0, 2.0), TOL);
        assert_eq!(r, Err(GeometryError::Collinear), "repeated point");
    }

    #[test]
    fn three_points_at_survey_coordinates() {
        let o = (500_000.0, 9_800_000.0);
        let (a, b, c) = (p(o.0 + 10.0, o.1), p(o.0, o.1 + 10.0), p(o.0 - 10.0, o.1));
        let arc = Arc::from_three_points(a, b, c, TOL).unwrap();
        assert!(arc.center().distance_to(p(o.0, o.1)) < 1e-8);
        assert!((arc.radius() - 10.0).abs() < 1e-8);
    }
}
