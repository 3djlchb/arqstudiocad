//! JOIN, as AutoCAD's: collinear lines into one line, arcs of one circle
//! into one arc, or the circle when they go all the way round, gaps
//! between them allowed and filled; and lines, arcs and polylines whose
//! ends touch chained into one polyline path (milestone 02.14,
//! `docs/design/join-command.md`).

use std::f64::consts::TAU;

use crate::angle;
use crate::arc::Arc;
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;

/// An open path JOIN chains: its vertices in order, each with the bulge of
/// the segment leaving it (0 for a straight one, positive counterclockwise,
/// 0 on the last vertex), and each segment's start and end widths.
#[derive(Clone, Debug, PartialEq)]
pub struct JoinPath {
    /// The vertices and the bulge of the segment leaving each.
    pub vertices: Vec<(Point2, f64)>,
    /// Each segment's start and end width, one fewer than the vertices.
    pub widths: Vec<(f64, f64)>,
}

impl JoinPath {
    /// A line's path, from its start to its end, with no width.
    pub fn line(line: Line) -> Self {
        Self {
            vertices: vec![(line.start(), 0.0), (line.end(), 0.0)],
            widths: vec![(0.0, 0.0)],
        }
    }

    /// An arc's path, counterclockwise from its start to its end, with no
    /// width.
    pub fn arc(arc: Arc) -> Self {
        let bulge = (arc.sweep() / 4.0).tan();
        Self {
            vertices: vec![(arc.start_point(), bulge), (arc.end_point(), 0.0)],
            widths: vec![(0.0, 0.0)],
        }
    }

    /// The first vertex.
    pub fn start(&self) -> Point2 {
        self.vertices[0].0
    }

    /// The last vertex.
    pub fn end(&self) -> Point2 {
        self.vertices[self.vertices.len() - 1].0
    }

    /// The same path the other way: the vertices reversed, each arc turning
    /// the other way, each segment's widths swapped.
    pub fn reversed(&self) -> Self {
        let n = self.vertices.len();
        let vertices = (0..n)
            .map(|k| {
                let point = self.vertices[n - 1 - k].0;
                let bulge = if k + 1 < n {
                    -self.vertices[n - 2 - k].1
                } else {
                    0.0
                };
                (point, bulge)
            })
            .collect();
        let widths = self.widths.iter().rev().map(|&(a, b)| (b, a)).collect();
        Self { vertices, widths }
    }

    /// `next`, which starts where this path ends, added after it.
    fn append(&mut self, next: &Self) {
        let last = self.vertices.len() - 1;
        self.vertices[last].1 = next.vertices[0].1;
        self.vertices.extend_from_slice(&next.vertices[1..]);
        self.widths.extend_from_slice(&next.widths);
    }
}

/// What [`chain_paths`] made: the path, whether it came back to its start
/// (then closed, without its last vertex, the closing segment the last
/// one), and the indices of the others it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    /// The chained path.
    pub path: JoinPath,
    /// Whether it closed on itself.
    pub closed: bool,
    /// Which of the others it took, in the order taken.
    pub taken: Vec<usize>,
}

