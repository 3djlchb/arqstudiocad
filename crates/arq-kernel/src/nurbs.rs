//! NURBS curves and surfaces (ADR 0024 D1, step 1b of D4): the B-spline
//! form of the free-form edges and faces a drawing holds, rational when
//! weighted. Evaluation follows de Boor's algorithm on homogeneous control
//! points; the closest point, used to check a body, is found by sampling
//! and then refined by Newton's method.
//!
//! The mathematics is the textbook one (Piegl and Tiller, *The NURBS
//! Book*); openNURBS was read as a reference (ADR 0020 amendment), nothing
//! of it copied.

use arq_geometry::{GeometryError, GeometryResult, Point3, Transform3, Vector3};

/// A NURBS curve: `degree`, a knot vector of `points.len() + degree + 1`
/// non-decreasing values, control points and their positive weights (all 1
/// for a non-rational curve). Defined over the knots `degree` to
/// `points.len()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    degree: usize,
    knots: Vec<f64>,
    points: Vec<Point3>,
    weights: Vec<f64>,
}

impl NurbsCurve {
    /// The curve, checked: degree at least 1, `degree + 1` points at least,
    /// knots and weights of the right counts, knots non-decreasing over a
    /// non-empty domain, weights positive and finite.
    pub fn new(
        degree: usize,
        knots: Vec<f64>,
        points: Vec<Point3>,
        weights: Vec<f64>,
    ) -> GeometryResult<Self> {
        check_direction(degree, &knots, points.len())?;
        check_weights(&weights, points.len())?;
        Ok(Self {
            degree,
            knots,
            points,
            weights,
        })
    }

    /// Its degree.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Its knot vector.
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Its control points.
    pub fn points(&self) -> &[Point3] {
        &self.points
    }

    /// Its weights.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// The parameters it is defined over.
    pub fn domain(&self) -> (f64, f64) {
        domain(self.degree, &self.knots, self.points.len())
    }

    /// The point at parameter `t`, clamped to the domain.
    pub fn point_at(&self, t: f64) -> Point3 {
        let h = de_boor(self.degree, &self.knots, self.points.len(), t, |i| {
            homogeneous(self.points[i], self.weights[i])
        });
        project(h)
    }

    /// The shortest distance from `p` to the curve, within its domain.
    pub fn distance(&self, p: Point3) -> f64 {
        let (a, b) = self.domain();
        let samples = 16 * (self.points.len() - self.degree);
        let step = (b - a) / samples as f64;
        let d2 = |t: f64| self.point_at(t).distance_squared_to(p);
        let mut best = a;
        for i in 0..=samples {
            let t = a + step * i as f64;
            if d2(t) < d2(best) {
                best = t;
            }
        }
        // Golden-section search around the best sample.
        let (mut lo, mut hi) = ((best - step).max(a), (best + step).min(b));
        let g = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..80 {
            let (x1, x2) = (hi - g * (hi - lo), lo + g * (hi - lo));
            if d2(x1) < d2(x2) {
                hi = x2;
            } else {
                lo = x1;
            }
        }
        d2((lo + hi) / 2.0).sqrt()
    }

    /// The curve moved by `t`: NURBS are invariant under affine maps of
    /// their control points.
    pub fn transformed(&self, t: Transform3) -> Self {
        Self {
            points: self.points.iter().map(|&p| t.apply_point(p)).collect(),
            ..self.clone()
        }
    }
}

/// A NURBS surface: degrees and knot vectors in `u` and `v`, a grid of
/// `count_u × count_v` control points (`u` the slower index: point
/// `i * count_v + j` is row `i`, column `j`) and their weights.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    degree_u: usize,
    degree_v: usize,
    knots_u: Vec<f64>,
    knots_v: Vec<f64>,
    count_u: usize,
    count_v: usize,
    points: Vec<Point3>,
    weights: Vec<f64>,
}

