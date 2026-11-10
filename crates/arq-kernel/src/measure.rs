//! Measuring a body (ADR 0024 D4, step 3): its volume, surface area,
//! centroid and bounds, as AutoCAD's MASSPROP reports them.
//!
//! # How
//!
//! The divergence theorem turns the volume and the centroid into integrals
//! over the faces: `V = ⅓∮ x·n dA`, `∫x_i dV = ½∮ x_i² n_i dA`. Each face's
//! integral is one over its parameters `(u, v)` (see [`Chart`]), and
//! Green's theorem turns that into an integral along the face's loops,
//! taken on the edges' exact curves: no face is triangulated, and the
//! result does not depend on how finely an edge is drawn (maintainer,
//! 2026-10-31).
//!
//! For an integrand `G(u, v)` the loops carry a 1-form `ω` with
//! `dω = G du∧dv`:
//!
//! - **Open in both directions** (a plane, most NURBS faces):
//!   `ω = F dv`, `F = ∫ G du` from a fixed `u`.
//! - **Closed in `u`** (cones, cylinders, spheres): `G` is split into its
//!   mean over a turn, `Ḡ(v)`, and the rest, whose integral around a turn
//!   is zero; `ω = F̃ dv − H du`, `F̃ = ∫(G − Ḡ) du`, `H = ∫Ḡ dv`. Both are
//!   periodic in `u`, so `ω` lives on the surface itself: no seam enters
//!   the sums. Where the surface collapses (a sphere's pole, a cone's apex)
//!   `ω` is not defined; a pole inside the face adds the line around it.
//!   Which poles a face holds follows from how often its loops wind round
//!   the axis, and, when that is zero, from the sign of its area. A loop
//!   passing through a pole (a sphere cut along its meridians, a cone's
//!   apex at a corner) runs along the pole's line from the meridian it
//!   arrives by to the one it leaves by: that stretch adds its part of the
//!   line around the pole, and of the turn round the axis.
//! - **Closed both ways** (tori): the same with `G`'s overall mean `c`
//!   taken out first; `c` times the face's parameter area is added, that
//!   area taken modulo the whole torus's. A spindle torus (one crossing
//!   its axis) has an area element with corners at its poles; the face's
//!   is taken signed instead, which is smooth and right within it.
//!
//! The 1-dimensional integrals use Gauss–Legendre rules, doubled until
//! the result settles; on a flat face bounded by lines and arcs the first
//! rules are already exact to the last digit, so they are taken once. On
//! a cone or cylinder the integrand is a trigonometric polynomial of degree
//! 3 in `u` with polynomial coefficients in `v`, found once per face from
//! a few samples and integrated in closed form (see `ConeTable`); on a
//! sphere or torus one of degree 3 in `u` and 4 in `v`, the same way (see
//! `AngleTable`).
//!
//! The bounds are those of the edges (exact for lines and arcs) and of the
//! points of spheres and tori that stand out within their faces.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_8, PI, TAU};

use arq_geometry::{Bounds3, Point3, Vector3};

use crate::brep::{Body, Coedge, Face};
use crate::chart::Chart;
use crate::curve::Curve;
use crate::nurbs::inner_knots;
use crate::surface::Surface;
use crate::tessellate::{
    Isoline, boundary_of, edge_polyline, ellipse_parameter, guides_of, inside_at, outward, side_of,
};

/// What [`Body::mass_properties`] measures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProperties {
    /// The volume enclosed.
    pub volume: f64,
    /// The area of its faces.
    pub area: f64,
    /// The center of its volume.
    pub centroid: Point3,
    /// The smallest box around it, along the axes.
    pub bounds: Bounds3,
}

/// Why a body could not be measured.
#[derive(Clone, Debug, PartialEq)]
pub enum MeasureError {
    /// The body has no face.
    Empty,
    /// Its faces do not close around a volume: their outward areas add up
    /// to `gap` times its surface area instead of nothing.
    NotClosed {
        /// How far from closed, relative to the area.
        gap: f64,
    },
    /// A face this version cannot integrate yet.
    Unsupported {
        /// The face's index.
        face: usize,
        /// Why.
        why: &'static str,
    },
    /// A face's integrals did not settle.
    NotConverged {
        /// The face's index.
        face: usize,
    },
}

impl std::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "the body has no face"),
            Self::NotClosed { gap } => {
                write!(f, "the faces do not close around a volume (gap {gap:e})")
            }
            Self::Unsupported { face, why } => write!(f, "face {face} cannot be measured: {why}"),
            Self::NotConverged { face } => write!(f, "face {face}'s integrals did not settle"),
        }
    }
}

impl std::error::Error for MeasureError {}

/// The integrands, at once: area, volume, the three first moments of the
/// volume (about the origin of the sums), and the three components of the
/// outward vector area (nothing, for a closed body: a check).
type Sums = [f64; 8];
const AREA: usize = 0;
const VOLUME: usize = 1;
const MOMENT: usize = 2;
const NORMAL: usize = 5;

fn add(total: &mut Sums, part: Sums, weight: f64) {
    for (t, p) in total.iter_mut().zip(part) {
        *t += p * weight;
    }
}

fn scaled(sums: Sums, factor: f64) -> Sums {
    sums.map(|s| s * factor)
}

fn minus(a: Sums, b: Sums) -> Sums {
    let mut out = a;
    add(&mut out, b, -1.0);
    out
}

