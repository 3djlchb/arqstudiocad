//! FILLET and CHAMFER's polylines: the corners where two straight
//! segments meet rounded by an arc or cut by a segment, written into the
//! polyline itself, as AutoCAD's Polyline option does.

use std::f64::consts::PI;

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;

/// How a polyline's corners are cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerCut {
    /// Rounded by an arc of this radius.
    Fillet(f64),
    /// Cut by a segment from the first distance back along the segment
    /// coming in to the second along the one going out.
    Chamfer(f64, f64),
    /// Cut by a segment from this length back along the segment coming in,
    /// leaving it at this angle (radians).
    ChamferAngle(f64, f64),
}

/// A polyline with its corners cut, and how many were and were not.
#[derive(Clone, Debug, PartialEq)]
pub struct CutCorners {
    /// The vertices, each with the bulge of the segment leaving it.
    pub vertices: Vec<(Point2, f64)>,
    /// The corners cut.
    pub done: usize,
    /// The corners whose segments were too short for the cut.
    pub too_short: usize,
}

/// The polyline's corners cut by `cut`: every corner between two straight
/// segments (round the start too when `closed`), or only the one at vertex
/// `only`. A corner whose cut would take more of a segment than the
/// segment, with what the corner before took, is left and counted too
/// short; corners in line and those next to arcs are left alone.
pub fn cut_polyline_corners(
    vertices: &[(Point2, f64)],
    closed: bool,
    cut: CornerCut,
    only: Option<usize>,
    tol: Tolerance,
) -> GeometryResult<CutCorners> {
    let n = vertices.len();
    let segments = if closed { n } else { n.saturating_sub(1) };
    if segments == 0 {
        return Err(GeometryError::ShortContour);
    }
    for value in match cut {
        CornerCut::Fillet(r) => [r, 0.0],
        CornerCut::Chamfer(a, b) | CornerCut::ChamferAngle(a, b) => [a, b],
    } {
        if !(value.is_finite() && value >= 0.0) {
            return Err(GeometryError::NegativeOrNonFinite { what: "corner" });
        }
    }
    let point = |i: usize| vertices[i % n].0;
    let length = |s: usize| point(s).distance_to(point(s + 1));
    // How much of each segment the corners at its ends take: start, end.
    let mut taken = vec![(0.0f64, 0.0f64); segments];
    // Each corner cut: the vertex, where its cut starts and ends, the
    // bulge across it.
    let mut cuts: Vec<(usize, Point2, Point2, f64)> = Vec::new();
    let mut too_short = 0;
    let corners: Vec<usize> = match only {
        Some(i) => vec![i],
        None if closed => (0..n).collect(),
        None => (1..n.saturating_sub(1)).collect(),
    };
    for i in corners {
        if i >= n || (!closed && (i == 0 || i == n - 1)) {
            return Err(GeometryError::OutOfRange { what: "corner" });
        }
        let (before, after) = ((i + n - 1) % n, i);
        // Only between two straight segments.
        if vertices[before].1 != 0.0 || vertices[after].1 != 0.0 {
            continue;
        }
        let (prev, here, next) = (point(before), point(i), point(i + 1));
        let (Ok(u_in), Ok(u_out)) = ((here - prev).normalized(), (next - here).normalized()) else {
            continue;
        };
        let turn = u_in.cross(u_out);
        // The angle the path turns through here; nothing in line.
        let deflection = u_in.dot(u_out).clamp(-1.0, 1.0).acos();
        if deflection <= 1e-9 || deflection >= PI - 1e-9 {
            continue;
        }
        let (back, on, bulge) = match cut {
            CornerCut::Fillet(r) => {
                if r <= tol.linear() {
                    continue;
                }
                let t = r * (deflection / 2.0).tan();
                (t, t, turn.signum() * (deflection / 4.0).tan())
            }
            CornerCut::Chamfer(a, b) => (a, b, 0.0),
            CornerCut::ChamferAngle(length, angle) => {
                // The triangle the two segments and the cut make: its angle
                // at the corner is the interior one.
                let interior = PI - deflection;
                let opposite = PI - angle - interior;
                if !(angle > 0.0 && opposite > 1e-12) {
                    too_short += 1;
                    continue;
                }
                (length, length * angle.sin() / opposite.sin(), 0.0)
            }
        };
        if back <= tol.linear() && on <= tol.linear() {
            continue;
        }
        let fits = taken[before].0 + back <= length(before) + tol.linear()
            && taken[after].1 + on <= length(after) + tol.linear();
        if !fits {
            too_short += 1;
            continue;
        }
        taken[before].1 += back;
        taken[after].0 += on;
        cuts.push((i, here - u_in * back, here + u_out * on, bulge));
    }
    let mut out: Vec<(Point2, f64)> = Vec::with_capacity(n + cuts.len());
    for (i, &(p, b)) in vertices.iter().enumerate() {
        match cuts.iter().find(|c| c.0 == i) {
            Some(&(_, from, to, bulge)) => {
                push(&mut out, from, bulge, tol);
                push(&mut out, to, b, tol);
            }
            None => push(&mut out, p, b, tol),
        }
    }
    if closed && out.len() > 1 && out[out.len() - 1].0.approx_eq(out[0].0, tol) {
        out.pop();
    }
    Ok(CutCorners {
        vertices: out,
        done: cuts.len(),
        too_short,
    })
}

/// Adds a vertex, unless it lies on the last one: then the last one takes
/// its bulge, so no segment has no length.
fn push(out: &mut Vec<(Point2, f64)>, p: Point2, bulge: f64, tol: Tolerance) {
    match out.last_mut() {
        Some(last) if last.0.approx_eq(p, tol) => last.1 = bulge,
        _ => out.push((p, bulge)),
    }
}

/// The vertex two segments of a path share, `a` and `b` being their
/// indices (the segment leaving vertex `i` is `i`); None when they are not
/// next to each other.
pub fn shared_vertex(a: usize, b: usize, segments: usize, closed: bool) -> Option<usize> {
    let (lo, hi) = (a.min(b), a.max(b));
    if hi == lo + 1 {
        Some(hi)
    } else if closed && lo == 0 && hi + 1 == segments {
        Some(0)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "polyline_corners_tests.rs"]
mod tests;
