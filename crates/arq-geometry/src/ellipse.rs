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

    /// The whole ellipse this arc lies on.
    pub fn whole(self) -> Self {
        Self {
            start: 0.0,
            sweep: TAU,
            ..self
        }
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

    /// The derivative of its point by the parameter at `t`: the tangent,
    /// counterclockwise, as long as the speed along it.
    pub fn derivative_at(self, t: f64) -> Vector2 {
        let (sin, cos) = t.sin_cos();
        self.minor_axis() * cos - self.major * sin
    }

    /// `p` in the ellipse's own frame, where the ellipse is the unit
    /// circle: its coordinates along the major and minor semi-axes, each in
    /// their lengths.
    pub(crate) fn local(self, p: Point2) -> (f64, f64) {
        let d = p - self.center;
        let (u, w) = (self.major, self.minor_axis());
        (d.dot(u) / u.length_squared(), d.dot(w) / w.length_squared())
    }

    /// A vector's components in the ellipse's own frame.
    pub(crate) fn local_vector(self, d: Vector2) -> (f64, f64) {
        let (u, w) = (self.major, self.minor_axis());
        (d.dot(u) / u.length_squared(), d.dot(w) / w.length_squared())
    }

    /// The point whose coordinates in the ellipse's own frame are `(x, y)`.
    pub(crate) fn at_local(self, x: f64, y: f64) -> Point2 {
        self.center + self.major * x + self.minor_axis() * y
    }

    /// The parameter of `p` seen from the center in the ellipse's own frame,
    /// in `[0, 2π)`: for a point on the whole ellipse, its own parameter.
    pub fn param_of(self, p: Point2) -> f64 {
        let (x, y) = self.local(p);
        angle::normalize(y.atan2(x))
    }

    /// Whether parameter `t` lies on the arc, or within `slack` (radians)
    /// beyond either end.
    pub(crate) fn admits_param(self, t: f64, slack: f64) -> bool {
        if self.is_full() {
            return true;
        }
        let offset = angle::ccw_sweep(self.start, t);
        offset <= self.sweep + slack || offset >= TAU - slack
    }

    /// Whether `p`, a point of the whole ellipse, lies on the arc, within
    /// `tol` beyond its ends.
    pub(crate) fn admits(self, p: Point2, tol: Tolerance) -> bool {
        let t = self.param_of(p);
        let speed = self.derivative_at(t).length();
        self.admits_param(t, tol.angular().max(tol.linear() / speed))
    }

    /// The parameter `fraction` of the way along the arc: 0 its start, 1
    /// its end.
    pub fn param_at(self, fraction: f64) -> f64 {
        self.start + self.sweep * fraction
    }

    /// The point `fraction` of the way along its parameter, `[0, 1]`: 0 the
    /// start, 1 the end (`Geometry2::point_at`).
    pub fn point_at(self, fraction: f64) -> Point2 {
        self.point_at_param(self.param_at(fraction))
    }

    /// The length along it from parameter `from` counterclockwise to `to`,
    /// `to` no smaller than `from`: the integral of its speed, by adaptive
    /// Gauss-Legendre quadrature to about 1e-12 of the length.
    pub fn length_between(self, from: f64, to: f64) -> f64 {
        let speed = |t: f64| self.derivative_at(t).length();
        let gauss = |a: f64, b: f64| {
            // Five-point Gauss-Legendre.
            const X: [f64; 5] = [
                0.0,
                0.538_469_310_105_683_1,
                -0.538_469_310_105_683_1,
                0.906_179_845_938_664,
                -0.906_179_845_938_664,
            ];
            const W: [f64; 5] = [
                0.568_888_888_888_888_9,
                0.478_628_670_499_366_5,
                0.478_628_670_499_366_5,
                0.236_926_885_056_189_1,
                0.236_926_885_056_189_1,
            ];
            let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
            half * X
                .iter()
                .zip(W)
                .map(|(x, w)| w * speed(mid + half * x))
                .sum::<f64>()
        };
        fn refine(
            gauss: &dyn Fn(f64, f64) -> f64,
            a: f64,
            b: f64,
            whole: f64,
            tol: f64,
            depth: u32,
        ) -> f64 {
            let m = (a + b) / 2.0;
            let (left, right) = (gauss(a, m), gauss(m, b));
            if depth == 0 || (left + right - whole).abs() <= tol {
                left + right
            } else {
                refine(gauss, a, m, left, tol / 2.0, depth - 1)
                    + refine(gauss, m, b, right, tol / 2.0, depth - 1)
            }
        }
        if to <= from {
            return 0.0;
        }
        // Panels of an eighth of a turn at most, so the quadrature's
        // estimate never misses a narrow end of the ellipse.
        let panels = ((to - from) / (TAU / 8.0)).ceil().max(1.0);
        let width = (to - from) / panels;
        let tol = 1e-13 * self.major.length() * (to - from);
        (0..panels as usize)
            .map(|i| {
                let a = from + width * i as f64;
                let b = a + width;
                refine(&gauss, a, b, gauss(a, b), tol / panels, 40)
            })
            .sum()
    }

    /// Its length.
    pub fn length(self) -> f64 {
        self.length_between(self.start, self.start + self.sweep)
    }

    /// The point `distance` along it from its start.
    pub fn point_at_distance(self, distance: f64) -> GeometryResult<Point2> {
        let length = self.length();
        if !(distance.is_finite() && (0.0..=length).contains(&distance)) {
            return Err(GeometryError::OutOfRange { what: "distance" });
        }
        Ok(self.point_at_param(self.param_at_distance(distance)))
    }

    /// The parameter `distance` along it from its start, `distance` within
    /// its length: Newton's method on the length, kept in a bracket.
    pub(crate) fn param_at_distance(self, distance: f64) -> f64 {
        let (mut lo, mut hi) = (self.start, self.start + self.sweep);
        let mut t = self.start + self.sweep * distance / self.length().max(f64::MIN_POSITIVE);
        for _ in 0..60 {
            let gone = self.length_between(self.start, t) - distance;
            if gone.abs() <= 1e-12 * self.major.length() {
                break;
            }
            if gone > 0.0 {
                hi = t;
            } else {
                lo = t;
            }
            let next = t - gone / self.derivative_at(t).length();
            t = if next > lo && next < hi {
                next
            } else {
                (lo + hi) / 2.0
            };
        }
        t
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