/// Gauss–Legendre nodes and weights on [-1, 1], 8 points: exact for
/// polynomials up to degree 15.
const NODES: [(f64, f64); 8] = [
    (-0.960_289_856_497_536_3, 0.101_228_536_290_376_3),
    (-0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (-0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (-0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (0.960_289_856_497_536_3, 0.101_228_536_290_376_3),
];

/// `∫_a^b f`, over `pieces` equal parts of `[a, b]`, and the knots `breaks`
/// in between.
fn integrate(
    a: f64,
    b: f64,
    pieces: usize,
    breaks: &[f64],
    mut f: impl FnMut(f64) -> Sums,
) -> Sums {
    let mut total = [0.0; 8];
    if a == b {
        return total;
    }
    let (lo, hi) = (a.min(b), a.max(b));
    let mut cuts = vec![lo];
    cuts.extend(breaks.iter().copied().filter(|&k| lo < k && k < hi));
    cuts.push(hi);
    for w in cuts.windows(2) {
        let n = pieces.max(1);
        let step = (w[1] - w[0]) / n as f64;
        for i in 0..n {
            let (x0, half) = (w[0] + step * i as f64, step / 2.0);
            for (x, wt) in NODES {
                add(&mut total, f(x0 + half * (1.0 + x)), wt * half);
            }
        }
    }
    if b < a { scaled(total, -1.0) } else { total }
}

/// The larger magnitude of `a` and `b`.
fn mid_abs(a: f64, b: f64) -> f64 {
    a.abs().max(b.abs())
}

/// How many parts an integral over `[a, b]` of an angle takes, `step` each
/// at most.
fn parts(a: f64, b: f64, step: f64) -> usize {
    ((b - a).abs() / step).ceil().max(1.0) as usize
}

#[cfg(test)]
thread_local! {
    /// How many times this thread has sampled an integrand `G`: what a
    /// measure costs, for the tests.
    pub(super) static SAMPLES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A face being integrated.
struct FaceIntegral<'a> {
    chart: Chart<'a>,
    /// Where the moments are taken about.
    origin: Point3,
    /// +1 when `∂S/∂u × ∂S/∂v` points out of the face, −1 when in.
    sigma: f64,
    periods: (Option<f64>, Option<f64>),
    starts: (f64, f64),
    breaks: (Vec<f64>, Vec<f64>),
    /// Whether `v` is an angle (spheres, tori).
    v_angle: bool,
    /// Whether the surface is an elliptic cylinder.
    elliptic: bool,
    /// How finely the inner integrals are taken.
    fine: usize,
    /// The overall mean of the integrands (tori), taken out of `H`.
    overall: Sums,
    /// `Ḡ` at Chebyshev points of the face's span of `v`, where it is a
    /// polynomial of degree 3 at most (cones, cylinders): interpolated
    /// from them, exactly, instead of integrated again.
    v_poly: Option<Vec<(f64, Sums)>>,
    /// `G` on a cone or cylinder in closed form (see [`ConeTable`]).
    cone: Option<ConeTable>,
    /// `G` on a sphere or torus in closed form (see [`AngleTable`]).
    angles: Option<AngleTable>,
    /// Where `H` starts.
    v_base: f64,
    /// A spindle torus's (one crossing its axis): the sign its distance
    /// from the axis has in the face. Its area element, that distance
    /// times the minor radius, then taken signed, not by its length, which
    /// has a corner at the poles no rule integrates exactly.
    tube_side: Option<f64>,
}

impl FaceIntegral<'_> {
    /// The integrands at `(u, v)`, per unit of parameter area.
    fn g(&self, u: f64, v: f64) -> Sums {
        #[cfg(test)]
        SAMPLES.with(|n| n.set(n.get() + 1));
        let p = self.chart.point(u, v);
        let (su, sv) = self.chart.partials(u, v);
        let n = su.cross(sv);
        let out = n * self.sigma;
        let x = p - self.origin;
        let (xx, xy, xz) = (x.x(), x.y(), x.z());
        let area = match (self.tube_side, &self.chart) {
            (Some(side), &Chart::Torus { big, small, .. }) => {
                side * (big + small * v.cos()) * small
            }
            // r² cos v, as the length is within a face (v a latitude), and
            // smooth beyond, where `AngleTable` samples it.
            (_, &Chart::Sphere { r, .. }) => r * r * v.cos(),
            _ => n.length(),
        };
        [
            area,
            x.dot(out) / 3.0,
            0.5 * xx * xx * out.x(),
            0.5 * xy * xy * out.y(),
            0.5 * xz * xz * out.z(),
            out.x(),
            out.y(),
            out.z(),
        ]
    }

    /// `∫_a^b G(s, v) ds`.
    fn along_u(&self, a: f64, b: f64, v: f64) -> Sums {
        integrate(a, b, self.pieces_along_u(a, b), &self.breaks.0, |s| {
            self.g(s, v)
        })
    }

    /// How many pieces the rule for [`Self::along_u`] takes. Where `u` is an
    /// angle (cones, cylinders, spheres, tori), one per eighth of a turn; a
    /// plane's `u` is a length, along which `G` is a polynomial of degree 3
    /// at most, that one piece integrates exactly at any size (in
    /// millimetres one per π/4 of a length made thousands).
    fn pieces_along_u(&self, a: f64, b: f64) -> usize {
        match self.chart {
            Chart::Plane { .. } | Chart::Nurbs { .. } => self.fine,
            _ => parts(a, b, FRAC_PI_4) * self.fine,
        }
    }

    /// `Ḡ(v)`: the mean of `G` over a turn of `u`.
    fn mean_u(&self, v: f64) -> Sums {
        let period = self.periods.0.expect("closed in u");
        let u0 = self.starts.0;
        if self.chart.is_analytic() {
            // Even samples of a period integrate a trigonometric polynomial
            // of lower degree exactly; these integrands are of degree 3,
            // but for an elliptic section's area element, which they
            // integrate ever closer as the samples double.
            let even = |n: usize| {
                let mut total = [0.0; 8];
                for i in 0..n {
                    add(
                        &mut total,
                        self.g(u0 + period * i as f64 / n as f64, v),
                        1.0 / n as f64,
                    );
                }
                total
            };
            let mut mean = even(16);
            if self.elliptic {
                for n in [32, 64, 128, 256, 512, 1024] {
                    let next = even(n);
                    let settled =
                        (0..8).all(|i| (next[i] - mean[i]).abs() <= 1e-14 * next[i].abs());
                    mean = next;
                    if settled {
                        break;
                    }
                }
            }
            mean
        } else {
            scaled(self.along_u(u0, u0 + period, v), 1.0 / period)
        }
    }

    /// `Ḡ(v)`, from the interpolating polynomial where there is one.
    fn mean_at(&self, v: f64) -> Sums {
        let Some(points) = &self.v_poly else {
            return self.mean_u(v);
        };
        let mut total = [0.0; 8];
        for (k, &(vk, values)) in points.iter().enumerate() {
            let weight: f64 = points
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != k)
                .map(|(_, &(vj, _))| (v - vj) / (vk - vj))
                .product();
            add(&mut total, values, weight);
        }
        total
    }

    /// Tabulates `Ḡ` over `[lo, hi]` for [`Self::mean_at`].
    fn tabulate(&mut self, lo: f64, hi: f64) {
        let (mid, half) = (
            (lo + hi) / 2.0,
            ((hi - lo) / 2.0).max(1e-9 * (1.0 + mid_abs(lo, hi))),
        );
        let n = 6;
        let points = (0..n)
            .map(|k| {
                let v = mid + half * (PI * (2 * k + 1) as f64 / (2 * n) as f64).cos();
                (v, self.mean_u(v))
            })
            .collect();
        self.v_poly = Some(points);
    }

    /// `F̃(u, v) = ∫ (G − Ḡ) du` from the start of the period, periodic in
    /// `u`.
    fn f_tilde(&self, u: f64, v: f64) -> Sums {
        if let Some(cone) = &self.cone {
            return cone.f_tilde(self.starts.0, u, v);
        }
        if let Some(angles) = &self.angles {
            return angles.f_tilde(self.starts.0, u, v);
        }
        let period = self.periods.0.expect("closed in u");
        let u0 = self.starts.0;
        let u = u0 + (u - u0).rem_euclid(period);
        minus(self.along_u(u0, u, v), scaled(self.mean_at(v), u - u0))
    }

    /// `H(v) = ∫ (Ḡ − c) dv` from `v_base`.
    fn h(&self, v: f64) -> Sums {
        if let Some(cone) = &self.cone {
            return cone.h(self.v_base, v);
        }
        let mut v = v;
        if let Some(period) = self.periods.1 {
            // Periodic once the overall mean is out.
            v = self.v_base + (v - self.v_base).rem_euclid(period);
        }
        if let Some(angles) = &self.angles {
            return minus(
                angles.h(self.v_base, v),
                scaled(self.overall, v - self.v_base),
            );
        }
        let pieces = if self.v_angle {
            parts(self.v_base, v, FRAC_PI_8)
        } else {
            2
        } * self.fine;
        integrate(self.v_base, v, pieces, &self.breaks.1, |t| {
            minus(self.mean_at(t), self.overall)
        })
    }
}

/// `G` on a cone or cylinder, exactly: a trigonometric polynomial of
/// degree 3 in `u` whose coefficients are polynomials of degree 3 in `v`
/// (the position is of degree 1 in both, the normal too, the moments' `x²n`
/// of degree 3; the area element `√(1 + k²) |r₀ + k v|` keeps its sign
/// within a face, which never crosses its apex). Eight even samples in `u`
/// give its Fourier coefficients and four Chebyshev points in `v` their
/// polynomials, both exactly; `F̃` and `H` are then integrals in closed
/// form, instead of a Gauss rule of up to 64 samples of `G` at every node
/// of the loops.
struct ConeTable {
    /// The middle and half width of the face's span of `v`: the
    /// polynomials are in `t = (v − mid) / half`, for their conditioning.
    mid: f64,
    half: f64,
    /// `coef[j][m]`: the coefficient of `t^j` times the `m`-th of `1, cos u,
    /// sin u, cos 2u, sin 2u, cos 3u, sin 3u`.
    coef: [[Sums; 7]; 4],
}

impl ConeTable {
    /// The table of `integral`'s `G` over `v` in `[lo, hi]`.
    fn new(integral: &FaceIntegral, lo: f64, hi: f64) -> Self {
        let (mid, half) = (
            (lo + hi) / 2.0,
            ((hi - lo) / 2.0).max(1e-9 * (1.0 + mid_abs(lo, hi))),
        );
        // Chebyshev coefficients in t of the Fourier coefficients in u.
        let mut cheb = [[[0.0; 8]; 7]; 4];
        for q in 0..4 {
            let t = (PI * (2 * q + 1) as f64 / 8.0).cos();
            let mut fourier = [[0.0; 8]; 7];
            for i in 0..8 {
                let u = TAU * i as f64 / 8.0;
                let g = integral.g(u, mid + half * t);
                add(&mut fourier[0], g, 1.0 / 8.0);
                for m in 1..=3 {
                    let (sin, cos) = (m as f64 * u).sin_cos();
                    add(&mut fourier[2 * m - 1], g, cos / 4.0);
                    add(&mut fourier[2 * m], g, sin / 4.0);
                }
            }
            let chebyshev = [1.0, t, 2.0 * t * t - 1.0, 4.0 * t * t * t - 3.0 * t];
            for (j, &tj) in chebyshev.iter().enumerate() {
                let weight = if j == 0 { 0.25 } else { 0.5 };
                for m in 0..7 {
                    add(&mut cheb[j][m], fourier[m], weight * tj);
                }
            }
        }
        // T₀ = 1, T₁ = t, T₂ = 2t² − 1, T₃ = 4t³ − 3t, into powers of t.
        let mut coef = [[[0.0; 8]; 7]; 4];
        for m in 0..7 {
            coef[0][m] = minus(cheb[0][m], cheb[2][m]);
            coef[1][m] = minus(cheb[1][m], scaled(cheb[3][m], 3.0));
            coef[2][m] = scaled(cheb[2][m], 2.0);
            coef[3][m] = scaled(cheb[3][m], 4.0);
        }
        Self { mid, half, coef }
    }

    /// The powers `1, t, t², t³` at `v`.
    fn powers(&self, v: f64) -> [f64; 4] {
        let t = (v - self.mid) / self.half;
        [1.0, t, t * t, t * t * t]
    }

    /// `F̃(u, v) = ∫ (G − Ḡ) du` from `u0`: the terms without a mean,
    /// integrated.
    fn f_tilde(&self, u0: f64, u: f64, v: f64) -> Sums {
        let mut basis = [0.0; 7];
        for m in 1..=3 {
            let mf = m as f64;
            let ((s1, c1), (s0, c0)) = ((mf * u).sin_cos(), (mf * u0).sin_cos());
            basis[2 * m - 1] = (s1 - s0) / mf;
            basis[2 * m] = (c0 - c1) / mf;
        }
        let mut total = [0.0; 8];
        for (j, tj) in self.powers(v).into_iter().enumerate() {
            for (&c, &b) in self.coef[j].iter().zip(&basis).skip(1) {
                add(&mut total, c, tj * b);
            }
        }
        total
    }

    /// `H(v) = ∫ Ḡ dv` from `base`.
    fn h(&self, base: f64, v: f64) -> Sums {
        let (from, to) = (self.powers(base), self.powers(v));
        let mut total = [0.0; 8];
        for j in 0..4 {
            // ∫ t^j dv = half · t^(j+1) / (j + 1).
            let rise = (to[j] * to[1] - from[j] * from[1]) / (j + 1) as f64;
            add(&mut total, self.coef[j][0], self.half * rise);
        }
        total
    }
}

/// `G` on a sphere or torus, exactly: a trigonometric polynomial of degree
/// 3 in `u` and 4 in `v` (the position and the unit normal are of degree 1
/// in both, the area element `r² cos v` or `r (R + r cos v)` of degree 1 in
/// `v`, the moments' `x²n` of degree 3 and 4). Eight even samples in `u`
/// and ten in `v` give its Fourier coefficients exactly; `Ḡ`, `F̃` and `H`
/// are then sums in closed form, instead of Gauss rules sampling `G` (`H`
/// sampled `Ḡ`, itself 16 samples, up to 64 times at every loop node).
struct AngleTable {
    /// `coef[m][l]`: the coefficient of the `m`-th of `1, cos u, sin u, …,
    /// sin 3u` times the `l`-th of `1, cos v, sin v, …, sin 4v`.
    coef: [[Sums; 9]; 7],
}

impl AngleTable {
    /// The table of `integral`'s `G`.
    fn new(integral: &FaceIntegral) -> Self {
        let mut coef = [[[0.0; 8]; 9]; 7];
        for j in 0..10 {
            let v = TAU * j as f64 / 10.0;
            let wv = fourier_weights::<9>(v, 10);
            for i in 0..8 {
                let u = TAU * i as f64 / 8.0;
                let wu = fourier_weights::<7>(u, 8);
                let g = integral.g(u, v);
                for (m, row) in coef.iter_mut().enumerate() {
                    for (l, c) in row.iter_mut().enumerate() {
                        add(c, g, wu[m] * wv[l]);
                    }
                }
            }
        }
        Self { coef }
    }

    /// `F̃(u, v) = ∫ (G − Ḡ) du` from `u0`.
    fn f_tilde(&self, u0: f64, u: f64, v: f64) -> Sums {
        let (across, at) = (trig_integrals::<7>(u0, u), trig_values::<9>(v));
        let mut total = [0.0; 8];
        for (row, &a) in self.coef.iter().zip(&across).skip(1) {
            for (&c, &b) in row.iter().zip(&at) {
                add(&mut total, c, a * b);
            }
        }
        total
    }

    /// `∫ Ḡ dv` from `base`.
    fn h(&self, base: f64, v: f64) -> Sums {
        let along = trig_integrals::<9>(base, v);
        let mut total = [0.0; 8];
        for (&c, &a) in self.coef[0].iter().zip(&along) {
            add(&mut total, c, a);
        }
        total
    }
}

/// `1, cos x, sin x, cos 2x, sin 2x, …` up to `N` terms.
fn trig_values<const N: usize>(x: f64) -> [f64; N] {
    let mut out = [0.0; N];
    out[0] = 1.0;
    for k in 1..=(N - 1) / 2 {
        let (sin, cos) = (k as f64 * x).sin_cos();
        out[2 * k - 1] = cos;
        out[2 * k] = sin;
    }
    out
}

/// The integrals of [`trig_values`] from `a` to `b`.
fn trig_integrals<const N: usize>(a: f64, b: f64) -> [f64; N] {
    let mut out = [0.0; N];
    out[0] = b - a;
    for k in 1..=(N - 1) / 2 {
        let kf = k as f64;
        let ((sb, cb), (sa, ca)) = ((kf * b).sin_cos(), (kf * a).sin_cos());
        out[2 * k - 1] = (sb - sa) / kf;
        out[2 * k] = (ca - cb) / kf;
    }
    out
}

/// The weights that turn `n` even samples of a trigonometric polynomial,
/// of degree below `n / 2`, into its coefficients on [`trig_values`]: the
/// sample at `x`'s share.
fn fourier_weights<const N: usize>(x: f64, n: usize) -> [f64; N] {
    let mut out = trig_values::<N>(x);
    out[0] = 1.0 / n as f64;
    for w in &mut out[1..] {
        *w *= 2.0 / n as f64;
    }
    out
}

/// A point of a loop where the integrals are sampled.
#[derive(Clone, Copy, Debug)]
struct Node {
    p: Point3,
    /// `dp/ds` times the rule's weight: the line element.
    dp: Vector3,
}

/// The nodes along a coedge, in the loop's direction.
fn coedge_nodes(body: &Body, coedge: &Coedge, fine: usize, out: &mut Vec<Node>) {
    let edge = body.edge(coedge.edge);
    let (start, end) = (body.vertex(edge.start).point, body.vertex(edge.end).point);
    // The curve from the edge's start (s = 0) to its end (s = 1) with its
    // derivative, and where its pieces join, in s.
    let mut cuts = vec![0.0, 1.0];
    let curve: Box<dyn Fn(f64) -> (Point3, Vector3)> = match &edge.curve {
        None => return, // a degenerate edge (an apex) has no length
        Some(Curve::Line(_)) => Box::new(move |s| (start.lerp(end, s), end - start)),
        Some(Curve::Ellipse(e)) => {
            let (ts, te) = (ellipse_parameter(e, start), ellipse_parameter(e, end));
            let sweep = if edge.start == edge.end {
                TAU
            } else if edge.reversed {
                (ts - te).rem_euclid(TAU)
            } else {
                (te - ts).rem_euclid(TAU)
            };
            let turn = if edge.reversed { -sweep } else { sweep };
            let (c, m) = (e.center(), e.major());
            let n = e.normal().cross(m) * e.ratio();
            Box::new(move |s| {
                let (sin, cos) = (ts + turn * s).sin_cos();
                (c + m * cos + n * sin, (n * cos - m * sin) * turn)
            })
        }
        Some(Curve::Nurbs(curve)) => {
            let (a, b) = curve.domain();
            let (ts, te) = if edge.start == edge.end {
                if edge.reversed { (b, a) } else { (a, b) }
            } else {
                (curve.closest_parameter(start), curve.closest_parameter(end))
            };
            // A spline is smooth only between its knots: the rule is cut
            // there, so a knot's kink never falls inside a piece.
            cuts.extend(
                inner_knots(curve.knots(), ts.min(te), ts.max(te))
                    .into_iter()
                    .map(|k| (k - ts) / (te - ts)),
            );
            let curve = curve.clone();
            Box::new(move |s| {
                let t = ts + (te - ts) * s;
                (curve.point_at(t), curve.derivative_at(t) * (te - ts))
            })
        }
    };
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    // An ellipse's sixteen pieces a turn; a spline's two between knots, at
    // least as many as an ellipse's in all (a conic's arc is one span).
    let per_cut = match &edge.curve {
        Some(Curve::Ellipse(_)) => 16,
        Some(Curve::Nurbs(_)) => 2usize.max(16_usize.div_ceil(cuts.len() - 1)),
        _ => 2,
    } * fine;
    let mut nodes = Vec::with_capacity((cuts.len() - 1) * per_cut * NODES.len());
    for w2 in cuts.windows(2) {
        let step = (w2[1] - w2[0]) / per_cut as f64;
        for i in 0..per_cut {
            for (x, w) in NODES {
                let s = w2[0] + step * (i as f64 + (1.0 + x) / 2.0);
                let (p, d) = curve(s);
                nodes.push(Node {
                    p,
                    dp: d * (w * step / 2.0),
                });
            }
        }
    }
    if coedge.reversed {
        nodes.reverse();
        for node in &mut nodes {
            node.dp = -node.dp;
        }
    }
    out.extend(nodes);
}

/// A loop's nodes with their parameters, lifted to vary continuously.
struct Lifted {
    nodes: Vec<(Node, f64, f64)>,
    /// How many periods the loop turns through in `u` and in `v`.
    winding: (i64, i64),
    /// Where it passes through a pole: the node it leaves by, and the
    /// pole's `v`.
    passages: Vec<(usize, f64)>,
}

fn lift(chart: &Chart, nodes: Vec<Node>) -> Lifted {
    let (pu, pv) = chart.periods();
    let near = |x: f64, previous: f64, period: Option<f64>| match period {
        Some(p) => x + p * ((previous - x) / p).round(),
        None => x,
    };
    let mut lifted: Vec<(Node, f64, f64)> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let (mut u, mut v) = chart.uv(node.p);
        if let Some(&(_, pu0, pv0)) = lifted.last() {
            u = near(u, pu0, pu);
            v = near(v, pv0, pv);
        }
        lifted.push((node, u, v));
    }
    let mut winding = (0, 0);
    if let (Some(&(first, u0, v0)), Some(&(_, u1, v1))) = (lifted.first(), lifted.last()) {
        // The first node again, after going round.
        let (u, v) = chart.uv(first.p);
        let turns = |x: f64, last: f64, start: f64, period: Option<f64>| {
            period.map_or(0, |p| ((near(x, last, period) - start) / p).round() as i64)
        };
        winding = (turns(u, u1, u0, pu), turns(v, v1, v0, pv));
    }
    Lifted {
        nodes: lifted,
        winding,
        passages: Vec::new(),
    }
}

