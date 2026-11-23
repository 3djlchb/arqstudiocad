//! A spline's pieces (ADR 0043 D4): the exact part between two of its
//! parameters, by knot insertion (Boehm's algorithm, The NURBS Book A5.1)
//! in homogeneous coordinates, so a rational spline's pieces are exact too.

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::spline::Spline;

/// A spline's control points in homogeneous coordinates, `(x w, y w, w)`,
/// and its knots, as knots are inserted.
struct Net {
    degree: usize,
    knots: Vec<f64>,
    points: Vec<[f64; 3]>,
}

impl Net {
    fn of(s: &Spline) -> Self {
        let points = s
            .points()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let w = s.weight_of(i);
                [c.x() * w, c.y() * w, w]
            })
            .collect();
        Self {
            degree: s.degree(),
            knots: s.knots().to_vec(),
            points,
        }
    }

    /// How many knots equal `u`.
    fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|&&k| k == u).count()
    }

    /// Inserts `u` once, `u` within the domain: new control points
    /// `lo..=k` blended, those before kept, those after shifted on. `k` is
    /// the span left of `u`, as if it were not yet a knot (the formula holds
    /// at a span's right end, so the domain's end and an existing knot are
    /// raised alike), or at the domain's start the span right of it, whose
    /// terms at `u` itself only shift.
    fn insert(&mut self, u: f64) {
        let p = self.degree;
        let k = if u > self.knots[p] {
            self.knots.iter().rposition(|&t| t < u)
        } else {
            self.knots.iter().rposition(|&t| t <= u)
        }
        .expect("u lies within the domain");
        let hi = k;
        let lo = k + 1 - p;
        let mut out = Vec::with_capacity(self.points.len() + 1);
        out.extend_from_slice(&self.points[..lo]);
        for i in lo..=hi {
            let alpha = (u - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
            let (a, b) = (self.points[i], self.points[i - 1]);
            out.push(std::array::from_fn(|c| alpha * a[c] + (1.0 - alpha) * b[c]));
        }
        out.extend_from_slice(&self.points[hi..]);
        self.points = out;
        self.knots.insert(k + 1, u);
    }

    /// Inserts `u` until it is a knot `degree` times at least.
    fn refine(&mut self, u: f64) {
        while self.multiplicity(u) < self.degree {
            self.insert(u);
        }
    }

    /// The piece from `u0` to `u1`, both knots `degree` times at least: its
    /// control points from the one at `u0` to the one at `u1`, its knots
    /// clamped at both ends.
    fn piece(&self, u0: f64, u1: f64) -> GeometryResult<Spline> {
        let p = self.degree;
        let k = &self.knots;
        let s = k.iter().position(|&t| t == u0).expect("u0 is a knot");
        let m = self.multiplicity(u0);
        let t = k.iter().position(|&t| t == u1).expect("u1 is a knot");
        let (i0, i1) = (s + m - p - 1, t - 1);
        let mut knots = vec![u0; p + 1];
        knots.extend_from_slice(&k[s + m..t]);
        knots.extend(std::iter::repeat_n(u1, p + 1));
        let net = &self.points[i0..=i1];
        let rational = net.iter().any(|h| (h[2] - 1.0).abs() > 1e-12);
        let points = net
            .iter()
            .map(|h| Point2::new(h[0] / h[2], h[1] / h[2]))
            .collect::<GeometryResult<Vec<_>>>()?;
        let weights = if rational {
            net.iter().map(|h| h[2]).collect()
        } else {
            Vec::new()
        };
        Spline::new(p, points, weights, knots, false)
    }
}

impl Spline {
    /// The open piece of it from parameter `u0` to `u1`, `u0` before `u1`,
    /// both within its domain: exact, the same curve there, its knots
    /// clamped at both ends. A parameter within 1e-12 of the domain's
    /// length from a knot is taken as that knot.
    pub fn between(&self, u0: f64, u1: f64) -> GeometryResult<Self> {
        let (a, b) = self.domain();
        if !(u0.is_finite() && u1.is_finite() && a <= u0 && u0 < u1 && u1 <= b) {
            return Err(GeometryError::OutOfRange { what: "parameter" });
        }
        let snap = |u: f64| {
            let near = 1e-12 * (b - a);
            self.knots()
                .iter()
                .copied()
                .find(|&k| (k - u).abs() <= near && (a..=b).contains(&k))
                .unwrap_or(u)
        };
        let (u0, u1) = (snap(u0), snap(u1));
        if u0 >= u1 {
            return Err(GeometryError::OutOfRange { what: "parameter" });
        }
        let mut net = Net::of(self);
        net.refine(u0);
        net.refine(u1);
        net.piece(u0, u1)
    }

    /// The open piece of a closed spline from `u0` round to `u1`, `u0`
    /// within its domain and `u1` past it, at most a period on: its piece to
    /// the end and its piece from the start, joined where it closes.
    pub fn between_round(&self, u0: f64, u1: f64) -> GeometryResult<Self> {
        let (a, b) = self.domain();
        let period = b - a;
        if !self.is_closed() || !(u0 < b && u1 > b && u1 - u0 <= period) {
            return Err(GeometryError::OutOfRange { what: "parameter" });
        }
        let first = self.between(u0, b)?;
        let second = self.between(a, u1 - period)?;
        join(&first, &second, period)
    }
}

/// `first` and then `second`, two clamped pieces of one degree, `second`
/// starting where `first` ends, its knots carried on by `shift`.
pub(crate) fn join(first: &Spline, second: &Spline, shift: f64) -> GeometryResult<Spline> {
    let p = first.degree();
    // A clamped curve's weights may all be scaled: the second's first is
    // made the first's last, so the shared point is one control point.
    let w1 = first.weight_of(first.points().len() - 1);
    let scale = w1 / second.weight_of(0);
    let mut points = first.points().to_vec();
    points.extend_from_slice(&second.points()[1..]);
    let n1 = first.points().len();
    let weights: Vec<f64> = (0..n1)
        .map(|i| first.weight_of(i))
        .chain((1..second.points().len()).map(|i| second.weight_of(i) * scale))
        .collect();
    let rational = weights.iter().any(|w| (w - 1.0).abs() > 1e-12);
    let k1 = first.knots();
    let mut knots = k1[..k1.len() - 1].to_vec();
    knots.extend(second.knots()[p + 1..].iter().map(|k| k + shift));
    Spline::new(
        p,
        points,
        if rational { weights } else { Vec::new() },
        knots,
        false,
    )
}

#[cfg(test)]
#[path = "spline_split_tests.rs"]
mod tests;
