//! The surfaces a B-rep face lies on (ADR 0024 D1): planes, circular cones
//! (cylinders included), elliptic cylinders, spheres, tori and NURBS
//! surfaces. Each is unbounded (a NURBS one bounded by its domain): the
//! face's loops trim it.

use arq_geometry::{GeometryError, GeometryResult, Point3, Transform3, Vector3};

use crate::curve::EllipseCurve;
use crate::nurbs::NurbsSurface;

/// A face's surface.
#[derive(Clone, Debug, PartialEq)]
pub enum Surface {
    /// A plane.
    Plane(Plane),
    /// A circular cone, or a cylinder when its half angle is zero.
    Cone(Cone),
    /// A cylinder of elliptic section.
    EllipticCylinder(EllipticCylinder),
    /// A sphere.
    Sphere(Sphere),
    /// A torus.
    Torus(Torus),
    /// A NURBS surface.
    Nurbs(NurbsSurface),
}

impl Surface {
    /// The shortest distance from `p` to the surface.
    pub fn distance(&self, p: Point3) -> f64 {
        match self {
            Self::Plane(s) => s.distance(p),
            Self::Cone(s) => s.distance(p),
            Self::EllipticCylinder(s) => s.distance(p),
            Self::Sphere(s) => s.distance(p),
            Self::Torus(s) => s.distance(p),
            Self::Nurbs(s) => s.distance(p),
        }
    }

    /// The surface moved by `t`, a rigid motion with uniform `scale`.
    pub fn transformed(&self, t: Transform3, scale: f64) -> GeometryResult<Self> {
        Ok(match self {
            Self::Plane(s) => Self::Plane(Plane::new(
                t.apply_point(s.origin),
                t.apply_vector(s.normal),
            )?),
            Self::Cone(s) => Self::Cone(Cone::new(
                t.apply_point(s.origin),
                t.apply_vector(s.axis),
                s.radius * scale,
                s.sin_half_angle,
                s.cos_half_angle,
            )?),
            Self::EllipticCylinder(s) => {
                Self::EllipticCylinder(EllipticCylinder::new(s.section.transformed(t)?))
            }
            Self::Sphere(s) => {
                Self::Sphere(Sphere::new(t.apply_point(s.center), s.radius * scale)?)
            }
            Self::Torus(s) => Self::Torus(Torus::new(
                t.apply_point(s.center),
                t.apply_vector(s.axis),
                s.major_radius * scale,
                s.minor_radius * scale,
            )?),
            Self::Nurbs(s) => Self::Nurbs(s.transformed(t)),
        })
    }
}

/// The plane through `origin` with unit `normal`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    origin: Point3,
    normal: Vector3,
}

impl Plane {
    /// The plane through `origin` perpendicular to `normal` (non-zero).
    pub fn new(origin: Point3, normal: Vector3) -> GeometryResult<Self> {
        Ok(Self {
            origin,
            normal: normal.normalized()?,
        })
    }

    /// A point on the plane.
    pub fn origin(&self) -> Point3 {
        self.origin
    }

    /// Its unit normal.
    pub fn normal(&self) -> Vector3 {
        self.normal
    }

    /// The distance from `p` to the plane.
    pub fn distance(&self, p: Point3) -> f64 {
        (p - self.origin).dot(self.normal).abs()
    }
}

/// A circular cone around the unit `axis` through `origin`, of `radius` at
/// `origin`, widening by `sin / cos` of its half angle per unit along the
/// axis; a cylinder when the sine is zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    origin: Point3,
    axis: Vector3,
    radius: f64,
    sin_half_angle: f64,
    cos_half_angle: f64,
}

impl Cone {
    /// The cone; `radius` not negative, the half angle's sine and cosine a
    /// unit pair with a positive cosine.
    pub fn new(
        origin: Point3,
        axis: Vector3,
        radius: f64,
        sin_half_angle: f64,
        cos_half_angle: f64,
    ) -> GeometryResult<Self> {
        if !(radius.is_finite() && radius >= 0.0) {
            return Err(GeometryError::InvalidRadius);
        }
        let unit = sin_half_angle.hypot(cos_half_angle);
        if !(cos_half_angle > 0.0 && (unit - 1.0).abs() < 1e-9) {
            return Err(GeometryError::OutOfRange { what: "half angle" });
        }
        Ok(Self {
            origin,
            axis: axis.normalized()?,
            radius,
            sin_half_angle,
            cos_half_angle,
        })
    }