/// The `v` of the pole `p` lies on, if it lies on one.
fn pole_at(chart: &Chart, p: Point3) -> Option<f64> {
    let scale = 1.0 + p.x().abs().max(p.y().abs()).max(p.z().abs());
    chart
        .poles()
        .into_iter()
        .find(|&v| chart.point(0.0, v).distance_to(p) <= 1e-9 * scale)
}

/// `(du/ds, dv/ds)` along `dp` at `(u, v)`.
fn rates(chart: &Chart, u: f64, v: f64, dp: Vector3) -> (f64, f64) {
    let (su, sv) = chart.partials(u, v);
    let (a, b, c) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let (e, f) = (su.dot(dp), sv.dot(dp));
    let det = a * c - b * b;
    if a <= 1e-24 * c.max(f64::MIN_POSITIVE) {
        // At a pole u is not defined and does not move.
        return (0.0, if c > 0.0 { f / c } else { 0.0 });
    }
    if det.abs() <= 1e-24 * (a * c).max(f64::MIN_POSITIVE) {
        return (e / a, 0.0);
    }
    ((e * c - b * f) / det, (a * f - b * e) / det)
}

/// Whether a face's loops are read the way round they enclose the face
/// (`false`) or as holes in the rest of its closed surface (`true`): its
/// result on a closed surface is its complement's.
struct FaceResult {
    sums: Sums,
    complement: bool,
    /// The points where the face's surface collapses inside it (a cone's
    /// apex): they bound it.
    poles: Vec<Point3>,
}

