//! Regions of the plane bounded by straight contours (polygons with holes)
//! and their booleans: union, intersection and difference (ADR 0032 D3).
//!
//! A [`Region`] is a set of closed contours, each a list of corners; the
//! region lies on the left of every contour, so an outside boundary runs
//! counterclockwise and a hole clockwise. Pieces may lie apart or nest (an
//! island in a hole); [`Region::pieces`] tells them apart.
//!
//! # Method
//!
//! The booleans select boundary edges. Both regions' edges are split where
//! they cross or touch, each piece of one is classified against the other
//! (inside, outside, or on its boundary running the same way or the
//! opposite way), the pieces the operation keeps are chosen, and the kept
//! pieces are chained into contours, turning as far left as possible at
//! each corner so that pieces touching at a corner stay separate contours.
//! Points closer than the tolerance are one point, and corners on a straight
//! run between their neighbours are dropped. The work is done in a frame at
//! the regions' first corner, so far coordinates keep their precision
//! (ADR 0010).

use std::collections::{HashMap, HashSet};

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;

/// Coordinates in the working frame.
type P = [f64; 2];

/// A part of the plane bounded by closed straight contours, the region on
/// the left of each one: counterclockwise outside, clockwise holes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    contours: Vec<Vec<Point2>>,
}

/// Where a point is with respect to a [`Region`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Containment {
    /// In its interior.
    Inside,
    /// On a contour, within the tolerance.
    Boundary,
    /// Outside it.
    Outside,
}

/// Which boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Union,
    Intersection,
    Difference,
}

/// How an edge piece of one region lies with respect to the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Inside,
    Outside,
    /// On the other's boundary, running the same way.
    Same,
    /// On the other's boundary, running the opposite way.
    Opposite,
}

impl Region {
    /// The region bounded by `contours`, each of at least three corners,
    /// the region on its left.
    pub fn new(contours: Vec<Vec<Point2>>) -> GeometryResult<Self> {
        if contours.iter().any(|c| c.len() < 3) {
            Err(GeometryError::ShortContour)
        } else {
            Ok(Self { contours })
        }
    }

    /// The region bounded by a closed set of directed edges in any order,
    /// the region on their left: each corner has as many edges leaving it
    /// as arriving. Edges shorter than the tolerance are ignored.
    pub fn from_edges(edges: &[(Point2, Point2)], tol: Tolerance) -> Self {
        let origin = edges.first().map_or(Point2::ORIGIN, |e| e.0);
        let mut graph = Graph::new(origin, tol);
        let mut kept = Vec::with_capacity(edges.len());
        for &(a, b) in edges {
            let (i, j) = (graph.weld(a), graph.weld(b));
            if i != j {
                kept.push((i, j));
            }
        }
        graph.contours(cancel(kept))
    }

    /// Its contours.
    pub fn contours(&self) -> &[Vec<Point2>] {
        &self.contours
    }

    /// Whether it has no contour.
    pub fn is_empty(&self) -> bool {
        self.contours.is_empty()
    }

    /// Its area: the outside contours' areas less the holes'.
    pub fn area(&self) -> f64 {
        let Some(origin) = self.first() else {
            return 0.0;
        };
        self.contours
            .iter()
            .map(|c| signed_area(&local(c, origin)))
            .sum()
    }

