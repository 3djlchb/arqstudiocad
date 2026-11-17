//! `Ellipse`: an ellipse or an elliptical arc in the plan, as DWG keeps it
//! (ADR 0040): its center, its major semi-axis as a vector, the ratio of
//! the minor axis to the major, and the arc's parameters, counterclockwise.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::angle;
use crate::bounds::Bounds2;
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::transform::Transform2;
use crate::vector::Vector2;

/// The largest rotation ELLIPSE's Rotation takes, as AutoCAD: 89.4°.
pub const MAX_ROTATION: f64 = 89.4 * PI / 180.0;

/// An ellipse, or an elliptical arc, in the plan: the points
/// `center + major·cos t + minor·sin t`, `minor` being `major` turned a
/// quarter counterclockwise and scaled by `ratio`, for `t` from `start`
/// counterclockwise through `sweep`. A sweep of a whole turn is the whole
/// ellipse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    center: Point2,
    major: Vector2,
    ratio: f64,
    start: f64,
    sweep: f64,
}

impl Ellipse {
    /// The ellipse about `center` with major semi-axis `major`, minor to
    /// major `ratio` in `(0, 1]`, counterclockwise from parameter `start`
    /// to `end` (radians); equal ones, after normalizing, make the whole
    /// ellipse, as DWG's 0 and 2π.
    pub fn new(
        center: Point2,
        major: Vector2,
        ratio: f64,
        start: f64,
        end: f64,
    ) -> GeometryResult<Self> {
        if !(ratio.is_finite() && start.is_finite() && end.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Ellipse" });
        }
        if major.length() == 0.0 {
            return Err(GeometryError::ZeroLength);
        }
        if !(ratio > 0.0 && ratio <= 1.0) {
            return Err(GeometryError::OutOfRange {
                what: "ellipse ratio",
            });
        }
        let start = angle::normalize(start);
        let sweep = match angle::ccw_sweep(start, angle::normalize(end)) {
            0.0 => TAU,
            s => s,
        };
        Ok(Self {
            center,
            major,
            ratio,
            start,
            sweep,
        })
    }

    /// The whole ellipse about `center` through `axis_end`, one end of an
    /// axis, its other semi-axis `other` long (ELLIPSE's distance to the
    /// other axis). The longer axis is the major one, as AutoCAD makes it.
    pub fn from_axis(center: Point2, axis_end: Point2, other: f64) -> GeometryResult<Self> {
        if !other.is_finite() {
            return Err(GeometryError::NonFinite { what: "Ellipse" });
        }
        let first = axis_end - center;
        let half = first.length();
        if half == 0.0 || other <= 0.0 {
            return Err(GeometryError::ZeroLength);
        }
        if other <= half {
            Self::new(center, first, other / half, 0.0, TAU)
        } else {
            let major = first.perp().normalized()? * other;
            Self::new(center, major, half / other, 0.0, TAU)
        }
    }

    /// The whole ellipse about `center` through `axis_end`, seen as a
    /// circle turned `rotation` radians about that axis (ELLIPSE's
    /// Rotation): the other semi-axis is `cos(rotation)` of the first.
    /// Refused beyond 89.4°, as AutoCAD.
    pub fn from_rotation(center: Point2, axis_end: Point2, rotation: f64) -> GeometryResult<Self> {
        if !rotation.is_finite() {
            return Err(GeometryError::NonFinite { what: "Ellipse" });
        }
        if !(0.0..=MAX_ROTATION).contains(&rotation) {
            return Err(GeometryError::OutOfRange {
                what: "ellipse rotation",
            });
        }
        let half = (axis_end - center).length();
        Self::from_axis(center, axis_end, half * rotation.cos())
    }

    /// This whole ellipse cut to the arc from `start` to `end`, measured
    /// counterclockwise from `first`, the direction of the first axis
    /// drawn: as angles about the center when `parameters` is false,
    /// as the ellipse's own parameters when it is true (ELLIPSE's Arc).
    pub fn arc_from(
        self,
        first: Vector2,
        start: f64,
        end: f64,
        parameters: bool,
    ) -> GeometryResult<Self> {
        if !(start.is_finite() && end.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Ellipse" });
        }
        let offset = first.angle() - self.major.angle();
        let to_param = |a: f64| {
            let a = a + offset;
            if parameters {
                // The first axis is at parameter 0 or a quarter turn.
                a
            } else {
                a.sin().atan2(self.ratio * a.cos())
            }
        };
        let (s, e) = (to_param(start), to_param(end));
        if angle::normalize(s) == angle::normalize(e) {
            return Err(GeometryError::ZeroSweep);
        }
        Self::new(self.center, self.major, self.ratio, s, e)
    }

    /// The center.
    pub fn center(self) -> Point2 {
        self.center
    }

    /// The major semi-axis, from the center to the end of the major axis.
    pub fn major_axis(self) -> Vector2 {
        self.major
    }

    /// The minor semi-axis: the major turned a quarter counterclockwise,
    /// scaled by the ratio.
    pub fn minor_axis(self) -> Vector2 {
        self.major.perp() * self.ratio
    }

    /// The minor axis's length over the major's, in `(0, 1]`.
    pub fn ratio(self) -> f64 {
        self.ratio
    }

    /// The start parameter, in `[0, 2π)`.
    pub fn start_param(self) -> f64 {
        self.start
    }

