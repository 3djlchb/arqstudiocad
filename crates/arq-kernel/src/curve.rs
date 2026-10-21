//! The curves a B-rep edge runs along (ADR 0024 D1): straight lines and
//! ellipses (circles included) for now; NURBS curves come with the next
//! step of reading.

use arq_geometry::{GeometryError, GeometryResult, Point3, Transform3, Vector3};

/// An edge's curve, unbounded: the edge's vertices bound it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    /// A straight line.
    Line(LineCurve),
    /// An ellipse, or a circle when its ratio is 1.
    Ellipse(EllipseCurve),
}

impl Curve {
    /// The shortest distance from `p` to the curve.
    pub fn distance(&self, p: Point3) -> f64 {
        match self {
            Self::Line(l) => l.distance(p),
            Self::Ellipse(e) => e.distance(p),
        }
    }

    /// The curve moved by `t`, a rigid motion with uniform scale.
    pub fn transformed(&self, t: Transform3) -> GeometryResult<Self> {
        Ok(match self {
            Self::Line(l) => Self::Line(LineCurve::new(
                t.apply_point(l.origin),
                t.apply_vector(l.direction),
            )?),
            Self::Ellipse(e) => Self::Ellipse(e.transformed(t)?),
        })
    }
}

/// A straight line through `origin` along a unit `direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCurve {
    origin: Point3,
    direction: Vector3,
}

impl LineCurve {
    /// The line through `origin` along `direction` (any non-zero length).
    pub fn new(origin: Point3, direction: Vector3) -> GeometryResult<Self> {
        Ok(Self {
            origin,
            direction: direction.normalized()?,
        })
    }

    /// A point on the line.
    pub fn origin(&self) -> Point3 {
        self.origin
    }

    /// Its unit direction.
    pub fn direction(&self) -> Vector3 {
        self.direction
    }

    /// The shortest distance from `p` to the line.
    pub fn distance(&self, p: Point3) -> f64 {
        (p - self.origin).cross(self.direction).length()
    }
}

/// An ellipse around `center` in the plane of unit `normal`: `major` is the
/// semi-major axis (its length the radius), the minor axis is
/// `normal × major` scaled by `ratio`. A circle has ratio 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EllipseCurve {
    center: Point3,
    normal: Vector3,
    major: Vector3,
    ratio: f64,
}

impl EllipseCurve {
    /// The ellipse; `major` must be perpendicular to `normal`, both non-zero,
    /// and `ratio` positive.
    pub fn new(
        center: Point3,
        normal: Vector3,
        major: Vector3,
        ratio: f64,
    ) -> GeometryResult<Self> {
        let normal = normal.normalized()?;
        let radius = major.length();
        if !(radius.is_finite() && radius > 0.0) {
            return Err(GeometryError::InvalidRadius);
        }
        if !(ratio.is_finite() && ratio > 0.0) {
            return Err(GeometryError::InvalidScale);
        }
        if major.dot(normal).abs() > 1e-9 * radius {
            return Err(GeometryError::OutOfRange {
                what: "ellipse axis",
            });
        }
        Ok(Self {
            center,
            normal,
            major,
            ratio,
        })
    }

    /// Its center.
    pub fn center(&self) -> Point3 {
        self.center
    }

    /// The unit normal of its plane.
    pub fn normal(&self) -> Vector3 {
        self.normal
    }

    /// The semi-major axis.
    pub fn major(&self) -> Vector3 {
        self.major
    }

    /// Minor over major radius.
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// The ellipse moved by `t`, a rigid motion with uniform scale.
    pub fn transformed(&self, t: Transform3) -> GeometryResult<Self> {
        Self::new(
            t.apply_point(self.center),
            t.apply_vector(self.normal),
            t.apply_vector(self.major),
            self.ratio,
        )
    }

    /// The point at parameter `t` (radians from the major axis).
    pub fn point_at(&self, t: f64) -> Point3 {
        let minor = self.normal.cross(self.major) * self.ratio;
        self.center + self.major * t.cos() + minor * t.sin()
    }

    /// The shortest distance from `p` to the ellipse: in its plane, the
    /// closest point by Newton's method from the point's own angle; off
    /// the plane, combined with the height.
    pub fn distance(&self, p: Point3) -> f64 {
        let a = self.major.length();
        let b = a * self.ratio;
        let u = self.major / a;
        let v = self.normal.cross(u);
        let d = p - self.center;
        let (x, y, h) = (d.dot(u), d.dot(v), d.dot(self.normal));
        let mut t = (y * a).atan2(x * b);
        for _ in 0..16 {
            let (s, c) = t.sin_cos();
            // f(t) = derivative of the squared distance, over 2.
            let f = (a * a - b * b) * s * c - x * a * s + y * b * c;
            let df = (a * a - b * b) * (c * c - s * s) - x * a * c - y * b * s;
            if df.abs() < f64::EPSILON {
                break;
            }
            let step = f / df;
            t -= step;
            if step.abs() < 1e-14 {
                break;
            }
        }
        let (s, c) = t.sin_cos();
        let (dx, dy) = (x - a * c, y - b * s);
        (dx * dx + dy * dy + h * h).sqrt()
    }
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod tests;
