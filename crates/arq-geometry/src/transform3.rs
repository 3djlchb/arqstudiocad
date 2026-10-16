//! `Transform3`: local-frame affine transformations in three dimensions.

use crate::error::{GeometryError, GeometryResult};
use crate::point3::Point3;
use crate::tolerance::Tolerance;
use crate::vector3::Vector3;

/// A finite affine transformation in three dimensions.
///
/// Points are mapped as `pivot + M * (point - pivot) + offset`. Keeping the
/// pivot explicit avoids folding large world coordinates into a translation
/// vector, which helps preserve precision in survey and georeferenced models.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3 {
    linear: [[f64; 3]; 3],
    pivot: Point3,
    offset: Vector3,
}

impl Transform3 {
    /// Leaves every point and vector unchanged.
    pub const IDENTITY: Self = Self {
        linear: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        pivot: Point3::ORIGIN,
        offset: Vector3::ZERO,
    };

    /// Creates an affine map from a finite linear matrix, pivot and offset.
    ///
    /// Singular maps are allowed, since they can represent projections or
    /// collapsed results; [`inverse`](Self::inverse) reports them explicitly.
    pub fn new(linear: [[f64; 3]; 3], pivot: Point3, offset: Vector3) -> GeometryResult<Self> {
        if linear.iter().flatten().all(|value| value.is_finite()) {
            Ok(Self {
                linear,
                pivot,
                offset,
            })
        } else {
            Err(GeometryError::NonFinite { what: "Transform3" })
        }
    }

    /// Moves points by `offset` (translations do not affect vectors).
    pub fn translation(offset: Vector3) -> Self {
        Self {
            offset,
            ..Self::IDENTITY
        }
    }

    /// Rotates around `axis` through `center` by `angle` radians.
    ///
    /// The axis must be non-zero. Positive rotation follows the
    /// right-hand rule; for example, +90° about positive Z maps +X to +Y.
    pub fn rotation_about(center: Point3, axis: Vector3, angle: f64) -> GeometryResult<Self> {
        if !angle.is_finite() {
            return Err(GeometryError::NonFinite {
                what: "rotation angle",
            });
        }
        let axis = axis.normalized()?;
        let (x, y, z) = (axis.x(), axis.y(), axis.z());
        let (sin, cos) = angle.sin_cos();
        let t = 1.0 - cos;
        Self::new(
            [
                [t * x * x + cos, t * x * y - sin * z, t * x * z + sin * y],
                [t * x * y + sin * z, t * y * y + cos, t * y * z - sin * x],
                [t * x * z - sin * y, t * y * z + sin * x, t * z * z + cos],
            ],
            center,
            Vector3::ZERO,
        )
    }

    /// Rotates around `axis` through the origin by `angle` radians.
    pub fn rotation(axis: Vector3, angle: f64) -> GeometryResult<Self> {
        Self::rotation_about(Point3::ORIGIN, axis, angle)
    }

    /// Scales independently along X, Y and Z around `center`.
    ///
    /// Every factor must be finite and strictly positive. Reflections can be
    /// represented by [`new`](Self::new) with a negative determinant.
    pub fn scaling(center: Point3, factors: Vector3) -> GeometryResult<Self> {
        if !(factors.x().is_finite()
            && factors.y().is_finite()
            && factors.z().is_finite()
            && factors.x() > 0.0
            && factors.y() > 0.0
            && factors.z() > 0.0)
        {
            return Err(GeometryError::InvalidScale);
        }
        Self::new(
            [
                [factors.x(), 0.0, 0.0],
                [0.0, factors.y(), 0.0],
                [0.0, 0.0, factors.z()],
            ],
            center,
            Vector3::ZERO,
        )
    }

    /// The 3×3 linear part of this transformation.
    pub fn linear_part(self) -> [[f64; 3]; 3] {
        self.linear
    }

    /// The point about which the linear part is applied.
    pub fn pivot(self) -> Point3 {
        self.pivot
    }

    /// The translation applied after the linear part.
    pub fn offset(self) -> Vector3 {
        self.offset
    }

    /// Image of a point, computed relative to the local pivot.
    pub fn apply_point(self, point: Point3) -> Point3 {
        self.pivot + self.mul(point - self.pivot) + self.offset
    }

    /// Image of a displacement. Translation does not affect vectors.
    pub fn apply_vector(self, vector: Vector3) -> Vector3 {
        self.mul(vector)
    }

    /// `self` followed by `next`, composed as one affine map.
    pub fn then(self, next: Self) -> Self {
        let pivot_delta = self.pivot - next.pivot;
        Self {
            linear: mul_matrix(next.linear, self.linear),
            pivot: self.pivot,
            offset: (next.pivot - self.pivot) + next.mul(pivot_delta + self.offset) + next.offset,
        }
    }

