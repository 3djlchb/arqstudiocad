//! Regions of the plane bounded by curved contours, and their booleans
//! (ADR 0033 D5): what a solid's curved face needs to be split in its own
//! parameters, and a flat face cut by a cylinder or a cone.
//!
//! A [`CurvedRegion`] is a set of closed contours, each a chain of
//! [`Trim`]s, the region on the left of each: counterclockwise outside,
//! clockwise holes, as [`Region`](crate::Region)'s. A trim is exact:
//!
//! - a segment;
//! - an arc of an ellipse, a circle among them;
//! - the graph of a rational trigonometric function: what a plane leaves on
//!   a cylinder or a cone, in its parameters `(u, v)`.
//!
//! # Method
//!
//! As `Region`'s, by edge selection, with the curves kept exact (maintainer,
//! 2026-11-07):
//!
//! - **Split:** both regions' trims are split where they cross or touch.
//!   Crossings are seeded where fine polylines of the trims meet and refined
//!   by Newton's method on the exact curves; a trim's end on another trim is
//!   found by projecting it. Where two trims run along each other, their
//!   ends split them.
//! - **Classify:** each piece of one region is inside, outside, or on the
//!   other's boundary the same or the opposite way. Two pieces are one when
//!   they share their ends and their middle.
//! - **Keep and chain:** the kept pieces are chained into contours, turning
//!   as far left as possible at each corner, ties broken by curvature.
//! - **Tidy:** consecutive pieces of one curve are joined again.
//! - **Precision:** points closer than the tolerance are one point, and the
//!   work is done in a frame at the regions' first corner (ADR 0010).

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// Coordinates in the working frame.
type P = [f64; 2];

/// A directed, exact piece of a contour, run by a parameter `t` from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trim {
    /// The segment from `from` to `to`.
    Segment {
        /// Where it starts.
        from: Point2,
        /// Where it ends.
        to: Point2,
    },
    /// The arc `center + a cos θ + b sin θ`, θ from `start` by `sweep`
    /// radians (negative: the other way); a circle when `a` and `b` are
    /// square and of one length.
    Ellipse {
        /// Its center.
        center: Point2,
        /// The point at θ = 0, from the center.
        a: Vector2,
        /// The point at θ = π/2, from the center.
        b: Vector2,
        /// Where it starts.
        start: f64,
        /// How far it turns.
        sweep: f64,
    },
    /// The graph `y = (n₀ + n₁ cos x + n₂ sin x) / (d₀ + d₁ cos x + d₂ sin x)`,
    /// `x` from `from` to `to`: a plane seen on a cylinder or a cone, in its
    /// parameters. The denominator must not vanish over the range.
    Graph {
        /// The numerator's coefficients.
        num: [f64; 3],
        /// The denominator's coefficients.
        den: [f64; 3],
        /// Where `x` starts.
        from: f64,
        /// Where `x` ends.
        to: f64,
    },
}

impl Trim {
    /// The point at `t`, from 0 (its start) to 1 (its end).
    pub fn point(&self, t: f64) -> Point2 {
        let p = self.at(t, [0.0, 0.0]);
        Point2::from_finite(p[0], p[1])
    }

    /// Where it starts.
    pub fn start(&self) -> Point2 {
        self.point(0.0)
    }

    /// Where it ends.
    pub fn end(&self) -> Point2 {
        self.point(1.0)
    }

    /// The same trim run the other way.
    pub fn reversed(&self) -> Self {
        match *self {
            Self::Segment { from, to } => Self::Segment { from: to, to: from },
            Self::Ellipse {
                center,
                a,
                b,
                start,
                sweep,
            } => Self::Ellipse {
                center,
                a,
                b,
                start: start + sweep,
                sweep: -sweep,
            },
            Self::Graph { num, den, from, to } => Self::Graph {
                num,
                den,
                from: to,
                to: from,
            },
        }
    }

    /// The part from `t0` to `t1`.
    fn part(&self, t0: f64, t1: f64) -> Self {
        match *self {
            Self::Segment { .. } => Self::Segment {
                from: self.point(t0),
                to: self.point(t1),
            },
            Self::Ellipse {
                center,
                a,
                b,
                start,
                sweep,
            } => Self::Ellipse {
                center,
                a,
                b,
                start: start + sweep * t0,
                sweep: sweep * (t1 - t0),
            },
            Self::Graph { num, den, from, to } => Self::Graph {
                num,
                den,
                from: from + (to - from) * t0,
                to: from + (to - from) * t1,
            },
        }
    }

