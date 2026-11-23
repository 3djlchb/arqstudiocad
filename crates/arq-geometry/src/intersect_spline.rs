//! Where a spline meets another curve (ADR 0043 D2): numerically, with no
//! closed form, the candidates found along the spline and refined on the
//! curves themselves.
//!
//! - **A line, circle, arc or ellipse:** the other curve's implicit
//!   function (signed, zero on it) taken along the spline, sampled
//!   [`STEPS`] times a span. A change of sign holds a crossing, refined by
//!   Newton's method kept within its bracket. A dip toward zero without one
//!   is a near tangency: its lowest point is found, and is a touching point
//!   when within the tolerance of the other curve, judged in the world, or
//!   holds two crossings when it changes sign.
//! - **Two splines:** their chords that cross give the start for Newton's
//!   method in both parameters; chords that pass close are refined toward
//!   their nearest points, a touching point when within the tolerance.

use crate::ellipse::Ellipse;
use crate::geometry::Geometry2;
use crate::intersect::Intersections;
use crate::point::Point2;
use crate::spline::Spline;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// Samples a span: enough that two crossings rarely share a step, and a
/// tangency rarely hides between two.
const STEPS: usize = 32;

/// A curve's carrier as an implicit function: signed, zero on it, its
/// gradient along it.
#[derive(Clone, Copy)]
pub(crate) enum Carrier {
    /// The line through `at` along the unit `along`.
    Line { at: Point2, normal: Vector2 },
    /// The circle about `center` of `radius`.
    Circle { center: Point2, radius: f64 },
    /// The whole ellipse.
    Ellipse(Ellipse),
}

impl Carrier {
    /// The carrier of a line, circle, arc or ellipse: its line, its circle,
    /// its whole ellipse. None for a spline.
    pub(crate) fn of(g: &Geometry2) -> Option<Self> {
        Some(match *g {
            Geometry2::Line(l) => Self::Line {
                at: l.start(),
                normal: l.direction().perp(),
            },
            Geometry2::Circle(c) => Self::Circle {
                center: c.center(),
                radius: c.radius(),
            },
            Geometry2::Arc(a) => Self::Circle {
                center: a.center(),
                radius: a.radius(),
            },
            Geometry2::Ellipse(e) => Self::Ellipse(e.whole()),
            Geometry2::Spline(_) => return None,
        })
    }

    /// Its value at `p` and gradient there.
    fn value(self, p: Point2) -> (f64, Vector2) {
        match self {
            Self::Line { at, normal } => (normal.dot(p - at), normal),
            Self::Circle { center, radius } => {
                let d = p - center;
                let r = d.length();
                let g = if r > 0.0 {
                    d * (1.0 / r)
                } else {
                    Vector2::ZERO
                };
                (r - radius, g)
            }
            Self::Ellipse(e) => {
                // In its own frame, x along the major axis: √((x/a)² + (y/b)²)
                // − 1, scaled by the minor semi-axis to read about as a
                // distance near the curve.
                let major = e.major_axis();
                let a = major.length();
                let b = a * e.ratio();
                let ux = major * (1.0 / a);
                let uy = ux.perp();
                let d = p - e.center();
                let (x, y) = (d.dot(ux) / a, d.dot(uy) / b);
                let q = x.hypot(y);
                let g = if q > 0.0 {
                    (ux * (x / (q * a)) + uy * (y / (q * b))) * b
                } else {
                    Vector2::ZERO
                };
                ((q - 1.0) * b, g)
            }
        }
    }
}

/// The parameters along `s` to sample: [`STEPS`] a span.
fn sample_params(s: &Spline) -> Vec<f64> {
    let breaks = s.span_breaks();
    let mut out = vec![breaks[0]];
    for w in breaks.windows(2) {
        for k in 1..=STEPS {
            out.push(w[0] + (w[1] - w[0]) * k as f64 / STEPS as f64);
        }
    }
    out
}

/// The zero of `h` in `[lo, hi]`, where it changes sign: Newton's method
/// on `h` and its slope `dh`, kept within the bracket by bisection.
fn root(h: &dyn Fn(f64) -> (f64, f64), mut lo: f64, mut hi: f64) -> f64 {
    let (flo, _) = h(lo);
    let mut u = (lo + hi) / 2.0;
    for _ in 0..100 {
        let (f, df) = h(u);
        if f == 0.0 {
            return u;
        }
        if (f < 0.0) == (flo < 0.0) {
            lo = u;
        } else {
            hi = u;
        }
        let next = if df != 0.0 { u - f / df } else { f64::NAN };
        let next = if next > lo && next < hi {
            next
        } else {
            (lo + hi) / 2.0
        };
        if (next - u).abs() <= 4.0 * f64::EPSILON * u.abs().max(1.0) {
            return next;
        }
        u = next;
    }
    u
}

