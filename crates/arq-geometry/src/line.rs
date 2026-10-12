//! `Line`: a straight segment between two points.

use crate::bounds::Bounds2;
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A line segment from `start` to `end`, with non-zero length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    start: Point2,
    end: Point2,
}

impl Line {
    /// A segment between two different points.
    pub fn new(start: Point2, end: Point2) -> GeometryResult<Self> {
        if start == end {
            Err(GeometryError::ZeroLength)
        } else {
            Ok(Self { start, end })
        }
    }

    /// First point.
    pub fn start(self) -> Point2 {
        self.start
    }

    /// Last point.
    pub fn end(self) -> Point2 {
        self.end
    }

    /// Displacement from `start` to `end`.
    pub fn vector(self) -> Vector2 {
        self.end - self.start
    }

    /// Unit vector from `start` towards `end`.
    pub fn direction(self) -> Vector2 {
        // Non-zero by construction.
        self.vector() / self.length()
    }

    /// Length of the segment.
    pub fn length(self) -> f64 {
        self.vector().length()
    }

    /// Point halfway along the segment.
    pub fn midpoint(self) -> Point2 {
        self.start.midpoint(self.end)
    }

    /// Bounding box.
    pub fn bounds(self) -> Bounds2 {
        Bounds2::new(self.start, self.end)
    }

    /// Point at normalized parameter `t`: `start` at 0, `end` at 1.
    pub fn point_at(self, t: f64) -> Point2 {
        self.start.lerp(self.end, t)
    }

    /// Point at `distance` along the segment from `start`.
    pub fn point_at_distance(self, distance: f64) -> GeometryResult<Point2> {
        let length = self.length();
        if !(distance.is_finite() && (0.0..=length).contains(&distance)) {
            return Err(GeometryError::OutOfRange { what: "distance" });
        }
        Ok(self.point_at(distance / length))
    }

    /// Parameter in `[0, 1]` of the point of the segment closest to `p`.
    pub fn closest_parameter(self, p: Point2) -> f64 {
        // Relative to `start`: the local frame keeps large coordinates precise.
        let v = self.vector();
        ((p - self.start).dot(v) / v.length_squared()).clamp(0.0, 1.0)
    }

    /// Point of the segment closest to `p`.
    pub fn closest_point(self, p: Point2) -> Point2 {
        self.point_at(self.closest_parameter(p))
    }

    /// Shortest distance from `p` to the segment.
    pub fn distance_to(self, p: Point2) -> f64 {
        p.distance_to(self.closest_point(p))
    }

    /// Whether `p` lies on the segment, within `tol.linear()`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        self.distance_to(p) <= tol.linear()
    }

    /// The same segment, from `end` to `start`.
    pub fn reversed(self) -> Self {
        Self {
            start: self.end,
            end: self.start,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Line {
        Line::new(p(x0, y0), p(x1, y1)).unwrap()
    }

    #[test]
    fn rejects_zero_length() {
        assert_eq!(
            Line::new(p(1.0, 1.0), p(1.0, 1.0)),
            Err(GeometryError::ZeroLength)
        );
    }

    #[test]
    fn measures() {
        let l = line(0.0, 0.0, 3.0, 4.0);
        assert_eq!(l.length(), 5.0);
        assert_eq!(l.midpoint(), p(1.5, 2.0));
        assert!(
            l.direction()
                .approx_eq(Vector2::new(0.6, 0.8).unwrap(), TOL)
        );
        assert_eq!(l.bounds(), Bounds2::new(p(0.0, 0.0), p(3.0, 4.0)));
        assert_eq!(l.reversed().start(), p(3.0, 4.0));
    }

    #[test]
    fn points_along() {
        let l = line(0.0, 0.0, 10.0, 0.0);
        assert_eq!(l.point_at(0.0), l.start());
        assert_eq!(l.point_at(1.0), l.end());
        assert_eq!(l.point_at_distance(2.5).unwrap(), p(2.5, 0.0));
        assert_eq!(l.point_at_distance(10.0).unwrap(), l.end());
        for bad in [-0.1, 10.1, f64::NAN] {
            assert_eq!(
                l.point_at_distance(bad),
                Err(GeometryError::OutOfRange { what: "distance" })
            );
        }
    }

    #[test]
    fn closest_point_clamps_to_the_segment() {
        let l = line(0.0, 0.0, 10.0, 0.0);
        assert_eq!(l.closest_point(p(4.0, 3.0)), p(4.0, 0.0));
        assert_eq!(l.distance_to(p(4.0, 3.0)), 3.0);
        assert_eq!(l.closest_point(p(-5.0, 1.0)), l.start(), "before the start");
        assert_eq!(l.closest_point(p(15.0, -1.0)), l.end(), "after the end");
        assert!(l.contains_point(p(7.0, 0.0), TOL));
        assert!(!l.contains_point(p(7.0, 1e-6), TOL));
        assert!(
            !l.contains_point(p(11.0, 0.0), TOL),
            "the segment does not extend"
        );
    }

    #[test]
    fn precise_at_survey_coordinates() {
        let base = (500_000.0, 9_800_000.0); // UTM, zone 17S (Ecuador)
        let l = line(base.0, base.1, base.0 + 10.0, base.1);
        let q = p(base.0 + 4.0, base.1 + 3.0);
        assert!((l.distance_to(q) - 3.0).abs() < 1e-8);
        assert!(l.contains_point(p(base.0 + 7.0, base.1), TOL));
    }
}