/// The face's integrals with rules `fine` times the base ones.
fn integrate_face(
    body: &Body,
    face: &Face,
    index: usize,
    origin: Point3,
    fine: usize,
) -> Result<FaceResult, MeasureError> {
    let unsupported = |why| MeasureError::Unsupported { face: index, why };
    let chart = Chart::new(&face.surface);
    let loops: Vec<Lifted> = face
        .loops
        .iter()
        .map(|lp| {
            let mut nodes = Vec::new();
            let mut passages: Vec<(usize, f64)> = Vec::new();
            for &c in &body.loops()[lp.index()].coedges {
                let coedge = &body.coedges()[c.index()];
                let edge = body.edge(coedge.edge);
                let from = if coedge.reversed {
                    edge.end
                } else {
                    edge.start
                };
                if let Some(v) = pole_at(&chart, body.vertex(from).point)
                    && passages.last().is_none_or(|&(at, _)| at != nodes.len())
                {
                    passages.push((nodes.len(), v));
                }
                coedge_nodes(body, coedge, fine, &mut nodes);
            }
            let count = nodes.len();
            let mut lifted = lift(&chart, nodes);
            lifted.passages = passages
                .into_iter()
                .map(|(at, v)| (if count == 0 { 0 } else { at % count }, v))
                .collect();
            lifted
        })
        .filter(|l| !l.nodes.is_empty())
        .collect();
    // The orientation, where the parametrization is regular.
    let probe = loops
        .iter()
        .flat_map(|l| l.nodes.iter())
        .map(|&(_, u, v)| (u, v))
        .max_by(|a, b| {
            let size = |&(u, v): &(f64, f64)| {
                let (su, sv) = chart.partials(u, v);
                su.cross(sv).length()
            };
            size(a).total_cmp(&size(b))
        })
        .unwrap_or((0.3, 0.3));
    let (su, sv) = chart.partials(probe.0, probe.1);
    let n_out = outward(face, chart.point(probe.0, probe.1)).ok_or(unsupported("no normal"))?;
    let sigma = if su.cross(sv).dot(n_out) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let periods = chart.periods();
    let v_mean = {
        let vs: Vec<f64> = loops
            .iter()
            .flat_map(|l| l.nodes.iter().map(|n| n.2))
            .collect();
        if vs.is_empty() {
            0.0
        } else {
            vs.iter().sum::<f64>() / vs.len() as f64
        }
    };
    let poles = chart.poles();
    let low = poles
        .iter()
        .copied()
        .filter(|&p| p < v_mean)
        .reduce(f64::max);
    let high = poles
        .iter()
        .copied()
        .filter(|&p| p > v_mean)
        .reduce(f64::min);
    let v_angle = matches!(face.surface, Surface::Sphere(_) | Surface::Torus(_));
    let analytic = chart.is_analytic();
    let elliptic = matches!(face.surface, Surface::EllipticCylinder(_));
    let mut integral = FaceIntegral {
        breaks: chart.breaks(),
        starts: (chart.u_start(), chart.v_start()),
        chart,
        origin,
        sigma,
        periods,
        v_angle,
        elliptic,
        // An analytic surface's inner integrals are exact, or settle on their
        // own; only a spline's are refined with the loops.
        fine: if analytic { 1 } else { fine },
        overall: [0.0; 8],
        v_poly: None,
        cone: None,
        angles: None,
        v_base: low.or(high).unwrap_or(v_mean),
        tube_side: None,
    };
    // Its side, at one of its points (a mean of v would not do: v turns).
    let first = loops.iter().find_map(|l| l.nodes.first());
    if let (Chart::Torus { big, small, .. }, Some(&(_, _, v))) = (&integral.chart, first)
        && big.abs() < *small
    {
        integral.tube_side = Some(if big + small * v.cos() < 0.0 {
            -1.0
        } else {
            1.0
        });
    }
    if matches!(integral.chart, Chart::Sphere { .. } | Chart::Torus { .. }) {
        integral.angles = Some(AngleTable::new(&integral));
    }
    if analytic && !v_angle && periods.0.is_some() {
        // A cone's or cylinder's: polynomial in v over the face's span.
        let vs = loops.iter().flat_map(|l| l.nodes.iter().map(|n| n.2));
        let (lo, hi) = vs
            .chain([integral.v_base])
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            });
        if matches!(integral.chart, Chart::Cone { .. }) {
            integral.cone = Some(ConeTable::new(&integral, lo, hi));
        } else {
            integral.tabulate(lo, hi);
        }
    }
    let mut sums = [0.0; 8];
    let mut complement = false;
    match periods {
        (None, None) => {
            // ω = F dv, F from the middle of the loops' u.
            let us: Vec<f64> = loops
                .iter()
                .flat_map(|l| l.nodes.iter().map(|n| n.1))
                .collect();
            if us.is_empty() {
                return Err(unsupported("an unbounded face"));
            }
            let u_ref = us.iter().sum::<f64>() / us.len() as f64;
            for l in &loops {
                for &(node, u, v) in &l.nodes {
                    let (_, dv) = rates(&integral.chart, u, v, node.dp);
                    add(&mut sums, integral.along_u(u_ref, u, v), dv);
                }
            }
        }
        (Some(pu), None) => {
            for l in &loops {
                for &(node, u, v) in &l.nodes {
                    let (du, dv) = rates(&integral.chart, u, v, node.dp);
                    add(&mut sums, integral.f_tilde(u, v), dv);
                    add(&mut sums, integral.h(v), -du);
                }
            }
            sums = scaled(sums, sigma);
            // Through a pole, along its line: in the face's own sense, from
            // the meridian it arrives by (a) to the one it leaves by (b),
            // the face's opening there; above, back in u, below, on.
            let mut winding = 0;
            for l in &loops {
                let n = l.nodes.len();
                if l.passages.is_empty() {
                    winding += sigma as i64 * l.winding.0;
                    continue;
                }
                // The turn round the axis, step by step: the nodes' own, the
                // way back to the first, and each stretch along a pole.
                let step = |i: usize| {
                    let d = l.nodes[i].1 - l.nodes[(i + n - 1) % n].1;
                    d - pu * (d / pu).round()
                };
                let mut turned: f64 = (0..n).map(step).sum();
                for &(at, v) in &l.passages {
                    let (before, after) = (l.nodes[(at + n - 1) % n].1, l.nodes[at].1);
                    let (a, b) = if sigma > 0.0 {
                        (before, after)
                    } else {
                        (after, before)
                    };
                    let above = high.is_some_and(|h| (v - h).abs() < 1e-9);
                    let opening = if above {
                        (a - b).rem_euclid(pu)
                    } else {
                        (b - a).rem_euclid(pu)
                    };
                    // Below, the pole is where H starts: the stretch adds
                    // nothing there.
                    if above {
                        add(&mut sums, integral.h(v), opening);
                    }
                    // In the loop's own sense this stretch turns by the
                    // opening, not by the step between its meridians.
                    let along = if above { -opening } else { opening };
                    turned += sigma * along - step(at);
                }
                winding += sigma as i64 * (turned / pu).round() as i64;
            }
            // The poles the face holds: from how its loops wind around.
            let (high_in, low_in) = match winding {
                _ if loops.is_empty() => (true, true),
                1 => (true, false),
                -1 => (false, true),
                0 if sums[AREA] < 0.0 => {
                    complement = true;
                    (true, true)
                }
                0 => (false, false),
                _ => return Err(unsupported("loops winding more than once around")),
            };
            let mut points = Vec::new();
            for (inside, pole, sign) in [(high_in, high, 1.0), (low_in, low, -1.0)] {
                if inside {
                    let v = pole.ok_or(unsupported("a face open around its axis"))?;
                    add(&mut sums, integral.h(v), sign * pu);
                    points.push(integral.chart.point(integral.starts.0, v));
                }
            }
            return Ok(FaceResult {
                sums,
                complement,
                poles: points,
            });
        }
        (Some(pu), Some(pv)) => {
            // The overall mean, out of H, comes back times the parameter area.
            if let Some(angles) = &integral.angles {
                integral.overall = angles.coef[0][0];
            } else {
                let n = 16 * fine;
                for j in 0..n {
                    let v = integral.starts.1 + pv * j as f64 / n as f64;
                    let mean = integral.mean_u(v);
                    add(&mut integral.overall, mean, 1.0 / n as f64);
                }
            }
            integral.v_base = integral.starts.1;
            let all_u = loops.iter().all(|l| l.winding.1 == 0);
            let all_v = loops.iter().all(|l| l.winding.0 == 0);
            if !all_u && !all_v {
                return Err(unsupported("loops winding both ways around a torus"));
            }
            let mut uv_area = 0.0;
            for l in &loops {
                // Each loop brought into one period of the other parameter.
                let (_, u_first, v_first) = l.nodes[0];
                let shift_u = pu * ((u_first - integral.starts.0) / pu).floor();
                let shift_v = pv * ((v_first - integral.starts.1) / pv).floor();
                for &(node, u, v) in &l.nodes {
                    let (du, dv) = rates(&integral.chart, u, v, node.dp);
                    add(&mut sums, integral.f_tilde(u, v), dv);
                    add(&mut sums, integral.h(v), -du);
                    uv_area += if all_u {
                        -(v - shift_v) * du
                    } else {
                        (u - shift_u) * dv
                    };
                }
            }
            sums = scaled(sums, sigma);
            let whole = pu * pv;
            let raw = sigma * uv_area;
            let area = if loops.is_empty() {
                whole
            } else {
                let area = raw.rem_euclid(whole);
                if area <= 1e-12 * whole || area >= whole * (1.0 - 1e-12) {
                    return Err(unsupported("a face that is all or none of its torus"));
                }
                area
            };
            complement = raw < 0.0;
            add(&mut sums, integral.overall, area);
            return Ok(FaceResult {
                sums,
                complement,
                poles: Vec::new(),
            });
        }
        (None, Some(_)) => return Err(unsupported("a surface closed in v only")),
    }
    Ok(FaceResult {
        sums: scaled(sums, sigma),
        complement,
        poles: Vec::new(),
    })
}

