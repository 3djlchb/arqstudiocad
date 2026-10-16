//! `Bounds3`: axis-aligned bounding boxes in three dimensions.

use crate::error::{GeometryError, GeometryResult};
use crate::point3::Point3;
use crate::tolerance::Tolerance;
use crate::vector3::Vector3;

/// The smallest axis-aligned box containing some 3D geometry.
///
/// It is never empty: minimum coordinates are always less than or equal to
/// maximum coordinates. An empty set of points is represented by `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds3 {
    min: Point3,
    max: Point3,
}

impl Bounds3 {
    /// The box with `a` and `b` as opposite corners, in any order.
    pub fn new(a: Point3, b: Point3) -> Self {
        Self {
            min: Point3::from_finite(a.x().min(b.x()), a.y().min(b.y()), a.z().min(b.z())),
            max: Point3::from_finite(a.x().max(b.x()), a.y().max(b.y()), a.z().max(b.z())),
        }
    }

    /// The zero-size box of a single point.
    pub fn from_point(point: Point3) -> Self {
        Self {
            min: point,
            max: point,
        }
    }

    /// The box of a set of points, or `None` if there are none.
    pub fn from_points(points: impl IntoIterator<Item = Point3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = Self::from_point(points.next()?);
        Some(points.fold(first, Self::including))
    }

    /// Minimum corner.
    pub fn min(self) -> Point3 {
        self.min
    }

    /// Maximum corner.
    pub fn max(self) -> Point3 {
        self.max
    }

    /// Width along X.
    pub fn width(self) -> f64 {
        self.max.x() - self.min.x()
    }

    /// Height along Y.
    pub fn height(self) -> f64 {
        self.max.y() - self.min.y()
    }

    /// Depth along Z.
    pub fn depth(self) -> f64 {
        self.max.z() - self.min.z()
    }

    /// Diagonal from the minimum to the maximum corner.
    pub fn size(self) -> Vector3 {
        self.max - self.min
    }

    /// Center of the box.
    pub fn center(self) -> Point3 {
        self.min.midpoint(self.max)
    }

    /// The eight corners, in deterministic order.
    pub fn corners(self) -> [Point3; 8] {
        let (x0, y0, z0) = (self.min.x(), self.min.y(), self.min.z());
        let (x1, y1, z1) = (self.max.x(), self.max.y(), self.max.z());
        [
            Point3::from_finite(x0, y0, z0),
            Point3::from_finite(x1, y0, z0),
            Point3::from_finite(x1, y1, z0),
            Point3::from_finite(x0, y1, z0),
            Point3::from_finite(x0, y0, z1),
            Point3::from_finite(x1, y0, z1),
            Point3::from_finite(x1, y1, z1),
            Point3::from_finite(x0, y1, z1),
        ]
    }

    /// The smallest box containing this box and `point`.
    pub fn including(self, point: Point3) -> Self {
        self.union(Self::from_point(point))
    }

    /// The smallest box containing both boxes.
    pub fn union(self, other: Self) -> Self {
        Self {
            min: Point3::from_finite(
                self.min.x().min(other.min.x()),
                self.min.y().min(other.min.y()),
                self.min.z().min(other.min.z()),
            ),
            max: Point3::from_finite(
                self.max.x().max(other.max.x()),
                self.max.y().max(other.max.y()),
                self.max.z().max(other.max.z()),
            ),
        }
    }

    /// This box grown by `margin` on every side.
    pub fn expanded(self, margin: f64) -> GeometryResult<Self> {
        if !(margin.is_finite() && margin >= 0.0) {
            return Err(GeometryError::NegativeOrNonFinite { what: "margin" });
        }
        let min = Point3::new(
            self.min.x() - margin,
            self.min.y() - margin,
            self.min.z() - margin,
        )?;
        let max = Point3::new(
            self.max.x() + margin,
            self.max.y() + margin,
            self.max.z() + margin,
        )?;
        Ok(Self { min, max })
    }

