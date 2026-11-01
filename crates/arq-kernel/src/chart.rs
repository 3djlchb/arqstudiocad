//! A face's surface as a map from parameters `(u, v)` to space, its
//! derivatives and its inverse: what integrating over a face needs (ADR
//! 0024 D4, step 3, measure).
//!
//! Each analytic surface gets the parametrization drawing already uses for
//! its isolines: `u` is the angle around the axis (periodic, `2π`); `v` is
//! the height along the axis (cones, cylinders), the latitude (spheres) or
//! the angle around the tube (tori, periodic too). A plane uses two
//! orthogonal unit directions. A NURBS surface uses its own parameters,
//! closed in a direction when its two ends meet, and degenerate along a
//! row that collapses to a point (a pole); its `u` and `v` are swapped when
//! that puts the closed direction, and the poles, the way the others have
//! them.
//!
//! Singular lines are where `∂S/∂u` vanishes: a sphere's poles, a cone's
//! apex, a collapsed NURBS row; each is a value of `v`.

use std::f64::consts::{FRAC_PI_2, TAU};

use arq_geometry::{Point3, Vector3};

use crate::nurbs::NurbsSurface;
use crate::surface::Surface;

/// A surface's parametrization.
#[derive(Clone, Debug)]
pub(crate) enum Chart<'a> {
    /// `o + e1 u + e2 v`.
    Plane { o: Point3, e1: Vector3, e2: Vector3 },
    /// `o + a v + dir(u) (r0 + k v)`: a cone, or a cylinder when `k` is 0.
    Cone {
        o: Point3,
        a: Vector3,
        e1: Vector3,
        e2: Vector3,
        r0: f64,
        k: f64,
    },
    /// `c + m cos u + n sin u + a v`.
    Elliptic {
        c: Point3,
        m: Vector3,
        n: Vector3,
        a: Vector3,
    },
    /// `c + r (cos v dir(u) + z sin v)`.
    Sphere {
        c: Point3,
        r: f64,
        z: Vector3,
        e1: Vector3,
        e2: Vector3,
    },
    /// `c + dir(u) (big + small cos v) + a small sin v`.
    Torus {
        c: Point3,
        a: Vector3,
        e1: Vector3,
        e2: Vector3,
        big: f64,
        small: f64,
    },
    /// The surface's own parameters, swapped when `swap`.
    Nurbs {
        surface: &'a NurbsSurface,
        swap: bool,
        u: (f64, f64),
        v: (f64, f64),
        closed_u: bool,
        closed_v: bool,
        poles: Vec<f64>,
    },
}

/// Two unit vectors perpendicular to unit `axis` and to each other, so
/// that `(e1, e2, axis)` is right-handed.
pub(crate) fn frame(axis: Vector3) -> (Vector3, Vector3) {
    let helper = if axis.x().abs() < 0.9 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    }
    .expect("finite");
    let e1 = axis.cross(helper).normalized().expect("not parallel");
    (e1, axis.cross(e1))
}

/// The unit direction at angle `u` in the plane of `e1` and `e2`, and its
/// derivative.
fn around(e1: Vector3, e2: Vector3, u: f64) -> (Vector3, Vector3) {
    let (s, c) = u.sin_cos();
    (e1 * c + e2 * s, e2 * c - e1 * s)
}

impl<'a> Chart<'a> {
    /// The chart of `surface`.
    pub(crate) fn new(surface: &'a Surface) -> Self {
        match surface {
            Surface::Plane(p) => {
                let (e1, e2) = frame(p.normal());
                Self::Plane {
                    o: p.origin(),
                    e1,
                    e2,
                }
            }
            Surface::Cone(c) => {
                let (e1, e2) = frame(c.axis());
                Self::Cone {
                    o: c.origin(),
                    a: c.axis(),
                    e1,
                    e2,
                    r0: c.radius(),
                    k: c.sin_half_angle() / c.cos_half_angle(),
                }
            }
            Surface::EllipticCylinder(e) => {
                let s = e.section();
                Self::Elliptic {
                    c: s.center(),
                    m: s.major(),
                    n: s.normal().cross(s.major()) * s.ratio(),
                    a: s.normal(),
                }
            }
            Surface::Sphere(s) => {
                let z = Vector3::new(0.0, 0.0, 1.0).expect("finite");
                let (e1, e2) = frame(z);
                Self::Sphere {
                    c: s.center(),
                    r: s.radius(),
                    z,
                    e1,
                    e2,
                }
            }
            Surface::Torus(t) => {
                let (e1, e2) = frame(t.axis());
                Self::Torus {
                    c: t.center(),
                    a: t.axis(),
                    e1,
                    e2,
                    big: t.major_radius(),
                    small: t.minor_radius(),
                }
            }
            Surface::Nurbs(n) => nurbs_chart(n),
        }
    }

