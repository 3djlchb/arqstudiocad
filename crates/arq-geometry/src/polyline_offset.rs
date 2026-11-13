//! OFFSET's polylines: a path of straight segments and arcs moved a
//! distance square to itself, its segments extended or trimmed to meet as
//! AutoCAD's OFFSETGAPTYPE 0 does, those that turn round dropped.

use std::f64::consts::{PI, TAU};

use crate::arc::Arc;
use crate::crossings::{circle_circle, line_circle, line_line};
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::tolerance::Tolerance;

/// One segment of a path, as offsetting reshapes it.
#[derive(Clone, Copy, Debug)]
enum Segment {
    Line {
        a: Point2,
        b: Point2,
    },
    /// An arc from its start angle through `sweep` (signed: positive
    /// counterclockwise).
    Arc {
        c: Point2,
        r: f64,
        from: f64,
        sweep: f64,
    },
}

impl Segment {
    fn start(self) -> Point2 {
        match self {
            Self::Line { a, .. } => a,
            Self::Arc { c, r, from, .. } => at(c, r, from),
        }
    }

    fn end(self) -> Point2 {
        match self {
            Self::Line { b, .. } => b,
            Self::Arc { c, r, from, sweep } => at(c, r, from + sweep),
        }
    }

    /// The bulge a polyline vertex stores for it.
    fn bulge(self) -> f64 {
        match self {
            Self::Line { .. } => 0.0,
            Self::Arc { sweep, .. } => (sweep / 4.0).tan(),
        }
    }

    /// The point of the segment closest to `p`, and on which side of it
    /// `p` lies: 1 to its left as it runs, -1 to its right, 0 on its line
    /// or circle.
    fn side(self, p: Point2, tol: Tolerance) -> (f64, f64) {
        match self {
            Self::Line { a, b } => {
                let d = b - a;
                let t = ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
                let q = Point2::new(a.x() + d.x() * t, a.y() + d.y() * t).unwrap_or(a);
                let across = d.cross(p - a) / d.length();
                let side = if across.abs() <= tol.linear() {
                    0.0
                } else {
                    across.signum()
                };
                (q.distance_to(p), side)
            }
            Self::Arc { c, r, from, sweep } => {
                let to_p = p.distance_to(c);
                let angle = (p - c).angle();
                let on = within(from, sweep, angle);
                let distance = if on {
                    (to_p - r).abs()
                } else {
                    self.start().distance_to(p).min(self.end().distance_to(p))
                };
                // Counterclockwise, the left is toward the center.
                let inside = to_p < r;
                let side = if (to_p - r).abs() <= tol.linear() {
                    0.0
                } else if inside == (sweep > 0.0) {
                    1.0
                } else {
                    -1.0
                };
                (distance, side)
            }
        }
    }

    /// How far `p` is from the segment's line or circle (OFFSET's Through).
    fn reach(self, p: Point2) -> f64 {
        match self {
            Self::Line { a, b } => {
                let d = b - a;
                (d.cross(p - a) / d.length()).abs()
            }
            Self::Arc { c, r, .. } => (p.distance_to(c) - r).abs(),
        }
    }

    /// The segment moved `d` to its left (negative: to its right); None
    /// when an arc would shrink to nothing.
    fn shifted(self, d: f64, tol: Tolerance) -> Option<Self> {
        match self {
            Self::Line { a, b } => {
                let left = (b - a).normalized().ok()?.perp() * d;
                Some(Self::Line {
                    a: a + left,
                    b: b + left,
                })
            }
            Self::Arc { c, r, from, sweep } => {
                // Left of a counterclockwise arc is toward its center.
                let r = if sweep > 0.0 { r - d } else { r + d };
                (r > tol.linear()).then_some(Self::Arc { c, r, from, sweep })
            }
        }
    }

    /// Whether trimming turned the segment round against `original`.
    fn reversed(self, original: Self, tol: Tolerance) -> bool {
        match (self, original) {
            (Self::Line { a, b }, Self::Line { a: a0, b: b0 }) => {
                (b - a).dot(b0 - a0) <= tol.linear() * (b0 - a0).length()
            }
            (Self::Arc { sweep, .. }, Self::Arc { sweep: s0, .. }) => {
                sweep * s0 <= 0.0 || sweep.abs() > s0.abs() + PI
            }
            _ => false,
        }
    }

    /// The segment with its end moved to `p`, which lies on its line or
    /// circle.
    fn with_end(self, p: Point2) -> Self {
        match self {
            Self::Line { a, .. } => Self::Line { a, b: p },
            Self::Arc { c, r, from, sweep } => Self::Arc {
                c,
                r,
                from,
                sweep: turn(from, (p - c).angle(), sweep),
            },
        }
    }