    /// The end parameter, in `[0, 2π)` (equal to the start for the whole
    /// ellipse).
    pub fn end_param(self) -> f64 {
        angle::normalize(self.start + self.sweep)
    }

    /// The parameter swept, in `(0, 2π]`.
    pub fn sweep(self) -> f64 {
        self.sweep
    }

    /// Whether it is the whole ellipse.
    pub fn is_full(self) -> bool {
        self.sweep >= TAU
    }

    /// The point at parameter `t`.
    pub fn point_at_param(self, t: f64) -> Point2 {
        self.center + self.major * t.cos() + self.minor_axis() * t.sin()
    }

    /// The arc's ends; None for the whole ellipse.
    pub fn ends(self) -> Option<(Point2, Point2)> {
        (!self.is_full()).then(|| {
            (
                self.point_at_param(self.start),
                self.point_at_param(self.start + self.sweep),
            )
        })
    }

    /// Whether parameter `t` lies on the arc.
    pub fn contains_param(self, t: f64) -> bool {
        self.is_full() || angle::ccw_sweep(self.start, t) <= self.sweep
    }

    /// The ends of its axes on it (the quadrant snaps), in parameter order.
    pub fn quadrants(self) -> Vec<Point2> {
        [0.0, FRAC_PI_2, PI, 1.5 * PI]
            .into_iter()
            .filter(|&t| self.contains_param(t))
            .map(|t| self.point_at_param(t))
            .collect()
    }

    /// The smallest box holding it: its ends and the points where it turns
    /// in x or in y.
    pub fn bounds(self) -> Bounds2 {
        let (u, v) = (self.major, self.minor_axis());
        // x(t) = u.x cos t + v.x sin t turns where tan t = v.x / u.x; y too.
        let turn_x = v.x().atan2(u.x());
        let turn_y = v.y().atan2(u.y());
        let mut points: Vec<Point2> = [turn_x, turn_x + PI, turn_y, turn_y + PI]
            .into_iter()
            .filter(|&t| self.contains_param(t))
            .map(|t| self.point_at_param(t))
            .collect();
        if let Some((a, b)) = self.ends() {
            points.extend([a, b]);
        }
        Bounds2::from_points(points).expect("an ellipse has points")
    }

    /// The point of it nearest `p`.
    pub fn closest_point(self, p: Point2) -> Point2 {
        self.point_at_param(self.closest_param(p))
    }

    /// The parameter of the point of it nearest `p`: the best of 64 samples
    /// along it, refined by golden-section search between its neighbors,
    /// then by Newton's method on where the distance stops changing, which
    /// comparing distances alone resolves only to about 1e-8.
    pub fn closest_param(self, p: Point2) -> f64 {
        const SAMPLES: usize = 64;
        let at = |i: usize| self.start + self.sweep * i as f64 / SAMPLES as f64;
        let distance = |t: f64| self.point_at_param(t).distance_squared_to(p);
        let best = (0..=SAMPLES)
            .min_by(|&a, &b| distance(at(a)).total_cmp(&distance(at(b))))
            .expect("samples");
        let (mut lo, mut hi) = (at(best.saturating_sub(1)), at((best + 1).min(SAMPLES)));
        let golden = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..60 {
            let (a, b) = (hi - golden * (hi - lo), lo + golden * (hi - lo));
            if distance(a) < distance(b) {
                hi = b;
            } else {
                lo = a;
            }
        }
        let (first, last) = (self.start, self.start + self.sweep);
        let (u, w) = (self.major, self.minor_axis());
        let mut t = (lo + hi) / 2.0;
        for _ in 0..8 {
            let (sin, cos) = t.sin_cos();
            let off = self.point_at_param(t) - p;
            let d1 = w * cos - u * sin; // the tangent
            let d2 = -(u * cos + w * sin);
            let slope = off.dot(d1);
            let curve = d1.dot(d1) + off.dot(d2);
            if curve <= 0.0 {
                break;
            }
            let next = t - slope / curve;
            if !next.is_finite() || (!self.is_full() && !(first..=last).contains(&next)) {
                break;
            }
            t = next;
        }
        t
    }

    /// How far `p` is from it.
    pub fn distance_to(self, p: Point2) -> f64 {
        self.closest_point(p).distance_to(p)
    }

    /// Whether `p` lies on it, within `tol`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        self.distance_to(p) <= tol.linear()
    }

    /// Points along it to draw it by, its ends included: sixteen a radian,
    /// eight at least and 128 at most, as the drawing has always flattened
    /// ellipses.
    pub fn flatten(self) -> Vec<Point2> {
        let steps = ((self.sweep * 16.0).ceil() as usize).clamp(8, 128);
        (0..=steps)
            .map(|i| self.point_at_param(self.start + self.sweep * i as f64 / steps as f64))
            .collect()
    }

    /// Its image by `t`, a similarity (a move, a turn, a uniform scale or a
    /// mirror): a mirror reverses its direction, so the parameters change
    /// sign to keep it counterclockwise.
    pub fn transformed(self, t: Transform2) -> GeometryResult<Self> {
        let center = t.apply_point(self.center);
        let major = t.apply_vector(self.major);
        if t.is_reflection() {
            let end = self.start + self.sweep;
            Self::new(center, major, self.ratio, -end, -self.start)
        } else {
            Self::new(
                center,
                major,
                self.ratio,
                self.start,
                self.start + self.sweep,
            )
        }
    }
}

#[cfg(test)]
#[path = "ellipse_tests.rs"]
mod tests;