    /// The point at `(u, v)`.
    pub(crate) fn point(&self, u: f64, v: f64) -> Point3 {
        match *self {
            Self::Plane { o, e1, e2 } => o + e1 * u + e2 * v,
            Self::Cone {
                o,
                a,
                e1,
                e2,
                r0,
                k,
            } => o + a * v + around(e1, e2, u).0 * (r0 + k * v),
            Self::Elliptic { c, m, n, a } => c + m * u.cos() + n * u.sin() + a * v,
            Self::Sphere { c, r, z, e1, e2 } => {
                c + (around(e1, e2, u).0 * v.cos() + z * v.sin()) * r
            }
            Self::Torus {
                c,
                a,
                e1,
                e2,
                big,
                small,
            } => c + around(e1, e2, u).0 * (big + small * v.cos()) + a * (small * v.sin()),
            Self::Nurbs {
                surface,
                swap,
                u: du,
                v: dv,
                closed_u,
                closed_v,
                ..
            } => {
                let u = if closed_u { wrap(u, du) } else { u };
                let v = if closed_v { wrap(v, dv) } else { v };
                if swap {
                    surface.point_at(v, u)
                } else {
                    surface.point_at(u, v)
                }
            }
        }
    }

    /// `(∂S/∂u, ∂S/∂v)` at `(u, v)`.
    pub(crate) fn partials(&self, u: f64, v: f64) -> (Vector3, Vector3) {
        match *self {
            Self::Plane { e1, e2, .. } => (e1, e2),
            Self::Cone {
                a, e1, e2, r0, k, ..
            } => {
                let (dir, turn) = around(e1, e2, u);
                (turn * (r0 + k * v), a + dir * k)
            }
            Self::Elliptic { m, n, a, .. } => (n * u.cos() - m * u.sin(), a),
            Self::Sphere { r, z, e1, e2, .. } => {
                let (dir, turn) = around(e1, e2, u);
                let (s, c) = v.sin_cos();
                (turn * (r * c), (z * c - dir * s) * r)
            }
            Self::Torus {
                a,
                e1,
                e2,
                big,
                small,
                ..
            } => {
                let (dir, turn) = around(e1, e2, u);
                let (s, c) = v.sin_cos();
                (turn * (big + small * c), (a * c - dir * s) * small)
            }
            Self::Nurbs {
                surface,
                swap,
                u: du,
                v: dv,
                closed_u,
                closed_v,
                ..
            } => {
                let u = if closed_u { wrap(u, du) } else { u };
                let v = if closed_v { wrap(v, dv) } else { v };
                if swap {
                    let (sv, su) = surface.derivatives(v, u);
                    (su, sv)
                } else {
                    surface.derivatives(u, v)
                }
            }
        }
    }

    /// The parameters of `p`, on or near the surface; an angle in
    /// `(-π, π]`, a closed NURBS parameter within its domain.
    pub(crate) fn uv(&self, p: Point3) -> (f64, f64) {
        match *self {
            Self::Plane { o, e1, e2 } => {
                let d = p - o;
                (d.dot(e1), d.dot(e2))
            }
            Self::Cone { o, a, e1, e2, .. } => {
                let d = p - o;
                (d.dot(e2).atan2(d.dot(e1)), d.dot(a))
            }
            Self::Elliptic { c, m, n, a } => {
                let d = p - c;
                let x = d.dot(m) / m.length_squared();
                let y = d.dot(n) / n.length_squared();
                (y.atan2(x), d.dot(a))
            }
            Self::Sphere { c, r, z, e1, e2 } => {
                let d = (p - c) / r;
                (d.dot(e2).atan2(d.dot(e1)), d.dot(z).clamp(-1.0, 1.0).asin())
            }
            Self::Torus {
                c, a, e1, e2, big, ..
            } => {
                let d = p - c;
                let h = d.dot(a);
                let (x, y) = (d.dot(e1), d.dot(e2));
                (y.atan2(x), h.atan2(x.hypot(y) - big))
            }
            Self::Nurbs { surface, swap, .. } => {
                let (u, v) = surface.closest_parameters(p);
                if swap { (v, u) } else { (u, v) }
            }
        }
    }

    /// The periods of `u` and `v`, for the directions that close.
    pub(crate) fn periods(&self) -> (Option<f64>, Option<f64>) {
        match *self {
            Self::Plane { .. } => (None, None),
            Self::Cone { .. } | Self::Elliptic { .. } | Self::Sphere { .. } => (Some(TAU), None),
            Self::Torus { .. } => (Some(TAU), Some(TAU)),
            Self::Nurbs {
                u,
                v,
                closed_u,
                closed_v,
                ..
            } => (closed_u.then_some(u.1 - u.0), closed_v.then_some(v.1 - v.0)),
        }
    }