    /// The transformation that undoes this one, or an error if it is singular.
    pub fn inverse(self) -> GeometryResult<Self> {
        let [[a, b, c], [d, e, f], [g, h, i]] = self.linear;
        let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        if !determinant.is_finite() {
            return Err(GeometryError::NonFinite {
                what: "Transform3 determinant",
            });
        }
        if determinant == 0.0 {
            return Err(GeometryError::SingularTransform);
        }
        let inverse = [
            [e * i - f * h, c * h - b * i, b * f - c * e],
            [f * g - d * i, a * i - c * g, c * d - a * f],
            [d * h - e * g, b * g - a * h, a * e - b * d],
        ]
        .map(|row| row.map(|value| value / determinant));
        let pivot = Point3::new(
            self.pivot.x() + self.offset.x(),
            self.pivot.y() + self.offset.y(),
            self.pivot.z() + self.offset.z(),
        )?;
        Self::new(inverse, pivot, -self.offset)
    }

    /// Whether this map sends the sample points to the same places within
    /// `tol.linear()` as `other`.
    pub fn approx_eq_at(self, other: Self, samples: &[Point3], tol: Tolerance) -> bool {
        samples.iter().all(|&point| {
            self.apply_point(point)
                .approx_eq(other.apply_point(point), tol)
        })
    }

    fn mul(self, vector: Vector3) -> Vector3 {
        mul_vector(self.linear, vector)
    }
}

fn mul_vector(matrix: [[f64; 3]; 3], vector: Vector3) -> Vector3 {
    Vector3::from_finite(
        matrix[0][0] * vector.x() + matrix[0][1] * vector.y() + matrix[0][2] * vector.z(),
        matrix[1][0] * vector.x() + matrix[1][1] * vector.y() + matrix[1][2] * vector.z(),
        matrix[2][0] * vector.x() + matrix[2][1] * vector.y() + matrix[2][2] * vector.z(),
    )
}

fn mul_matrix(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            a[row][0] * b[0][column] + a[row][1] * b[1][column] + a[row][2] * b[2][column]
        })
    })
}

impl Default for Transform3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    fn v(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3::new(x, y, z).unwrap()
    }

    #[test]
    fn translation_moves_points_but_not_vectors() {
        let transform = Transform3::translation(v(3.0, -1.0, 5.0));
        assert_eq!(transform.apply_point(p(1.0, 1.0, 2.0)), p(4.0, 0.0, 7.0));
        assert_eq!(transform.apply_vector(v(1.0, 1.0, 2.0)), v(1.0, 1.0, 2.0));
        assert_eq!(
            Transform3::IDENTITY.apply_point(p(7.0, 8.0, 9.0)),
            p(7.0, 8.0, 9.0)
        );
    }

    #[test]
    fn axis_angle_rotation_uses_right_hand_rule() {
        let turn = Transform3::rotation(Vector3::Z, FRAC_PI_2).unwrap();
        assert!(turn.apply_vector(Vector3::X).approx_eq(Vector3::Y, TOL));
        assert!(turn.apply_vector(Vector3::Y).approx_eq(-Vector3::X, TOL));
        assert!(Transform3::rotation(Vector3::ZERO, 0.5).is_err());
        assert!(Transform3::rotation(Vector3::Z, f64::NAN).is_err());
    }

    #[test]
    fn rotation_about_center_keeps_center_fixed() {
        let center = p(500_000.0, 9_800_000.0, 1_250.0);
        let transform = Transform3::rotation_about(center, Vector3::Z, FRAC_PI_2).unwrap();
        assert!(transform.apply_point(center).approx_eq(center, TOL));
        let point = p(center.x() + 1.0, center.y(), center.z());
        assert!(
            transform
                .apply_point(point)
                .approx_eq(p(center.x(), center.y() + 1.0, center.z()), TOL)
        );
    }

    #[test]
    fn non_uniform_scaling() {
        let center = p(1.0, 2.0, 3.0);
        let transform = Transform3::scaling(center, v(2.0, 3.0, 4.0)).unwrap();
        assert_eq!(transform.apply_point(p(2.0, 3.0, 4.0)), p(3.0, 5.0, 7.0));
        assert_eq!(
            Transform3::scaling(center, v(1.0, 0.0, 1.0)),
            Err(GeometryError::InvalidScale)
        );
    }

    #[test]
    fn composition_and_inverse() {
        let rotate = Transform3::rotation(Vector3::Z, FRAC_PI_2).unwrap();
        let translate = Transform3::translation(v(10.0, 0.0, 2.0));
        let combined = rotate.then(translate);
        assert!(
            combined
                .apply_point(p(1.0, 0.0, 0.0))
                .approx_eq(p(10.0, 1.0, 2.0), TOL)
        );
        let inverse = combined.inverse().unwrap();
        let samples = [
            p(0.0, 0.0, 0.0),
            p(3.0, -2.0, 7.0),
            p(500_000.0, 9_800_000.0, 30.0),
        ];
        assert!(
            combined
                .then(inverse)
                .approx_eq_at(Transform3::IDENTITY, &samples, TOL)
        );
        for point in samples {
            assert!(
                inverse
                    .apply_point(combined.apply_point(point))
                    .approx_eq(point, TOL)
            );
        }
    }

    #[test]
    fn inverse_reports_singular_and_non_finite_matrices() {
        let singular = Transform3::new(
            [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            Point3::ORIGIN,
            Vector3::ZERO,
        )
        .unwrap();
        assert_eq!(singular.inverse(), Err(GeometryError::SingularTransform));
        assert!(
            Transform3::new(
                [[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                Point3::ORIGIN,
                Vector3::ZERO,
            )
            .is_err()
        );
    }
}