/// Where `|h|` is lowest in `[lo, hi]`: golden-section search.
fn lowest(h: &dyn Fn(f64) -> (f64, f64), mut lo: f64, mut hi: f64) -> f64 {
    let golden = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..90 {
        let (a, b) = (hi - golden * (hi - lo), lo + golden * (hi - lo));
        if h(a).0.abs() < h(b).0.abs() {
            hi = b;
        } else {
            lo = a;
        }
    }
    (lo + hi) / 2.0
}

/// The parameters of `s` where `h`, a function along it with its slope,
/// is zero: at each change of sign between samples, and at each dip toward
/// zero that `near` accepts (a touching point) or that changes sign (two).
pub(crate) fn zeros_along(
    s: &Spline,
    h: &dyn Fn(f64) -> (f64, f64),
    near: &dyn Fn(f64) -> bool,
) -> Vec<f64> {
    let params = sample_params(s);
    let values: Vec<f64> = params.iter().map(|&u| h(u).0).collect();
    let mut out = Vec::new();
    for i in 0..params.len() {
        let f = values[i];
        if f == 0.0 {
            out.push(params[i]);
            continue;
        }
        if i + 1 < params.len() {
            let g = values[i + 1];
            if g != 0.0 && (f < 0.0) != (g < 0.0) {
                out.push(root(h, params[i], params[i + 1]));
            }
        }
        // A dip: lower than both neighbors, of one sign with them.
        if i > 0 && i + 1 < params.len() {
            let (before, after) = (values[i - 1], values[i + 1]);
            let one_sign = (before < 0.0) == (f < 0.0) && (after < 0.0) == (f < 0.0);
            if one_sign && f.abs() < before.abs() && f.abs() <= after.abs() {
                let (lo, hi) = (params[i - 1], params[i + 1]);
                let m = lowest(h, lo, hi);
                let fm = h(m).0;
                if fm == 0.0 || (fm < 0.0) != (f < 0.0) {
                    out.push(root(h, lo, m));
                    out.push(root(h, m, hi));
                } else if near(m) {
                    out.push(m);
                }
            }
        }
    }
    // A zero on a sample is found from both its steps; a closed spline's
    // end is its start.
    let (a, b) = s.domain();
    let same = 1e-9 * (b - a);
    if s.is_closed() {
        for u in &mut out {
            if b - *u <= same {
                *u = a;
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|x, y| (*x - *y).abs() <= same);
    out
}

/// Where `s` meets `carrier`, the line, circle or whole ellipse of another
/// curve: points on the spline, not yet restricted to the other curve.
pub(crate) fn spline_carrier(s: &Spline, carrier: Carrier, tol: Tolerance) -> Vec<Point2> {
    let h = |u: f64| {
        let (p, d) = s.point_and_derivative(u);
        let (f, g) = carrier.value(p);
        (f, g.dot(d))
    };
    let near = |u: f64| h(u).0.abs() <= tol.linear();
    let mut points: Vec<Point2> = zeros_along(s, &h, &near)
        .into_iter()
        .map(|u| s.point_at(u))
        .collect();
    // An end on it counts, though it neither crosses nor dips there.
    if !s.is_closed() {
        let (a, b) = s.ends();
        for end in [a, b] {
            if carrier.value(end).0.abs() <= tol.linear() {
                points.push(end);
            }
        }
    }
    points
}

/// Where spline `s` meets `other`, a line, circle, arc or ellipse.
pub(crate) fn spline_curve(s: &Spline, other: &Geometry2, tol: Tolerance, out: &mut Intersections) {
    let carrier = Carrier::of(other).expect("not a spline");
    for p in spline_carrier(s, carrier, tol) {
        if other.distance_to(p) <= tol.linear() {
            out.push_point(p, tol);
        }
    }
}

/// The points of `s` where `h(u, p, d, d2)`, a function of its point,
/// first and second derivatives at `u`, is zero, by its changes of sign.
fn where_zero(s: &Spline, h: &dyn Fn(Point2, Vector2, Vector2) -> (f64, f64)) -> Vec<Point2> {
    let (a, b) = s.domain();
    let e = 1e-7 * (b - a);
    let at = |u: f64| {
        let (p, d) = s.point_and_derivative(u);
        // The second derivative by central differences of the exact first.
        let (lo, hi) = ((u - e).max(a), (u + e).min(b));
        let d2 = (s.derivative_at(hi) - s.derivative_at(lo)) * (1.0 / (hi - lo));
        h(p, d, d2)
    };
    zeros_along(s, &at, &|_| false)
        .into_iter()
        .map(|u| s.point_at(u))
        .collect()
}

/// The feet of the perpendiculars from `from` to `s`: where
/// `(P(u) − from) · P′(u)` is zero (ADR 0043 D3).
pub(crate) fn spline_perpendiculars(s: &Spline, from: Point2) -> Vec<Point2> {
    where_zero(s, &|p, d, d2| {
        let off = p - from;
        (off.dot(d), d.dot(d) + off.dot(d2))
    })
}

/// The points of `s` a line from `from` touches: where
/// `(P(u) − from) × P′(u)` is zero (ADR 0043 D3). From a point on it,
/// that point itself, once, as a circle's (its zero there is double, found
/// only to about √ε along the curve).
pub(crate) fn spline_tangents(s: &Spline, from: Point2, tol: Tolerance) -> Vec<Point2> {
    let b = s.bounds();
    let near = 1e-6 * b.width().hypot(b.height());
    let on = s.distance_to(from) <= tol.linear();
    let mut out: Vec<Point2> = Vec::new();
    let found = where_zero(s, &|p, d, d2| {
        let off = p - from;
        (off.cross(d), off.cross(d2))
    });
    for q in found {
        let q = if on && q.distance_to(from) <= near {
            s.closest_point(from)
        } else {
            q
        };
        if !out.iter().any(|k| k.distance_to(q) <= tol.linear()) {
            out.push(q);
        }
    }
    out
}

/// A chord of a spline: its ends' parameters and points.
#[derive(Clone, Copy)]
struct Chord {
    u: (f64, f64),
    p: (Point2, Point2),
}

fn chords(s: &Spline) -> Vec<Chord> {
    let params = sample_params(s);
    let points: Vec<Point2> = params.iter().map(|&u| s.point_at(u)).collect();
    (0..params.len() - 1)
        .map(|i| Chord {
            u: (params[i], params[i + 1]),
            p: (points[i], points[i + 1]),
        })
        .collect()
}

/// Whether two segments cross, their ends included.
fn segments_cross(a: (Point2, Point2), b: (Point2, Point2)) -> bool {
    let (r, s) = (a.1 - a.0, b.1 - b.0);
    let denom = r.cross(s);
    if denom == 0.0 {
        return false;
    }
    let q = b.0 - a.0;
    let (t, w) = (q.cross(s) / denom, q.cross(r) / denom);
    let slack = 1e-9;
    (-slack..=1.0 + slack).contains(&t) && (-slack..=1.0 + slack).contains(&w)
}

/// How near two segments come.
fn segments_gap(a: (Point2, Point2), b: (Point2, Point2)) -> f64 {
    let to_segment = |p: Point2, (s0, s1): (Point2, Point2)| {
        let d = s1 - s0;
        let t = if d.length_squared() > 0.0 {
            ((p - s0).dot(d) / d.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        p.distance_to(s0 + d * t)
    };
    to_segment(a.0, b)
        .min(to_segment(a.1, b))
        .min(to_segment(b.0, a))
        .min(to_segment(b.1, a))
}

/// Newton's method in both parameters from `(u, v)`, toward where `a(u)`
/// and `b(v)` coincide (`nearest` false), or come nearest (`nearest`
/// true, Gauss–Newton on the distance), kept within the domains.
fn refine_pair(a: &Spline, b: &Spline, mut u: f64, mut v: f64, nearest: bool) -> (f64, f64) {
    let (da, db) = (a.domain(), b.domain());
    for _ in 0..40 {
        let (pa, ta) = a.point_and_derivative(u);
        let (pb, tb) = b.point_and_derivative(v);
        let f = pa - pb;
        let (du, dv) = if nearest {
            // Minimize |a(u) − b(v)|²: its gradient is zero.
            let (g1, g2) = (f.dot(ta), -f.dot(tb));
            let (h11, h12, h22) = (ta.dot(ta), -ta.dot(tb), tb.dot(tb));
            let det = h11 * h22 - h12 * h12;
            if det.abs() <= f64::EPSILON * h11 * h22 {
                break;
            }
            ((-g1 * h22 + g2 * h12) / det, (g1 * h12 - g2 * h11) / det)
        } else {
            // a(u) − b(v) = 0: [ta, −tb] (du, dv) = −f.
            let det = ta.cross(-tb);
            if det == 0.0 {
                break;
            }
            ((-f).cross(-tb) / det, ta.cross(-f) / det)
        };
        let (nu, nv) = ((u + du).clamp(da.0, da.1), (v + dv).clamp(db.0, db.1));
        let moved = (nu - u).abs() + (nv - v).abs();
        (u, v) = (nu, nv);
        if moved <= 4.0 * f64::EPSILON * (u.abs() + v.abs()).max(1.0) {
            break;
        }
    }
    (u, v)
}

/// Where two splines meet. One spline with itself shares itself.
pub(crate) fn spline_spline(a: &Spline, b: &Spline, tol: Tolerance, out: &mut Intersections) {
    if a == b {
        out.push_overlap(Geometry2::from(a.clone()));
        return;
    }
    let (ca, cb) = (chords(a), chords(b));
    let boxes = |c: &[Chord]| -> Vec<(f64, f64, f64, f64)> {
        c.iter()
            .map(|k| {
                let (p, q) = k.p;
                (
                    p.x().min(q.x()),
                    p.y().min(q.y()),
                    p.x().max(q.x()),
                    p.y().max(q.y()),
                )
            })
            .collect()
    };
    let (ba, bb) = (boxes(&ca), boxes(&cb));
    // The longest chord of each, for how near two pass.
    let longest = |c: &[Chord]| {
        c.iter()
            .map(|k| k.p.0.distance_to(k.p.1))
            .fold(0.0, f64::max)
    };
    let slack = 0.05 * (longest(&ca) + longest(&cb)) + tol.linear();
    // Spans' boxes first, STEPS chords each: only chords of spans whose
    // boxes meet are compared.
    let union = |b: &[(f64, f64, f64, f64)]| {
        b.iter().fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |u, k| (u.0.min(k.0), u.1.min(k.1), u.2.max(k.2), u.3.max(k.3)),
        )
    };
    let apart = |p: (f64, f64, f64, f64), q: (f64, f64, f64, f64), reach: f64| {
        p.0 > q.2 + reach || q.0 > p.2 + reach || p.1 > q.3 + reach || q.1 > p.3 + reach
    };
    if apart(union(&ba), union(&bb), slack) {
        return;
    }
    let spans_a: Vec<_> = ba.chunks(STEPS).map(union).collect();
    let spans_b: Vec<_> = bb.chunks(STEPS).map(union).collect();
    let mut met: Vec<(Point2, f64)> = Vec::new();
    let pairs = (0..ca.len()).flat_map(|i| {
        let spans_b = &spans_b;
        let span_a = spans_a[i / STEPS];
        (0..cb.len())
            .filter(move |&j| !apart(span_a, spans_b[j / STEPS], slack))
            .map(move |j| (i, j))
    });
    for (i, j) in pairs {
        let (x, y) = (&ca[i], &cb[j]);
        let lx = x.p.0.distance_to(x.p.1);
        {
            let ly = y.p.0.distance_to(y.p.1);
            // Chords pass close when within a twentieth of their lengths:
            // a span's chord strays from its curve by far less.
            let reach = 0.05 * (lx + ly) + tol.linear();
            let (p, q) = (ba[i], bb[j]);
            if p.0 > q.2 + reach || q.0 > p.2 + reach || p.1 > q.3 + reach || q.1 > p.3 + reach {
                continue;
            }
            // From the chords' middles: Newton's method converges from
            // there, a crossing within half a chord.
            let mid = |c: &Chord| (c.u.0 + c.u.1) / 2.0;
            let found = if segments_cross(x.p, y.p) {
                Some(refine_pair(a, b, mid(x), mid(y), false))
            } else if segments_gap(x.p, y.p) <= reach {
                Some(refine_pair(a, b, mid(x), mid(y), true))
            } else {
                None
            };
            if let Some((u, v)) = found {
                let (pa, pb) = (a.point_at(u), b.point_at(v));
                let gap = pa.distance_to(pb);
                if gap <= tol.linear() {
                    met.push((pa.lerp(pb, 0.5), gap));
                }
            }
        }
    }
    // Near a tangency the nearest points are known only to about √ε of
    // the curves' size along them, and neighbouring chords find several:
    // those within a millionth of the size are one, the closest kept.
    let size = |s: &Spline| {
        let b = s.bounds();
        b.width().hypot(b.height())
    };
    let merge = (1e-6 * (size(a) + size(b))).max(tol.linear());
    met.sort_by(|x, y| x.1.total_cmp(&y.1));
    let mut kept: Vec<Point2> = Vec::new();
    for (q, _) in met {
        if !kept.iter().any(|k| k.distance_to(q) <= merge) {
            kept.push(q);
        }
    }
    for q in kept {
        out.push_point(q, tol);
    }
}

#[cfg(test)]
#[path = "intersect_spline_tests.rs"]
mod tests;