impl NurbsSurface {
    /// The surface, checked in each direction as a curve is.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        degree_u: usize,
        degree_v: usize,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        count_u: usize,
        count_v: usize,
        points: Vec<Point3>,
        weights: Vec<f64>,
    ) -> GeometryResult<Self> {
        check_direction(degree_u, &knots_u, count_u)?;
        check_direction(degree_v, &knots_v, count_v)?;
        if points.len() != count_u * count_v {
            return Err(GeometryError::OutOfRange {
                what: "control points",
            });
        }
        check_weights(&weights, points.len())?;
        Ok(Self {
            degree_u,
            degree_v,
            knots_u,
            knots_v,
            count_u,
            count_v,
            points,
            weights,
        })
    }

    /// Its degrees in `u` and `v`.
    pub fn degrees(&self) -> (usize, usize) {
        (self.degree_u, self.degree_v)
    }

    /// The parameters it is defined over: `((u0, u1), (v0, v1))`.
    pub fn domain(&self) -> ((f64, f64), (f64, f64)) {
        (
            domain(self.degree_u, &self.knots_u, self.count_u),
            domain(self.degree_v, &self.knots_v, self.count_v),
        )
    }

    /// The point at `(u, v)`, clamped to the domain.
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        // Each row's curve at v, then the column of those points at u.
        let rows: Vec<[f64; 4]> = (0..self.count_u)
            .map(|i| {
                de_boor(self.degree_v, &self.knots_v, self.count_v, v, |j| {
                    let k = i * self.count_v + j;
                    homogeneous(self.points[k], self.weights[k])
                })
            })
            .collect();
        project(de_boor(
            self.degree_u,
            &self.knots_u,
            self.count_u,
            u,
            |i| rows[i],
        ))
    }

    /// The shortest distance from `p` to the surface, within its domain:
    /// the closest of a grid of samples, refined by Newton steps on the
    /// squared distance with derivatives taken by finite differences, each
    /// step halved until it brings the point closer.
    pub fn distance(&self, p: Point3) -> f64 {
        let ((u0, u1), (v0, v1)) = self.domain();
        let nu = 8 * (self.count_u - self.degree_u);
        let nv = 8 * (self.count_v - self.degree_v);
        let mut best = (u0, v0, f64::INFINITY);
        for i in 0..=nu {
            for j in 0..=nv {
                let u = u0 + (u1 - u0) * i as f64 / nu as f64;
                let v = v0 + (v1 - v0) * j as f64 / nv as f64;
                let d = self.point_at(u, v).distance_squared_to(p);
                if d < best.2 {
                    best = (u, v, d);
                }
            }
        }
        let (mut u, mut v) = (best.0, best.1);
        let (hu, hv) = ((u1 - u0) * 1e-7, (v1 - v0) * 1e-7);
        for _ in 0..40 {
            let s = self.point_at(u, v);
            let su = (self.point_at((u + hu).min(u1), v) - self.point_at((u - hu).max(u0), v))
                / (((u + hu).min(u1) - (u - hu).max(u0)).max(f64::MIN_POSITIVE));
            let sv = (self.point_at(u, (v + hv).min(v1)) - self.point_at(u, (v - hv).max(v0)))
                / (((v + hv).min(v1) - (v - hv).max(v0)).max(f64::MIN_POSITIVE));
            let r: Vector3 = p - s;
            let (a, b, c) = (su.dot(su), su.dot(sv), sv.dot(sv));
            let (e, f) = (su.dot(r), sv.dot(r));
            let det = a * c - b * b;
            if det.abs() < f64::MIN_POSITIVE {
                break;
            }
            let du = (e * c - b * f) / det;
            let dv = (a * f - b * e) / det;
            // Far from a curved surface the Gauss-Newton step overshoots:
            // halve it until the point comes closer.
            let here = r.dot(r);
            let mut step = 1.0;
            let next = loop {
                let at = ((u + step * du).clamp(u0, u1), (v + step * dv).clamp(v0, v1));
                if self.point_at(at.0, at.1).distance_squared_to(p) < here || step < 1e-6 {
                    break at;
                }
                step /= 2.0;
            };
            let moved = (next.0 - u).abs() + (next.1 - v).abs();
            (u, v) = next;
            if moved < 1e-15 * (1.0 + u.abs() + v.abs()) {
                break;
            }
        }
        self.point_at(u, v).distance_to(p).min(best.2.sqrt())
    }

    /// The surface moved by `t`.
    pub fn transformed(&self, t: Transform3) -> Self {
        Self {
            points: self.points.iter().map(|&p| t.apply_point(p)).collect(),
            ..self.clone()
        }
    }
}

/// The knots and point count of one direction are a valid B-spline.
fn check_direction(degree: usize, knots: &[f64], count: usize) -> GeometryResult<()> {
    let bad = |what| Err(GeometryError::OutOfRange { what });
    if degree == 0 || count <= degree {
        return bad("degree");
    }
    if knots.len() != count + degree + 1 {
        return bad("knot count");
    }
    if knots.iter().any(|k| !k.is_finite()) {
        return Err(GeometryError::NonFinite { what: "knot" });
    }
    if knots.windows(2).any(|w| w[1] < w[0]) {
        return bad("knot order");
    }
    let (a, b) = domain(degree, knots, count);
    if a >= b {
        return bad("knot domain");
    }
    Ok(())
}

fn check_weights(weights: &[f64], count: usize) -> GeometryResult<()> {
    if weights.len() != count {
        return Err(GeometryError::OutOfRange { what: "weights" });
    }
    if weights.iter().any(|w| !(w.is_finite() && *w > 0.0)) {
        return Err(GeometryError::OutOfRange { what: "weight" });
    }
    Ok(())
}

fn domain(degree: usize, knots: &[f64], count: usize) -> (f64, f64) {
    (knots[degree], knots[count])
}

fn homogeneous(p: Point3, w: f64) -> [f64; 4] {
    [p.x() * w, p.y() * w, p.z() * w, w]
}

fn project(h: [f64; 4]) -> Point3 {
    Point3::new(h[0] / h[3], h[1] / h[3], h[2] / h[3])
        .expect("positive weights keep the point finite")
}

/// De Boor's algorithm: the homogeneous point at `t` (clamped to the
/// domain) of the B-spline with these knots and `count` control points,
/// `point(i)` giving the `i`-th.
fn de_boor(
    degree: usize,
    knots: &[f64],
    count: usize,
    t: f64,
    point: impl Fn(usize) -> [f64; 4],
) -> [f64; 4] {
    let (a, b) = domain(degree, knots, count);
    let t = t.clamp(a, b);
    // The span: knots[k] <= t < knots[k + 1], the last non-empty one at b.
    let mut k = degree;
    while k + 1 < count && knots[k + 1] <= t {
        k += 1;
    }
    let mut d: Vec<[f64; 4]> = (0..=degree).map(|j| point(j + k - degree)).collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = j + k - degree;
            let span = knots[i + degree + 1 - r] - knots[i];
            let alpha = if span > 0.0 {
                (t - knots[i]) / span
            } else {
                0.0
            };
            let previous = d[j - 1];
            for (value, before) in d[j].iter_mut().zip(previous) {
                *value = (1.0 - alpha) * before + alpha * *value;
            }
        }
    }
    d[degree]
}

#[cfg(test)]
#[path = "nurbs_tests.rs"]
mod tests;