    /// The point at `t` in a frame at `o`.
    fn at(&self, t: f64, o: P) -> P {
        match *self {
            Self::Segment { from, to } => [
                from.x() - o[0] + (to.x() - from.x()) * t,
                from.y() - o[1] + (to.y() - from.y()) * t,
            ],
            Self::Ellipse {
                center,
                a,
                b,
                start,
                sweep,
            } => {
                let (s, c) = (start + sweep * t).sin_cos();
                [
                    center.x() - o[0] + a.x() * c + b.x() * s,
                    center.y() - o[1] + a.y() * c + b.y() * s,
                ]
            }
            Self::Graph { num, den, from, to } => {
                let x = from + (to - from) * t;
                [x - o[0], graph(num, den, x) - o[1]]
            }
        }
    }

    /// `d point / dt` at `t`.
    fn derivative(&self, t: f64) -> P {
        match *self {
            Self::Segment { from, to } => [to.x() - from.x(), to.y() - from.y()],
            Self::Ellipse {
                a, b, start, sweep, ..
            } => {
                let (s, c) = (start + sweep * t).sin_cos();
                [
                    sweep * (b.x() * c - a.x() * s),
                    sweep * (b.y() * c - a.y() * s),
                ]
            }
            Self::Graph { num, den, from, to } => {
                let x = from + (to - from) * t;
                let (n, dn) = trig(num, x);
                let (d, dd) = trig(den, x);
                [to - from, (to - from) * (dn * d - n * dd) / (d * d)]
            }
        }
    }

    /// Whether `self` and `next`, which starts where it ends, run along one
    /// curve the same way: then one trim, returned.
    fn joined(&self, next: &Self, tol: f64) -> Option<Self> {
        match (*self, *next) {
            (Self::Segment { from, to }, Self::Segment { to: end, .. }) => {
                let (d, e) = (to - from, end - from);
                let along = (to - from).dot(e);
                let straight = d.cross(e).abs() <= tol * e.length() && along > 0.0;
                (straight && along < e.dot(e)).then_some(Self::Segment { from, to: end })
            }
            (
                Self::Ellipse {
                    center,
                    a,
                    b,
                    start,
                    sweep,
                },
                Self::Ellipse {
                    center: c2,
                    a: a2,
                    b: b2,
                    start: s2,
                    sweep: w2,
                },
            ) => {
                let same = (center - c2).length() <= tol
                    && (a - a2).length() <= tol
                    && (b - b2).length() <= tol;
                let size = a.length().max(b.length()).max(f64::MIN_POSITIVE);
                let gap = (start + sweep - s2).rem_euclid(TAU);
                let meets = gap.min(TAU - gap) * size <= tol;
                let total = sweep + w2;
                (same && meets && sweep * w2 > 0.0 && total.abs() <= TAU + tol / size).then_some(
                    Self::Ellipse {
                        center,
                        a,
                        b,
                        start,
                        sweep: total,
                    },
                )
            }
            (
                Self::Graph { num, den, from, to },
                Self::Graph {
                    num: n2,
                    den: d2,
                    from: f2,
                    to: t2,
                },
            ) => {
                let close = |p: [f64; 3], q: [f64; 3]| {
                    let scale = p.iter().chain(&q).fold(0.0f64, |m, v| m.max(v.abs()));
                    p.iter()
                        .zip(&q)
                        .all(|(x, y)| (x - y).abs() <= 1e-12 * scale.max(1.0))
                };
                let onward = (to - from) * (t2 - f2) > 0.0;
                (close(num, n2) && close(den, d2) && (to - f2).abs() <= tol && onward).then_some(
                    Self::Graph {
                        num,
                        den,
                        from,
                        to: t2,
                    },
                )
            }
            _ => None,
        }
    }
}

/// `(n₀ + n₁ cos x + n₂ sin x, its derivative)`.
fn trig(c: [f64; 3], x: f64) -> (f64, f64) {
    let (s, co) = x.sin_cos();
    (c[0] + c[1] * co + c[2] * s, c[2] * co - c[1] * s)
}

fn graph(num: [f64; 3], den: [f64; 3], x: f64) -> f64 {
    trig(num, x).0 / trig(den, x).0
}

/// A part of the plane bounded by closed contours of [`Trim`]s, the region
/// on the left of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurvedRegion {
    contours: Vec<Vec<Trim>>,
}

/// Which boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Union,
    Intersection,
    Difference,
}

/// How a piece of one region lies with respect to the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Inside,
    Outside,
    Same,
    Opposite,
}

