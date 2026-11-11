//! `Rectangle`: the closed polyline the RECTANG command draws, its sides
//! along a rotation and square to it.

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A rectangle from a first corner to the opposite one, its sides along
/// `rotation` (radians, from East) and square to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rectangle {
    first: Point2,
    rotation: f64,
    /// The way to the opposite corner along the rotation and square to it,
    /// signed; neither is zero.
    along: f64,
    across: f64,
}

impl Rectangle {
    /// The rectangle from `first` to the opposite corner `other`, its sides
    /// along `rotation`: `other` is seen in the rotated frame. Refused when
    /// it lies on a side's line through `first`, which leaves no area.
    pub fn from_corners(
        first: Point2,
        other: Point2,
        rotation: f64,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        let (u, v) = frame(rotation)?;
        let (dx, dy) = (other.x() - first.x(), other.y() - first.y());
        if !(dx.is_finite() && dy.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Rectangle" });
        }
        let along = dx * u.x() + dy * u.y();
        let across = dx * v.x() + dy * v.y();
        Self::checked(first, rotation, along, across, tol)
    }

    /// The rectangle `length` along the rotation and `width` square to it,
    /// from `first` into the quarter `toward` lies in (RECTANG's
    /// Dimensions): a point on a side's line through `first` takes the
    /// positive way.
    pub fn from_size(
        first: Point2,
        length: f64,
        width: f64,
        rotation: f64,
        toward: Point2,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        let (u, v) = frame(rotation)?;
        check_size(length, "length")?;
        check_size(width, "width")?;
        // Only the signs matter: halves keep far coordinates finite.
        let (dx, dy) = (
            toward.x() / 2.0 - first.x() / 2.0,
            toward.y() / 2.0 - first.y() / 2.0,
        );
        let way = |s: f64| if s < 0.0 { -1.0 } else { 1.0 };
        let along = length * way(dx * u.x() + dy * u.y());
        let across = width * way(dx * v.x() + dy * v.y());
        Self::checked(first, rotation, along, across, tol)
    }

    /// The rectangle of `area` with one side given, from `first` the
    /// positive way along the rotation and square to it (RECTANG's Area):
    /// `side` is the length when `side_is_length`, else the width, and the
    /// other side is the area over it.
    pub fn from_area(
        first: Point2,
        area: f64,
        side: f64,
        side_is_length: bool,
        rotation: f64,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        frame(rotation)?;
        check_size(area, "area")?;
        check_size(side, "side")?;
        if side == 0.0 {
            return Err(GeometryError::ZeroLength);
        }
        let other = area / side;
        let (along, across) = if side_is_length {
            (side, other)
        } else {
            (other, side)
        };
        Self::checked(first, rotation, along, across, tol)
    }

    fn checked(
        first: Point2,
        rotation: f64,
        along: f64,
        across: f64,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        if !(along.is_finite() && across.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Rectangle" });
        }
        if along.abs() <= tol.linear() || across.abs() <= tol.linear() {
            return Err(GeometryError::ZeroLength);
        }
        // Every corner must be finite too, before any is built.
        let (sin, cos) = rotation.sin_cos();
        let steps = [
            (cos * along, sin * along),
            (-sin * across, cos * across),
            (cos * along - sin * across, sin * along + cos * across),
        ];
        let finite = steps.iter().all(|&(dx, dy)| {
            [dx, dy, first.x() + dx, first.y() + dy]
                .iter()
                .all(|c| c.is_finite())
        });
        if !finite {
            return Err(GeometryError::NonFinite { what: "Rectangle" });
        }
        Ok(Self {
            first,
            rotation,
            along,
            across,
        })
    }

    /// The first corner.
    pub fn first(self) -> Point2 {
        self.first
    }

    /// The corner opposite the first.
    pub fn other(self) -> Point2 {
        self.corners()[2]
    }

    /// The direction of the length, radians.
    pub fn rotation(self) -> f64 {
        self.rotation
    }

    /// The size along the rotation.
    pub fn length(self) -> f64 {
        self.along.abs()
    }

    /// The size square to the rotation.
    pub fn width(self) -> f64 {
        self.across.abs()
    }

    /// Its area.
    pub fn area(self) -> f64 {
        self.length() * self.width()
    }

    /// The four corners, counterclockwise from the first: the closed
    /// polyline RECTANG draws.
    pub fn corners(self) -> [Point2; 4] {
        let (u, v) = frame(self.rotation).expect("checked when built");
        let (a, b) = (u * self.along, v * self.across);
        let p = self.first;
        // The rotated frame keeps the turn: counterclockwise when the
        // opposite corner lies to the left of the length.
        if self.along * self.across > 0.0 {
            [p, p + a, p + a + b, p + b]
        } else {
            [p, p + b, p + a + b, p + a]
        }
    }
}

/// The rotation's unit directions: along it and square to it, to its left.
fn frame(rotation: f64) -> GeometryResult<(Vector2, Vector2)> {
    if !rotation.is_finite() {
        return Err(GeometryError::NonFinite { what: "rotation" });
    }
    let u = Vector2::from_polar(1.0, rotation)?;
    Ok((u, u.perp()))
}

fn check_size(value: f64, what: &'static str) -> GeometryResult<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(GeometryError::NegativeOrNonFinite { what })
    }
}

#[cfg(test)]
#[path = "rectangle_tests.rs"]
mod tests;
