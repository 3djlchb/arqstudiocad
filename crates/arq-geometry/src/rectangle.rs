//! `Rectangle`: the closed polyline the RECTANG command draws, its sides
//! along a rotation and square to it, its corners square, filleted or
//! chamfered.

use std::f64::consts::{FRAC_PI_4, FRAC_PI_8};

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// How a rectangle's four corners are drawn (RECTANG's Fillet and Chamfer).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corners {
    /// Square corners.
    Square,
    /// Each corner rounded by a quarter circle of this radius.
    Fillet(f64),
    /// Each corner cut by a straight segment: the first distance along the
    /// sides that run with the rotation (the length), the second along the
    /// sides square to it (the width).
    Chamfer(f64, f64),
}

impl Corners {
    /// The corners as given, refused when a radius or distance is negative
    /// or not finite; a zero radius or a zero distance leaves them square.
    pub fn checked(self) -> GeometryResult<Self> {
        match self {
            Self::Square => Ok(self),
            Self::Fillet(r) => {
                check_size(r, "fillet radius")?;
                Ok(if r == 0.0 { Self::Square } else { self })
            }
            Self::Chamfer(a, b) => {
                check_size(a, "chamfer distance")?;
                check_size(b, "chamfer distance")?;
                // A zero distance cuts nothing (AutoCAD's CHAMFER meets the
                // sides at the corner then).
                Ok(if a == 0.0 || b == 0.0 {
                    Self::Square
                } else {
                    self
                })
            }
        }
    }

    /// The area the four corners take from the rectangle's.
    pub fn area_lost(self) -> f64 {
        match self.checked().unwrap_or(Self::Square) {
            Self::Square => 0.0,
            // A square of r² less a quarter circle, four times.
            Self::Fillet(r) => 4.0 * r * r * (1.0 - FRAC_PI_4),
            // A right triangle with legs a and b, four times.
            Self::Chamfer(a, b) => 2.0 * a * b,
        }
    }

    /// How far back each corner cuts the sides along the rotation and
    /// square to it.
    fn cuts(self) -> (f64, f64) {
        match self.checked().unwrap_or(Self::Square) {
            Self::Square => (0.0, 0.0),
            Self::Fillet(r) => (r, r),
            Self::Chamfer(a, b) => (a, b),
        }
    }
}

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
    /// `side` is the length when `side_is_length`, else the width. The area
    /// is the one `corners` leave, as AutoCAD counts it: the other side is
    /// the area and what the corners take, over the side given.
    pub fn from_area(
        first: Point2,
        area: f64,
        side: f64,
        side_is_length: bool,
        rotation: f64,
        corners: Corners,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        frame(rotation)?;
        check_size(area, "area")?;
        check_size(side, "side")?;
        let corners = corners.checked()?;
        if side == 0.0 {
            return Err(GeometryError::ZeroLength);
        }
        let other = (area + corners.area_lost()) / side;
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

    /// Its area with `corners`: what the fillets or chamfers leave.
    pub fn area_with(self, corners: Corners) -> f64 {
        self.area() - corners.area_lost()
    }

    /// Whether `corners` fit: no two of them overlap on a side.
    pub fn fits(self, corners: Corners, tol: Tolerance) -> bool {
        let (along, across) = corners.cuts();
        2.0 * along <= self.length() + tol.linear() && 2.0 * across <= self.width() + tol.linear()
    }

    /// The closed polyline RECTANG draws with `corners`, each vertex with
    /// the bulge of the segment leaving it: counterclockwise, from the
    /// first corner, or from where its fillet or chamfer ends. Corners too
    /// large for the sides are refused (`OutOfRange`); ones that take a
    /// whole side leave no straight segment there.
    pub fn vertices(self, corners: Corners, tol: Tolerance) -> GeometryResult<Vec<(Point2, f64)>> {
        let corners = corners.checked()?;
        if !self.fits(corners, tol) {
            return Err(GeometryError::OutOfRange { what: "corner" });
        }
        let ring = self.corners();
        if corners == Corners::Square {
            return Ok(ring.iter().map(|&p| (p, 0.0)).collect());
        }
        let (cut_along, cut_across) = corners.cuts();
        // A quarter circle turning left, counterclockwise: tan(90° / 4).
        let bulge = if matches!(corners, Corners::Fillet(_)) {
            FRAC_PI_8.tan()
        } else {
            0.0
        };
        let (u, _) = frame(self.rotation)?;
        let cut = |from: Point2, to: Point2| {
            let d = to - from;
            // Sides run along the rotation or square to it.
            if d.dot(u).abs() > d.length() / 2.0 {
                cut_along
            } else {
                cut_across
            }
        };
        let mut out: Vec<(Point2, f64)> = Vec::with_capacity(8);
        for i in 0..4 {
            let (before, corner, after) = (ring[(i + 3) % 4], ring[i], ring[(i + 1) % 4]);
            let back = (corner - before).normalized()?;
            let on = (after - corner).normalized()?;
            // Where the corner's cut begins, with the bulge across it, and
            // where it ends, with the straight side after it.
            push(&mut out, corner - back * cut(before, corner), bulge, tol);
            push(&mut out, corner + on * cut(corner, after), 0.0, tol);
        }
        // The last vertex may land on the first: the side between them is
        // gone, and the first keeps the bulge leaving it.
        if out.len() > 1 && out[out.len() - 1].0.approx_eq(out[0].0, tol) {
            out.pop();
        }
        // From where the first corner's cut ends.
        out.rotate_left(1);
        Ok(out)
    }

    /// The four corners, counterclockwise from the first: the closed
    /// polyline RECTANG draws with square corners.
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

/// Adds a vertex, unless it lies on the last one: then the last one takes
/// its bulge, so no segment has zero length.
fn push(out: &mut Vec<(Point2, f64)>, point: Point2, bulge: f64, tol: Tolerance) {
    match out.last_mut() {
        Some(last) if last.0.approx_eq(point, tol) => last.1 = bulge,
        _ => out.push((point, bulge)),
    }
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
