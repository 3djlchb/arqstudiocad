//! TRIM and EXTEND's polylines: a path of straight segments and arcs cut
//! back to the edges nearest a pick, or its end segment grown to an edge.
//!
//! A point of the path is placed by a parameter: the index of its segment
//! plus how far along that segment it is (0 at its start, 1 at its end; an
//! arc by the share of its turn).

use crate::arc::Arc;
use crate::error::{GeometryError, GeometryResult};
use crate::geometry::Geometry2;
use crate::intersect::intersect;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::trim::extend;

/// An open polyline's vertices, each with the bulge of the segment
/// leaving it, the last with none.
pub type PolylinePath = Vec<(Point2, f64)>;

/// One segment of a path, run from its start vertex to the next.
#[derive(Clone, Copy, Debug)]
struct Segment {
    start: Point2,
    end: Point2,
    bulge: f64,
    /// An arc's center, radius, start angle and signed turn.
    arc: Option<(Point2, f64, f64, f64)>,
}

impl Segment {
    fn new(start: Point2, end: Point2, bulge: f64) -> GeometryResult<Self> {
        if !bulge.is_finite() {
            return Err(GeometryError::NonFinite { what: "bulge" });
        }
        let arc = if bulge == 0.0 {
            None
        } else {
            let (a, _) = Arc::from_bulge(start, end, bulge)?;
            Some((
                a.center(),
                a.radius(),
                (start - a.center()).angle(),
                4.0 * bulge.atan(),
            ))
        };
        Ok(Self {
            start,
            end,
            bulge,
            arc,
        })
    }

    /// The point `f` of the way along it.
    fn at(self, f: f64) -> Point2 {
        match self.arc {
            None => Point2::new(
                self.start.x() + (self.end.x() - self.start.x()) * f,
                self.start.y() + (self.end.y() - self.start.y()) * f,
            )
            .unwrap_or(self.start),
            Some((c, r, from, turn)) => {
                let (s, co) = (from + turn * f).sin_cos();
                Point2::new(c.x() + r * co, c.y() + r * s).unwrap_or(self.start)
            }
        }
    }

    /// How far along it `q`, a point on it, lies.
    fn fraction(self, q: Point2) -> f64 {
        match self.arc {
            None => {
                let d = self.end - self.start;
                (q - self.start).dot(d) / d.length_squared()
            }
            Some((c, _, from, turn)) => {
                let angle = (q - c).angle();
                let full = std::f64::consts::TAU;
                let gone = if turn > 0.0 {
                    (angle - from).rem_euclid(full)
                } else {
                    (from - angle).rem_euclid(full)
                };
                gone / turn.abs()
            }
        }
    }

    /// The curve it draws, for crossings and nearest points.
    fn curve(self) -> GeometryResult<Geometry2> {
        Ok(match self.arc {
            None => Geometry2::Line(Line::new(self.start, self.end)?),
            Some(_) => Geometry2::Arc(Arc::from_bulge(self.start, self.end, self.bulge)?.0),
        })
    }

    /// The bulge of the part from `f0` to `f1` of it.
    fn bulge_between(self, f0: f64, f1: f64) -> f64 {
        match self.arc {
            None => 0.0,
            Some((.., turn)) => (turn * (f1 - f0) / 4.0).tan(),
        }
    }
}

fn segments(vertices: &[(Point2, f64)], closed: bool) -> GeometryResult<Vec<Segment>> {
    let n = vertices.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    if count == 0 {
        return Err(GeometryError::ShortContour);
    }
    (0..count)
        .map(|i| Segment::new(vertices[i].0, vertices[(i + 1) % n].0, vertices[i].1))
        .collect()
}

/// The path's parameter nearest `p`.
fn nearest(path: &[Segment], p: Point2) -> GeometryResult<f64> {
    let mut best: Option<(f64, f64)> = None;
    for (i, s) in path.iter().enumerate() {
        let curve = s.curve()?;
        let q = curve.closest_point(p);
        let d = q.distance_to(p);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, i as f64 + s.fraction(q).clamp(0.0, 1.0)));
        }
    }
    best.map(|(_, t)| t).ok_or(GeometryError::ShortContour)
}

/// The path from parameter `from` to `to` (going on round a closed path
/// past its end), as an open polyline's vertices; None when it has no
/// length.
/// Source segment and fractional interval, for interpolating per-vertex widths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolylineSpan {
    /// Zero-based source segment index.
    pub segment: usize,
    /// Start fraction in the source segment, in [0, 1].
    pub from: f64,
    /// End fraction in the source segment, in [0, 1].
    pub to: f64,
}

/// One open BREAK result with the provenance of every output segment.
#[derive(Clone, Debug, PartialEq)]
pub struct BrokenPolyline {
    /// Open output path, with the last bulge zero.
    pub vertices: PolylinePath,
    /// One source interval per output segment.
    pub spans: Vec<PolylineSpan>,
}

fn sub_path(path: &[Segment], from: f64, to: f64, tol: Tolerance) -> Option<Vec<(Point2, f64)>> {
    slice_path(path, from, to, tol, None)
}

