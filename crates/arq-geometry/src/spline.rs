//! `Spline`: a NURBS curve in the plan, as DWG's SPLINE keeps it (ADR
//! 0042): its degree, knots, control points and, when rational, their
//! weights. Drawn by the very points the codec has always drawn it by.

use std::sync::OnceLock;

use crate::bounds::Bounds2;
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::transform::Transform2;

/// Segments drawn in each span between distinct knots, as the codec's
/// `nurbs_points` (16), so nothing drawn changes.
const SEGMENTS: usize = 16;

/// The highest degree evaluated: DWG's own limit.
pub(crate) const MAX_DEGREE: usize = 25;

/// A NURBS curve in the plan: `degree`, a nondecreasing knot vector of
/// `points + degree + 1` values, its control points and their weights
/// (all one when it is not rational). `closed` is DWG's flag: a closed
/// spline is drawn back to its start. What it is drawn by and its exact
/// box are computed when first asked for, so a spline read, checked or
/// compared costs little (ADR 0043 D4); two are equal by their data.
#[derive(Clone, Debug)]
pub struct Spline {
    degree: usize,
    knots: Vec<f64>,
    points: Vec<Point2>,
    weights: Vec<f64>,
    closed: bool,
    /// The parameters and points it is drawn by, computed once.
    samples: OnceLock<Vec<(f64, Point2)>>,
    /// Those points, back to the start when closed.
    drawn: OnceLock<Vec<Point2>>,
    bounds: OnceLock<Bounds2>,
}

impl PartialEq for Spline {
    fn eq(&self, other: &Self) -> bool {
        self.degree == other.degree
            && self.knots == other.knots
            && self.points == other.points
            && self.weights == other.weights
            && self.closed == other.closed
    }
}

impl Spline {
    /// The spline of `degree` through control `points` with `knots` and
    /// `weights` (empty, or one a point: rational when any is not one).
    /// Refused for fewer than two points, a degree of zero or past the
    /// points, a knot vector of the wrong length or not nondecreasing or
    /// with no span, and weights that are not finite and positive.
    pub fn new(
        degree: usize,
        points: Vec<Point2>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        closed: bool,
    ) -> GeometryResult<Self> {
        Self::check(degree, &points, &weights, &knots)?;
        // Not rational unless a weight says so, as the codec decides.
        let rational = weights.iter().any(|w| (*w - 1.0).abs() > 1e-12);
        let weights = if rational { weights } else { Vec::new() };
        Ok(Self {
            degree,
            knots,
            points,
            weights,
            closed,
            samples: OnceLock::new(),
            drawn: OnceLock::new(),
            bounds: OnceLock::new(),
        })
    }

    /// The parameters and points it is drawn by, before it closes.
    fn samples(&self) -> &[(f64, Point2)] {
        self.samples.get_or_init(|| self.compute_samples())
    }

    /// Whether [`Spline::new`] takes these, without making the spline.
    pub fn check(
        degree: usize,
        points: &[Point2],
        weights: &[f64],
        knots: &[f64],
    ) -> GeometryResult<()> {
        let n = points.len();
        if n < 2 || degree == 0 || degree >= n || degree > MAX_DEGREE {
            return Err(GeometryError::OutOfRange {
                what: "spline degree",
            });
        }
        if knots.len() != n + degree + 1
            || !knots.iter().all(|k| k.is_finite())
            || !knots.windows(2).all(|w| w[1] >= w[0])
            || knots[n] <= knots[degree]
        {
            return Err(GeometryError::OutOfRange {
                what: "spline knots",
            });
        }
        if !(weights.is_empty() || weights.len() == n)
            || !weights.iter().all(|w| w.is_finite() && *w > 0.0)
        {
            return Err(GeometryError::OutOfRange {
                what: "spline weights",
            });
        }
        Ok(())
    }

    /// Clamped uniform knots for `count` control points of `degree`: the
    /// first and last repeated `degree + 1` times, as the codec makes them
    /// for a spline whose own are unusable.
    pub fn clamped_knots(degree: usize, count: usize) -> Vec<f64> {
        let inner = count - degree;
        (0..count + degree + 1)
            .map(|i| {
                if i <= degree {
                    0.0
                } else if i >= count {
                    1.0
                } else {
                    (i - degree) as f64 / inner as f64
                }
            })
            .collect()
    }

    /// Its degree.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Its knots.
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Its control points.
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    /// Its weights: empty when it is not rational.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// Its own control vertices: all of them but the ones a closed spline
    /// repeats at its end, its degree's when periodic, its first when
    /// clamped round to it. What its grips show and STRETCH moves.
    pub fn own_vertices(&self) -> &[Point2] {
        let (p, points) = (self.degree, &self.points);
        let n = points.len();
        let repeated = if !self.closed {
            0
        } else if n > p && points[n - p..] == points[..p] {
            p
        } else if points[n - 1] == points[0] {
            1
        } else {
            0
        };
        &points[..n - repeated]
    }