    /// Whether it is a cylinder (no half angle).
    pub fn is_cylinder(&self) -> bool {
        self.sin_half_angle == 0.0
    }

    /// The center of its reference circle.
    pub fn origin(&self) -> Point3 {
        self.origin
    }

    /// Its unit axis.
    pub fn axis(&self) -> Vector3 {
        self.axis
    }

    /// The radius of its reference circle.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// The sine of its half angle (0 for a cylinder).
    pub fn sin_half_angle(&self) -> f64 {
        self.sin_half_angle
    }

    /// The cosine of its half angle (positive).
    pub fn cos_half_angle(&self) -> f64 {
        self.cos_half_angle
    }

    /// The distance from `p` to the cone: across its side, measured
    /// perpendicular to it.
    pub fn distance(&self, p: Point3) -> f64 {
        let d = p - self.origin;
        let h = d.dot(self.axis);
        let rho = (d - self.axis * h).length();
        let at = self.radius + h * self.sin_half_angle / self.cos_half_angle;
        (rho - at).abs() * self.cos_half_angle
    }
}

/// A cylinder whose section is an ellipse, along the ellipse's normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EllipticCylinder {
    section: EllipseCurve,
}

impl EllipticCylinder {
    /// The cylinder of `section`, along its normal.
    pub fn new(section: EllipseCurve) -> Self {
        Self { section }
    }

    /// Its section at the reference point.
    pub fn section(&self) -> EllipseCurve {
        self.section
    }

    /// The distance from `p` to the cylinder: from `p`'s projection along
    /// the axis to the section.
    pub fn distance(&self, p: Point3) -> f64 {
        let n = self.section.normal();
        let along = (p - self.section.center()).dot(n);
        self.section.distance(p - n * along)
    }
}

/// A sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    center: Point3,
    radius: f64,
}

impl Sphere {
    /// The sphere of positive `radius` around `center`.
    pub fn new(center: Point3, radius: f64) -> GeometryResult<Self> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(GeometryError::InvalidRadius);
        }
        Ok(Self { center, radius })
    }

    /// Its center.
    pub fn center(&self) -> Point3 {
        self.center
    }

    /// Its radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// The distance from `p` to the sphere.
    pub fn distance(&self, p: Point3) -> f64 {
        ((p - self.center).length() - self.radius).abs()
    }
}

/// A torus around the unit `axis` through `center`: a circle of
/// `minor_radius` swept around the axis at `major_radius`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Torus {
    center: Point3,
    axis: Vector3,
    major_radius: f64,
    minor_radius: f64,
}

impl Torus {
    /// The torus; the minor radius positive, the major one finite (an apple
    /// or lemon torus has a major radius smaller than the minor one).
    pub fn new(
        center: Point3,
        axis: Vector3,
        major_radius: f64,
        minor_radius: f64,
    ) -> GeometryResult<Self> {
        if !(major_radius.is_finite() && minor_radius.is_finite() && minor_radius > 0.0) {
            return Err(GeometryError::InvalidRadius);
        }
        Ok(Self {
            center,
            axis: axis.normalized()?,
            major_radius,
            minor_radius,
        })
    }

    /// Its center.
    pub fn center(&self) -> Point3 {
        self.center
    }

    /// Its unit axis.
    pub fn axis(&self) -> Vector3 {
        self.axis
    }

    /// The distance from the center to the tube's center circle.
    pub fn major_radius(&self) -> f64 {
        self.major_radius
    }

    /// The tube's radius.
    pub fn minor_radius(&self) -> f64 {
        self.minor_radius
    }

    /// The distance from `p` to the torus.
    pub fn distance(&self, p: Point3) -> f64 {
        let d = p - self.center;
        let h = d.dot(self.axis);
        let rho = (d - self.axis * h).length();
        ((rho - self.major_radius).hypot(h) - self.minor_radius).abs()
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
