//! The area a wide polyline covers (ADR 0038): each segment's band, its
//! width growing or shrinking along it, as triangles to fill.

use std::f64::consts::TAU;

use crate::arc::Arc;
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// Steps an arc's band takes per full turn: the space views' circle
/// (`drawing.rs`, `CIRCLE_SEGMENTS`), fine to a sagitta of 0.03% of the
/// radius.
const TURN_STEPS: f64 = 128.0;

/// A miter longer than this many half-widths is not drawn: the two bands
/// meet at the vertex as they end, as at a sharp turn AutoCAD's would.
const MITER_LIMIT: f64 = 8.0;

/// One vertex of a wide path: where it is, the bulge of the segment
/// leaving it (0 straight, positive counterclockwise) and that segment's
/// width at its start and at its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandVertex {
    /// The vertex.
    pub point: Point2,
    /// The bulge of the segment leaving it.
    pub bulge: f64,
    /// The segment's width where it leaves the vertex.
    pub start_width: f64,
    /// The segment's width where it reaches the next vertex.
    pub end_width: f64,
}

/// One side of a band at a point along it: the edge to the left of the
/// path, then the one to its right.
type Rung = (Point2, Point2);

/// The triangles that fill the band of a polyline path with widths, as
/// AutoCAD draws a wide polyline: straight segments as quadrilaterals,
/// arcs as rings stepped finely, and where two straight segments of the
/// same width meet, their edges extended to meet (a miter), round the
/// last vertex back to the first when `closed`. Segments of no width, of
/// no length, or whose widths are not finite, cover nothing.
pub fn band(vertices: &[BandVertex], closed: bool, tol: Tolerance) -> Vec<[Point2; 3]> {
    let n = vertices.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    // Each segment's rungs, from its start to its end; None when it covers
    // nothing.
    let mut strips: Vec<Option<(Vec<Rung>, bool)>> = (0..count)
        .map(|i| strip(vertices[i], vertices[(i + 1) % n].point, tol))
        .collect();
    // Miters where two straight strips of one width meet.
    let joints = if closed {
        count
    } else {
        count.saturating_sub(1)
    };
    for i in 0..joints {
        let j = (i + 1) % count;
        if i == j {
            continue;
        }
        let same_width = (vertices[i].end_width - vertices[j].start_width).abs() <= tol.linear();
        let (Some((before, true)), Some((after, true))) = (&strips[i], &strips[j]) else {
            continue;
        };
        if !same_width {
            continue;
        }
        let (Some(left), Some(right)) = (
            meet(before[0].0, before[1].0, after[0].0, after[1].0),
            meet(before[0].1, before[1].1, after[0].1, after[1].1),
        ) else {
            continue; // in line: they meet already
        };
        let half = vertices[j].start_width / 2.0;
        let corner = vertices[j].point;
        if corner.distance_to(left) > MITER_LIMIT * half
            || corner.distance_to(right) > MITER_LIMIT * half
        {
            continue;
        }
        if let Some((rungs, _)) = &mut strips[i] {
            rungs[1] = (left, right);
        }
        if let Some((rungs, _)) = &mut strips[j] {
            rungs[0] = (left, right);
        }
    }
    let mut out = Vec::new();
    for (rungs, _) in strips.into_iter().flatten() {
        for pair in rungs.windows(2) {
            let ((l0, r0), (l1, r1)) = (pair[0], pair[1]);
            for t in [[r0, r1, l1], [r0, l1, l0]] {
                if area2(t).abs() > tol.linear() * tol.linear() {
                    out.push(t);
                }
            }
        }
    }
    out
}

/// One segment's rungs from `v` to `end`, and whether it is straight.
fn strip(v: BandVertex, end: Point2, tol: Tolerance) -> Option<(Vec<Rung>, bool)> {
    let (w0, w1) = (v.start_width, v.end_width);
    let finite = w0.is_finite() && w1.is_finite() && v.bulge.is_finite();
    if !finite || (w0 <= 0.0 && w1 <= 0.0) || v.point.approx_eq(end, tol) {
        return None;
    }
    let (w0, w1) = (w0.max(0.0), w1.max(0.0));
    if v.bulge == 0.0 {
        let along = (end - v.point).normalized().ok()?;
        let left = along.perp();
        let rung = |p: Point2, w: f64| (p + left * (w / 2.0), p - left * (w / 2.0));
        return Some((vec![rung(v.point, w0), rung(end, w1)], true));
    }
    let (arc, _) = Arc::from_bulge(v.point, end, v.bulge).ok()?;
    let (c, r) = (arc.center(), arc.radius());
    let sweep = 4.0 * v.bulge.atan();
    let from = (v.point - c).angle();
    let steps = ((sweep.abs() / (TAU / TURN_STEPS)).ceil() as usize).max(1);
    // Counterclockwise the path's left is toward the center; clockwise, away.
    let inward = if sweep > 0.0 { 1.0 } else { -1.0 };
    let rungs = (0..=steps)
        .map(|k| {
            let t = k as f64 / steps as f64;
            let w = w0 + (w1 - w0) * t;
            let a = from + sweep * t;
            let out = Vector2::from_polar(1.0, a).expect("finite angle");
            let at = |radius: f64| c + out * radius.max(0.0);
            (at(r - inward * w / 2.0), at(r + inward * w / 2.0))
        })
        .collect();
    Some((rungs, false))
}

/// Where the line through `a0`, `a1` meets the one through `b0`, `b1`;
/// None when they run alike.
fn meet(a0: Point2, a1: Point2, b0: Point2, b1: Point2) -> Option<Point2> {
    let (da, db) = (a1 - a0, b1 - b0);
    let cross = da.cross(db);
    let scale = da.length() * db.length();
    if scale == 0.0 || cross.abs() <= 1e-12 * scale {
        return None;
    }
    let t = (b0 - a0).cross(db) / cross;
    Point2::new(a0.x() + da.x() * t, a0.y() + da.y() * t).ok()
}

/// Twice a triangle's signed area.
fn area2([a, b, c]: [Point2; 3]) -> f64 {
    (b - a).cross(c - a)
}

#[cfg(test)]
#[path = "band_tests.rs"]
mod tests;