impl CurvedRegion {
    /// The region bounded by `contours`, each a chain of trims, each
    /// starting within `tol` of where the one before ends, the last back to
    /// the first.
    pub fn new(contours: Vec<Vec<Trim>>, tol: Tolerance) -> GeometryResult<Self> {
        for c in &contours {
            if c.is_empty() {
                return Err(GeometryError::ShortContour);
            }
            for k in 0..c.len() {
                let (end, start) = (c[k].end(), c[(k + 1) % c.len()].start());
                if end.distance_to(start) > tol.linear() {
                    return Err(GeometryError::OpenContour);
                }
            }
        }
        Ok(Self { contours })
    }

    /// The region bounded by a closed set of trims in any order, the region
    /// on their left. Trims shorter than the tolerance are ignored.
    pub fn from_trims(trims: &[Trim], tol: Tolerance) -> Self {
        let origin = trims.first().map_or(Point2::ORIGIN, Trim::start);
        let mut work = Work::new(origin, tol);
        let edges: Vec<Edge> = trims.iter().filter_map(|t| work.edge(*t)).collect();
        work.contours(cancel(edges, &work))
    }

    /// Its contours.
    pub fn contours(&self) -> &[Vec<Trim>] {
        &self.contours
    }

    /// Whether it has no contour.
    pub fn is_empty(&self) -> bool {
        self.contours.is_empty()
    }

    /// Its area: the outside contours' less the holes'.
    pub fn area(&self) -> f64 {
        let Some(o) = self.first() else {
            return 0.0;
        };
        self.contours.iter().map(|c| contour_area(c, o)).sum()
    }

    /// Where `point` is: inside, on a contour within `tol`, or outside.
    pub fn contains(&self, point: Point2, tol: Tolerance) -> crate::Containment {
        let p = [point.x(), point.y()];
        let trims = self.contours.iter().flatten();
        if trims
            .clone()
            .any(|t| project(t, p, [0.0, 0.0]).1 <= tol.linear())
        {
            crate::Containment::Boundary
        } else if winding(p, trims.copied(), [0.0, 0.0]) != 0 {
            crate::Containment::Inside
        } else {
            crate::Containment::Outside
        }
    }

    /// The points in either region.
    pub fn union(&self, other: &Self, tol: Tolerance) -> Self {
        boolean(self, other, Op::Union, tol)
    }

    /// The points in both regions.
    pub fn intersection(&self, other: &Self, tol: Tolerance) -> Self {
        boolean(self, other, Op::Intersection, tol)
    }

    /// The points in this region and not in `other`.
    pub fn difference(&self, other: &Self, tol: Tolerance) -> Self {
        boolean(self, other, Op::Difference, tol)
    }

    /// Its connected pieces, each one outside contour (first) and the holes
    /// in it, a hole going to the smallest outside contour around it; a hole
    /// around nothing is dropped.
    pub fn pieces(&self, tol: Tolerance) -> Vec<Self> {
        let Some(o) = self.first() else {
            return Vec::new();
        };
        let areas: Vec<f64> = self.contours.iter().map(|c| contour_area(c, o)).collect();
        let outsides: Vec<usize> = (0..areas.len()).filter(|&i| areas[i] > 0.0).collect();
        let mut pieces: Vec<Vec<usize>> = outsides.iter().map(|&i| vec![i]).collect();
        for hole in (0..areas.len()).filter(|&i| areas[i] <= 0.0) {
            let probe = probe_point(
                &self.contours[hole],
                outsides.iter().map(|&i| &self.contours[i]),
                tol,
            );
            let around = outsides
                .iter()
                .enumerate()
                .filter(|&(_, &k)| {
                    winding(probe, self.contours[k].iter().copied(), [0.0, 0.0]) != 0
                })
                .min_by(|a, b| areas[*a.1].total_cmp(&areas[*b.1]));
            if let Some((k, _)) = around {
                pieces[k].push(hole);
            }
        }
        pieces
            .into_iter()
            .map(|ids| Self {
                contours: ids.into_iter().map(|i| self.contours[i].clone()).collect(),
            })
            .collect()
    }

    fn first(&self) -> Option<Point2> {
        self.contours
            .first()
            .and_then(|c| c.first())
            .map(Trim::start)
    }
}

impl From<&crate::Region> for CurvedRegion {
    fn from(region: &crate::Region) -> Self {
        Self {
            contours: region
                .contours()
                .iter()
                .map(|c| {
                    (0..c.len())
                        .map(|k| Trim::Segment {
                            from: c[k],
                            to: c[(k + 1) % c.len()],
                        })
                        .collect()
                })
                .collect(),
        }
    }
}