/// The face's integrals, the rules refined until they settle: each
/// quantity changes by less than `1e-11` of the body's size to its power.
fn face_sums(
    body: &Body,
    face: &Face,
    index: usize,
    origin: Point3,
    size: f64,
) -> Result<FaceResult, MeasureError> {
    let scale = [
        size * size,
        size.powi(3),
        size.powi(4),
        size.powi(4),
        size.powi(4),
        size * size,
        size * size,
        size * size,
    ];
    let mut previous = integrate_face(body, face, index, origin, 1)?;
    if settles_at_once(body, face) {
        return Ok(previous);
    }
    for fine in [2, 4, 8, 16] {
        let next = integrate_face(body, face, index, origin, fine)?;
        let settled = (0..8).all(|i| (next.sums[i] - previous.sums[i]).abs() <= 1e-11 * scale[i]);
        if settled {
            return Ok(next);
        }
        previous = next;
    }
    Err(MeasureError::NotConverged { face: index })
}

/// Whether a face's first rules already give its integrals: a flat face
/// bounded by lines and arcs of circles or ellipses. On a plane `F` is a
/// polynomial of degree 3 in the plane's coordinates. Along a line the loop
/// integrand is then a polynomial the 8-point rule integrates exactly. Along
/// an arc it is a trigonometric polynomial of degree 4 in the arc's angle,
/// taken in pieces of π/8 at most; there the rule's remainder,
/// `h¹⁷ (8!)⁴ / (17 (16!)³)` times a 16th derivative of at most `4¹⁶` times
/// the integrand's size, is below `1e-20` of it, far under the `1e-11` the
/// refinement settles to.
fn settles_at_once(body: &Body, face: &Face) -> bool {
    matches!(face.surface, Surface::Plane(_))
        && face.loops.iter().all(|lp| {
            body.loops()[lp.index()].coedges.iter().all(|c| {
                matches!(
                    body.edge(body.coedges()[c.index()].edge).curve,
                    Some(Curve::Line(_) | Curve::Ellipse(_))
                )
            })
        })
}

