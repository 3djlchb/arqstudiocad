//! TRIM and EXTEND's curves: a line, arc or circle cut back to the edges
//! nearest a picked point, or a line or arc grown to the nearest boundary.

use std::f64::consts::TAU;

use crate::arc::Arc;
use crate::crossings::{circle_circle, line_circle, line_line};
use crate::error::{GeometryError, GeometryResult};
use crate::geometry::Geometry2;
use crate::intersect::intersect;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;

/// Where a curve stands at a point on it: a line's fraction from its start,
/// an arc's turn from its start, a circle's angle.
fn parameter(curve: Geometry2, p: Point2) -> f64 {
    match curve {
        Geometry2::Line(l) => {
            let d = l.vector();
            (p - l.start()).dot(d) / d.length_squared()
        }
        Geometry2::Arc(a) => ((p - a.center()).angle() - a.start_angle()).rem_euclid(TAU),
        Geometry2::Circle(c) => (p - c.center()).angle().rem_euclid(TAU),
    }
}

/// The curve's whole span of parameters.
fn span(curve: Geometry2) -> f64 {
    match curve {
        Geometry2::Line(_) => 1.0,
        Geometry2::Arc(a) => a.sweep(),
        Geometry2::Circle(_) => TAU,
    }
}

/// The part of `curve` between two of its parameters (`from` before `to`,
/// a circle's going round through 2π); None when it has no length.
fn piece(curve: Geometry2, from: f64, to: f64, tol: Tolerance) -> Option<Geometry2> {
    match curve {
        Geometry2::Line(l) => Line::new(l.point_at(from), l.point_at(to))
            .ok()
            .filter(|n| n.length() > tol.linear())
            .map(Geometry2::Line),
        Geometry2::Arc(a) => {
            let start = a.start_angle();
            Arc::new(a.center(), a.radius(), start + from, start + to)
                .ok()
                .filter(|n| n.length() > tol.linear())
                .map(Geometry2::Arc)
        }
        Geometry2::Circle(c) => Arc::new(c.center(), c.radius(), from, to)
            .ok()
            .filter(|n| n.length() > tol.linear())
            .map(Geometry2::Arc),
    }
}

/// The intersections of `curve` with `edges`, as AutoCAD's TRIM finds its
/// cutting points: where each edge really crosses or touches it.
pub fn cutting_points(curve: Geometry2, edges: &[Geometry2], tol: Tolerance) -> Vec<Point2> {
    edges
        .iter()
        .flat_map(|&e| intersect(curve, e, tol).points().to_vec())
        .collect()
}

/// `curve` trimmed at `cuts` (points on it, as [`cutting_points`] gives)
/// around `pick`: the part between the cuts nearest either side of the
/// picked point is taken away, as AutoCAD's TRIM does; what is left, none,
/// one piece or two. None when no cut bounds that part (TRIM's Quick mode
/// then erases the whole object). A circle needs two cuts, and becomes the
/// arc left between them.
pub fn trim(
    curve: Geometry2,
    cuts: &[Point2],
    pick: Point2,
    tol: Tolerance,
) -> GeometryResult<Option<Vec<Geometry2>>> {
    let total = span(curve);
    let at = parameter(curve, curve.closest_point(pick));
    let end_slack = tol.linear() / curve.length().max(tol.linear()) * total;
    let mut params: Vec<f64> = cuts
        .iter()
        .map(|&q| parameter(curve, q))
        .filter(|&t| t.is_finite())
        .collect();
    params.sort_by(f64::total_cmp);
    params.dedup_by(|a, b| (*a - *b).abs() <= end_slack);
    if let Geometry2::Circle(_) = curve {
        if params.len() < 2 {
            return Ok(None);
        }
        // The cuts either side of the pick, going round.
        let after = params
            .iter()
            .copied()
            .find(|&t| t > at)
            .unwrap_or(params[0]);
        let before = params
            .iter()
            .rev()
            .copied()
            .find(|&t| t < at)
            .unwrap_or(params[params.len() - 1]);
        // Left: from the cut after the pick round to the one before it.
        let to = if before <= after {
            before + TAU
        } else {
            before
        };
        return Ok(piece(curve, after, to, tol).map(|p| vec![p]));
    }
    // Cuts at the curve's own ends cut nothing off.
    let inner: Vec<f64> = params
        .into_iter()
        .filter(|&t| t > end_slack && t < total - end_slack)
        .collect();
    let before = inner.iter().rev().copied().find(|&t| t < at);
    let after = inner.iter().copied().find(|&t| t > at);
    if before.is_none() && after.is_none() {
        return Ok(None);
    }
    let mut left = Vec::new();
    if let Some(b) = before {
        left.extend(piece(curve, 0.0, b, tol));
    }
    if let Some(a) = after {
        left.extend(piece(curve, a, total, tol));
    }
    Ok(Some(left))
}