/// A piece of a contour between welded points: its ends' indices and the
/// index of its middle, which tells apart pieces between the same ends.
#[derive(Clone, Copy, Debug)]
struct Edge {
    trim: Trim,
    from: usize,
    to: usize,
    middle: usize,
}

impl Edge {
    /// What names it, whichever way it runs, and whether it runs that way:
    /// a closed piece's way is the sign of its area.
    fn key(&self, work: &Work) -> ((usize, usize, usize), bool) {
        if self.from == self.to {
            let forward = contour_area(&[self.trim], work.origin) > 0.0;
            ((self.from, self.to, self.middle), forward)
        } else {
            (
                (self.from.min(self.to), self.from.max(self.to), self.middle),
                self.from < self.to,
            )
        }
    }
}

/// Points welded within a tolerance, in a frame at `origin`; middles
/// welded apart from ends.
struct Work {
    origin: Point2,
    tol: f64,
    ends: Pool,
    middles: Pool,
}

impl Work {
    fn new(origin: Point2, tol: Tolerance) -> Self {
        Self {
            origin,
            tol: tol.linear(),
            ends: Pool::new(tol.linear()),
            middles: Pool::new(tol.linear()),
        }
    }

    fn o(&self) -> P {
        [self.origin.x(), self.origin.y()]
    }

    /// `trim` welded, if it is longer than the tolerance.
    fn edge(&mut self, trim: Trim) -> Option<Edge> {
        let o = self.o();
        let (a, m, b) = (trim.at(0.0, o), trim.at(0.5, o), trim.at(1.0, o));
        let long = dist(a, m) > self.tol || dist(m, b) > self.tol;
        if !long {
            return None;
        }
        Some(Edge {
            trim,
            from: self.ends.weld(a),
            to: self.ends.weld(b),
            middle: self.middles.weld(m),
        })
    }

    /// The contours chained from `edges`, each used once.
    fn contours(&self, edges: Vec<Edge>) -> CurvedRegion {
        let o = self.o();
        let mut leaving: HashMap<usize, Vec<usize>> = HashMap::new();
        for (k, e) in edges.iter().enumerate() {
            leaving.entry(e.from).or_default().push(k);
        }
        let unit = |v: P| {
            let n = v[0].hypot(v[1]);
            if n > 0.0 { [v[0] / n, v[1] / n] } else { v }
        };
        let mut used = vec![false; edges.len()];
        let mut contours = Vec::new();
        for start in 0..edges.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let mut chain = vec![edges[start].trim];
            let mut current = start;
            let closed = loop {
                let e = edges[current];
                let d = unit(e.trim.derivative(1.0));
                // The leftmost turn among the unused edges leaving the end,
                // the first edge too: by the tangent, then, where tangents
                // agree, by which bends left sooner (its curvature).
                let score = |k: usize| {
                    let t = &edges[k].trim;
                    (turn(d, unit(t.derivative(0.0))), curvature(t))
                };
                let next = leaving
                    .get(&e.to)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|&k| !used[k] || k == start)
                    .max_by(|&k, &l| {
                        let ((tk, bk), (tl, bl)) = (score(k), score(l));
                        if (tk - tl).abs() > 1e-9 {
                            tk.total_cmp(&tl)
                        } else {
                            bk.total_cmp(&bl)
                        }
                    });
                match next {
                    None => break false,
                    Some(k) if k == start => break true,
                    Some(k) => {
                        used[k] = true;
                        chain.push(edges[k].trim);
                        current = k;
                    }
                }
            };
            if closed {
                let chain = tidy(chain, self.tol);
                let perimeter: f64 = chain.iter().map(|t| length(t, o)).sum();
                if contour_area(&chain, self.origin).abs() > self.tol * perimeter / 2.0 {
                    contours.push(chain);
                }
            }
        }
        CurvedRegion { contours }
    }
}

/// Points welded within a tolerance.
struct Pool {
    tol: f64,
    points: Vec<P>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl Pool {
    fn new(tol: f64) -> Self {
        Self {
            tol,
            points: Vec::new(),
            cells: HashMap::new(),
        }
    }

    fn weld(&mut self, q: P) -> usize {
        let size = 2.0 * self.tol;
        let cell = ((q[0] / size).floor() as i64, (q[1] / size).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let key = (cell.0.saturating_add(dx), cell.1.saturating_add(dy));
                if let Some(found) = self.cells.get(&key) {
                    for &i in found {
                        if dist(self.points[i], q) <= self.tol {
                            return i;
                        }
                    }
                }
            }
        }
        self.points.push(q);
        let index = self.points.len() - 1;
        self.cells.entry(cell).or_default().push(index);
        index
    }
}

