//! `Point3`: a position in three-dimensional model space.

use std::ops::{Add, Sub};

use crate::error::{GeometryError, GeometryResult};
use crate::tolerance::Tolerance;
use crate::vector3::Vector3;

/// A 3D position in right-handed Cartesian model coordinates (Z up).
///
/// Positions and displacements are different types: point + vector = point,
/// point - point = vector, and point + point does not compile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Point3 {
    /// The origin (0, 0, 0).
    pub const ORIGIN: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// A point from its coordinates; rejects NaN and infinity.
    pub fn new(x: f64, y: f64, z: f64) -> GeometryResult<Self> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Self { x, y, z })
        } else {
            Err(GeometryError::NonFinite { what: "Point3" })
        }
    }

    /// Construction from coordinates already known to be finite.
    pub(crate) fn from_finite(x: f64, y: f64, z: f64) -> Self {
        debug_assert!(
            x.is_finite() && y.is_finite() && z.is_finite(),
            "non-finite Point3({x}, {y}, {z})"
        );
        Self { x, y, z }
    }

    /// X coordinate.
    pub fn x(self) -> f64 {
        self.x
    }

    /// Y coordinate.
    pub fn y(self) -> f64 {
        self.y
    }

    /// Z coordinate (positive up).
    pub fn z(self) -> f64 {
        self.z
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
        Self::from_finite(
            self.x * (1.0 - t) + other.x * t,
            self.y * (1.0 - t) + other.y * t,
            self.z * (1.0 - t) + other.z * t,
        )
    }

    /// The displacement from the origin to this point.
    pub fn to_vector(self) -> Vector3 {
        Vector3::from_finite(self.x, self.y, self.z)
    }

    /// Whether both points are within `tol.linear()` of each other.
    pub fn approx_eq(self, other: Self, tol: Tolerance) -> bool {
        self.distance_to(other) <= tol.linear()
    }
}

impl Add<Vector3> for Point3 {
    type Output = Self;
    fn add(self, v: Vector3) -> Self {
        Self::from_finite(self.x + v.x(), self.y + v.y(), self.z + v.z())
    }
}

impl Sub<Vector3> for Point3 {
    type Output = Self;
    fn sub(self, v: Vector3) -> Self {
        Self::from_finite(self.x - v.x(), self.y - v.y(), self.z - v.z())
    }
}

impl Sub for Point3 {
    type Output = Vector3;
    fn sub(self, other: Self) -> Vector3 {
        Vector3::from_finite(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    #[test]
    fn rejects_non_finite_coordinates() {
        for (x, y, z) in [
            (f64::NAN, 0.0, 0.0),
            (0.0, f64::INFINITY, 0.0),
            (0.0, 0.0, f64::NEG_INFINITY),
        ] {
            assert_eq!(
                Point3::new(x, y, z),
                Err(GeometryError::NonFinite { what: "Point3" })
            );
        }
    }

    #[test]
    fn point_vector_algebra_and_distance() {
        let a = p(1.0, 1.0, 1.0);
        let b = p(4.0, 5.0, 13.0);
        let d = b - a;
        assert_eq!(d, Vector3::new(3.0, 4.0, 12.0).unwrap());
        assert_eq!(a + d, b);
        assert_eq!(b - d, a);
        assert_eq!(a.distance_to(b), 13.0);
        assert_eq!(a.distance_squared_to(b), 169.0);
    }

    #[test]
    fn midpoint_lerp_and_approximate_equality() {
        let a = p(0.0, 0.0, -4.0);
        let b = p(10.0, -4.0, 8.0);
        assert_eq!(a.midpoint(b), p(5.0, -2.0, 2.0));
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);

        let survey = p(500_000.0, 9_800_000.0, 1_250.0);
        let near = p(500_000.0 + 5e-10, 9_800_000.0, 1_250.0);
        assert!(survey.approx_eq(near, Tolerance::DEFAULT));
    }
}
