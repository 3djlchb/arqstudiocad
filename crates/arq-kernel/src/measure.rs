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
//!   the axis, and, when that is zero, from the sign of its area.
//! - **Closed both ways** (tori): the same with `G`'s overall mean `c`
//!   taken out first; `c` times the face's parameter area is added, that
//!   area taken modulo the whole torus's.
//!
//! The 1-dimensional integrals use Gauss–Legendre rules, doubled until
//! the result settles.
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
    /// Where `H` starts.
    v_base: f64,
}

impl FaceIntegral<'_> {
    /// The integrands at `(u, v)`, per unit of parameter area.
    fn g(&self, u: f64, v: f64) -> Sums {
        let p = self.chart.point(u, v);
        let (su, sv) = self.chart.partials(u, v);
        let n = su.cross(sv);
        let out = n * self.sigma;
        let x = p - self.origin;
        let (xx, xy, xz) = (x.x(), x.y(), x.z());
        [
            n.length(),
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
        let pieces = if self.chart.is_analytic() {
            parts(a, b, FRAC_PI_4)
        } else {
            1
        } * self.fine;
        integrate(a, b, pieces, &self.breaks.0, |s| self.g(s, v))
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
        let period = self.periods.0.expect("closed in u");
        let u0 = self.starts.0;
        let u = u0 + (u - u0).rem_euclid(period);
        minus(self.along_u(u0, u, v), scaled(self.mean_at(v), u - u0))
    }

    /// `H(v) = ∫ (Ḡ − c) dv` from `v_base`.
    fn h(&self, v: f64) -> Sums {
        let mut v = v;
        if let Some(period) = self.periods.1 {
            // Periodic once the overall mean is out.
            v = self.v_base + (v - self.v_base).rem_euclid(period);
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
    // The curve from the edge's start (s = 0) to its end (s = 1), its
    // derivative taken over `h` of s; and where its pieces join, in s.
    let mut cuts = vec![0.0, 1.0];
    let curve: Box<dyn Fn(f64, f64) -> (Point3, Vector3)> = match &edge.curve {
        None => return, // a degenerate edge (an apex) has no length
        Some(Curve::Line(_)) => Box::new(move |s, _| (start.lerp(end, s), end - start)),
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
            Box::new(move |s, _| {
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
            Box::new(move |s, h| {
                let t = ts + (te - ts) * s;
                let (lo, hi) = (ts + (te - ts) * (s - h), ts + (te - ts) * (s + h));
                let d = (curve.point_at(hi) - curve.point_at(lo)) / (hi - lo) * (te - ts);
                (curve.point_at(t), d)
            })
        }
    };
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let per_cut = match &edge.curve {
        Some(Curve::Ellipse(_)) => 16,
        _ => 2,
    } * fine;
    let mut nodes = Vec::with_capacity((cuts.len() - 1) * per_cut * NODES.len());
    for w2 in cuts.windows(2) {
        let step = (w2[1] - w2[0]) / per_cut as f64;
        for i in 0..per_cut {
            for (x, w) in NODES {
                let s = w2[0] + step * (i as f64 + (1.0 + x) / 2.0);
                // Within the piece: the nodes stay 2 % of it from its ends.
                let (p, d) = curve(s, 1e-4 * step);
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
    }
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
            for &c in &body.loops()[lp.index()].coedges {
                coedge_nodes(body, &body.coedges()[c.index()], fine, &mut nodes);
            }
            lift(&chart, nodes)
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
        v_base: low.or(high).unwrap_or(v_mean),
    };
    if analytic && !v_angle && periods.0.is_some() {
        // A cone's or cylinder's: polynomial in v over the face's span.
        let vs = loops.iter().flat_map(|l| l.nodes.iter().map(|n| n.2));
        let (lo, hi) = vs
            .chain([integral.v_base])
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            });
        integral.tabulate(lo, hi);
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
            // The poles the face holds: from how its loops wind around.
            let winding = sigma as i64 * loops.iter().map(|l| l.winding.0).sum::<i64>();
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
            let n = 16 * fine;
            for j in 0..n {
                let v = integral.starts.1 + pv * j as f64 / n as f64;
                let mean = integral.mean_u(v);
                add(&mut integral.overall, mean, 1.0 / n as f64);
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
            for (line, t) in standing_out(face) {
                let q = line(t);
                let boundary = boundary_of(self, face, &edges);
                let inside = match guides_of(face, &boundary) {
                    None => true, // the whole surface
                    Some((guides, reach)) => {
                        inside_at(&line, t, &|p| side_of(&guides, p), reach).unwrap_or(complement)
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
                    let (e1, e2) = crate::chart::frame(a);
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
