//! `Vector2`: a displacement (direction and length), not a position.

use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::angle;
use crate::error::{GeometryError, GeometryResult};
use crate::tolerance::Tolerance;

/// A 2D displacement in model units.
///
/// Built through [`Vector2::new`], which rejects NaN and infinity. Arithmetic
/// operators assume finite operands (asserted in debug builds); input from
/// outside the core is always validated through the constructors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector2 {
    x: f64,
    y: f64,
}

impl Vector2 {
    /// The zero vector.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    /// Unit vector pointing East (+X), angle 0.
    pub const X: Self = Self { x: 1.0, y: 0.0 };
    /// Unit vector pointing North (+Y), angle π/2.
    pub const Y: Self = Self { x: 0.0, y: 1.0 };

    /// A vector from its components; rejects NaN and infinity.
    pub fn new(x: f64, y: f64) -> GeometryResult<Self> {
        if x.is_finite() && y.is_finite() {
            Ok(Self { x, y })
        } else {
            Err(GeometryError::NonFinite { what: "Vector2" })
        }
    }

    /// A vector of `length` pointing at `angle` (radians, counterclockwise
    /// from East).
    pub fn from_polar(length: f64, angle: f64) -> GeometryResult<Self> {
        if !(length.is_finite() && angle.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Vector2" });
        }
        let (sin, cos) = angle.sin_cos();
        Self::new(length * cos, length * sin)
    }

    /// Construction from components already known to be finite.
    pub(crate) fn from_finite(x: f64, y: f64) -> Self {
        debug_assert!(
            x.is_finite() && y.is_finite(),
            "non-finite Vector2({x}, {y})"
        );
        Self { x, y }
    }

    /// X component.
    pub fn x(self) -> f64 {
        self.x
    }

    /// Y component.
    pub fn y(self) -> f64 {
        self.y
    }

    /// Length (Euclidean norm).
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// Squared length; cheaper than [`length`](Self::length) for comparisons.
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Dot product.
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// 2D cross product (the Z of the 3D cross product): positive when
    /// `other` is counterclockwise from `self`.
    pub fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    /// The same direction with length 1. Fails for the zero vector.
    pub fn normalized(self) -> GeometryResult<Self> {
        let len = self.length();
        if len > 0.0 {
            Ok(Self::from_finite(self.x / len, self.y / len))
        } else {
            Err(GeometryError::ZeroVector)
        }
    }

    /// Direction in `[0, 2π)`, counterclockwise from East. The zero vector
    /// returns 0.
    pub fn angle(self) -> f64 {
        angle::normalize(self.y.atan2(self.x))
    }

    /// Signed angle in `(-π, π]` that rotates `self` onto `other`'s direction.
    pub fn angle_to(self, other: Self) -> f64 {
        self.cross(other).atan2(self.dot(other))
    }

    /// This vector rotated by `angle` radians, counterclockwise.
    pub fn rotated(self, angle: f64) -> Self {
        debug_assert!(angle.is_finite(), "non-finite rotation angle {angle}");
        let (sin, cos) = angle.sin_cos();
        Self::from_finite(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
    }

    /// This vector rotated 90° counterclockwise (exact, no trigonometry).
    pub fn perp(self) -> Self {
        Self::from_finite(-self.y, self.x)
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

impl Add for Vector2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::from_finite(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vector2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::from_finite(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Neg for Vector2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::from_finite(-self.x, -self.y)
    }
}

impl Mul<f64> for Vector2 {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::from_finite(self.x * k, self.y * k)
    }
}

impl Mul<Vector2> for f64 {
    type Output = Vector2;
    fn mul(self, v: Vector2) -> Vector2 {
        v * self
    }
}

impl Div<f64> for Vector2 {
    type Output = Self;
    fn div(self, k: f64) -> Self {
        debug_assert!(k != 0.0, "Vector2 divided by zero");
        Self::from_finite(self.x / k, self.y / k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn v(x: f64, y: f64) -> Vector2 {
        Vector2::new(x, y).unwrap()
    }

    #[test]
    fn rejects_non_finite() {
        for (x, y) in [
            (f64::NAN, 0.0),
            (0.0, f64::INFINITY),
            (f64::NEG_INFINITY, 1.0),
        ] {
            assert_eq!(
                Vector2::new(x, y),
                Err(GeometryError::NonFinite { what: "Vector2" })
            );
        }
        assert!(Vector2::from_polar(f64::NAN, 0.0).is_err());
        assert!(Vector2::from_polar(1.0, f64::INFINITY).is_err());
    }

    #[test]
    fn length_dot_cross() {
        let a = v(3.0, 4.0);
        assert_eq!(a.length(), 5.0);
        assert_eq!(a.length_squared(), 25.0);
        assert_eq!(a.dot(v(1.0, 0.0)), 3.0);
        assert_eq!(
            Vector2::X.cross(Vector2::Y),
            1.0,
            "Y is counterclockwise from X"
        );
        assert_eq!(Vector2::Y.cross(Vector2::X), -1.0);
    }

    #[test]
    fn angles_follow_east_counterclockwise() {
        assert_eq!(Vector2::X.angle(), 0.0);
        assert_eq!(Vector2::Y.angle(), FRAC_PI_2);
        assert_eq!(v(-1.0, 0.0).angle(), PI);
        assert!((v(0.0, -1.0).angle() - 3.0 * FRAC_PI_2).abs() < 1e-15);
        assert_eq!(Vector2::ZERO.angle(), 0.0);
        assert!((Vector2::X.angle_to(Vector2::Y) - FRAC_PI_2).abs() < 1e-15);
        assert!((Vector2::Y.angle_to(Vector2::X) + FRAC_PI_2).abs() < 1e-15);
    }

    #[test]
    fn rotation_and_perp() {
        let r = Vector2::X.rotated(FRAC_PI_2);
        assert!(r.approx_eq(Vector2::Y, Tolerance::DEFAULT));
        assert_eq!(v(2.0, 3.0).perp(), v(-3.0, 2.0));
        let p = Vector2::from_polar(2.0, PI).unwrap();
        assert!(p.approx_eq(v(-2.0, 0.0), Tolerance::DEFAULT));
    }

    #[test]
    fn normalized_and_zero() {
        let n = v(3.0, 4.0).normalized().unwrap();
        assert!((n.length() - 1.0).abs() < 1e-15);
        assert_eq!(Vector2::ZERO.normalized(), Err(GeometryError::ZeroVector));
        assert!(v(1e-10, 0.0).is_zero(Tolerance::DEFAULT));
        assert!(!v(1e-8, 0.0).is_zero(Tolerance::DEFAULT));
    }

    #[test]
    fn operators() {
        let a = v(1.0, 2.0);
        let b = v(3.0, -1.0);
        assert_eq!(a + b, v(4.0, 1.0));
        assert_eq!(a - b, v(-2.0, 3.0));
        assert_eq!(-a, v(-1.0, -2.0));
        assert_eq!(a * 2.0, v(2.0, 4.0));
        assert_eq!(2.0 * a, v(2.0, 4.0));
        assert_eq!(a / 2.0, v(0.5, 1.0));
    }
}