/// The boolean `op` of `a` and `b`.
fn boolean(a: &CurvedRegion, b: &CurvedRegion, op: Op, tol: Tolerance) -> CurvedRegion {
    let Some(origin) = a.first().or(b.first()) else {
        return CurvedRegion::default();
    };
    let mut work = Work::new(origin, tol);
    let o = work.o();
    let t = tol.linear();
    let (ta, tb): (Vec<Trim>, Vec<Trim>) = (
        a.contours.iter().flatten().copied().collect(),
        b.contours.iter().flatten().copied().collect(),
    );
    let apart = match (bounds(&ta, o), bounds(&tb, o)) {
        (Some(p), Some(q)) => {
            p[0] > q[2] + t || q[0] > p[2] + t || p[1] > q[3] + t || q[1] > p[3] + t
        }
        _ => true,
    };
    if apart {
        return match op {
            Op::Union => CurvedRegion {
                contours: a.contours.iter().chain(&b.contours).cloned().collect(),
            },
            Op::Intersection => CurvedRegion::default(),
            Op::Difference => a.clone(),
        };
    }
    let mut pieces = |own: &[Trim], other: &[Trim]| {
        let parts: Vec<Edge> = split(own, other, o, t)
            .into_iter()
            .filter_map(|p| work.edge(p))
            .collect();
        parts
    };
    let (fa, fb) = (pieces(&ta, &tb), pieces(&tb, &ta));
    let (fa, fb) = (cancel(fa, &work), cancel(fb, &work));
    let classes = |own: &[Edge], other: &[Edge], whole: &[Trim]| -> Vec<Class> {
        let keyed: HashMap<(usize, usize, usize), bool> =
            other.iter().map(|e| e.key(&work)).collect();
        own.iter()
            .map(|e| {
                let (key, forward) = e.key(&work);
                match keyed.get(&key) {
                    Some(&f) if f == forward => Class::Same,
                    Some(_) => Class::Opposite,
                    None => {
                        let m = e.trim.at(0.5, [0.0, 0.0]);
                        if winding(m, whole.iter().copied(), [0.0, 0.0]) != 0 {
                            Class::Inside
                        } else {
                            Class::Outside
                        }
                    }
                }
            })
            .collect()
    };
    let (ca, cb) = (classes(&fa, &fb, &tb), classes(&fb, &fa, &ta));
    let mut kept = Vec::new();
    for (e, class) in fa.iter().zip(&ca) {
        let keep = match op {
            Op::Union => matches!(class, Class::Outside | Class::Same),
            Op::Intersection => matches!(class, Class::Inside | Class::Same),
            Op::Difference => matches!(class, Class::Outside | Class::Opposite),
        };
        if keep {
            kept.push(*e);
        }
    }
    for (e, class) in fb.iter().zip(&cb) {
        match (op, class) {
            (Op::Union, Class::Outside) | (Op::Intersection, Class::Inside) => kept.push(*e),
            (Op::Difference, Class::Inside) => kept.push(Edge {
                trim: e.trim.reversed(),
                from: e.to,
                to: e.from,
                middle: e.middle,
            }),
            _ => {}
        }
    }
    work.contours(cancel(kept, &work))
}

/// `edges` with each piece's uses one way and the other netted out.
fn cancel(edges: Vec<Edge>, work: &Work) -> Vec<Edge> {
    let mut net: HashMap<(usize, usize, usize), (i64, Vec<Edge>)> = HashMap::new();
    let mut order = Vec::new();
    for e in edges {
        let (key, forward) = e.key(work);
        let entry = net.entry(key).or_insert_with(|| {
            order.push(key);
            (0, Vec::new())
        });
        entry.0 += if forward { 1 } else { -1 };
        entry.1.push(e);
    }
    order
        .into_iter()
        .filter_map(|key| {
            let (count, uses) = &net[&key];
            let forward = *count > 0;
            (*count != 0).then(|| {
                *uses
                    .iter()
                    .find(|e| e.key(work).1 == forward)
                    .expect("a use runs the net way")
            })
        })
        .collect()
}