    /// The segment with its start moved to `p`, on its line or circle.
    fn with_start(self, p: Point2) -> Self {
        match self {
            Self::Line { b, .. } => Self::Line { a: p, b },
            Self::Arc { c, r, from, sweep } => {
                let end = from + sweep;
                let start = (p - c).angle();
                Self::Arc {
                    c,
                    r,
                    from: start,
                    sweep: turn(start, end, sweep),
                }
            }
        }
    }
}

fn at(c: Point2, r: f64, angle: f64) -> Point2 {
    let (s, co) = angle.sin_cos();
    Point2::new(c.x() + r * co, c.y() + r * s).unwrap_or(c)
}

/// The signed turn from angle `from` to `to` the way `like` turns, in
/// `(-2π, 2π)`.
fn turn(from: f64, to: f64, like: f64) -> f64 {
    let ccw = (to - from).rem_euclid(TAU);
    if like > 0.0 { ccw } else { ccw - TAU }
}

/// Whether `angle` lies on the arc from `from` through `sweep`.
fn within(from: f64, sweep: f64, angle: f64) -> bool {
    let t = turn(from, angle, sweep);
    t.abs() <= sweep.abs()
}

/// The polyline offset `distance` toward the side `toward` lies on,
/// AutoCAD's way: every segment moved square to itself (arcs about their
/// centers), each pair meeting where their lines or circles cross, gaps
/// at outer corners filled by extending them (OFFSETGAPTYPE 0), segments
/// that turn round dropped and their neighbours made to meet instead.
/// Vertices as a polyline stores them, each with the bulge of the segment
/// leaving it. Refused when `toward` lies on the path (no side), for a bad
/// distance, and when nothing would be left. A path crossing itself far
/// from a corner after the offset is kept as it comes.
pub fn offset_polyline(
    vertices: &[(Point2, f64)],
    closed: bool,
    distance: f64,
    toward: Point2,
    tol: Tolerance,
) -> GeometryResult<Vec<(Point2, f64)>> {
    if !(distance.is_finite() && distance > 0.0) {
        return Err(GeometryError::NegativeOrNonFinite {
            what: "offset distance",
        });
    }
    let path = segments(vertices, closed, tol)?;
    let side = side_of(&path, toward, tol)?;
    offset_by(&path, closed, side * distance, tol)
}

/// The polyline offset to pass through `through` (OFFSET's Through): the
/// distance is how far `through` is from the line or circle of the segment
/// nearest it.
pub fn offset_polyline_through(
    vertices: &[(Point2, f64)],
    closed: bool,
    through: Point2,
    tol: Tolerance,
) -> GeometryResult<Vec<(Point2, f64)>> {
    let path = segments(vertices, closed, tol)?;
    let nearest = path
        .iter()
        .min_by(|a, b| a.side(through, tol).0.total_cmp(&b.side(through, tol).0))
        .ok_or(GeometryError::ShortContour)?;
    let distance = nearest.reach(through);
    if distance <= tol.linear() {
        return Err(GeometryError::OutOfRange { what: "side" });
    }
    let side = side_of(&path, through, tol)?;
    offset_by(&path, closed, side * distance, tol)
}

/// The path's segments; refused with fewer than one, or one of no length.
fn segments(
    vertices: &[(Point2, f64)],
    closed: bool,
    tol: Tolerance,
) -> GeometryResult<Vec<Segment>> {
    let n = vertices.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    if count == 0 {
        return Err(GeometryError::ShortContour);
    }
    (0..count)
        .map(|i| {
            let ((a, bulge), b) = (vertices[i], vertices[(i + 1) % n].0);
            if a.approx_eq(b, tol) {
                return Err(GeometryError::ZeroLength);
            }
            if !bulge.is_finite() {
                return Err(GeometryError::NonFinite { what: "bulge" });
            }
            if bulge == 0.0 {
                return Ok(Segment::Line { a, b });
            }
            let (arc, _) = Arc::from_bulge(a, b, bulge)?;
            let (c, r) = (arc.center(), arc.radius());
            Ok(Segment::Arc {
                c,
                r,
                from: (a - c).angle(),
                sweep: 4.0 * bulge.atan(),
            })
        })
        .collect()
}

/// 1 when `p` is left of the path where it comes nearest, -1 to its right.
fn side_of(path: &[Segment], p: Point2, tol: Tolerance) -> GeometryResult<f64> {
    let (_, side) = path
        .iter()
        .map(|s| s.side(p, tol))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .ok_or(GeometryError::ShortContour)?;
    if side == 0.0 {
        Err(GeometryError::OutOfRange { what: "side" })
    } else {
        Ok(side)
    }
}

