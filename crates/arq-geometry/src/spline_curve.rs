//! The spline as a curve (ADR 0043 D1): its derivative, its length and
//! the point at a distance along it.

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::spline::{MAX_DEGREE, Spline};
use crate::vector::Vector2;

/// Five-point Gauss–Legendre: nodes on [-1, 1] and their weights.
const GAUSS_X: [f64; 5] = [
    0.0,
    0.538_469_310_105_683_1,
    -0.538_469_310_105_683_1,
    0.906_179_845_938_664,
    -0.906_179_845_938_664,
];
const GAUSS_W: [f64; 5] = [
    0.568_888_888_888_888_9,
    0.478_628_670_499_366_5,
    0.478_628_670_499_366_5,
    0.236_926_885_056_189_1,
    0.236_926_885_056_189_1,
];

impl Spline {
    /// The basis functions not zero on span `k` at `u`, and their first
    /// derivatives (The NURBS Book, A2.3, to the first derivative).
    fn basis_and_slope(&self, k: usize, u: f64) -> ([f64; MAX_DEGREE + 1], [f64; MAX_DEGREE + 1]) {
        let p = self.degree();
        let knots = self.knots();
        let mut ndu = [[0.0f64; MAX_DEGREE + 1]; MAX_DEGREE + 1];
        let (mut left, mut right) = ([0.0f64; MAX_DEGREE + 1], [0.0f64; MAX_DEGREE + 1]);
        ndu[0][0] = 1.0;
        for j in 1..=p {
            left[j] = u - knots[k + 1 - j];
            right[j] = knots[k + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                ndu[j][r] = right[r + 1] + left[j - r];
                let temp = if ndu[j][r] == 0.0 {
                    0.0
                } else {
                    ndu[r][j - 1] / ndu[j][r]
                };
                ndu[r][j] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            ndu[j][j] = saved;
        }
        let mut value = [0.0f64; MAX_DEGREE + 1];
        let mut slope = [0.0f64; MAX_DEGREE + 1];
        let ratio = |num: f64, den: f64| if den == 0.0 { 0.0 } else { num / den };
        for r in 0..=p {
            value[r] = ndu[r][p];
            let before = if r >= 1 {
                ratio(ndu[r - 1][p - 1], ndu[p][r - 1])
            } else {
                0.0
            };
            let after = if r < p {
                ratio(ndu[r][p - 1], ndu[p][r])
            } else {
                0.0
            };
            slope[r] = p as f64 * (before - after);
        }
        (value, slope)
    }

    /// The point and the derivative at parameter `u`, clamped to the
    /// domain: exact, the quotient rule on the homogeneous curve.
    pub fn point_and_derivative(&self, u: f64) -> (Point2, Vector2) {
        let (a, b) = self.domain();
        let u = u.clamp(a, b);
        let k = self.span_of(u);
        let p = self.degree();
        let (value, slope) = self.basis_and_slope(k, u);
        let (mut aw, mut daw) = ([0.0f64; 3], [0.0f64; 3]);
        for r in 0..=p {
            let i = k - p + r;
            let (c, w) = (self.points()[i], self.weight_of(i));
            let h = [c.x() * w, c.y() * w, w];
            for d in 0..3 {
                aw[d] += value[r] * h[d];
                daw[d] += slope[r] * h[d];
            }
        }
        let w = aw[2];
        let (x, y) = (aw[0] / w, aw[1] / w);
        let dx = (daw[0] - daw[2] * x) / w;
        let dy = (daw[1] - daw[2] * y) / w;
        (
            Point2::new(x, y).expect("a combination of finite points"),
            Vector2::new(dx, dy).expect("a finite derivative"),
        )
    }

    /// The derivative at parameter `u`.
    pub fn derivative_at(&self, u: f64) -> Vector2 {
        self.point_and_derivative(u).1
    }

    /// The parameters bounding its spans: its distinct knots within the
    /// domain, in order.
    pub fn span_breaks(&self) -> Vec<f64> {
        let (a, b) = self.domain();
        let mut out = vec![a];
        for &k in self.knots() {
            if k > a && k < b && k > *out.last().expect("one at least") {
                out.push(k);
            }
        }
        out.push(b);
        out
    }

    /// The length along it from parameter `from` to `to` (`to` no smaller
    /// than `from`, both within the domain): the integral of its speed, by
    /// adaptive Gauss–Legendre quadrature over each span, to about 1e-12 of
    /// the length.
    pub fn length_between(&self, from: f64, to: f64) -> f64 {
        if to <= from {
            return 0.0;
        }
        let speed = |u: f64| self.derivative_at(u).length();
        let gauss = |a: f64, b: f64| {
            let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
            half * GAUSS_X
                .iter()
                .zip(GAUSS_W)
                .map(|(x, w)| w * speed(mid + half * x))
                .sum::<f64>()
        };
        fn refine(
            gauss: &dyn Fn(f64, f64) -> f64,
            a: f64,
            b: f64,
            whole: f64,
            tol: f64,
            depth: u32,
        ) -> f64 {
            let m = (a + b) / 2.0;
            let (left, right) = (gauss(a, m), gauss(m, b));
            if depth == 0 || (left + right - whole).abs() <= tol {
                left + right
            } else {
                refine(gauss, a, m, left, tol / 2.0, depth - 1)
                    + refine(gauss, m, b, right, tol / 2.0, depth - 1)
            }
        }
        let b = self.bounds();
        let size = b.width().hypot(b.height()).max(f64::MIN_POSITIVE);
        let breaks = self.span_breaks();
        breaks
            .windows(2)
            .filter_map(|w| {
                let (a, b) = (w[0].max(from), w[1].min(to));
                (b > a).then_some((a, b))
            })
            .map(|(a, b)| refine(&gauss, a, b, gauss(a, b), 1e-13 * size, 40))
            .sum()
    }

    /// Its length.
    pub fn length(&self) -> f64 {
        let (a, b) = self.domain();
        self.length_between(a, b)
    }

    /// The parameter `distance` along it from its start: Newton's method on
    /// the length, kept in a bracket. Refused past its length.
    pub fn param_at_distance(&self, distance: f64) -> GeometryResult<f64> {
        let length = self.length();
        let slack = 1e-12 * length.max(1.0);
        if !(distance.is_finite() && distance >= -slack && distance <= length + slack) {
            return Err(GeometryError::OutOfRange { what: "distance" });
        }
        let distance = distance.clamp(0.0, length);
        let (start, end) = self.domain();
        let (mut lo, mut hi) = (start, end);
        let mut u = start + (end - start) * distance / length.max(f64::MIN_POSITIVE);
        for _ in 0..80 {
            let gone = self.length_between(start, u) - distance;
            if gone.abs() <= 1e-12 * length.max(f64::MIN_POSITIVE) {
                break;
            }
            if gone > 0.0 {
                hi = u;
            } else {
                lo = u;
            }
            let speed = self.derivative_at(u).length();
            let next = if speed > 0.0 {
                u - gone / speed
            } else {
                f64::NAN
            };
            u = if next > lo && next < hi {
                next
            } else {
                (lo + hi) / 2.0
            };
        }
        Ok(u)
    }

    /// The point `distance` along it from its start.
    pub fn point_at_distance(&self, distance: f64) -> GeometryResult<Point2> {
        Ok(self.point_at(self.param_at_distance(distance)?))
    }
}

#[cfg(test)]
#[path = "spline_curve_tests.rs"]
mod tests;