    /// Where `point` is: inside, on a contour within `tol`, or outside.
    pub fn contains(&self, point: Point2, tol: Tolerance) -> Containment {
        let segments = segments(self, point);
        let q = [0.0, 0.0];
        if segments
            .iter()
            .any(|&(a, b)| distance_to_segment(q, a, b) <= tol.linear())
        {
            Containment::Boundary
        } else if winding(q, &segments) != 0 {
            Containment::Inside
        } else {
            Containment::Outside
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
    /// in it; a hole goes to the smallest outside contour around it. A hole
    /// around nothing is dropped.
    pub fn pieces(&self, tol: Tolerance) -> Vec<Self> {
        let Some(origin) = self.first() else {
            return Vec::new();
        };
        let locals: Vec<Vec<P>> = self.contours.iter().map(|c| local(c, origin)).collect();
        let areas: Vec<f64> = locals.iter().map(|c| signed_area(c)).collect();
        let outsides: Vec<usize> = (0..locals.len()).filter(|&i| areas[i] > 0.0).collect();
        let mut pieces: Vec<Vec<usize>> = outsides.iter().map(|&i| vec![i]).collect();
        for hole in (0..locals.len()).filter(|&i| areas[i] <= 0.0) {
            let probe = probe_point(&locals[hole], outsides.iter().map(|&i| &locals[i]), tol);
            let around = outsides
                .iter()
                .enumerate()
                .filter(|&(_, &o)| winding(probe, &closed(&locals[o])) != 0)
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

    /// Its first corner, the working frame's origin.
    fn first(&self) -> Option<Point2> {
        self.contours.first().and_then(|c| c.first()).copied()
    }
}

/// The boolean `op` of `a` and `b`.
fn boolean(a: &Region, b: &Region, op: Op, tol: Tolerance) -> Region {
    let Some(origin) = a.first().or(b.first()) else {
        return Region::default();
    };
    let t = tol.linear();
    let (sa, sb) = (segments(a, origin), segments(b, origin));
    let apart = match (bounds(&sa), bounds(&sb)) {
        (Some(p), Some(q)) => {
            p[0] > q[2] + t || q[0] > p[2] + t || p[1] > q[3] + t || q[1] > p[3] + t
        }
        _ => true,
    };
    if apart {
        return match op {
            Op::Union => Region {
                contours: a.contours.iter().chain(&b.contours).cloned().collect(),
            },
            Op::Intersection => Region::default(),
            Op::Difference => a.clone(),
        };
    }
    let mut graph = Graph::new(origin, tol);
    let mut pieces = |own: &[(P, P)], other: &[(P, P)]| {
        let welded: Vec<(usize, usize)> = split(own, other, t)
            .into_iter()
            .map(|(p, q)| (graph.weld_local(p), graph.weld_local(q)))
            .filter(|(i, j)| i != j)
            .collect();
        cancel(welded)
    };
    let (fa, fb) = (pieces(&sa, &sb), pieces(&sb, &sa));
    let classes = |own: &[(usize, usize)], other: &[(usize, usize)], whole: &[(P, P)]| {
        let set: HashSet<(usize, usize)> = other.iter().copied().collect();
        own.iter()
            .map(|&(i, j)| {
                if set.contains(&(i, j)) {
                    Class::Same
                } else if set.contains(&(j, i)) {
                    Class::Opposite
                } else {
                    let (p, q) = (graph.points[i], graph.points[j]);
                    let mid = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
                    if winding(mid, whole) != 0 {
                        Class::Inside
                    } else {
                        Class::Outside
                    }
                }
            })
            .collect::<Vec<_>>()
    };
    let (ca, cb) = (classes(&fa, &fb, &sb), classes(&fb, &fa, &sa));
    let mut kept = Vec::new();
    for (&e, &class) in fa.iter().zip(&ca) {
        let keep = match op {
            Op::Union => matches!(class, Class::Outside | Class::Same),
            Op::Intersection => matches!(class, Class::Inside | Class::Same),
            Op::Difference => matches!(class, Class::Outside | Class::Opposite),
        };
        if keep {
            kept.push(e);
        }
    }
    // The pieces on both boundaries were decided with `a`'s.
    for (&(i, j), &class) in fb.iter().zip(&cb) {
        match (op, class) {
            (Op::Union, Class::Outside) | (Op::Intersection, Class::Inside) => kept.push((i, j)),
            (Op::Difference, Class::Inside) => kept.push((j, i)),
            _ => {}
        }
    }
    graph.contours(cancel(kept))
}

/// Points welded within a tolerance, in a frame at `origin`.
struct Graph {
    origin: Point2,
    tol: f64,
    points: Vec<P>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl Graph {
    fn new(origin: Point2, tol: Tolerance) -> Self {
        Self {
            origin,
            tol: tol.linear(),
            points: Vec::new(),
            cells: HashMap::new(),
        }
    }

    /// The index of `p`, a point of the plane.
    fn weld(&mut self, p: Point2) -> usize {
        self.weld_local([p.x() - self.origin.x(), p.y() - self.origin.y()])
    }

    /// The index of the welded point within the tolerance of `q`, in the
    /// working frame, a new one if none is.
    fn weld_local(&mut self, q: P) -> usize {
        // Cells twice the tolerance wide: a point within it is in one of the
        // nine around `q`'s. (Casts saturate, so far points still land.)
        let size = 2.0 * self.tol;
        let cell = ((q[0] / size).floor() as i64, (q[1] / size).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let key = (cell.0.saturating_add(dx), cell.1.saturating_add(dy));
                if let Some(found) = self.cells.get(&key) {
                    for &i in found {
                        let p = self.points[i];
                        if (p[0] - q[0]).hypot(p[1] - q[1]) <= self.tol {
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

    /// The contours chained from `edges`, directed and each used once.
    fn contours(&self, edges: Vec<(usize, usize)>) -> Region {
        let mut leaving: HashMap<usize, Vec<usize>> = HashMap::new();
        for (k, &(i, _)) in edges.iter().enumerate() {
            leaving.entry(i).or_default().push(k);
        }
        let direction = |k: usize| {
            let (i, j) = edges[k];
            let (p, q) = (self.points[i], self.points[j]);
            [q[0] - p[0], q[1] - p[1]]
        };
        let mut used = vec![false; edges.len()];
        let mut contours = Vec::new();
        for start in 0..edges.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let mut corners = vec![edges[start].0];
            let mut current = start;
            let closed = loop {
                let to = edges[current].1;
                let d = direction(current);
                // The leftmost turn among the unused edges leaving `to`, the
                // first edge too, closing the contour. (No edge leads straight
                // back: `cancel` netted such pairs out.)
                let next = leaving
                    .get(&to)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|&k| !used[k] || k == start);
                let next =
                    next.max_by(|&k, &l| turn(d, direction(k)).total_cmp(&turn(d, direction(l))));
                match next {
                    None => break false,
                    Some(k) if k == start => break true,
                    Some(k) => {
                        used[k] = true;
                        corners.push(to);
                        current = k;
                    }
                }
            };
            if closed {
                let contour = self.tidy(corners);
                if !contour.is_empty() {
                    contours.push(contour);
                }
            }
        }
        Region { contours }
    }

    /// `corners` without those on a straight run between their neighbours,
    /// back in the plane's coordinates; nothing if no area is left.
    fn tidy(&self, mut corners: Vec<usize>) -> Vec<Point2> {
        let t = self.tol;
        let mut changed = true;
        while changed && corners.len() >= 3 {
            changed = false;
            let n = corners.len();
            for k in 0..n {
                let (a, v, b) = (
                    self.points[corners[(k + n - 1) % n]],
                    self.points[corners[k]],
                    self.points[corners[(k + 1) % n]],
                );
                let ab = [b[0] - a[0], b[1] - a[1]];
                let along = (v[0] - a[0]) * ab[0] + (v[1] - a[1]) * ab[1];
                let between = along > 0.0 && along < ab[0] * ab[0] + ab[1] * ab[1];
                if between && distance_to_segment(v, a, b) <= t {
                    corners.remove(k);
                    changed = true;
                    break;
                }
            }
        }
        if corners.len() < 3 {
            return Vec::new();
        }
        let points: Vec<P> = corners.iter().map(|&i| self.points[i]).collect();
        let perimeter: f64 = (0..points.len())
            .map(|k| {
                let (p, q) = (points[k], points[(k + 1) % points.len()]);
                (q[0] - p[0]).hypot(q[1] - p[1])
            })
            .sum();
        // A sliver thinner than the tolerance encloses nothing.
        if signed_area(&points).abs() <= t * perimeter / 2.0 {
            return Vec::new();
        }
        points
            .into_iter()
            .map(|p| Point2::from_finite(p[0] + self.origin.x(), p[1] + self.origin.y()))
            .collect()
    }
}

/// `edges` with each edge's uses one way and the other netted out: a run
/// back and forth over the same edge bounds nothing.
fn cancel(edges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut net: HashMap<(usize, usize), i64> = HashMap::new();
    let mut order = Vec::new();
    for (i, j) in edges {
        let key = (i.min(j), i.max(j));
        let count = net.entry(key).or_insert_with(|| {
            order.push(key);
            0
        });
        *count += if i < j { 1 } else { -1 };
    }
    order
        .into_iter()
        .filter_map(|key| match net[&key] {
            0 => None,
            n if n > 0 => Some(key),
            _ => Some((key.1, key.0)),
        })
        .collect()
}

/// The pieces of the edges `own` split where an edge of `other` crosses
/// them, and where an end of any edge lies on them.
fn split(own: &[(P, P)], other: &[(P, P)], t: f64) -> Vec<(P, P)> {
    let mut out = Vec::with_capacity(own.len());
    for &(a, b) in own {
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = d[0].hypot(d[1]);
        if len <= t {
            continue;
        }
        let mut cuts = Vec::new();
        let inner = |s: f64| s * len > t && (1.0 - s) * len > t;
        for &(c, e) in other.iter().chain(own) {
            for q in [c, e] {
                let w = [q[0] - a[0], q[1] - a[1]];
                let s = (w[0] * d[0] + w[1] * d[1]) / (len * len);
                if inner(s) && cross(d, w).abs() / len <= t {
                    cuts.push(s);
                }
            }
        }
        for &(c, e) in other {
            let f = [e[0] - c[0], e[1] - c[1]];
            let flen = f[0].hypot(f[1]);
            let den = cross(d, f);
            if den.abs() <= 1e-12 * len * flen {
                continue;
            }
            let w = [c[0] - a[0], c[1] - a[1]];
            let (s, u) = (cross(w, f) / den, cross(w, d) / den);
            if inner(s) && u * flen > t && (1.0 - u) * flen > t {
                cuts.push(s);
            }
        }
        cuts.sort_by(f64::total_cmp);
        let mut from = a;
        for s in cuts {
            let to = [a[0] + s * d[0], a[1] + s * d[1]];
            out.push((from, to));
            from = to;
        }
        out.push((from, b));
    }
    out
}

/// The region's edges in a frame at `origin`.
fn segments(region: &Region, origin: Point2) -> Vec<(P, P)> {
    region
        .contours
        .iter()
        .flat_map(|c| closed(&local(c, origin)))
        .collect()
}

/// A contour's corners in a frame at `origin`.
fn local(contour: &[Point2], origin: Point2) -> Vec<P> {
    contour
        .iter()
        .map(|p| [p.x() - origin.x(), p.y() - origin.y()])
        .collect()
}

/// A contour's edges, the last back to the first corner.
fn closed(corners: &[P]) -> Vec<(P, P)> {
    (0..corners.len())
        .map(|k| (corners[k], corners[(k + 1) % corners.len()]))
        .collect()
}

/// Smallest and largest coordinates of the edges: `[x0, y0, x1, y1]`.
fn bounds(segments: &[(P, P)]) -> Option<[f64; 4]> {
    let mut points = segments.iter().map(|s| s.0);
    let first = points.next()?;
    Some(
        points.fold([first[0], first[1], first[0], first[1]], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        }),
    )
}

/// The signed area of a contour, positive counterclockwise.
fn signed_area(corners: &[P]) -> f64 {
    let n = corners.len();
    (0..n)
        .map(|k| cross(corners[k], corners[(k + 1) % n]))
        .sum::<f64>()
        / 2.0
}

/// How many times the edges wind counterclockwise around `p`.
fn winding(p: P, segments: &[(P, P)]) -> i32 {
    let mut w = 0;
    for &(a, b) in segments {
        let left = cross([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
        if a[1] <= p[1] {
            if b[1] > p[1] && left > 0.0 {
                w += 1;
            }
        } else if b[1] <= p[1] && left < 0.0 {
            w -= 1;
        }
    }
    w
}

/// A point of a hole's edges clear of the outside contours, to tell
/// which is around it (the first edge's middle if none is clear).
fn probe_point<'a>(
    hole: &[P],
    outsides: impl Iterator<Item = &'a Vec<P>> + Clone,
    tol: Tolerance,
) -> P {
    let middle = |(p, q): (P, P)| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    let edges = closed(hole);
    edges
        .iter()
        .map(|&e| middle(e))
        .find(|&m| {
            outsides.clone().all(|o| {
                closed(o)
                    .iter()
                    .all(|&(a, b)| distance_to_segment(m, a, b) > tol.linear())
            })
        })
        .unwrap_or_else(|| middle(edges[0]))
}

/// The distance from `p` to the segment from `a` to `b`.
fn distance_to_segment(p: P, a: P, b: P) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let w = [p[0] - a[0], p[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let s = if len2 > 0.0 {
        ((w[0] * d[0] + w[1] * d[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (w[0] - s * d[0]).hypot(w[1] - s * d[1])
}

/// The turn from direction `d` to direction `e`, in radians, positive to
/// the left.
fn turn(d: P, e: P) -> f64 {
    cross(d, e).atan2(d[0] * e[0] + d[1] * e[1])
}

fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

#[cfg(test)]
#[path = "region_tests.rs"]
mod tests;