/// The trims `own`, split where a trim of `other` crosses them and where an
/// end of any trim lies on them.
fn split(own: &[Trim], other: &[Trim], o: P, t: f64) -> Vec<Trim> {
    let mut out = Vec::with_capacity(own.len());
    for a in own {
        let mut cuts: Vec<f64> = Vec::new();
        let ends =
            |s: f64| dist(a.at(s, o), a.at(0.0, o)) > t && dist(a.at(s, o), a.at(1.0, o)) > t;
        for b in other.iter().chain(own) {
            for q in [b.at(0.0, o), b.at(1.0, o)] {
                let (s, d) = project(a, [q[0] + o[0], q[1] + o[1]], o);
                if d <= t && ends(s) {
                    cuts.push(s);
                }
            }
        }
        for b in other {
            for (s, u) in crossings(a, b, o, t) {
                let b_inner =
                    dist(b.at(u, o), b.at(0.0, o)) > t && dist(b.at(u, o), b.at(1.0, o)) > t;
                if ends(s) && b_inner {
                    cuts.push(s);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|x, y| dist(a.at(*x, o), a.at(*y, o)) <= t);
        let mut from = 0.0;
        for s in cuts {
            out.push(a.part(from, s));
            from = s;
        }
        out.push(a.part(from, 1.0));
    }
    out
}

/// How many straight pieces a trim is followed by when seeking crossings.
fn samples(trim: &Trim) -> usize {
    match trim {
        Trim::Segment { .. } => 1,
        _ => 64,
    }
}

/// The parameters `(s, u)` where `a` and `b` cross, `a(s) = b(u)` within
/// `t`; where they run along each other, none (their ends split them).
fn crossings(a: &Trim, b: &Trim, o: P, t: f64) -> Vec<(f64, f64)> {
    let (na, nb) = (samples(a), samples(b));
    let pa: Vec<P> = (0..=na).map(|i| a.at(i as f64 / na as f64, o)).collect();
    let pb: Vec<P> = (0..=nb).map(|j| b.at(j as f64 / nb as f64, o)).collect();
    // How far a polyline can stray from its curve.
    let stray = |p: &[P]| p.windows(2).map(|w| dist(w[0], w[1])).fold(0.0, f64::max) * 0.25;
    let margin = stray(&pa).max(stray(&pb)) + t;
    let box_of = |p: &[P]| {
        p.iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |b, q| {
                [
                    b[0].min(q[0]),
                    b[1].min(q[1]),
                    b[2].max(q[0]),
                    b[3].max(q[1]),
                ]
            },
        )
    };
    let (ba, bb) = (box_of(&pa), box_of(&pb));
    if ba[0] > bb[2] + margin
        || bb[0] > ba[2] + margin
        || ba[1] > bb[3] + margin
        || bb[1] > ba[3] + margin
    {
        return Vec::new();
    }
    let mut found: Vec<(f64, f64)> = Vec::new();
    for i in 0..na {
        for j in 0..nb {
            let (s0, u0) = closest_on_segments(pa[i], pa[i + 1], pb[j], pb[j + 1]);
            let (p, q) = (lerp(pa[i], pa[i + 1], s0), lerp(pb[j], pb[j + 1], u0));
            if dist(p, q) > margin {
                continue;
            }
            let seed = ((i as f64 + s0) / na as f64, (j as f64 + u0) / nb as f64);
            let Some((s, u)) = refine(a, b, seed, o, t) else {
                continue;
            };
            if found
                .iter()
                .any(|&(s2, _)| dist(a.at(s, o), a.at(s2, o)) <= t)
            {
                continue;
            }
            if along(a, b, s, o, t) {
                continue;
            }
            found.push((s, u));
        }
    }
    found
}

/// Newton's method on `a(s) = b(u)` from `seed`, damped and kept in the
/// trims; the root if it meets within `t`.
fn refine(a: &Trim, b: &Trim, seed: (f64, f64), o: P, t: f64) -> Option<(f64, f64)> {
    let (mut s, mut u) = seed;
    for _ in 0..60 {
        let (p, q) = (a.at(s, o), b.at(u, o));
        let r = [p[0] - q[0], p[1] - q[1]];
        if r[0].hypot(r[1]) <= t * 1e-3 {
            break;
        }
        let (da, db) = (a.derivative(s), b.derivative(u));
        // r + da ds − db du = 0.
        let det = -da[0] * db[1] + da[1] * db[0];
        if det.abs() <= 1e-300 {
            break;
        }
        let ds = (-r[0] * -db[1] - -r[1] * -db[0]) / det;
        let du = (da[0] * -r[1] - da[1] * -r[0]) / det;
        let step = 1.0f64.min(0.25 / ds.abs().max(du.abs()).max(1e-300));
        s = (s + ds * step).clamp(0.0, 1.0);
        u = (u + du * step).clamp(0.0, 1.0);
    }
    (dist(a.at(s, o), b.at(u, o)) <= t).then_some((s, u))
}

/// Whether `a` runs along `b` at `a(s)`: a little way either side, it is
/// still on `b`.
fn along(a: &Trim, b: &Trim, s: f64, o: P, t: f64) -> bool {
    let near = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        let p = a.at(x, o);
        project(b, [p[0] + o[0], p[1] + o[1]], o).1 <= t
    };
    near(s - 0.01) && near(s + 0.01)
}