/// `curve` grown at the end nearer `pick` to the nearest of `edges` its
/// line or circle reaches beyond that end, as AutoCAD's EXTEND: a line
/// longer along itself, an arc further round its circle (never round onto
/// itself). None when no edge lies that way. A circle cannot be extended.
pub fn extend(
    curve: Geometry2,
    edges: &[Geometry2],
    pick: Point2,
    tol: Tolerance,
) -> GeometryResult<Option<Geometry2>> {
    let at = parameter(curve, curve.closest_point(pick));
    let at_end = at > span(curve) / 2.0;
    // Where the curve's line or circle meets each edge, on the edge.
    let reach: Vec<Point2> = edges
        .iter()
        .flat_map(|&edge| {
            let found = match (curve, edge) {
                (Geometry2::Line(l), Geometry2::Line(e)) => {
                    line_line(l.start(), l.vector(), e.start(), e.vector())
                        .into_iter()
                        .collect()
                }
                (Geometry2::Line(l), Geometry2::Circle(c)) => {
                    line_circle(l.start(), l.vector(), c.center(), c.radius())
                }
                (Geometry2::Line(l), Geometry2::Arc(a)) => {
                    line_circle(l.start(), l.vector(), a.center(), a.radius())
                }
                (Geometry2::Arc(a), Geometry2::Line(e)) => {
                    line_circle(e.start(), e.vector(), a.center(), a.radius())
                }
                (Geometry2::Arc(a), Geometry2::Circle(c)) => {
                    circle_circle(a.center(), a.radius(), c.center(), c.radius(), tol)
                }
                (Geometry2::Arc(a), Geometry2::Arc(b)) => {
                    circle_circle(a.center(), a.radius(), b.center(), b.radius(), tol)
                }
                (Geometry2::Circle(_), _) => Vec::new(),
            };
            found
                .into_iter()
                .filter(move |&q| on(edge, q))
                .collect::<Vec<_>>()
        })
        .collect();
    match curve {
        Geometry2::Circle(_) => Err(GeometryError::OutOfRange { what: "curve" }),
        Geometry2::Line(l) => {
            // Beyond the end picked, the nearest.
            let beyond = |t: f64| if at_end { t - 1.0 } else { -t };
            let best = reach
                .iter()
                .map(|&q| (parameter(curve, q), q))
                .filter(|&(t, _)| beyond(t) * l.length() > tol.linear())
                .min_by(|a, b| beyond(a.0).total_cmp(&beyond(b.0)));
            Ok(best
                .map(|(_, q)| {
                    if at_end {
                        Line::new(l.start(), q)
                    } else {
                        Line::new(q, l.end())
                    }
                })
                .transpose()?
                .map(Geometry2::Line))
        }
        Geometry2::Arc(a) => {
            // Further round its circle from the end picked, short of
            // reaching its other end.
            let room = TAU - a.sweep();
            let slack = tol.linear() / a.radius();
            let turn = |q: Point2| {
                let angle = (q - a.center()).angle();
                if at_end {
                    (angle - a.end_angle()).rem_euclid(TAU)
                } else {
                    (a.start_angle() - angle).rem_euclid(TAU)
                }
            };
            let best = reach
                .iter()
                .map(|&q| turn(q))
                .filter(|&t| t > slack && t < room - slack)
                .min_by(f64::total_cmp);
            Ok(best
                .map(|t| {
                    if at_end {
                        Arc::new(a.center(), a.radius(), a.start_angle(), a.end_angle() + t)
                    } else {
                        Arc::new(a.center(), a.radius(), a.start_angle() - t, a.end_angle())
                    }
                })
                .transpose()?
                .map(Geometry2::Arc))
        }
    }
}

/// Whether `q`, computed where two curves meet, lies on `edge`: within a
/// hair, relative to the coordinates' size.
fn on(edge: Geometry2, q: Point2) -> bool {
    let scale = q.x().abs().max(q.y().abs()).max(1.0);
    edge.distance_to(q) <= 1e-9 * scale
}

#[cfg(test)]
#[path = "trim_tests.rs"]
mod tests;