/// `first` with every one of `others` whose ends touch its own chained on,
/// at its end or before its start, turned round when it runs the other
/// way, as AutoCAD's JOIN chains a polyline: each time the earliest of
/// `others` that touches, until none does. Branches beyond the first one
/// at a point are left out. A chain that comes back to its start closes.
pub fn chain_paths(first: &JoinPath, others: &[JoinPath], tol: Tolerance) -> Chain {
    let touch = |a: Point2, b: Point2| a.distance_to(b) <= tol.linear();
    let mut path = first.clone();
    let mut taken = Vec::new();
    let mut free = vec![true; others.len()];
    loop {
        let mut grew = false;
        for (i, other) in others.iter().enumerate() {
            if !free[i] || touch(path.start(), path.end()) && !taken.is_empty() {
                continue;
            }
            let attached = if touch(path.end(), other.start()) {
                path.append(other);
                true
            } else if touch(path.end(), other.end()) {
                path.append(&other.reversed());
                true
            } else if touch(path.start(), other.end()) {
                let mut before = other.clone();
                before.append(&path);
                path = before;
                true
            } else if touch(path.start(), other.start()) {
                let mut before = other.reversed();
                before.append(&path);
                path = before;
                true
            } else {
                false
            };
            if attached {
                free[i] = false;
                taken.push(i);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let closed = !taken.is_empty() && path.widths.len() >= 2 && touch(path.start(), path.end());
    if closed {
        path.vertices.pop();
    }
    Chain {
        path,
        closed,
        taken,
    }
}

/// The line `source` becomes joined with every line of `others` on its
/// infinite line, gaps allowed: from the farthest end behind its start to
/// the farthest beyond its end, in its direction. Also the indices of the
/// `others` it took; the rest are not collinear with it. The ends are the
/// lines' own points, never recomputed, so a line joined with nothing is
/// returned unchanged.
pub fn join_lines(source: Line, others: &[Line], tol: Tolerance) -> (Line, Vec<usize>) {
    let joined: Vec<usize> = (0..others.len())
        .filter(|&i| collinear(source, others[i], tol))
        .collect();
    let direction = source.direction();
    let along = |q: crate::Point2| (q - source.start()).dot(direction);
    let (mut first, mut last) = (source.start(), source.end());
    let (mut low, mut high) = (0.0, along(source.end()));
    for &i in &joined {
        for q in [others[i].start(), others[i].end()] {
            let t = along(q);
            if t < low {
                (low, first) = (t, q);
            } else if t > high {
                (high, last) = (t, q);
            }
        }
    }
    let line = Line::new(first, last).expect("its ends are a line's ends, apart");
    (line, joined)
}

/// The curve `source` becomes joined with every arc of `others` on its
/// circle, gaps allowed: counterclockwise from its start to the farthest
/// end of any of them, as AutoCAD joins arcs. The circle when that goes all
/// the way round. Also the indices of the `others` it took; the rest lie on
/// another circle. An arc joined with nothing, or with arcs inside it, is
/// returned unchanged.
pub fn join_arcs(source: Arc, others: &[Arc], tol: Tolerance) -> (Geometry2, Vec<usize>) {
    let joined: Vec<usize> = (0..others.len())
        .filter(|&i| same_circle(source, others[i], tol))
        .collect();
    let slack = angular_slack(source, tol);
    let start = source.start_angle();
    let mut reach = source.sweep();
    for &i in &joined {
        let arc = others[i];
        let mut from = angle::ccw_sweep(start, arc.start_angle());
        if from > TAU - slack {
            from -= TAU; // starts with the source, a rounding before it
        }
        reach = reach.max(from + arc.sweep());
    }
    let curve = if reach >= TAU - slack {
        Geometry2::Circle(source.circle())
    } else if reach == source.sweep() {
        Geometry2::Arc(source)
    } else {
        let arc = Arc::new(source.center(), source.radius(), start, start + reach)
            .expect("the source's circle and a sweep short of a turn");
        Geometry2::Arc(arc)
    };
    (curve, joined)
}

/// JOIN's groups when many objects are joined at once, as AutoCAD makes
/// them: collinear lines together, arcs of one circle together. Each group
/// lists indices into `curves` in selection order, the first being the
/// object its result is joined from; a group has two curves at least. A
/// curve in no group joins nothing (a circle never does).
pub fn join_groups(curves: &[Geometry2], tol: Tolerance) -> Vec<Vec<usize>> {
    let mut taken = vec![false; curves.len()];
    let mut groups = Vec::new();
    for first in 0..curves.len() {
        if taken[first] {
            continue;
        }
        let mut group = vec![first];
        for other in first + 1..curves.len() {
            if taken[other] {
                continue;
            }
            let together = match (curves[first], curves[other]) {
                (Geometry2::Line(a), Geometry2::Line(b)) => collinear(a, b, tol),
                (Geometry2::Arc(a), Geometry2::Arc(b)) => same_circle(a, b, tol),
                _ => false,
            };
            if together {
                group.push(other);
            }
        }
        if group.len() > 1 {
            for &i in &group {
                taken[i] = true;
            }
            groups.push(group);
        }
    }
    groups
}

/// Whether both ends of `other` lie on `line`'s infinite line.
fn collinear(line: Line, other: Line, tol: Tolerance) -> bool {
    let direction = line.direction();
    [other.start(), other.end()]
        .into_iter()
        .all(|q| (q - line.start()).cross(direction).abs() <= tol.linear())
}

/// Whether `other` lies on `arc`'s circle: the same center and radius.
fn same_circle(arc: Arc, other: Arc, tol: Tolerance) -> bool {
    arc.center().distance_to(other.center()) <= tol.linear()
        && (arc.radius() - other.radius()).abs() <= tol.linear()
}

/// The angle the linear tolerance spans on `arc`'s circle, at least the
/// angular tolerance: ends that close are the same end.
fn angular_slack(arc: Arc, tol: Tolerance) -> f64 {
    (tol.linear() / arc.radius()).max(tol.angular())
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod tests;
