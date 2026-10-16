//! `Vector3`: a three-dimensional displacement, not a position.

use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::error::{GeometryError, GeometryResult};
use crate::tolerance::Tolerance;

/// A 3D displacement in right-handed Cartesian model coordinates (Z up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vector3 {
    /// The zero vector.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    /// Unit vector along positive X.
    pub const X: Self = Self {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    /// Unit vector along positive Y.
    pub const Y: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    /// Unit vector along positive Z (up).
    pub const Z: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// A vector from its components; rejects NaN and infinity.
    pub fn new(x: f64, y: f64, z: f64) -> GeometryResult<Self> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Self { x, y, z })
        } else {
            Err(GeometryError::NonFinite { what: "Vector3" })
        }
    }

    /// Construction from components already known to be finite.
    pub(crate) fn from_finite(x: f64, y: f64, z: f64) -> Self {
        debug_assert!(
            x.is_finite() && y.is_finite() && z.is_finite(),
            "non-finite Vector3({x}, {y}, {z})"
        );
        Self { x, y, z }
    }

    /// X component.
    pub fn x(self) -> f64 {
        self.x
    }

    /// Y component.
    pub fn y(self) -> f64 {
        self.y
    }

    /// Z component.
    pub fn z(self) -> f64 {
        self.z
    }

    /// Length (Euclidean norm).
    pub fn length(self) -> f64 {
        self.x.hypot(self.y).hypot(self.z)
    }

    /// Squared length; cheaper for comparisons.
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Dot product.
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Right-handed cross product (`X × Y = +Z`).
    pub fn cross(self, other: Self) -> Self {
        Self::from_finite(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// The same direction with length 1. Fails for the zero vector.
    pub fn normalized(self) -> GeometryResult<Self> {
        let len = self.length();
        if len > 0.0 {
            Ok(Self::from_finite(self.x / len, self.y / len, self.z / len))
        } else {
            Err(GeometryError::ZeroVector)
        }
    }

    /// Whether both vectors differ by at most `tol.linear()`.
    pub fn approx_eq(self, other: Self, tol: Tolerance) -> bool {
        (self - other).length() <= tol.linear()
    }

    /// Whether the length is at most `tol.linear()`.
    pub fn is_zero(self, tol: Tolerance) -> bool {
        self.length() <= tol.linear()
    }
}

impl Add for Vector3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::from_finite(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vector3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::from_finite(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Neg for Vector3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::from_finite(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vector3 {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::from_finite(self.x * k, self.y * k, self.z * k)
    }
}

impl Mul<Vector3> for f64 {
    type Output = Vector3;
    fn mul(self, v: Vector3) -> Vector3 {
        v * self
    }
}

impl Div<f64> for Vector3 {
    type Output = Self;
    fn div(self, k: f64) -> Self {
        debug_assert!(k != 0.0, "Vector3 divided by zero");
        Self::from_finite(self.x / k, self.y / k, self.z / k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3::new(x, y, z).unwrap()
    }

    #[test]
    fn rejects_non_finite_components() {
        for (x, y, z) in [
            (f64::NAN, 0.0, 0.0),
            (0.0, f64::INFINITY, 0.0),
            (0.0, 0.0, f64::NEG_INFINITY),
        ] {
            assert_eq!(
                Vector3::new(x, y, z),
                Err(GeometryError::NonFinite { what: "Vector3" })
            );
        }
    }

    #[test]
    fn length_dot_cross_and_right_handed_axes() {
        let a = v(2.0, 3.0, 6.0);
        assert_eq!(a.length(), 7.0);
        assert_eq!(a.length_squared(), 49.0);
        assert_eq!(a.dot(v(1.0, 0.0, 1.0)), 8.0);
        assert_eq!(Vector3::X.cross(Vector3::Y), Vector3::Z);
        assert_eq!(Vector3::Y.cross(Vector3::X), -Vector3::Z);
        assert_eq!(Vector3::X.dot(Vector3::Y), 0.0);
    }

    #[test]
    fn normalized_and_zero() {
        let n = v(2.0, 3.0, 6.0).normalized().unwrap();
        assert!((n.length() - 1.0).abs() < 1e-15);
        assert_eq!(Vector3::ZERO.normalized(), Err(GeometryError::ZeroVector));
        assert!(v(1e-10, 0.0, 0.0).is_zero(Tolerance::DEFAULT));
        assert!(!v(1e-8, 0.0, 0.0).is_zero(Tolerance::DEFAULT));
    }

    #[test]
    fn arithmetic() {
        let a = v(1.0, 2.0, 3.0);
        let b = v(3.0, -1.0, 2.0);
        assert_eq!(a + b, v(4.0, 1.0, 5.0));
        assert_eq!(a - b, v(-2.0, 3.0, 1.0));
        assert_eq!(-a, v(-1.0, -2.0, -3.0));
        assert_eq!(a * 2.0, v(2.0, 4.0, 6.0));
        assert_eq!(2.0 * a, v(2.0, 4.0, 6.0));
        assert_eq!(a / 2.0, v(0.5, 1.0, 1.5));
    }
}