/// The parameter of the point of `trim` nearest `q` (in the plane's own
/// coordinates), and its distance.
fn project(trim: &Trim, q: P, o: P) -> (f64, f64) {
    let q = [q[0] - o[0], q[1] - o[1]];
    let n = samples(trim).max(8) * 2;
    let mut best: Vec<(f64, f64)> = (0..=n)
        .map(|i| {
            let s = i as f64 / n as f64;
            (s, dist(trim.at(s, o), q))
        })
        .collect();
    best.sort_by(|x, y| x.1.total_cmp(&y.1));
    let mut answer = best[0];
    for &(seed, _) in best.iter().take(3) {
        let mut s = seed;
        for _ in 0..40 {
            let p = trim.at(s, o);
            let d = trim.derivative(s);
            let w = [p[0] - q[0], p[1] - q[1]];
            let h = 1e-6;
            let (d1, d0) = (
                trim.derivative((s + h).min(1.0)),
                trim.derivative((s - h).max(0.0)),
            );
            let span = (s + h).min(1.0) - (s - h).max(0.0);
            let dd = [(d1[0] - d0[0]) / span, (d1[1] - d0[1]) / span];
            let f = w[0] * d[0] + w[1] * d[1];
            let df = d[0] * d[0] + d[1] * d[1] + w[0] * dd[0] + w[1] * dd[1];
            if df.abs() <= 1e-300 {
                break;
            }
            let next = (s - f / df).clamp(0.0, 1.0);
            if (next - s).abs() <= 1e-15 {
                s = next;
                break;
            }
            s = next;
        }
        let d = dist(trim.at(s, o), q);
        if d < answer.1 {
            answer = (s, d);
        }
    }
    answer
}

/// How many times `trims` wind counterclockwise round `p`, in a frame at
/// `o`: the angle each sweeps seen from `p`, by halves until each step is
/// small.
fn winding(p: P, trims: impl Iterator<Item = Trim>, o: P) -> i64 {
    let p = [p[0] - o[0], p[1] - o[1]];
    let total: f64 = trims.map(|trim| swept(&trim, p, o, (0.0, 1.0), 0)).sum();
    (total / TAU).round() as i64
}

/// The angle `trim` sweeps from `t0` to `t1`, seen from `p`.
fn swept(trim: &Trim, p: P, o: P, (t0, t1): (f64, f64), depth: u32) -> f64 {
    let angle = |t: f64| {
        let q = trim.at(t, o);
        (q[1] - p[1]).atan2(q[0] - p[0])
    };
    let mut d = angle(t1) - angle(t0);
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    // A segment sweeps less than a half turn; a curve is halved until its
    // steps are small.
    let straight = matches!(trim, Trim::Segment { .. });
    if straight || depth >= 24 || (d.abs() < 0.25 && depth >= 2) {
        return d;
    }
    let tm = (t0 + t1) / 2.0;
    swept(trim, p, o, (t0, tm), depth + 1) + swept(trim, p, o, (tm, t1), depth + 1)
}

/// A contour's signed area, positive counterclockwise, by Green's theorem.
fn contour_area(contour: &[Trim], origin: Point2) -> f64 {
    let o = [origin.x(), origin.y()];
    contour
        .iter()
        .map(|trim| match trim {
            Trim::Segment { .. } => {
                let (a, b) = (trim.at(0.0, o), trim.at(1.0, o));
                (a[0] * b[1] - a[1] * b[0]) / 2.0
            }
            _ => gauss(|s| {
                let (p, d) = (trim.at(s, o), trim.derivative(s));
                (p[0] * d[1] - p[1] * d[0]) / 2.0
            }),
        })
        .sum()
}

/// A trim's length.
fn length(trim: &Trim, o: P) -> f64 {
    match trim {
        Trim::Segment { .. } => dist(trim.at(0.0, o), trim.at(1.0, o)),
        _ => gauss(|s| {
            let d = trim.derivative(s);
            d[0].hypot(d[1])
        }),
    }
}

