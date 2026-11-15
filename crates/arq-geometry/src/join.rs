//! JOIN, as AutoCAD's: collinear lines into one line, arcs of one circle
//! into one arc, or the circle when they go all the way round (milestone
//! 02.14, `docs/design/join-command.md`). Gaps between them are allowed,
//! and filled.

use std::f64::consts::TAU;

use crate::angle;
use crate::arc::Arc;
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::tolerance::Tolerance;

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
