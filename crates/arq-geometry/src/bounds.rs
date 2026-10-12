//! `Bounds2`: axis-aligned bounding boxes.

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// The smallest axis-aligned rectangle containing some geometry.
///
/// Never empty: `min <= max` on both axes always holds. "No bounds" (an
/// empty selection, an empty drawing) is `Option<Bounds2>::None`, so callers
/// must handle it. A single point has valid bounds of zero width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds2 {
    min: Point2,
    max: Point2,
}

impl Bounds2 {
    /// The box with `a` and `b` as opposite corners, in any order.
    pub fn new(a: Point2, b: Point2) -> Self {
        Self {
            min: Point2::from_finite(a.x().min(b.x()), a.y().min(b.y())),
            max: Point2::from_finite(a.x().max(b.x()), a.y().max(b.y())),
        }
    }

    /// The zero-size box of a single point.
    pub fn from_point(p: Point2) -> Self {
        Self { min: p, max: p }
    }

    /// The box of a set of points, or `None` if there are none.
    pub fn from_points(points: impl IntoIterator<Item = Point2>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = Self::from_point(points.next()?);
        Some(points.fold(first, Self::including))
    }

    /// Lower-left corner (minimum x and y).
    pub fn min(self) -> Point2 {
        self.min
    }

    /// Upper-right corner (maximum x and y).
    pub fn max(self) -> Point2 {
        self.max
    }

    /// Extent along X.
    pub fn width(self) -> f64 {
        self.max.x() - self.min.x()
    }

    /// Extent along Y.
    pub fn height(self) -> f64 {
        self.max.y() - self.min.y()
    }

    /// Diagonal from `min` to `max`.
    pub fn size(self) -> Vector2 {
        self.max - self.min
    }

    /// Center of the box.
    pub fn center(self) -> Point2 {
        self.min.midpoint(self.max)
    }

    /// The four corners, counterclockwise from `min`.
    pub fn corners(self) -> [Point2; 4] {
        [
            self.min,
            Point2::from_finite(self.max.x(), self.min.y()),
            self.max,
            Point2::from_finite(self.min.x(), self.max.y()),
        ]
    }

    /// The smallest box containing this one and `p`.
    pub fn including(self, p: Point2) -> Self {
        self.union(Self::from_point(p))
    }

    /// The smallest box containing both boxes.
    pub fn union(self, other: Self) -> Self {
        Self {
            min: Point2::from_finite(
                self.min.x().min(other.min.x()),
                self.min.y().min(other.min.y()),
            ),
            max: Point2::from_finite(
                self.max.x().max(other.max.x()),
                self.max.y().max(other.max.y()),
            ),
        }
    }

    /// This box grown by `margin` on every side (e.g. a pick aperture).
    pub fn expanded(self, margin: f64) -> GeometryResult<Self> {
        if !(margin.is_finite() && margin >= 0.0) {
            return Err(GeometryError::NegativeOrNonFinite { what: "margin" });
        }
        let d = Vector2::from_finite(margin, margin);
        Ok(Self {
            min: self.min - d,
            max: self.max + d,
        })
    }

