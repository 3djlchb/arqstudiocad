//! `Transform2`: moves, rotations, uniform scalings and mirrors.

use crate::angle;
use crate::arc::Arc;
use crate::circle::Circle;
use crate::error::{GeometryError, GeometryResult};
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// A similarity transformation: any combination of translation, rotation,
/// uniform scaling and mirroring. Circles stay circles and arcs stay arcs.
///
/// Applied as `pivot + M (p - pivot) + t`: subtracting the pivot first is the
/// local-frame rule (ADR 0010), so rotating or scaling around a point with
/// survey coordinates keeps full precision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2 {
    // Linear part [[a, b], [c, d]]: always a scaled rotation or reflection.
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    pivot: Point2,
    t: Vector2,
}

impl Transform2 {
    /// Leaves everything where it is.
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        pivot: Point2::ORIGIN,
        t: Vector2::ZERO,
    };

    fn linear(a: f64, b: f64, c: f64, d: f64, pivot: Point2) -> Self {
        Self {
            a,
            b,
            c,
            d,
            pivot,
            t: Vector2::ZERO,
        }
    }

    /// Moves by `offset` (MOVE, COPY).
    pub fn translation(offset: Vector2) -> Self {
        Self {
            t: offset,
            ..Self::IDENTITY
        }
    }

    /// Rotates by `angle` radians counterclockwise around `center` (ROTATE).
    pub fn rotation(center: Point2, angle: f64) -> GeometryResult<Self> {
        if !angle.is_finite() {
            return Err(GeometryError::NonFinite {
                what: "rotation angle",
            });
        }
        let (sin, cos) = angle.sin_cos();
        Ok(Self::linear(cos, -sin, sin, cos, center))
    }

    /// Scales by `factor` around `center` (SCALE). The factor must be finite
    /// and greater than zero.
    pub fn scaling(center: Point2, factor: f64) -> GeometryResult<Self> {
        if !(factor.is_finite() && factor > 0.0) {
            return Err(GeometryError::InvalidScale);
        }
        Ok(Self::linear(factor, 0.0, 0.0, factor, center))
    }

    /// Mirrors across the infinite line through `axis` (MIRROR).
    pub fn mirror(axis: Line) -> Self {
        let u = axis.direction();
        let (ux, uy) = (u.x(), u.y());
        let (cos2, sin2) = (ux * ux - uy * uy, 2.0 * ux * uy);
        Self::linear(cos2, sin2, sin2, -cos2, axis.start())
    }

    /// `self` followed by `next`, as a single transformation.
    pub fn then(self, next: Self) -> Self {
        // next(self(p)) = p1 + M2 M1 (p - p1) + t', with the pivot of `self`
        // kept and every term a difference of pivots (local frame).
        let (a, b, c, d) = (
            next.a * self.a + next.b * self.c,
            next.a * self.b + next.b * self.d,
            next.c * self.a + next.d * self.c,
            next.c * self.b + next.d * self.d,
        );
        let t = (next.pivot - self.pivot) + next.mul(self.pivot - next.pivot + self.t) + next.t;
        Self {
            a,
            b,
            c,
            d,
            pivot: self.pivot,
            t,
        }
    }

    /// The transformation that undoes this one.
    pub fn inverse(self) -> Self {
        let det = self.a * self.d - self.b * self.c;
        Self {
            a: self.d / det,
            b: -self.b / det,
            c: -self.c / det,
            d: self.a / det,
            pivot: self.pivot + self.t,
            t: -self.t,
        }
    }

    /// Linear part applied to a vector.
    fn mul(self, v: Vector2) -> Vector2 {
        Vector2::from_finite(
            self.a * v.x() + self.b * v.y(),
            self.c * v.x() + self.d * v.y(),
        )
    }

    /// Uniform scale factor (lengths are multiplied by it).
    pub fn scale_factor(self) -> f64 {
        self.a.hypot(self.c)
    }

    /// Whether it mirrors, reversing the turning direction of curves.
    pub fn is_reflection(self) -> bool {
        self.a * self.d - self.b * self.c < 0.0
    }

    /// Angle in `[0, 2π)` of the image of East: the rotation, or for a
    /// mirror twice the axis angle.
    fn turn(self) -> f64 {
        angle::normalize(self.c.atan2(self.a))
    }

    /// Image of a point.
    pub fn apply_point(self, p: Point2) -> Point2 {
        self.pivot + self.mul(p - self.pivot) + self.t
    }

    /// Image of a displacement (translations do not affect vectors).
    pub fn apply_vector(self, v: Vector2) -> Vector2 {
        self.mul(v)
    }

    /// Image of a line.
    pub fn apply_line(self, l: Line) -> GeometryResult<Line> {
        Line::new(self.apply_point(l.start()), self.apply_point(l.end()))
    }

    /// Image of a circle.
    pub fn apply_circle(self, c: Circle) -> GeometryResult<Circle> {
        Circle::new(
            self.apply_point(c.center()),
            c.radius() * self.scale_factor(),
        )
    }

    /// Image of an arc. A mirror reverses its direction, so start and end
    /// swap to keep it counterclockwise (as DWG does).
    pub fn apply_arc(self, arc: Arc) -> GeometryResult<Arc> {
        let center = self.apply_point(arc.center());
        let radius = arc.radius() * self.scale_factor();
        let turn = self.turn();
        if self.is_reflection() {
            // A mirror sends direction α to `turn - α`.
            Arc::new(
                center,
                radius,
                turn - arc.end_angle(),
                turn - arc.start_angle(),
            )
        } else {
            Arc::new(
                center,
                radius,
                arc.start_angle() + turn,
                arc.end_angle() + turn,
            )
        }
    }

    /// Image of any curve.
    pub fn apply(self, g: Geometry2) -> GeometryResult<Geometry2> {
        Ok(match g {
            Geometry2::Line(l) => self.apply_line(l)?.into(),
            Geometry2::Circle(c) => self.apply_circle(c)?.into(),
            Geometry2::Arc(a) => self.apply_arc(a)?.into(),
        })
    }

    /// Whether both transformations move the given sample points to the
    /// same places, within `tol.linear()`.
    pub fn approx_eq_at(self, other: Self, samples: &[Point2], tol: Tolerance) -> bool {
        samples
            .iter()
            .all(|&p| self.apply_point(p).approx_eq(other.apply_point(p), tol))
    }
}