fn slice_path(
    path: &[Segment],
    from: f64,
    to: f64,
    tol: Tolerance,
    mut spans: Option<&mut Vec<PolylineSpan>>,
) -> Option<PolylinePath> {
    const EPS: f64 = 1e-12;
    let mut out: Vec<(Point2, f64)> = Vec::new();
    let mut t = from;
    while t < to - EPS {
        let i = (t.floor() as usize).min(path.len() - 1);
        let s = path[i];
        let f0 = t - i as f64;
        let stop = to.min((i + 1) as f64);
        let f1 = stop - i as f64;
        if f1 - f0 > EPS {
            let start = s.at(f0);
            let bulge = s.bulge_between(f0, f1);
            match out.last_mut() {
                // Where the last piece ended: the bulge is this piece's.
                Some(last) if last.0.approx_eq(start, tol) => last.1 = bulge,
                _ => {
                    if !out.is_empty()
                        && let Some(spans) = spans.as_deref_mut()
                    {
                        spans.push(PolylineSpan {
                            segment: i,
                            from: f0,
                            to: f0,
                        });
                    }
                    out.push((start, bulge));
                }
            }
            out.push((s.at(f1), 0.0));
            if let Some(spans) = spans.as_deref_mut() {
                spans.push(PolylineSpan {
                    segment: i,
                    from: f0,
                    to: f1,
                });
            }
        }
        t = stop.max(t + EPS);
    }
    let length: f64 = out.windows(2).map(|w| w[0].0.distance_to(w[1].0)).sum();
    (out.len() >= 2 && length > tol.linear()).then_some(out)
}

/// BREAK an open path at its nearest projected points. Reversing the
/// picks removes the same gap; an equal interior point splits without a gap.
/// Equal endpoints are refused as a no-op. Source segment fractions preserve
/// width metadata without geometric reprojection (including self-crossings).
pub fn break_polyline(
    vertices: &[(Point2, f64)],
    first: Point2,
    second: Point2,
    tol: Tolerance,
) -> GeometryResult<Vec<BrokenPolyline>> {
    let path = segments(vertices, false)?;
    let n = path.len() as f64;
    let a = nearest(&path, first)?;
    let b = nearest(&path, second)?;
    if !a.is_finite() || !b.is_finite() {
        return Err(GeometryError::NonFinite {
            what: "break parameter",
        });
    }
    let (from, to) = (a.min(b), a.max(b));
    if from == to && (from == 0.0 || from == n) {
        return Err(GeometryError::OutOfRange {
            what: "break points",
        });
    }
    let mut pieces = Vec::new();
    for (start, end) in [(0.0, from), (to, n)] {
        let mut spans = Vec::new();
        if let Some(vertices) = slice_path(&path, start, end, tol, Some(&mut spans)) {
            pieces.push(BrokenPolyline { vertices, spans });
        }
    }
    Ok(pieces)
}

/// BREAK a closed path using its stored vertex parameters. Keep the span
/// from the larger parameter through vertex zero to the smaller parameter,
/// independent of pick order. Equal parameters open the complete path there.
/// Width provenance always refers to the original, not unrolled, segments.
pub fn break_closed_polyline(
    vertices: &[(Point2, f64)],
    first: Point2,
    second: Point2,
    tol: Tolerance,
) -> GeometryResult<BrokenPolyline> {
    let path = segments(vertices, true)?;
    let n = path.len() as f64;
    let a = nearest(&path, first)?.rem_euclid(n);
    let b = nearest(&path, second)?.rem_euclid(n);
    if !a.is_finite() || !b.is_finite() {
        return Err(GeometryError::NonFinite {
            what: "break parameter",
        });
    }
    let unrolled: Vec<Segment> = path.iter().chain(&path).copied().collect();
    let mut spans = Vec::new();
    let vertices = slice_path(&unrolled, a.max(b), a.min(b) + n, tol, Some(&mut spans)).ok_or(
        GeometryError::OutOfRange {
            what: "break points",
        },
    )?;
    for span in &mut spans {
        span.segment %= path.len();
    }
    Ok(BrokenPolyline { vertices, spans })
}

