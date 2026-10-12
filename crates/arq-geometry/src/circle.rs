//! `Circle`: a full circle, parameterized counterclockwise from East.

use std::f64::consts::{PI, TAU};

use crate::bounds::Bounds2;
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A circle with a finite, strictly positive radius.
///
/// Its parameter starts at angle 0 (East) and runs counterclockwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    center: Point2,
    radius: f64,
}

/// Whether `radius` is usable for a circle or an arc.
pub(crate) fn check_radius(radius: f64) -> GeometryResult<()> {
    if radius.is_finite() && radius > 0.0 {
        Ok(())
    } else {
        Err(GeometryError::InvalidRadius)
    }
}

impl Circle {
    /// A circle of `radius` around `center`.
    pub fn new(center: Point2, radius: f64) -> GeometryResult<Self> {
        check_radius(radius)?;
        Ok(Self { center, radius })
    }

    /// Center point.
    pub fn center(self) -> Point2 {
        self.center
    }

    /// Radius.
    pub fn radius(self) -> f64 {
        self.radius
    }

    /// Diameter.
    pub fn diameter(self) -> f64 {
        2.0 * self.radius
    }

    /// Circumference.
    pub fn length(self) -> f64 {
        TAU * self.radius
    }

    /// Enclosed area.
    pub fn area(self) -> f64 {
        PI * self.radius * self.radius
    }

    /// Bounding box.
    pub fn bounds(self) -> Bounds2 {
        let r = Vector2::from_finite(self.radius, self.radius);
        Bounds2::new(self.center - r, self.center + r)
    }

    /// Point at `angle` radians, counterclockwise from East.
    pub fn point_at_angle(self, angle: f64) -> Point2 {
        debug_assert!(angle.is_finite(), "non-finite angle {angle}");
        let (sin, cos) = angle.sin_cos();
        self.center + Vector2::from_finite(self.radius * cos, self.radius * sin)
    }

    /// Point at normalized parameter `t` (0 and 1 are both East).
    pub fn point_at(self, t: f64) -> Point2 {
        self.point_at_angle(t * TAU)
    }

    /// Point at `distance` along the circumference from East, counterclockwise.
    pub fn point_at_distance(self, distance: f64) -> GeometryResult<Point2> {
        if !(distance.is_finite() && (0.0..=self.length()).contains(&distance)) {
            return Err(GeometryError::OutOfRange { what: "distance" });
        }
        Ok(self.point_at_angle(distance / self.radius))
    }

    /// Point of the circle closest to `p`. Every point is equally close to
    /// the center itself; East is returned then.
    pub fn closest_point(self, p: Point2) -> Point2 {
        match (p - self.center).normalized() {
            Ok(dir) => self.center + dir * self.radius,
            Err(_) => self.point_at_angle(0.0),
        }
    }

    /// Shortest distance from `p` to the circle (zero on it, not inside it).
    pub fn distance_to(self, p: Point2) -> f64 {
        (p.distance_to(self.center) - self.radius).abs()
    }

    /// Whether `p` lies on the circle, within `tol.linear()`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        self.distance_to(p) <= tol.linear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn rejects_bad_radius() {
        for r in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Circle::new(p(0.0, 0.0), r),
                Err(GeometryError::InvalidRadius)
            );
        }
    }

    #[test]
    fn measures() {
        let c = Circle::new(p(1.0, 2.0), 3.0).unwrap();
        assert_eq!(c.diameter(), 6.0);
        assert_eq!(c.length(), TAU * 3.0);
        assert_eq!(c.area(), PI * 9.0);
        assert_eq!(c.bounds(), Bounds2::new(p(-2.0, -1.0), p(4.0, 5.0)));
    }

    #[test]
    fn parameterization_starts_east_and_runs_ccw() {
        let c = Circle::new(p(0.0, 0.0), 2.0).unwrap();
        assert!(c.point_at(0.0).approx_eq(p(2.0, 0.0), TOL));
        assert!(
            c.point_at(0.25).approx_eq(p(0.0, 2.0), TOL),
            "a quarter turn reaches North"
        );
        assert!(
            c.point_at_angle(FRAC_PI_2 * 3.0)
                .approx_eq(p(0.0, -2.0), TOL)
        );
        // Radius 2: a distance of π is a quarter turn (North), 2π half a turn (West).
        assert!(c.point_at_distance(PI).unwrap().approx_eq(p(0.0, 2.0), TOL));
        assert!(
            c.point_at_distance(2.0 * PI)
                .unwrap()
                .approx_eq(p(-2.0, 0.0), TOL)
        );
        assert!(c.point_at_distance(-1.0).is_err());
        assert!(c.point_at_distance(c.length() + 1.0).is_err());
    }

    #[test]
    fn closest_point_and_distance() {
        let c = Circle::new(p(0.0, 0.0), 5.0).unwrap();
        assert!(c.closest_point(p(10.0, 0.0)).approx_eq(p(5.0, 0.0), TOL));
        assert!(
            c.closest_point(p(0.0, 1.0)).approx_eq(p(0.0, 5.0), TOL),
            "from inside too"
        );
        assert_eq!(
            c.closest_point(c.center()),
            p(5.0, 0.0),
            "center: East by convention"
        );
        assert_eq!(c.distance_to(p(8.0, 0.0)), 3.0);
        assert_eq!(c.distance_to(p(1.0, 0.0)), 4.0);
        assert!(c.contains_point(p(3.0, 4.0), TOL));
        assert!(
            !c.contains_point(p(0.0, 0.0), TOL),
            "the center is not on the circle"
        );
    }
}