impl Body {
    /// The body's volume, surface area, centroid and bounds, if its faces
    /// close around a volume.
    pub fn mass_properties(&self) -> Result<MassProperties, MeasureError> {
        // The faces of its shells, each once.
        let mut faces: Vec<usize> = self
            .lumps()
            .iter()
            .flat_map(|l| &l.shells)
            .flat_map(|s| &self.shells()[s.index()].faces)
            .map(|f| f.index())
            .collect();
        faces.sort_unstable();
        faces.dedup();
        let reach = self.rough_bounds(&faces).ok_or(MeasureError::Empty)?;
        let origin = reach.center();
        let size = reach.size().length().max(f64::MIN_POSITIVE);
        let mut total = [0.0; 8];
        let mut complements = Vec::with_capacity(faces.len());
        let mut poles = Vec::new();
        for &i in &faces {
            let result = face_sums(self, &self.faces()[i], i, origin, size)?;
            add(&mut total, result.sums, 1.0);
            complements.push((i, result.complement));
            poles.extend(result.poles);
        }
        let area = total[AREA];
        let gap = Vector3::new(total[NORMAL], total[NORMAL + 1], total[NORMAL + 2])
            .map_or(f64::INFINITY, Vector3::length)
            / area.max(f64::MIN_POSITIVE);
        if gap.is_nan() || gap > 1e-6 {
            return Err(MeasureError::NotClosed { gap });
        }
        let volume = total[VOLUME];
        let shift = Vector3::new(
            total[MOMENT] / volume,
            total[MOMENT + 1] / volume,
            total[MOMENT + 2] / volume,
        )
        .map_err(|_| MeasureError::NotClosed { gap })?;
        Ok(MassProperties {
            volume,
            area,
            centroid: origin + shift,
            bounds: self.exact_bounds(&complements, poles),
        })
    }