/// The polyline trimmed at its crossings with `edges` around `pick`, as
/// AutoCAD's TRIM: the part between the crossings nearest either side of
/// the picked point is taken away. An open polyline is left as none, one
/// or two open polylines; a closed one, cut at least twice, opens into the
/// one left. None when no crossing bounds that part (Quick mode then
/// erases the whole polyline). Each piece is an open polyline's vertices,
/// the last with no bulge.
pub fn trim_polyline(
    vertices: &[(Point2, f64)],
    closed: bool,
    edges: &[Geometry2],
    pick: Point2,
    tol: Tolerance,
) -> GeometryResult<Option<Vec<PolylinePath>>> {
    let path = segments(vertices, closed)?;
    let n = path.len() as f64;
    let mut cuts: Vec<f64> = Vec::new();
    for (i, s) in path.iter().enumerate() {
        let curve = s.curve()?;
        for &e in edges {
            for &q in intersect(curve, e, tol).points() {
                cuts.push(i as f64 + s.fraction(q).clamp(0.0, 1.0));
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    let at = nearest(&path, pick)?;
    if closed {
        // Round the path, the cut ends meeting at its start.
        cuts.retain(|&t| t < n - 1e-9);
        if cuts.len() < 2 {
            return Ok(None);
        }
        let after = cuts
            .iter()
            .copied()
            .find(|&t| t > at)
            .unwrap_or(cuts[0] + n);
        let before = cuts
            .iter()
            .rev()
            .copied()
            .find(|&t| t < at)
            .unwrap_or(cuts[cuts.len() - 1] - n);
        // Left: from the cut after the pick on round to the one before it.
        let (from, to) = (after, before + n);
        let unrolled: Vec<Segment> = path.iter().chain(path.iter()).copied().collect();
        return Ok(sub_path(&unrolled, from, to, tol).map(|p| vec![p]));
    }
    // Cuts at the path's own ends cut nothing off.
    cuts.retain(|&t| t > 1e-9 && t < n - 1e-9);
    let before = cuts.iter().rev().copied().find(|&t| t < at);
    let after = cuts.iter().copied().find(|&t| t > at);
    if before.is_none() && after.is_none() {
        return Ok(None);
    }
    let mut left = Vec::new();
    if let Some(b) = before {
        left.extend(sub_path(&path, 0.0, b, tol));
    }
    if let Some(a) = after {
        left.extend(sub_path(&path, a, n, tol));
    }
    Ok(Some(left))
}

/// The open polyline with its end segment nearer `pick` grown to the
/// nearest of `edges` its line or circle reaches beyond that end, as
/// AutoCAD's EXTEND: its other vertices unchanged, that segment's end
/// vertex moved and an arc's bulge grown with it. None when no edge lies
/// that way. A closed polyline has no end to extend.
pub fn extend_polyline(
    vertices: &[(Point2, f64)],
    closed: bool,
    edges: &[Geometry2],
    pick: Point2,
    tol: Tolerance,
) -> GeometryResult<Option<Vec<(Point2, f64)>>> {
    let (index, s, at_end) = extension_segment(vertices, closed, pick)?;
    let tip = if at_end { s.end } else { s.start };
    let curve = s.curve()?;
    let Some(grown) = extend(curve, edges, tip, tol)? else {
        return Ok(None);
    };
    let mut out = vertices.to_vec();
    match grown {
        Geometry2::Line(l) => {
            if at_end {
                out[index + 1].0 = l.end();
            } else {
                out[0].0 = l.start();
            }
        }
        Geometry2::Arc(a) => {
            // Stored counterclockwise: a clockwise segment runs from the
            // arc's end to its start.
            let forward = s.bulge > 0.0;
            let new_tip = match (at_end, forward) {
                (true, true) | (false, false) => a.end_point(),
                _ => a.start_point(),
            };
            if at_end {
                out[index + 1].0 = new_tip;
            } else {
                out[0].0 = new_tip;
            }
            out[index].1 = s.bulge.signum() * (a.sweep() / 4.0).tan();
        }
        Geometry2::Circle(_) => return Ok(None),
    }
    Ok(Some(out))
}

/// The very same terminal curve and pick that EXTEND uses, in world plan.
pub fn polyline_extension_target(
    vertices: &[(Point2, f64)],
    closed: bool,
    pick: Point2,
) -> GeometryResult<(Geometry2, Point2)> {
    let (_, segment, at_end) = extension_segment(vertices, closed, pick)?;
    Ok((
        segment.curve()?,
        if at_end { segment.end } else { segment.start },
    ))
}

fn extension_segment(
    vertices: &[(Point2, f64)],
    closed: bool,
    pick: Point2,
) -> GeometryResult<(usize, Segment, bool)> {
    if closed {
        return Err(GeometryError::OutOfRange {
            what: "closed polyline",
        });
    }
    let path = segments(vertices, false)?;
    let at_end = nearest(&path, pick)? > path.len() as f64 / 2.0;
    let index = if at_end { path.len() - 1 } else { 0 };
    Ok((index, path[index], at_end))
}

/// Conservative operative bounds, including angular endpoint slack of arcs.
pub fn polyline_trim_bounds(
    vertices: &[(Point2, f64)],
    closed: bool,
    tol: Tolerance,
) -> GeometryResult<crate::Bounds2> {
    let mut bounds = None;
    for segment in segments(vertices, closed)? {
        let curve = segment.curve()?;
        let margin = match curve {
            Geometry2::Arc(a) => a.radius() * tol.angular(),
            _ => 0.0,
        } + tol.linear();
        let area = curve.bounds().expanded(margin)?;
        bounds = Some(bounds.map_or(area, |old: crate::Bounds2| old.union(area)));
    }
    bounds.ok_or(GeometryError::ShortContour)
}

#[cfg(test)]
#[path = "polyline_trim_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "polyline_break_closed_tests.rs"]
mod closed_break_tests;