impl Default for Transform2 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn v(x: f64, y: f64) -> Vector2 {
        Vector2::new(x, y).unwrap()
    }

    #[test]
    fn translation_moves_points_not_vectors() {
        let t = Transform2::translation(v(3.0, -1.0));
        assert_eq!(t.apply_point(p(1.0, 1.0)), p(4.0, 0.0));
        assert_eq!(t.apply_vector(v(1.0, 1.0)), v(1.0, 1.0));
        assert_eq!(Transform2::IDENTITY.apply_point(p(7.0, 8.0)), p(7.0, 8.0));
    }

    #[test]
    fn rotation_around_a_point() {
        let r = Transform2::rotation(p(1.0, 1.0), FRAC_PI_2).unwrap();
        assert!(r.apply_point(p(2.0, 1.0)).approx_eq(p(1.0, 2.0), TOL));
        assert!(
            r.apply_point(p(1.0, 1.0)).approx_eq(p(1.0, 1.0), TOL),
            "the center stays"
        );
        assert!(Transform2::rotation(p(0.0, 0.0), f64::NAN).is_err());
    }

    #[test]
    fn scaling_around_a_point() {
        let s = Transform2::scaling(p(1.0, 1.0), 3.0).unwrap();
        assert_eq!(s.apply_point(p(2.0, 1.0)), p(4.0, 1.0));
        assert_eq!(s.scale_factor(), 3.0);
        for bad in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Transform2::scaling(p(0.0, 0.0), bad),
                Err(GeometryError::InvalidScale)
            );
        }
    }

    #[test]
    fn mirror_across_axes() {
        let x_axis = Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
        let m = Transform2::mirror(x_axis);
        assert!(m.is_reflection());
        assert!(m.apply_point(p(3.0, 2.0)).approx_eq(p(3.0, -2.0), TOL));
        let diagonal = Line::new(p(0.0, 0.0), p(1.0, 1.0)).unwrap();
        assert!(
            Transform2::mirror(diagonal)
                .apply_point(p(3.0, 1.0))
                .approx_eq(p(1.0, 3.0), TOL)
        );
        let offset_axis = Line::new(p(0.0, 5.0), p(10.0, 5.0)).unwrap();
        assert!(
            Transform2::mirror(offset_axis)
                .apply_point(p(2.0, 7.0))
                .approx_eq(p(2.0, 3.0), TOL)
        );
    }

    #[test]
    fn composition_and_inverse() {
        let r = Transform2::rotation(p(0.0, 0.0), FRAC_PI_2).unwrap();
        let t = Transform2::translation(v(10.0, 0.0));
        let both = r.then(t);
        assert!(
            both.apply_point(p(1.0, 0.0)).approx_eq(p(10.0, 1.0), TOL),
            "rotate, then move"
        );
        assert!(
            t.then(r)
                .apply_point(p(1.0, 0.0))
                .approx_eq(p(0.0, 11.0), TOL),
            "order matters"
        );
        let back = both.inverse();
        assert!(back.apply_point(p(10.0, 1.0)).approx_eq(p(1.0, 0.0), TOL));
        let samples = [p(0.0, 0.0), p(3.0, -2.0), p(-7.0, 5.0)];
        assert!(
            both.then(back)
                .approx_eq_at(Transform2::IDENTITY, &samples, TOL)
        );
    }

    #[test]
    fn circles_scale_their_radius() {
        let c = Circle::new(p(1.0, 0.0), 2.0).unwrap();
        let s = Transform2::scaling(p(0.0, 0.0), 2.5).unwrap();
        let out = s.apply_circle(c).unwrap();
        assert_eq!(out.center(), p(2.5, 0.0));
        assert_eq!(out.radius(), 5.0);
    }

    #[test]
    fn rotating_an_arc_turns_its_angles() {
        let a = Arc::new(p(0.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
        let r = Transform2::rotation(p(0.0, 0.0), PI).unwrap();
        let out = r.apply_arc(a).unwrap();
        assert!((out.start_angle() - PI).abs() < 1e-12);
        assert!((out.end_angle() - 3.0 * FRAC_PI_2).abs() < 1e-12);
        assert!((out.sweep() - a.sweep()).abs() < 1e-12);
    }

    #[test]
    fn mirroring_an_arc_swaps_its_ends() {
        // Quarter arc in the first quadrant, mirrored across the X axis:
        // the image lies in the fourth quadrant, still counterclockwise.
        let a = Arc::new(p(0.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
        let x_axis = Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
        let out = Transform2::mirror(x_axis).apply_arc(a).unwrap();
        assert!(
            out.start_point().approx_eq(p(0.0, -1.0), TOL),
            "old end becomes the start"
        );
        assert!(out.end_point().approx_eq(p(1.0, 0.0), TOL));
        assert!(out.contains_point(p(0.5_f64.sqrt(), -(0.5_f64.sqrt())), TOL));
        assert!((out.sweep() - FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn mirroring_twice_is_identity() {
        let axis = Line::new(p(2.0, 3.0), p(5.0, -1.0)).unwrap();
        let m = Transform2::mirror(axis);
        let samples = [p(0.0, 0.0), p(4.0, 4.0), p(-3.0, 9.0)];
        assert!(m.then(m).approx_eq_at(Transform2::IDENTITY, &samples, TOL));
        assert!(!m.then(m).is_reflection());
    }

    #[test]
    fn precise_at_survey_coordinates() {
        let c = p(500_000.0, 9_800_000.0);
        let r = Transform2::rotation(c, FRAC_PI_2).unwrap();
        let q = r.apply_point(p(500_001.0, 9_800_000.0));
        assert!(q.approx_eq(p(500_000.0, 9_800_001.0), TOL));
        assert!(
            r.then(r.inverse())
                .approx_eq_at(Transform2::IDENTITY, &[q, c], TOL)
        );
    }
}