    /// A box around the faces, for the scale of the sums: their edges
    /// drawn coarsely, and the whole spheres and tori among them.
    fn rough_bounds(&self, faces: &[usize]) -> Option<Bounds3> {
        let mut points: Vec<Point3> = Vec::new();
        for &i in faces {
            let face = &self.faces()[i];
            for lp in &face.loops {
                for &c in &self.loops()[lp.index()].coedges {
                    let edge = self.edge(self.coedges()[c.index()].edge);
                    points.extend(edge_polyline(self, edge, f64::INFINITY));
                }
            }
            if face.loops.is_empty() {
                let (c, r) = match face.surface {
                    Surface::Sphere(s) => (s.center(), s.radius()),
                    Surface::Torus(t) => (t.center(), t.major_radius().abs() + t.minor_radius()),
                    _ => continue,
                };
                if let Ok(b) = Bounds3::from_point(c).expanded(r) {
                    points.extend(b.corners());
                }
            }
        }
        Bounds3::from_points(points)
    }

    /// The bounds of the edges, exact for lines and arcs, with the points
    /// of spheres and tori that stand out inside their faces;
    /// `faces` pairs each face's index with whether it is the rest of its
    /// surface around its loops; `poles` are where faces collapse inside.
    fn exact_bounds(&self, faces: &[(usize, bool)], poles: Vec<Point3>) -> Bounds3 {
        let mut points: Vec<Point3> = self.vertices().iter().map(|v| v.point).collect();
        points.extend(poles);
        for edge in self.edges() {
            match &edge.curve {
                Some(Curve::Ellipse(e)) => {
                    let (start, end) = (self.vertex(edge.start).point, self.vertex(edge.end).point);
                    let (ts, te) = (ellipse_parameter(e, start), ellipse_parameter(e, end));
                    let closed = edge.start == edge.end;
                    let (m, n) = (e.major(), e.normal().cross(e.major()) * e.ratio());
                    for (mi, ni) in [(m.x(), n.x()), (m.y(), n.y()), (m.z(), n.z())] {
                        let t = ni.atan2(mi);
                        for t in [t, t + PI] {
                            // On the arc from the edge's start in its direction.
                            let along = if edge.reversed { ts - t } else { t - ts };
                            let span = if edge.reversed { ts - te } else { te - ts };
                            if closed || along.rem_euclid(TAU) <= span.rem_euclid(TAU) {
                                points.push(e.center() + m * t.cos() + n * t.sin());
                            }
                        }
                    }
                }
                Some(Curve::Nurbs(n)) => {
                    let (a, b) = n.domain();
                    let samples = 64 * (n.points().len() - n.degree());
                    points.extend(
                        (0..=samples).map(|i| n.point_at(a + (b - a) * i as f64 / samples as f64)),
                    );
                }
                _ => {}
            }
        }
        let size = Bounds3::from_points(points.iter().copied()).map_or(1.0, |b| b.size().length());
        let edges: Vec<Vec<Point3>> = self
            .edges()
            .iter()
            .map(|e| edge_polyline(self, e, 1e-4 * size.max(f64::MIN_POSITIVE)))
            .collect();
        for &(i, complement) in faces {
            let face = &self.faces()[i];
            let candidates = standing_out(face);
            if candidates.is_empty() {
                continue;
            }
            // The face's boundary and guides, once for all its candidates.
            let boundary = boundary_of(self, face, &edges);
            let guides = guides_of(face, &boundary);
            for (line, t) in candidates {
                let q = line(t);
                let inside = match &guides {
                    None => true, // the whole surface
                    Some((guides, reach)) => {
                        inside_at(&line, t, &|p| side_of(guides, p), *reach).unwrap_or(complement)
                    }
                };
                if inside {
                    points.push(q);
                }
            }
        }
        Bounds3::from_points(points).expect("a body has points")
    }
}