    /// Its control points with vertex `index` of [`Spline::own_vertices`]
    /// put at `to`, the copies a closed one repeats with it.
    pub fn with_vertex(&self, index: usize, to: Point2) -> GeometryResult<Self> {
        let own = self.own_vertices().len();
        if index >= own {
            return Err(GeometryError::OutOfRange { what: "vertex" });
        }
        let points = (0..self.points.len())
            .map(|i| if i % own == index { to } else { self.points[i] })
            .collect();
        Self::new(
            self.degree,
            points,
            self.weights.clone(),
            self.knots.clone(),
            self.closed,
        )
    }

    /// Whether DWG calls it closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The parameters it runs over.
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.points.len()])
    }

    pub(crate) fn weight_of(&self, i: usize) -> f64 {
        self.weights.get(i).copied().unwrap_or(1.0)
    }

    /// The span whose knots hold `u`, `degree..points`, the last for the
    /// domain's end.
    pub(crate) fn span_of(&self, u: f64) -> usize {
        let (p, n) = (self.degree, self.points.len());
        let mut k = p;
        while k + 1 < n && self.knots[k + 1] <= u {
            k += 1;
        }
        k
    }

    /// De Boor's algorithm in homogeneous coordinates over span `k`, as the
    /// codec evaluates, so the points come out the same.
    fn eval(&self, u: f64, k: usize) -> Point2 {
        let p = self.degree;
        let mut d = [[0.0f64; 3]; MAX_DEGREE + 1];
        for (j, slot) in d.iter_mut().enumerate().take(p + 1) {
            let i = j + k - p;
            let (c, w) = (self.points[i], self.weight_of(i));
            *slot = [c.x() * w, c.y() * w, w];
        }
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = j + k - p;
                let denom = self.knots[i + p + 1 - r] - self.knots[i];
                let alpha = if denom.abs() < 1e-15 {
                    0.0
                } else {
                    (u - self.knots[i]) / denom
                };
                let (prev, cur) = (d[j - 1], d[j]);
                d[j] = std::array::from_fn(|c| (1.0 - alpha) * prev[c] + alpha * cur[c]);
            }
        }
        let w = if d[p][2].abs() < 1e-15 { 1.0 } else { d[p][2] };
        Point2::new(d[p][0] / w, d[p][1] / w).expect("a combination of finite points")
    }

    /// The point at parameter `u`, clamped to the domain.
    pub fn point_at(&self, u: f64) -> Point2 {
        let (a, b) = self.domain();
        let u = u.clamp(a, b);
        self.eval(u, self.span_of(u))
    }

    /// Points along it to draw it by: sixteen segments in each span, as the
    /// codec has always drawn splines, back to its start when closed.
    /// Computed once.
    pub fn flatten(&self) -> &[Point2] {
        self.drawn.get_or_init(|| {
            let mut drawn: Vec<Point2> = self.samples().iter().map(|(_, q)| *q).collect();
            if self.closed && drawn.len() > 2 && drawn[0] != drawn[drawn.len() - 1] {
                drawn.push(drawn[0]);
            }
            drawn
        })
    }

    /// The parameters and points of [`Spline::flatten`], before it closes.
    fn compute_samples(&self) -> Vec<(f64, Point2)> {
        let (p, n) = (self.degree, self.points.len());
        let mut out = Vec::new();
        for k in p..n {
            let (u0, u1) = (self.knots[k], self.knots[k + 1]);
            if u1 - u0 <= 1e-15 {
                continue;
            }
            let first = usize::from(!out.is_empty());
            for s in first..=SEGMENTS {
                let u = u0 + (u1 - u0) * s as f64 / SEGMENTS as f64;
                out.push((u, self.eval(u, k)));
            }
        }
        out
    }

    /// Its ends, start and end of its domain.
    pub fn ends(&self) -> (Point2, Point2) {
        let (a, b) = self.domain();
        (self.point_at(a), self.point_at(b))
    }

    /// The parameter of its point nearest `p`: the nearest of the drawn
    /// points, then golden-section search between its neighbors. A closed
    /// spline's first and last samples are neighbors across where it
    /// closes, so both sides of that are searched.
    pub fn closest_param(&self, p: Point2) -> f64 {
        let samples = self.samples();
        let best = (0..samples.len())
            .min_by(|&a, &b| {
                samples[a]
                    .1
                    .distance_squared_to(p)
                    .total_cmp(&samples[b].1.distance_squared_to(p))
            })
            .expect("a spline has samples");
        let last = samples.len() - 1;
        let bracket = |i: usize| (samples[i.saturating_sub(1)].0, samples[(i + 1).min(last)].0);
        let mut found = self.closest_between(p, bracket(best));
        if self.closed && (best == 0 || best == last) {
            let other = self.closest_between(p, bracket(if best == 0 { last } else { 0 }));
            if self.point_at(other).distance_squared_to(p)
                < self.point_at(found).distance_squared_to(p)
            {
                found = other;
            }
        }
        found
    }

    /// The parameter of its point nearest `p` between `lo` and `hi`.
    fn closest_between(&self, p: Point2, (mut lo, mut hi): (f64, f64)) -> f64 {
        let distance = |u: f64| self.point_at(u).distance_squared_to(p);
        let (first, last) = (lo, hi);
        let golden = (5f64.sqrt() - 1.0) / 2.0;
        for _ in 0..100 {
            let (a, b) = (hi - golden * (hi - lo), lo + golden * (hi - lo));
            if distance(a) < distance(b) {
                hi = b;
            } else {
                lo = a;
            }
        }
        // Comparing distances resolves only to about 1e-8: Newton's method
        // on where the distance stops changing, the derivatives by central
        // differences, kept between the neighbors.
        let (a, b) = self.domain();
        let h = 1e-6 * (b - a);
        let mut u = (lo + hi) / 2.0;
        for _ in 0..8 {
            let (here, before, after) =
                (self.point_at(u), self.point_at(u - h), self.point_at(u + h));
            let d1 = (after - before) / (2.0 * h);
            let d2 = ((after - here) - (here - before)) / (h * h);
            let off = here - p;
            let slope = off.dot(d1);
            let curve = d1.dot(d1) + off.dot(d2);
            if curve <= 0.0 {
                break;
            }
            let next = u - slope / curve;
            if !(first..=last).contains(&next) {
                break;
            }
            u = next;
        }
        u
    }

    /// The point of it nearest `p`.
    pub fn closest_point(&self, p: Point2) -> Point2 {
        self.point_at(self.closest_param(p))
    }

    /// How far `p` is from it.
    pub fn distance_to(&self, p: Point2) -> f64 {
        self.closest_point(p).distance_to(p)
    }

    /// Whether `p` lies on it, within `tol`.
    pub fn contains_point(&self, p: Point2, tol: Tolerance) -> bool {
        self.distance_to(p) <= tol.linear()
    }

    /// The smallest box holding it, computed once: its ends and where it
    /// turns in x or in y, found between the drawn points and refined.
    pub fn bounds(&self) -> Bounds2 {
        *self.bounds.get_or_init(|| self.compute_bounds())
    }

    fn compute_bounds(&self) -> Bounds2 {
        let samples = self.samples();
        let mut points: Vec<Point2> = samples.iter().map(|(_, q)| *q).collect();
        // A turn between two samples: where x (or y) stops changing, by
        // Newton's method from the sample, the derivatives by central
        // differences, kept between its neighbors. The drawn sample stays
        // among the points, so the box is never smaller than the drawing.
        let refine = |lo: f64, mid: f64, hi: f64, value: &dyn Fn(f64) -> f64| {
            let h = (hi - lo) * 1e-4;
            let mut u = mid;
            for _ in 0..6 {
                let (before, here, after) = (value(u - h), value(u), value(u + h));
                let slope = (after - before) / (2.0 * h);
                let bend = (after - 2.0 * here + before) / (h * h);
                if bend == 0.0 {
                    break;
                }
                let next = u - slope / bend;
                if !(lo..=hi).contains(&next) {
                    break;
                }
                u = next;
            }
            self.point_at(u)
        };
        let drawn = Bounds2::from_points(points.iter().copied()).expect("a spline has points");
        for axis in 0..2 {
            let coord = |q: Point2| if axis == 0 { q.x() } else { q.y() };
            let (low, high) = if axis == 0 {
                (drawn.min().x(), drawn.max().x())
            } else {
                (drawn.min().y(), drawn.max().y())
            };
            // Only a turn near the drawn box's side can move it: a chord
            // falls short of its curve by far less than a tenth of the
            // spline's size, sixteen of them to a span.
            let near = 0.1 * (high - low);
            for w in samples.windows(3) {
                let (a, b, c) = (coord(w[0].1), coord(w[1].1), coord(w[2].1));
                let peak = b >= a && b >= c && (b > a || b > c) && high - b <= near;
                let dip = b <= a && b <= c && (b < a || b < c) && b - low <= near;
                if peak || dip {
                    points.push(refine(w[0].0, w[1].0, w[2].0, &|u| coord(self.point_at(u))));
                }
            }
        }
        Bounds2::from_points(points).expect("a spline has points")
    }

    /// Its image by `t`, a similarity: the control points carried, the
    /// knots and weights kept (a NURBS is carried by its control points).
    pub fn transformed(&self, t: Transform2) -> GeometryResult<Self> {
        Self::new(
            self.degree,
            self.points.iter().map(|&q| t.apply_point(q)).collect(),
            self.weights.clone(),
            self.knots.clone(),
            self.closed,
        )
    }
}

#[cfg(test)]
#[path = "spline_tests.rs"]
mod tests;