    /// Whether `p` lies inside or on the border, within `tol.linear()`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        let t = tol.linear();
        p.x() >= self.min.x() - t
            && p.x() <= self.max.x() + t
            && p.y() >= self.min.y() - t
            && p.y() <= self.max.y() + t
    }

    /// Whether `other` lies entirely inside this box, within `tol.linear()`.
    /// A box-level test for window selection.
    pub fn contains(self, other: Self, tol: Tolerance) -> bool {
        self.contains_point(other.min, tol) && self.contains_point(other.max, tol)
    }

    /// Whether the boxes overlap or touch, within `tol.linear()`. A fast
    /// box-level test: geometry whose boxes do not intersect cannot intersect.
    pub fn intersects(self, other: Self, tol: Tolerance) -> bool {
        let t = tol.linear();
        self.min.x() <= other.max.x() + t
            && other.min.x() <= self.max.x() + t
            && self.min.y() <= other.max.y() + t
            && other.min.y() <= self.max.y() + t
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

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn b(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds2 {
        Bounds2::new(p(x0, y0), p(x1, y1))
    }

    #[test]
    fn corners_in_any_order() {
        let a = b(10.0, 0.0, 0.0, 5.0);
        assert_eq!(a.min(), p(0.0, 0.0));
        assert_eq!(a.max(), p(10.0, 5.0));
        assert_eq!(a, b(0.0, 5.0, 10.0, 0.0));
    }

    #[test]
    fn measures() {
        let a = b(1.0, 2.0, 5.0, 8.0);
        assert_eq!(a.width(), 4.0);
        assert_eq!(a.height(), 6.0);
        assert_eq!(a.size(), Vector2::new(4.0, 6.0).unwrap());
        assert_eq!(a.center(), p(3.0, 5.0));
        assert_eq!(
            a.corners(),
            [p(1.0, 2.0), p(5.0, 2.0), p(5.0, 8.0), p(1.0, 8.0)]
        );
    }

    #[test]
    fn from_points_and_empty() {
        assert_eq!(Bounds2::from_points([]), None);
        let one = Bounds2::from_points([p(3.0, 4.0)]).unwrap();
        assert_eq!((one.width(), one.height()), (0.0, 0.0));
        let many = Bounds2::from_points([p(1.0, 5.0), p(-2.0, 3.0), p(4.0, -1.0)]).unwrap();
        assert_eq!(many, b(-2.0, -1.0, 4.0, 5.0));
    }

    #[test]
    fn union_and_including() {
        let u = b(0.0, 0.0, 1.0, 1.0).union(b(2.0, -1.0, 3.0, 0.5));
        assert_eq!(u, b(0.0, -1.0, 3.0, 1.0));
        assert_eq!(
            b(0.0, 0.0, 1.0, 1.0).including(p(-1.0, 2.0)),
            b(-1.0, 0.0, 1.0, 2.0)
        );
    }

    #[test]
    fn expanded() {
        assert_eq!(
            b(0.0, 0.0, 1.0, 1.0).expanded(0.5).unwrap(),
            b(-0.5, -0.5, 1.5, 1.5)
        );
        assert_eq!(
            b(0.0, 0.0, 1.0, 1.0).expanded(0.0).unwrap(),
            b(0.0, 0.0, 1.0, 1.0)
        );
        for bad in [-0.1, f64::NAN, f64::INFINITY] {
            assert_eq!(
                b(0.0, 0.0, 1.0, 1.0).expanded(bad),
                Err(GeometryError::NegativeOrNonFinite { what: "margin" })
            );
        }
    }

    #[test]
    fn containment_includes_border_and_tolerance() {
        let a = b(0.0, 0.0, 10.0, 10.0);
        assert!(a.contains_point(p(5.0, 5.0), TOL));
        assert!(a.contains_point(p(10.0, 0.0), TOL), "border counts");
        assert!(
            a.contains_point(p(10.0 + 5e-10, 0.0), TOL),
            "within tolerance"
        );
        assert!(!a.contains_point(p(10.0 + 1e-6, 0.0), TOL));
        assert!(a.contains(b(1.0, 1.0, 9.0, 9.0), TOL));
        assert!(a.contains(a, TOL));
        assert!(!a.contains(b(5.0, 5.0, 11.0, 6.0), TOL));
    }

    #[test]
    fn intersection_of_boxes() {
        let a = b(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(b(5.0, 5.0, 15.0, 15.0), TOL));
        assert!(
            a.intersects(b(10.0, 0.0, 20.0, 10.0), TOL),
            "touching edges count"
        );
        assert!(a.intersects(b(2.0, 2.0, 3.0, 3.0), TOL), "inside counts");
        assert!(!a.intersects(b(11.0, 0.0, 20.0, 10.0), TOL));
        assert!(!a.intersects(b(0.0, 11.0, 10.0, 20.0), TOL));
    }
}