/// The points of a sphere or torus where it reaches furthest along each
/// axis, each with a line on the surface through it (and its parameter
/// there) to tell whether its face holds it; none for the other surfaces,
/// which reach furthest on their boundary.
fn standing_out(face: &Face) -> Vec<(Isoline, f64)> {
    let mut out: Vec<(Isoline, f64)> = Vec::new();
    let Some(chart) = Chart::new(&face.surface).to_owned_analytic() else {
        return out;
    };
    let axes = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)]
        .map(|(x, y, z)| Vector3::new(x, y, z).expect("finite"));
    match face.surface {
        Surface::Sphere(s) => {
            for d in axes {
                for sign in [1.0, -1.0] {
                    let (u, v) = chart.uv(s.center() + d * (sign * s.radius()));
                    // The meridian through it, from pole to pole.
                    let chart = chart.clone();
                    let meridian: Isoline = Box::new(move |t| chart.point(u, -FRAC_PI_2 + PI * t));
                    out.push((meridian, (v + FRAC_PI_2) / PI));
                }
            }
        }
        Surface::Torus(t) => {
            let a = t.axis();
            for d in axes {
                let along = a.dot(d);
                let flat = d - a * along;
                let us: Vec<f64> = if flat.length() <= 1e-12 {
                    // Along the axis: the top and bottom circles, sampled.
                    (0..16).map(|i| TAU * f64::from(i) / 16.0).collect()
                } else {
                    let (u, _) = chart.uv(t.center() + flat);
                    vec![u, u + PI]
                };
                for u in us {
                    // Where the tube's circle at u reaches furthest along d.
                    let (s, c) = u.sin_cos();
                    let (e1, e2) = (t.reference(), a.cross(t.reference()));
                    let v = along.atan2((e1 * c + e2 * s).dot(d));
                    for v in [v, v + PI] {
                        let chart = chart.clone();
                        let tube: Isoline = Box::new(move |w| chart.point(u, v - PI + TAU * w));
                        out.push((tube, 0.5));
                    }
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