/// `∫₀¹ f`, by a 16-point Gauss–Legendre rule on each of 16 pieces.
fn gauss(f: impl Fn(f64) -> f64) -> f64 {
    const X: [f64; 8] = [
        0.095_012_509_837_637_44,
        0.281_603_550_779_258_9,
        0.458_016_777_657_227_4,
        0.617_876_244_402_643_7,
        0.755_404_408_355_003,
        0.865_631_202_387_831_7,
        0.944_575_023_073_232_6,
        0.989_400_934_991_649_9,
    ];
    const W: [f64; 8] = [
        0.189_450_610_455_068_5,
        0.182_603_415_044_923_6,
        0.169_156_519_395_002_5,
        0.149_595_988_816_576_7,
        0.124_628_971_255_533_9,
        0.095_158_511_682_492_78,
        0.062_253_523_938_647_89,
        0.027_152_459_411_754_09,
    ];
    let pieces = 16;
    let h = 1.0 / pieces as f64;
    (0..pieces)
        .map(|k| {
            let mid = (k as f64 + 0.5) * h;
            X.iter()
                .zip(&W)
                .map(|(x, w)| w * (f(mid - x * h / 2.0) + f(mid + x * h / 2.0)))
                .sum::<f64>()
                * h
                / 2.0
        })
        .sum()
}

/// Consecutive trims of one curve joined, round the contour.
fn tidy(mut chain: Vec<Trim>, tol: f64) -> Vec<Trim> {
    let mut changed = true;
    while changed && chain.len() > 1 {
        changed = false;
        for k in 0..chain.len() {
            let next = (k + 1) % chain.len();
            if let Some(joined) = chain[k].joined(&chain[next], tol) {
                chain[k] = joined;
                chain.remove(next);
                changed = true;
                break;
            }
        }
    }
    chain
}

/// A point inside a hole's contour, beside a trim's middle, clear of the
/// outside contours.
fn probe_point<'a>(
    hole: &[Trim],
    outsides: impl Iterator<Item = &'a Vec<Trim>> + Clone,
    tol: Tolerance,
) -> P {
    let middles: Vec<P> = hole.iter().map(|t| t.at(0.5, [0.0, 0.0])).collect();
    middles
        .iter()
        .copied()
        .find(|&m| {
            outsides
                .clone()
                .all(|c| c.iter().all(|t| project(t, m, [0.0, 0.0]).1 > tol.linear()))
        })
        .unwrap_or(middles[0])
}

/// Smallest and largest coordinates of the trims' samples: `[x0, y0, x1, y1]`.
fn bounds(trims: &[Trim], o: P) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for trim in trims {
        let n = samples(trim);
        for i in 0..=n {
            let p = trim.at(i as f64 / n as f64, o);
            b = Some(b.map_or([p[0], p[1], p[0], p[1]], |b| {
                [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ]
            }));
        }
    }
    // Samples miss a curve's bulge between them: widen by it.
    b.map(|b| {
        let pad = 0.05 * (b[2] - b[0]).max(b[3] - b[1]);
        [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad]
    })
}

/// The closest points of two segments, as their parameters.
fn closest_on_segments(a: P, b: P, c: P, d: P) -> (f64, f64) {
    let (u, v, w) = (sub(b, a), sub(d, c), sub(a, c));
    let (uu, uv, vv, uw, vw) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let den = uu * vv - uv * uv;
    let mut s = if den > 1e-300 {
        ((uv * vw - vv * uw) / den).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut t = if vv > 1e-300 {
        ((uv * s + vw) / vv).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if uu > 1e-300 {
        s = ((uv * t - uw) / uu).clamp(0.0, 1.0);
    }
    if vv > 1e-300 {
        t = ((uv * s + vw) / vv).clamp(0.0, 1.0);
    }
    (s, t)
}

/// The signed curvature of `trim` where it starts, positive to the left.
fn curvature(trim: &Trim) -> f64 {
    let h = 1e-4;
    let (d, d1) = (trim.derivative(0.0), trim.derivative(h));
    let dd = [(d1[0] - d[0]) / h, (d1[1] - d[1]) / h];
    let speed = d[0].hypot(d[1]);
    if speed == 0.0 {
        return 0.0;
    }
    (d[0] * dd[1] - d[1] * dd[0]) / speed.powi(3)
}

fn turn(d: P, e: P) -> f64 {
    (d[0] * e[1] - d[1] * e[0]).atan2(d[0] * e[0] + d[1] * e[1])
}

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn lerp(a: P, b: P, s: f64) -> P {
    [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s]
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

#[cfg(test)]
#[path = "curved_region_tests.rs"]
mod tests;
