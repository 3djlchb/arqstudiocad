//! OFFSET's curves: a line, circle or arc moved a distance square to
//! itself, to the side a point is on (AutoCAD's OFFSET).

use crate::arc::Arc;
use crate::circle::Circle;
use crate::error::{GeometryError, GeometryResult};
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;

impl Geometry2 {
    /// The curve `distance` away, on the side `toward` lies: a line
    /// parallel to this one and as long, a circle or arc about the same
    /// center, wider outside it and narrower inside, an arc between the same
    /// angles. Refused when `distance` is not finite and greater than zero,
    /// when `toward` lies on the curve's line or circle (no side), and when
    /// a circle or arc would shrink to nothing.
    pub fn offset(self, distance: f64, toward: Point2, tol: Tolerance) -> GeometryResult<Self> {
        if !(distance.is_finite() && distance > 0.0) {
            return Err(GeometryError::NegativeOrNonFinite {
                what: "offset distance",
            });
        }
        match self {
            Self::Line(l) => {
                let side = side_of_line(l, toward, tol)?;
                let shift = l.direction().perp() * (side * distance);
                Ok(Self::Line(Line::new(l.start() + shift, l.end() + shift)?))
            }
            Self::Circle(c) => {
                let radius = offset_radius(c.center(), c.radius(), distance, toward, tol)?;
                Ok(Self::Circle(Circle::new(c.center(), radius)?))
            }
            Self::Arc(a) => {
                let radius = offset_radius(a.center(), a.radius(), distance, toward, tol)?;
                Ok(Self::Arc(Arc::new(
                    a.center(),
                    radius,
                    a.start_angle(),
                    a.end_angle(),
                )?))
            }
        }
    }

    /// The curve offset so that it passes through `through` (OFFSET's
    /// Through): the distance is how far `through` is from this curve's
    /// line or circle, as AutoCAD measures it, not from its ends.
    pub fn offset_through(self, through: Point2, tol: Tolerance) -> GeometryResult<Self> {
        let distance = match self {
            Self::Line(l) => l.direction().cross(through - l.start()).abs(),
            Self::Circle(c) => (c.center().distance_to(through) - c.radius()).abs(),
            Self::Arc(a) => (a.center().distance_to(through) - a.radius()).abs(),
        };
        if distance <= tol.linear() {
            return Err(GeometryError::OutOfRange { what: "side" });
        }
        self.offset(distance, through, tol)
    }
}

/// 1 when `p` is to the left of the line's direction, -1 to its right;
/// refused on the line (or its extension).
fn side_of_line(l: Line, p: Point2, tol: Tolerance) -> GeometryResult<f64> {
    let across = l.direction().cross(p - l.start());
    if across.abs() <= tol.linear() {
        return Err(GeometryError::OutOfRange { what: "side" });
    }
    Ok(across.signum())
}

/// A circle's radius offset toward `p`: outward when `p` is outside it,
/// inward when inside, refused on it and when nothing would be left.
fn offset_radius(
    center: Point2,
    radius: f64,
    distance: f64,
    p: Point2,
    tol: Tolerance,
) -> GeometryResult<f64> {
    let from_center = center.distance_to(p);
    if (from_center - radius).abs() <= tol.linear() {
        return Err(GeometryError::OutOfRange { what: "side" });
    }
    let r = if from_center > radius {
        radius + distance
    } else {
        radius - distance
    };
    if r <= tol.linear() {
        return Err(GeometryError::InvalidRadius);
    }
    Ok(r)
}

#[cfg(test)]
#[path = "offset_tests.rs"]
mod tests;
