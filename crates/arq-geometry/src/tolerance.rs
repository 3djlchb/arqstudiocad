//! Explicit tolerances for comparisons and intersections (ADR 0010).

use crate::error::{GeometryError, GeometryResult};

/// How close two values must be to count as equal.
///
/// Passed explicitly to every operation that compares or intersects; the
/// code never uses ad-hoc epsilons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    linear: f64,
    angular: f64,
}

impl Tolerance {
    /// 1e-9 model units and 1e-10 radians: meaningful even at survey
    /// coordinates (~1e6), where `f64` still resolves about 1e-10.
    pub const DEFAULT: Self = Self {
        linear: 1e-9,
        angular: 1e-10,
    };

    /// A tolerance with the given linear (model units) and angular (radians) limits.
    pub fn new(linear: f64, angular: f64) -> GeometryResult<Self> {
        let valid = |v: f64| v.is_finite() && v > 0.0;
        if valid(linear) && valid(angular) {
            Ok(Self { linear, angular })
        } else {
            Err(GeometryError::InvalidTolerance)
        }
    }

    /// Largest distance, in model units, still considered zero.
    pub fn linear(self) -> f64 {
        self.linear
    }

    /// Largest angle, in radians, still considered zero.
    pub fn angular(self) -> f64 {
        self.angular
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let tol = Tolerance::default();
        assert_eq!(tol.linear(), 1e-9);
        assert_eq!(tol.angular(), 1e-10);
    }

    #[test]
    fn rejects_zero_negative_and_non_finite() {
        for (l, a) in [
            (0.0, 1e-10),
            (1e-9, -1.0),
            (f64::NAN, 1e-10),
            (1e-9, f64::INFINITY),
        ] {
            assert_eq!(Tolerance::new(l, a), Err(GeometryError::InvalidTolerance));
        }
        assert!(Tolerance::new(1e-6, 1e-8).is_ok());
    }
}