    /// Whether `point` lies inside or on the border, within `tol.linear()`.
    pub fn contains_point(self, point: Point3, tol: Tolerance) -> bool {
        let t = tol.linear();
        point.x() >= self.min.x() - t
            && point.x() <= self.max.x() + t
            && point.y() >= self.min.y() - t
            && point.y() <= self.max.y() + t
            && point.z() >= self.min.z() - t
            && point.z() <= self.max.z() + t
    }

    /// Whether `other` lies entirely inside this box, within `tol.linear()`.
    pub fn contains(self, other: Self, tol: Tolerance) -> bool {
        self.contains_point(other.min, tol) && self.contains_point(other.max, tol)
    }

    /// Whether the boxes overlap or touch, within `tol.linear()`.
    pub fn intersects(self, other: Self, tol: Tolerance) -> bool {
        let t = tol.linear();
        self.min.x() <= other.max.x() + t
            && other.min.x() <= self.max.x() + t
            && self.min.y() <= other.max.y() + t
            && other.min.y() <= self.max.y() + t
            && self.min.z() <= other.max.z() + t
            && other.min.z() <= self.max.z() + t
    }

    /// Whether both corners match within `tol.linear()`.
    pub fn approx_eq(self, other: Self, tol: Tolerance) -> bool {
        self.min.approx_eq(other.min, tol) && self.max.approx_eq(other.max, tol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    fn b(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Bounds3 {
        Bounds3::new(p(x0, y0, z0), p(x1, y1, z1))
    }

    #[test]
    fn corners_measures_and_center() {
        let bounds = b(10.0, 5.0, -2.0, 0.0, -1.0, 4.0);
        assert_eq!(bounds.min(), p(0.0, -1.0, -2.0));
        assert_eq!(bounds.max(), p(10.0, 5.0, 4.0));
        assert_eq!(
            (bounds.width(), bounds.height(), bounds.depth()),
            (10.0, 6.0, 6.0)
        );
        assert_eq!(bounds.size(), Vector3::new(10.0, 6.0, 6.0).unwrap());
        assert_eq!(bounds.center(), p(5.0, 2.0, 1.0));
        assert_eq!(bounds.corners().len(), 8);
        assert_eq!(bounds.corners()[6], p(10.0, 5.0, 4.0));
    }

    #[test]
    fn from_points_union_and_include() {
        assert_eq!(Bounds3::from_points([]), None);
        let point_bounds = Bounds3::from_points([p(2.0, 3.0, 4.0)]).unwrap();
        assert_eq!(point_bounds, b(2.0, 3.0, 4.0, 2.0, 3.0, 4.0));
        assert_eq!(
            b(0.0, 0.0, 0.0, 1.0, 1.0, 1.0).union(b(2.0, -1.0, 0.5, 3.0, 0.0, 2.0)),
            b(0.0, -1.0, 0.0, 3.0, 1.0, 2.0)
        );
        assert_eq!(
            b(0.0, 0.0, 0.0, 1.0, 1.0, 1.0).including(p(-1.0, 2.0, 3.0)),
            b(-1.0, 0.0, 0.0, 1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn expansion_and_invalid_margin() {
        assert_eq!(
            b(0.0, 1.0, 2.0, 3.0, 4.0, 5.0).expanded(0.5).unwrap(),
            b(-0.5, 0.5, 1.5, 3.5, 4.5, 5.5)
        );
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                b(0.0, 0.0, 0.0, 1.0, 1.0, 1.0).expanded(bad),
                Err(GeometryError::NegativeOrNonFinite { what: "margin" })
            );
        }
        assert!(
            b(f64::MAX, 0.0, 0.0, f64::MAX, 0.0, 0.0)
                .expanded(f64::MAX)
                .is_err()
        );
    }

    #[test]
    fn containment_and_intersection() {
        let outer = b(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);
        assert!(outer.contains_point(p(10.0, 5.0, 0.0), TOL));
        assert!(outer.contains(b(1.0, 1.0, 1.0, 9.0, 9.0, 9.0), TOL));
        assert!(outer.intersects(b(10.0, 0.0, 0.0, 12.0, 2.0, 2.0), TOL));
        assert!(!outer.intersects(b(11.0, 0.0, 0.0, 12.0, 2.0, 2.0), TOL));
    }
}