    /// Where a period of `u` starts.
    pub(crate) fn u_start(&self) -> f64 {
        match *self {
            Self::Nurbs { u, .. } => u.0,
            _ => 0.0,
        }
    }

    /// Where a period of `v` starts.
    pub(crate) fn v_start(&self) -> f64 {
        match *self {
            Self::Nurbs { v, .. } => v.0,
            _ => 0.0,
        }
    }

    /// The values of `v` along which the surface collapses to a point.
    pub(crate) fn poles(&self) -> Vec<f64> {
        match *self {
            Self::Sphere { .. } => vec![-FRAC_PI_2, FRAC_PI_2],
            Self::Cone { r0, k, .. } if k != 0.0 => vec![-r0 / k],
            Self::Nurbs { ref poles, .. } => poles.clone(),
            _ => Vec::new(),
        }
    }

    /// The chart, owning everything it needs; `None` for a NURBS surface's,
    /// which borrows it.
    pub(crate) fn to_owned_analytic(&self) -> Option<Chart<'static>> {
        Some(match *self {
            Self::Plane { o, e1, e2 } => Chart::Plane { o, e1, e2 },
            Self::Cone {
                o,
                a,
                e1,
                e2,
                r0,
                k,
            } => Chart::Cone {
                o,
                a,
                e1,
                e2,
                r0,
                k,
            },
            Self::Elliptic { c, m, n, a } => Chart::Elliptic { c, m, n, a },
            Self::Sphere { c, r, z, e1, e2 } => Chart::Sphere { c, r, z, e1, e2 },
            Self::Torus {
                c,
                a,
                e1,
                e2,
                big,
                small,
            } => Chart::Torus {
                c,
                a,
                e1,
                e2,
                big,
                small,
            },
            Self::Nurbs { .. } => return None,
        })
    }

    /// Whether `u` and `v` are angles (their integrals behave as angles do:
    /// exact on even samples of a period).
    pub(crate) fn is_analytic(&self) -> bool {
        !matches!(self, Self::Nurbs { .. })
    }

    /// The knots where a NURBS surface's pieces join, along `u` and `v`
    /// (after a swap); empty for the others.
    pub(crate) fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        match *self {
            Self::Nurbs { surface, swap, .. } => {
                let (ku, kv) = surface.breaks();
                if swap { (kv, ku) } else { (ku, kv) }
            }
            _ => (Vec::new(), Vec::new()),
        }
    }
}

/// `x` brought into the period `[a, b)`.
fn wrap(x: f64, (a, b): (f64, f64)) -> f64 {
    a + (x - a).rem_euclid(b - a)
}

/// A NURBS surface's chart: which directions close, which rows collapse,
/// and whether `u` and `v` are swapped so that a closed direction, and the
/// poles across it, come as `u` and `v`.
fn nurbs_chart(surface: &NurbsSurface) -> Chart<'_> {
    let ((u0, u1), (v0, v1)) = surface.domain();
    let size = surface.size().max(f64::MIN_POSITIVE);
    let near = |a: Point3, b: Point3| a.distance_to(b) <= 1e-9 * size;
    let samples = |a: f64, b: f64| (0..=8).map(move |i| a + (b - a) * i as f64 / 8.0);
    let closed_u = samples(v0, v1).all(|v| near(surface.point_at(u0, v), surface.point_at(u1, v)));
    let closed_v = samples(u0, u1).all(|u| near(surface.point_at(u, v0), surface.point_at(u, v1)));
    // A row of constant v that is one point is a pole across u.
    let collapses_v = |v: f64| {
        let first = surface.point_at(u0, v);
        samples(u0, u1).all(|u| near(surface.point_at(u, v), first))
    };
    let collapses_u = |u: f64| {
        let first = surface.point_at(u, v0);
        samples(v0, v1).all(|v| near(surface.point_at(u, v), first))
    };
    let poles_v: Vec<f64> = [v0, v1].into_iter().filter(|&v| collapses_v(v)).collect();
    let poles_u: Vec<f64> = [u0, u1].into_iter().filter(|&u| collapses_u(u)).collect();
    // The closed direction, or the one the poles lie across, becomes u.
    let swap = (closed_v && !closed_u) || (poles_v.is_empty() && !poles_u.is_empty());
    if swap {
        Chart::Nurbs {
            surface,
            swap,
            u: (v0, v1),
            v: (u0, u1),
            closed_u: closed_v,
            closed_v: closed_u,
            poles: poles_u,
        }
    } else {
        Chart::Nurbs {
            surface,
            swap,
            u: (u0, u1),
            v: (v0, v1),
            closed_u,
            closed_v,
            poles: poles_v,
        }
    }
}

#[cfg(test)]
#[path = "chart_tests.rs"]
mod tests;