/// The path moved `d` to its left, joined, cleaned of segments that turned
/// round.
fn offset_by(
    path: &[Segment],
    closed: bool,
    d: f64,
    tol: Tolerance,
) -> GeometryResult<Vec<(Point2, f64)>> {
    // Each kept segment: its index in the path, and its offset.
    let mut kept: Vec<(usize, Segment)> = path
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.shifted(d, tol).map(|o| (i, o)))
        .collect();
    loop {
        if kept.is_empty() {
            return Err(GeometryError::OutOfRange {
                what: "offset distance",
            });
        }
        let (trimmed, bridges) = join(&kept, closed, tol);
        let turned: Vec<usize> = (0..trimmed.len())
            .filter(|&k| trimmed[k].reversed(kept[k].1, tol))
            .collect();
        if turned.is_empty() {
            let mut joined = trimmed;
            // Bridges go after the segment they leave, last first.
            for (i, bridge) in bridges.into_iter().rev() {
                joined.insert(i + 1, bridge);
            }
            // A closed path offset inward too far turns inside out or flat:
            // nothing is left of it.
            if closed {
                let before = area(path);
                let after = area(&joined);
                if after * before <= 0.0 || after.abs() <= tol.linear() * tol.linear() {
                    return Err(GeometryError::OutOfRange {
                        what: "offset distance",
                    });
                }
            }
            return Ok(vertices_of(&joined, closed));
        }
        // Drop what turned round and join the rest again.
        let mut k = 0;
        kept.retain(|_| {
            let keep = !turned.contains(&k);
            k += 1;
            keep
        });
    }
}

/// The segments with each pair of neighbours meeting at the crossing of
/// their lines or circles nearest the gap between them; and, where they
/// never cross, a straight bridge to go after segment `i`.
type Bridges = Vec<(usize, Segment)>;

fn join(kept: &[(usize, Segment)], closed: bool, tol: Tolerance) -> (Vec<Segment>, Bridges) {
    let mut out: Vec<Segment> = kept.iter().map(|&(_, s)| s).collect();
    let n = out.len();
    let joints = if closed { n } else { n.saturating_sub(1) };
    let mut bridges: Bridges = Vec::new();
    for i in 0..joints {
        let j = (i + 1) % n;
        if i == j {
            continue;
        }
        let (a, b) = (out[i], out[j]);
        if a.end().approx_eq(b.start(), tol) {
            continue;
        }
        let gap = mid(a.end(), b.start());
        match crossing(a, b, gap, tol) {
            Some(x) => {
                out[i] = a.with_end(x);
                out[j] = out[j].with_start(x);
            }
            None => bridges.push((
                i,
                Segment::Line {
                    a: a.end(),
                    b: b.start(),
                },
            )),
        }
    }
    (out, bridges)
}

/// The signed area a closed path of segments encloses, its arcs'
/// segments included: positive counterclockwise.
fn area(path: &[Segment]) -> f64 {
    path.iter()
        .map(|&s| {
            let (a, b) = (s.start(), s.end());
            let chord = (a.x() * b.y() - b.x() * a.y()) / 2.0;
            match s {
                Segment::Line { .. } => chord,
                // The segment between the arc and its chord, on the arc's
                // side: r²/2 (θ − sin θ), signed as the sweep.
                Segment::Arc { r, sweep, .. } => chord + r * r / 2.0 * (sweep - sweep.sin()),
            }
        })
        .sum()
}

fn mid(a: Point2, b: Point2) -> Point2 {
    Point2::new((a.x() + b.x()) / 2.0, (a.y() + b.y()) / 2.0).unwrap_or(a)
}

/// Where the lines or circles of `a` and `b` cross, nearest `near`.
fn crossing(a: Segment, b: Segment, near: Point2, tol: Tolerance) -> Option<Point2> {
    let points = match (a, b) {
        (Segment::Line { a: p, b: q }, Segment::Line { a: r, b: s }) => {
            line_line(p, q - p, r, s - r)
                .into_iter()
                .collect::<Vec<_>>()
        }
        (Segment::Line { a: p, b: q }, Segment::Arc { c, r, .. })
        | (Segment::Arc { c, r, .. }, Segment::Line { a: p, b: q }) => line_circle(p, q - p, c, r),
        (Segment::Arc { c: c1, r: r1, .. }, Segment::Arc { c: c2, r: r2, .. }) => {
            circle_circle(c1, r1, c2, r2, tol)
        }
    };
    points
        .into_iter()
        .min_by(|x, y| x.distance_to(near).total_cmp(&y.distance_to(near)))
}

/// The vertices of joined segments: each start with its bulge, and the last
/// end too when the path is open.
fn vertices_of(joined: &[Segment], closed: bool) -> Vec<(Point2, f64)> {
    let mut out: Vec<(Point2, f64)> = joined.iter().map(|s| (s.start(), s.bulge())).collect();
    if !closed && let Some(last) = joined.last() {
        out.push((last.end(), 0.0));
    }
    out
}

#[cfg(test)]
#[path = "polyline_offset_tests.rs"]
mod tests;
