//! `Point2`: a position in model space.

use std::ops::{Add, Sub};

use crate::error::{GeometryError, GeometryResult};
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A 2D position in model units.
///
/// Positions and displacements are different types: point + vector = point,
/// point - point = vector, and point + point does not compile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2 {
    x: f64,
    y: f64,
}

impl Point2 {
    /// The origin (0, 0).
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

    /// A point from its coordinates; rejects NaN and infinity.
    pub fn new(x: f64, y: f64) -> GeometryResult<Self> {
        if x.is_finite() && y.is_finite() {
            Ok(Self { x, y })
        } else {
            Err(GeometryError::NonFinite { what: "Point2" })
        }
    }

    /// Construction from coordinates already known to be finite.
    pub(crate) fn from_finite(x: f64, y: f64) -> Self {
        debug_assert!(
            x.is_finite() && y.is_finite(),
            "non-finite Point2({x}, {y})"
        );
        Self { x, y }
    }

    /// X coordinate.
    pub fn x(self) -> f64 {
        self.x
    }

    /// Y coordinate.
    pub fn y(self) -> f64 {
        self.y
    }

    /// Distance to `other`.
    pub fn distance_to(self, other: Self) -> f64 {
        (other - self).length()
    }

    /// Squared distance to `other`; cheaper for comparisons.
    pub fn distance_squared_to(self, other: Self) -> f64 {
        (other - self).length_squared()
    }

    /// The point halfway between `self` and `other`.
    pub fn midpoint(self, other: Self) -> Self {
        self.lerp(other, 0.5)
    }

    /// Linear interpolation: `self` at `t = 0`, `other` at `t = 1`.
    pub fn lerp(self, other: Self, t: f64) -> Self {
        debug_assert!(t.is_finite(), "non-finite interpolation parameter {t}");
        // Written as a weighted sum so t = 1 yields `other` exactly.
        Self::from_finite(
            self.x * (1.0 - t) + other.x * t,
            self.y * (1.0 - t) + other.y * t,
        )
    }

    /// The displacement from the origin to this point.
    pub fn to_vector(self) -> Vector2 {
        Vector2::from_finite(self.x, self.y)
    }

    /// Whether both points are within `tol.linear()` of each other.
    pub fn approx_eq(self, other: Self, tol: Tolerance) -> bool {
        self.distance_to(other) <= tol.linear()
    }
}

impl Add<Vector2> for Point2 {
    type Output = Self;
    fn add(self, v: Vector2) -> Self {
        Self::from_finite(self.x + v.x(), self.y + v.y())
    }
}

impl Sub<Vector2> for Point2 {
    type Output = Self;
    fn sub(self, v: Vector2) -> Self {
        Self::from_finite(self.x - v.x(), self.y - v.y())
    }
}

impl Sub for Point2 {
    type Output = Vector2;
    fn sub(self, other: Self) -> Vector2 {
        Vector2::from_finite(self.x - other.x, self.y - other.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn rejects_non_finite() {
        assert_eq!(
            Point2::new(f64::NAN, 0.0),
            Err(GeometryError::NonFinite { what: "Point2" })
        );
        assert!(Point2::new(0.0, f64::INFINITY).is_err());
    }

    #[test]
    fn point_vector_algebra() {
        let a = p(1.0, 1.0);
        let b = p(4.0, 5.0);
        let d = b - a;
        assert_eq!(d, Vector2::new(3.0, 4.0).unwrap());
        assert_eq!(a + d, b);
        assert_eq!(b - d, a);
        assert_eq!(a.distance_to(b), 5.0);
        assert_eq!(a.distance_squared_to(b), 25.0);
    }

    #[test]
    fn midpoint_and_lerp() {
        let a = p(0.0, 0.0);
        let b = p(10.0, -4.0);
        assert_eq!(a.midpoint(b), p(5.0, -2.0));
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b, "t = 1 must give the end point exactly");
        assert_eq!(a.lerp(b, 0.25), p(2.5, -1.0));
    }

    #[test]
    fn approx_eq_uses_linear_tolerance() {
        let a = p(1_000_000.0, 2_000_000.0);
        let near = p(1_000_000.0 + 5e-10, 2_000_000.0);
        let far = p(1_000_000.0 + 5e-9, 2_000_000.0);
        assert!(a.approx_eq(near, Tolerance::DEFAULT));
        assert!(!a.approx_eq(far, Tolerance::DEFAULT));
    }

    #[test]
    fn to_vector() {
        assert_eq!(p(2.0, 3.0).to_vector(), Vector2::new(2.0, 3.0).unwrap());
        assert_eq!(Point2::ORIGIN.to_vector(), Vector2::ZERO);
    }
}
