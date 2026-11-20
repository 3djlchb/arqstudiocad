//! `Arc`: part of a circle, counterclockwise from `start_angle` to
//! `end_angle` (DWG convention, ADR 0010).

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::angle;
use crate::bounds::Bounds2;
use crate::circle::{Circle, check_radius};
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A circular arc running **counterclockwise** from `start_angle` to
/// `end_angle`. Both angles are normalized to `[0, 2π)`; the sweep is in
/// `(0, 2π)` (a full turn is a [`Circle`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    center: Point2,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
}

impl Arc {
    /// An arc around `center`, counterclockwise from `start_angle` to
    /// `end_angle` (radians, from East).
    pub fn new(
        center: Point2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> GeometryResult<Self> {
        check_radius(radius)?;
        if !(start_angle.is_finite() && end_angle.is_finite()) {
            return Err(GeometryError::NonFinite { what: "Arc" });
        }
        let (start_angle, end_angle) = (angle::normalize(start_angle), angle::normalize(end_angle));
        if start_angle == end_angle {
            return Err(GeometryError::ZeroSweep);
        }
        Ok(Self {
            center,
            radius,
            start_angle,
            end_angle,
        })
    }

    /// The arc that passes through `a`, then `b`, then `c` (the ARC command's
    /// three-point mode). Stored counterclockwise: when `a → b → c` turns
    /// clockwise, the result runs from `c` to `a`.
    pub fn from_three_points(
        a: Point2,
        b: Point2,
        c: Point2,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        // Local frame at `a`: keeps survey coordinates precise.
        let ab = b - a;
        let ac = c - a;
        let cross = ab.cross(ac);
        let scale = ab.length() * ac.length();
        if scale == 0.0 || (cross / scale).abs() <= tol.angular() {
            return Err(GeometryError::Collinear);
        }
        let (ab2, ac2) = (ab.length_squared(), ac.length_squared());
        let d = 2.0 * cross;
        let offset = Vector2::new(
            (ac.y() * ab2 - ab.y() * ac2) / d,
            (ab.x() * ac2 - ac.x() * ab2) / d,
        )?;
        let center = a + offset;
        let angle_of = |p: Point2| (p - center).angle();
        let (start, end) = if cross > 0.0 { (a, c) } else { (c, a) };
        Self::new(center, offset.length(), angle_of(start), angle_of(end))
    }

    /// The arc around `center` from `start`, counterclockwise to the
    /// direction of `end` (the ARC command's start, center, end mode). As in
    /// AutoCAD, the radius is the distance to `start` and `end` only gives
    /// the end angle.
    pub fn from_start_center_end(
        start: Point2,
        center: Point2,
        end: Point2,
    ) -> GeometryResult<Self> {
        let end_angle = (end - center).normalized()?.angle();
        // A start on the center gives a zero radius, which `new` refuses.
        Self::new(
            center,
            start.distance_to(center),
            (start - center).angle(),
            end_angle,
        )
    }

    /// The arc that leaves `start` along `direction` (radians) and ends at
    /// `end`, as a polyline vertex stores it: its bulge (the tangent of a
    /// quarter of its sweep, positive counterclockwise, 0 when `end` lies
    /// straight ahead), and the direction it ends in (PLINE's arcs, each
    /// tangent to the segment before it). An `end` on `start` is refused,
    /// and one straight behind it, which no tangent arc reaches.
    pub fn tangent_bulge(start: Point2, direction: f64, end: Point2) -> GeometryResult<(f64, f64)> {
        if !direction.is_finite() {
            return Err(GeometryError::NonFinite { what: "direction" });
        }
        // Half the sweep: from the start tangent to the chord.
        let half = angle::difference(
            direction,
            (end - start)
                .normalized()
                .map_err(|_| GeometryError::ZeroLength)?
                .angle(),
        );
        if half.abs() >= std::f64::consts::PI {
            return Err(GeometryError::Collinear);
        }
        Ok(((half / 2.0).tan(), angle::normalize(direction + 2.0 * half)))
    }

    /// The arc a polyline segment from `start` to `end` with `bulge` draws
    /// (the tangent of a quarter of its sweep, positive counterclockwise),
    /// and whether it runs backward: stored counterclockwise, a negative
    /// bulge's arc runs from `end` to `start`. A zero or non-finite bulge
    /// is a straight segment, not an arc, and is refused; so are `start`
    /// and `end` on one point.
    pub fn from_bulge(start: Point2, end: Point2, bulge: f64) -> GeometryResult<(Self, bool)> {
        if !bulge.is_finite() {
            return Err(GeometryError::NonFinite { what: "bulge" });
        }
        if bulge == 0.0 {
            return Err(GeometryError::OutOfRange { what: "bulge" });
        }
        let chord = end - start;
        let c = chord.length();
        if c == 0.0 {
            return Err(GeometryError::ZeroLength);
        }
        // The center lies off the chord's middle, to its left for a positive
        // bulge below a half circle: by (1 - b²) / 4b of the chord.
        let left = (-chord.y() / c, chord.x() / c);
        let off = (1.0 - bulge * bulge) / (4.0 * bulge) * c;
        let center = Point2::new(
            (start.x() + end.x()) / 2.0 + left.0 * off,
            (start.y() + end.y()) / 2.0 + left.1 * off,
        )?;
        let radius = center.distance_to(start);
        let angle = |p: Point2| {
            (p.y() - center.y())
                .atan2(p.x() - center.x())
                .rem_euclid(TAU)
        };
        let (from, to, backward) = if bulge > 0.0 {
            (angle(start), angle(end), false)
        } else {
            (angle(end), angle(start), true)
        };
        Ok((Self::new(center, radius, from, to)?, backward))
    }

    /// Replace the endpoints while retaining this arc's sagitta and its
    /// counterclockwise direction (the approved partial STRETCH rule).
    /// A chord within `tol.linear()` of zero is refused. Calculations use
    /// a local frame and finite intermediates; unrepresentable endpoints
    /// are refused rather than silently moving them.
    pub fn with_endpoints_preserving_sagitta(
        self,
        start: Point2,
        end: Point2,
        tol: Tolerance,
    ) -> GeometryResult<Self> {
        let chord = Vector2::new(end.x() - start.x(), end.y() - start.y())?;
        let length = chord.length();
        if length <= tol.linear() {
            return Err(GeometryError::ZeroLength);
        }
        if !length.is_finite() {
            return Err(GeometryError::NonFinite { what: "arc chord" });
        }
        if start == self.start_point() && end == self.end_point() {
            return Ok(self);
        }
        // 2 r sin²(theta/4) avoids cancellation for a shallow arc. The
        // perpendicular offset changes sign when the result is a major arc.
        let sine = (self.sweep() / 4.0).sin();
        let height = (self.radius * sine) * (2.0 * sine);
        if !(height.is_finite() && height > 0.0) {
            return Err(GeometryError::InvalidRadius);
        }
        let half = length / 2.0;
        let off = (half / 2.0) * (half / height) - height / 2.0;
        let x = chord.x() / 2.0 - chord.y() / length * off;
        let y = chord.y() / 2.0 + chord.x() / length * off;
        let center = Point2::new(start.x() + x, start.y() + y)?;
        let radius = half.hypot(off);
        let first = (-y).atan2(-x);
        let last = (chord.y() - y).atan2(chord.x() - x);
        let result = Self::new(center, radius, first, last)?;
        if !(result.start_point().approx_eq(start, tol) && result.end_point().approx_eq(end, tol)) {
            return Err(GeometryError::OutOfRange {
                what: "stretched arc endpoints",
            });
        }
        Ok(result)
    }

    /// Center point.
    pub fn center(self) -> Point2 {
        self.center
    }

    /// Radius.
    pub fn radius(self) -> f64 {
        self.radius
    }

    /// Start angle in `[0, 2π)`.
    pub fn start_angle(self) -> f64 {
        self.start_angle
    }

    /// End angle in `[0, 2π)`.
    pub fn end_angle(self) -> f64 {
        self.end_angle
    }

    /// Counterclockwise angle from start to end, in `(0, 2π)`.
    pub fn sweep(self) -> f64 {
        angle::ccw_sweep(self.start_angle, self.end_angle)
    }

    /// The full circle this arc belongs to.
    pub fn circle(self) -> Circle {
        Circle::new(self.center, self.radius).expect("arc radius is valid")
    }

    /// Point at the start angle.
    pub fn start_point(self) -> Point2 {
        self.circle().point_at_angle(self.start_angle)
    }

    /// Point at the end angle.
    pub fn end_point(self) -> Point2 {
        self.circle().point_at_angle(self.end_angle)
    }

    /// Point halfway along the arc.
    pub fn midpoint(self) -> Point2 {
        self.point_at(0.5)
    }

    /// Arc length.
    pub fn length(self) -> f64 {
        self.radius * self.sweep()
    }

    /// Whether the direction `angle` (radians) falls within the arc's sweep,
    /// within `tol.angular()` beyond either end.
    pub fn contains_angle(self, angle: f64, tol: Tolerance) -> bool {
        let offset = angle::ccw_sweep(self.start_angle, angle);
        offset <= self.sweep() + tol.angular() || offset >= TAU - tol.angular()
    }

    /// Bounding box: the end points plus every axis extreme (East, North,
    /// West, South) the arc passes through.
    pub fn bounds(self) -> Bounds2 {
        let sweep = self.sweep();
        let quadrants = (0..4)
            .map(|i| f64::from(i) * FRAC_PI_2)
            .filter(|&q| angle::ccw_sweep(self.start_angle, q) <= sweep)
            .map(|q| self.circle().point_at_angle(q));
        Bounds2::from_points(
            [self.start_point(), self.end_point()]
                .into_iter()
                .chain(quadrants),
        )
        .expect("at least two points")
    }

    /// Point at normalized parameter `t`: start at 0, end at 1.
    pub fn point_at(self, t: f64) -> Point2 {
        if t == 1.0 {
            return self.end_point();
        }
        self.circle()
            .point_at_angle(self.start_angle + t * self.sweep())
    }

    /// Point at `distance` along the arc from its start.
    pub fn point_at_distance(self, distance: f64) -> GeometryResult<Point2> {
        if !(distance.is_finite() && (0.0..=self.length()).contains(&distance)) {
            return Err(GeometryError::OutOfRange { what: "distance" });
        }
        Ok(self.point_at(distance / self.length()))
    }

    /// Point of the arc closest to `p`: the radial projection when it falls
    /// within the sweep, otherwise the nearer end point. For the center
    /// itself (every point equally close) the start point is returned.
    pub fn closest_point(self, p: Point2) -> Point2 {
        let Ok(dir) = (p - self.center).normalized() else {
            return self.start_point();
        };
        if angle::ccw_sweep(self.start_angle, dir.angle()) <= self.sweep() {
            return self.center + dir * self.radius;
        }
        let (s, e) = (self.start_point(), self.end_point());
        if p.distance_squared_to(s) <= p.distance_squared_to(e) {
            s
        } else {
            e
        }
    }

    /// Shortest distance from `p` to the arc.
    pub fn distance_to(self, p: Point2) -> f64 {
        p.distance_to(self.closest_point(p))
    }

    /// Whether `p` lies on the arc, within `tol.linear()`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        self.distance_to(p) <= tol.linear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn arcs_from_polyline_bulges() {
        let at = |x: f64, y: f64| Point2::new(x, y).unwrap();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
        // Bulge 1: a half circle, to the left of (0,0) -> (2,0) when positive.
        let (arc, backward) = Arc::from_bulge(at(0.0, 0.0), at(2.0, 0.0), 1.0).unwrap();
        assert!(!backward && arc.center().approx_eq(at(1.0, 0.0), Tolerance::DEFAULT));
        assert!(
            close(arc.radius(), 1.0) && close(arc.start_angle(), PI) && close(arc.end_angle(), 0.0)
        );
        let (arc, backward) = Arc::from_bulge(at(0.0, 0.0), at(2.0, 0.0), -1.0).unwrap();
        // Negative: clockwise from the start, stored counterclockwise from the end.
        assert!(backward && close(arc.start_angle(), 0.0) && close(arc.end_angle(), PI));
        assert!(
            arc.start_point()
                .approx_eq(at(2.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(arc.end_point().approx_eq(at(0.0, 0.0), Tolerance::DEFAULT));
        assert!(close(arc.sweep(), PI));
        // A quarter circle: tan(π/8), from (1,0) to (0,1) about the origin.
        let (arc, _) = Arc::from_bulge(at(1.0, 0.0), at(0.0, 1.0), (PI / 8.0).tan()).unwrap();
        assert!(arc.center().approx_eq(at(0.0, 0.0), Tolerance::DEFAULT));
        assert!(close(arc.radius(), 1.0) && close(arc.sweep(), PI / 2.0));
        // Survey coordinates keep the arc.
        let (arc, _) = Arc::from_bulge(at(5e5, 9e6), at(5e5 + 2.0, 9e6), 1.0).unwrap();
        assert!((arc.radius() - 1.0).abs() < 1e-9);
        // Refused: no arc.
        assert_eq!(
            Arc::from_bulge(at(0.0, 0.0), at(2.0, 0.0), 0.0),
            Err(GeometryError::OutOfRange { what: "bulge" })
        );
        assert_eq!(
            Arc::from_bulge(at(0.0, 0.0), at(2.0, 0.0), f64::NAN),
            Err(GeometryError::NonFinite { what: "bulge" })
        );
        assert_eq!(
            Arc::from_bulge(at(1.0, 1.0), at(1.0, 1.0), 1.0),
            Err(GeometryError::ZeroLength)
        );
    }

    #[test]
    fn tangent_arcs_as_polyline_bulges() {
        let o = Point2::ORIGIN;
        let at = |x: f64, y: f64| Point2::new(x, y).unwrap();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
        // Leaving East: straight ahead, no arc.
        let (bulge, end) = Arc::tangent_bulge(o, 0.0, at(2.0, 0.0)).unwrap();
        assert!(close(bulge, 0.0) && close(end, 0.0));
        // To (0,2): a half circle counterclockwise, ending West.
        let (bulge, end) = Arc::tangent_bulge(o, 0.0, at(0.0, 2.0)).unwrap();
        assert!(close(bulge, 1.0) && close(end, PI), "{bulge} {end}");
        // To (1,1): a quarter, tan(22.5°), ending North.
        let (bulge, end) = Arc::tangent_bulge(o, 0.0, at(1.0, 1.0)).unwrap();
        assert!(close(bulge, (PI / 8.0).tan()) && close(end, PI / 2.0));
        // To (1,-1): a quarter clockwise, ending South.
        let (bulge, end) = Arc::tangent_bulge(o, 0.0, at(1.0, -1.0)).unwrap();
        assert!(close(bulge, -(PI / 8.0).tan()) && close(end, 1.5 * PI));
        // Straight behind, or on the start: no tangent arc.
        assert_eq!(
            Arc::tangent_bulge(o, 0.0, at(-2.0, 0.0)),
            Err(GeometryError::Collinear)
        );
        assert_eq!(
            Arc::tangent_bulge(o, 0.0, o),
            Err(GeometryError::ZeroLength)
        );
    }

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn deg(d: f64) -> f64 {
        d.to_radians()
    }

    #[test]
    fn rejects_invalid_arcs() {
        let c = p(0.0, 0.0);
        assert_eq!(
            Arc::new(c, 0.0, 0.0, 1.0),
            Err(GeometryError::InvalidRadius)
        );
        assert_eq!(
            Arc::new(c, 1.0, f64::NAN, 1.0),
            Err(GeometryError::NonFinite { what: "Arc" })
        );
        assert_eq!(Arc::new(c, 1.0, 1.0, 1.0), Err(GeometryError::ZeroSweep));
        assert_eq!(
            Arc::new(c, 1.0, 0.0, TAU),
            Err(GeometryError::ZeroSweep),
            "a full turn is a circle"
        );
    }

    #[test]
    fn from_start_center_end_runs_counterclockwise_to_the_end_direction() {
        let a = Arc::from_start_center_end(p(10.0, 0.0), p(0.0, 0.0), p(0.0, 3.0)).unwrap();
        assert!((a.radius() - 10.0).abs() < 1e-12);
        assert!(a.start_point().approx_eq(p(10.0, 0.0), TOL));
        assert!(
            a.end_point().approx_eq(p(0.0, 10.0), TOL),
            "the end is on the arc's circle"
        );
        assert!((a.sweep() - FRAC_PI_2).abs() < 1e-12);
        // Clockwise of the start is the long way round.
        let b = Arc::from_start_center_end(p(10.0, 0.0), p(0.0, 0.0), p(0.0, -3.0)).unwrap();
        assert!((b.sweep() - 3.0 * FRAC_PI_2).abs() < 1e-12);
        let o = p(0.0, 0.0);
        assert_eq!(
            Arc::from_start_center_end(o, o, p(1.0, 0.0)),
            Err(GeometryError::InvalidRadius)
        );
        assert_eq!(
            Arc::from_start_center_end(p(1.0, 0.0), o, o),
            Err(GeometryError::ZeroVector)
        );
        assert_eq!(
            Arc::from_start_center_end(p(1.0, 0.0), o, p(5.0, 0.0)),
            Err(GeometryError::ZeroSweep)
        );
    }

    #[test]
    fn angles_are_normalized() {
        let a = Arc::new(p(0.0, 0.0), 1.0, -FRAC_PI_2, 5.0 * PI).unwrap();
        assert!((a.start_angle() - 3.0 * FRAC_PI_2).abs() < 1e-12);
        assert!((a.end_angle() - PI).abs() < 1e-12);
    }

    #[test]
    fn sweep_and_points_follow_dwg() {
        // 0° to 90°: a quarter, counterclockwise through 45°.
        let a = Arc::new(p(0.0, 0.0), 10.0, 0.0, FRAC_PI_2).unwrap();
        assert!((a.sweep() - FRAC_PI_2).abs() < 1e-15);
        assert!(a.start_point().approx_eq(p(10.0, 0.0), TOL));
        assert!(a.end_point().approx_eq(p(0.0, 10.0), TOL));
        assert!((a.length() - 5.0 * PI).abs() < 1e-12);
        let m = a.midpoint();
        assert!(m.approx_eq(p(10.0 * deg(45.0).cos(), 10.0 * deg(45.0).sin()), TOL));
        // 90° to 0° is the other three quarters, through 180°.
        let b = Arc::new(p(0.0, 0.0), 10.0, FRAC_PI_2, 0.0).unwrap();
        assert!((b.sweep() - 3.0 * FRAC_PI_2).abs() < 1e-12);
        assert!(
            b.midpoint()
                .approx_eq(p(10.0 * deg(225.0).cos(), 10.0 * deg(225.0).sin()), TOL)
        );
    }

    #[test]
    fn crossing_east_wraps_around() {
        // 350° to 10° counterclockwise passes through East (0°).
        let a = Arc::new(p(0.0, 0.0), 1.0, deg(350.0), deg(10.0)).unwrap();
        assert!((a.sweep() - deg(20.0)).abs() < 1e-12);
        assert!(a.contains_angle(0.0, TOL));
        assert!(!a.contains_angle(PI, TOL));
        assert!(a.midpoint().approx_eq(p(1.0, 0.0), TOL));
    }

    #[test]
    fn bounds_include_crossed_quadrants_only() {
        let c = p(0.0, 0.0);
        // 45° → 135° crosses North only.
        let a = Arc::new(c, 1.0, deg(45.0), deg(135.0)).unwrap();
        let b = a.bounds();
        assert!((b.max().y() - 1.0).abs() < 1e-15, "reaches North");
        assert!((b.min().y() - deg(45.0).sin()).abs() < 1e-15);
        assert!((b.width() - 2.0 * deg(45.0).cos()).abs() < 1e-15);
        // 350° → 10° crosses East only.
        let e = Arc::new(c, 1.0, deg(350.0), deg(10.0)).unwrap().bounds();
        assert!((e.max().x() - 1.0).abs() < 1e-15);
        assert!(e.min().x() > 0.9);
    }

    #[test]
    fn points_along() {
        let a = Arc::new(p(0.0, 0.0), 2.0, 0.0, PI).unwrap();
        assert_eq!(a.point_at(0.0), a.start_point());
        assert_eq!(a.point_at(1.0), a.end_point());
        assert!(a.point_at_distance(PI).unwrap().approx_eq(p(0.0, 2.0), TOL));
        assert!(a.point_at_distance(a.length() + 0.1).is_err());
    }

    #[test]
    fn closest_point_inside_and_outside_the_sweep() {
        let a = Arc::new(p(0.0, 0.0), 5.0, 0.0, FRAC_PI_2).unwrap();
        assert!(
            a.closest_point(p(10.0, 10.0))
                .approx_eq(p(5.0 * deg(45.0).cos(), 5.0 * deg(45.0).sin()), TOL)
        );
        // Outside the sweep: the nearer end point.
        assert!(
            a.closest_point(p(3.0, -4.0)).approx_eq(p(5.0, 0.0), TOL),
            "start is nearer"
        );
        assert!(
            a.closest_point(p(-3.0, -1.0)).approx_eq(p(0.0, 5.0), TOL),
            "end is nearer"
        );
        assert!(a.closest_point(p(-1.0, 3.0)).approx_eq(p(0.0, 5.0), TOL));
        assert_eq!(a.closest_point(a.center()), a.start_point());
        assert!(a.contains_point(p(3.0, 4.0), TOL));
        assert!(
            !a.contains_point(p(-3.0, -4.0), TOL),
            "on the circle but not on the arc"
        );
    }

    #[test]
    fn three_points_ccw_and_cw() {
        let (a, b, c) = (p(1.0, 0.0), p(0.0, 1.0), p(-1.0, 0.0));
        let ccw = Arc::from_three_points(a, b, c, TOL).unwrap();
        assert!(ccw.center().approx_eq(p(0.0, 0.0), TOL));
        assert!((ccw.radius() - 1.0).abs() < 1e-12);
        assert!(ccw.start_point().approx_eq(a, TOL) && ccw.end_point().approx_eq(c, TOL));
        assert!(ccw.contains_point(b, TOL));
        // Same points through the South: clockwise, stored from c to a.
        let cw = Arc::from_three_points(a, p(0.0, -1.0), c, TOL).unwrap();
        assert!(cw.start_point().approx_eq(c, TOL) && cw.end_point().approx_eq(a, TOL));
        assert!(cw.contains_point(p(0.0, -1.0), TOL));
    }

    #[test]
    fn three_points_rejects_collinear() {
        let r = Arc::from_three_points(p(0.0, 0.0), p(1.0, 1.0), p(2.0, 2.0), TOL);
        assert_eq!(r, Err(GeometryError::Collinear));
        let r = Arc::from_three_points(p(0.0, 0.0), p(0.0, 0.0), p(2.0, 2.0), TOL);
        assert_eq!(r, Err(GeometryError::Collinear), "repeated point");
    }

    #[test]
    fn three_points_at_survey_coordinates() {
        let o = (500_000.0, 9_800_000.0);
        let (a, b, c) = (p(o.0 + 10.0, o.1), p(o.0, o.1 + 10.0), p(o.0 - 10.0, o.1));
        let arc = Arc::from_three_points(a, b, c, TOL).unwrap();
        assert!(arc.center().distance_to(p(o.0, o.1)) < 1e-8);
        assert!((arc.radius() - 10.0).abs() < 1e-8);
    }
}

#[cfg(test)]
mod stretch_tests {
    use super::*;
    use std::f64::consts::PI;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn stretch_arc_matches_six_independent_native_measurements() {
        // ProgeCAD 2026: docs/testing/stretch-arcs-windows.md, A1-A4b.
        // Expected values are recorded measurements, not reconstructed here.
        for (sweep, move_start, cx, cy, radius, first, last) in [
            (
                90.0_f64,
                false,
                0.3542858741,
                2.4583673926,
                5.2560660172,
                332.1135992961_f64,
                59.7771925057_f64,
            ),
            (
                90.0,
                true,
                2.0692751777,
                -1.6485995261,
                6.9631727984,
                31.5999528450,
                107.2879567159,
            ),
            (
                180.0,
                false,
                1.8227973253,
                2.1297906385,
                3.825,
                326.1646634679,
                181.9445447302,
            ),
            (
                180.0,
                true,
                1.7775043613,
                -0.8037783482,
                6.825,
                24.2557288003,
                173.2365957248,
            ),
            (
                270.0,
                false,
                0.6074094784,
                0.7617270144,
                4.4581475452,
                350.1620716040,
                302.4577933440,
            ),
            (
                270.0,
                true,
                2.2793799644,
                0.4664228978,
                5.9226136393,
                15.0069296776,
                247.3649206538,
            ),
        ] {
            let before = Arc::new(p(0.0, 0.0), 5.0, 0.0, sweep.to_radians()).unwrap();
            let shift = Vector2::new(3.0, 2.0).unwrap();
            let start = before.start_point() + if move_start { shift } else { Vector2::ZERO };
            let end = before.end_point() + if move_start { Vector2::ZERO } else { shift };
            let after = before
                .with_endpoints_preserving_sagitta(start, end, Tolerance::DEFAULT)
                .unwrap();
            assert!(
                after.center().distance_to(p(cx, cy)) < 1e-10,
                "{sweep} {move_start}: {after:?}"
            );
            assert!((after.radius() - radius).abs() < 1e-10);
            assert!((after.start_angle() - first.to_radians()).abs() < 2e-12);
            assert!((after.end_angle() - last.to_radians()).abs() < 2e-12);
            assert!(after.start_point().approx_eq(start, Tolerance::DEFAULT));
            assert!(after.end_point().approx_eq(end, Tolerance::DEFAULT));
        }
    }

    #[test]
    fn stretch_arc_retains_sagitta_across_scales_and_survey_coordinates() {
        for scale in [1e-4, 1.0, 1e4] {
            for (x, y) in [(0.0, 0.0), (1e6, 2e6)] {
                for sweep in [90.0_f64, 180.0, 270.0, 359.0] {
                    let old = Arc::new(p(x, y), 5.0 * scale, 0.0, sweep.to_radians()).unwrap();
                    let end = p(
                        old.end_point().x() + 3.0 * scale,
                        old.end_point().y() + 2.0 * scale,
                    );
                    let new = old
                        .with_endpoints_preserving_sagitta(
                            old.start_point(),
                            end,
                            Tolerance::DEFAULT,
                        )
                        .unwrap_or_else(|e| panic!("scale={scale} x={x} sweep={sweep}: {e}"));
                    let height = |a: Arc| 2.0 * a.radius() * (a.sweep() / 4.0).sin().powi(2);
                    assert!(
                        (height(old) - height(new)).abs() < 1e-8 * scale.max(1.0),
                        "{scale} {sweep}"
                    );
                    assert!(
                        new.start_point()
                            .approx_eq(old.start_point(), Tolerance::DEFAULT)
                    );
                    assert!(new.end_point().approx_eq(end, Tolerance::DEFAULT));
                }
            }
        }
    }

    #[test]
    fn stretch_arc_refuses_collapsed_chords_and_nonfinite_intermediates() {
        let old = Arc::new(p(0.0, 0.0), 5.0, 0.0, PI / 2.0).unwrap();
        for end in [p(5.0, 0.0), p(5.0 + 1e-10, 0.0)] {
            assert_eq!(
                old.with_endpoints_preserving_sagitta(p(5.0, 0.0), end, Tolerance::DEFAULT),
                Err(GeometryError::ZeroLength)
            );
        }
        assert!(
            old.with_endpoints_preserving_sagitta(
                p(-1e308, 0.0),
                p(1e308, 0.0),
                Tolerance::DEFAULT
            )
            .is_err()
        );
        let shallow = Arc::new(p(0.0, 0.0), 5.0, 0.0, 1e-8).unwrap();
        assert!(
            shallow
                .with_endpoints_preserving_sagitta(
                    shallow.start_point(),
                    p(3.0, 2.0),
                    Tolerance::DEFAULT
                )
                .is_err(),
            "do not accept endpoints that f64 cannot reproduce"
        );
        assert_eq!(
            old.with_endpoints_preserving_sagitta(
                old.start_point(),
                old.end_point(),
                Tolerance::DEFAULT
            )
            .unwrap(),
            old
        );
    }
}
