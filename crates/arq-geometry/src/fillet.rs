//! FILLET and CHAMFER's corners (AutoCAD's): two lines, or a line and an
//! arc, met by a tangent arc of a radius, or two lines cut across by a
//! straight segment; each object kept on the side it was picked on.

use std::f64::consts::{PI, TAU};

use crate::arc::Arc;
use crate::crossings::{line_circle, line_line};
use crate::error::{GeometryError, GeometryResult};
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// What a FILLET or CHAMFER makes of two objects: each one's new geometry
/// (None when trimming is off, or it is not changed), and the arc or
/// segment joining them (None for a sharp corner, radius or distances 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    /// The first object, trimmed or extended to the joint.
    pub first: Option<Geometry2>,
    /// The second object, likewise.
    pub second: Option<Geometry2>,
    /// The fillet's arc or the chamfer's segment.
    pub joint: Option<Geometry2>,
}

/// Two lines' corner: where their lines meet, and each one's unit direction
/// from there toward the side it was picked on.
struct LineCorner {
    meet: Point2,
    first: (Line, Vector2),
    second: (Line, Vector2),
}

fn not_parallel() -> GeometryError {
    GeometryError::OutOfRange { what: "parallel" }
}

fn lines_corner(a: Line, pa: Point2, b: Line, pb: Point2) -> GeometryResult<LineCorner> {
    let meet = line_line(a.start(), a.vector(), b.start(), b.vector()).ok_or_else(not_parallel)?;
    // The side of the meeting point the pick is on, along each line; a pick
    // right on it takes the line's far end.
    let toward = |l: Line, pick: Point2| -> Vector2 {
        let d = l.direction();
        let t = (pick - meet).dot(d);
        let side = if t.abs() > 1e-12 {
            t.signum()
        } else {
            let far = if (l.start() - meet).length() > (l.end() - meet).length() {
                l.start()
            } else {
                l.end()
            };
            (far - meet).dot(d).signum()
        };
        d * side
    };
    Ok(LineCorner {
        meet,
        first: (a, toward(a, pa)),
        second: (b, toward(b, pb)),
    })
}

/// The line kept from its far end on its side down to `to`, its own
/// direction kept; refused when `to` lies beyond that far end.
fn line_to(l: Line, side: Vector2, meet: Point2, to: Point2) -> GeometryResult<Line> {
    let reach = |p: Point2| (p - meet).dot(side);
    let far = if reach(l.start()) >= reach(l.end()) {
        l.start()
    } else {
        l.end()
    };
    if reach(far) <= reach(to) {
        return Err(GeometryError::OutOfRange { what: "too large" });
    }
    if far == l.start() {
        Line::new(far, to)
    } else {
        Line::new(to, far)
    }
}

/// FILLET: `a` and `b` (lines, or a line and an arc) met by an arc of
/// `radius` tangent to both, each kept on the side it was picked (`pa`,
/// `pb`) and, when `trim`, trimmed or extended to the arc. Radius 0 makes
/// a sharp corner: the two met where they cross. Refused for parallel
/// lines, two arcs or a circle (later), and a radius too large for them.
pub fn fillet(
    a: Geometry2,
    pa: Point2,
    b: Geometry2,
    pb: Point2,
    radius: f64,
    trim: bool,
    tol: Tolerance,
) -> GeometryResult<Corner> {
    if !(radius.is_finite() && radius >= 0.0) {
        return Err(GeometryError::NegativeOrNonFinite {
            what: "fillet radius",
        });
    }
    match (a, b) {
        (Geometry2::Line(la), Geometry2::Line(lb)) => {
            fillet_lines(la, pa, lb, pb, radius, trim, tol)
        }
        (Geometry2::Line(l), Geometry2::Arc(arc)) => {
            fillet_line_arc(l, pa, arc, pb, radius, trim, tol)
        }
        (Geometry2::Arc(arc), Geometry2::Line(l)) => {
            let c = fillet_line_arc(l, pb, arc, pa, radius, trim, tol)?;
            Ok(Corner {
                first: c.second,
                second: c.first,
                joint: c.joint,
            })
        }
        _ => Err(GeometryError::OutOfRange { what: "curves" }),
    }
}

fn fillet_lines(
    a: Line,
    pa: Point2,
    b: Line,
    pb: Point2,
    radius: f64,
    trim: bool,
    tol: Tolerance,
) -> GeometryResult<Corner> {
    let c = lines_corner(a, pa, b, pb)?;
    let ((la, ua), (lb, ub)) = (c.first, c.second);
    if radius <= tol.linear() {
        return Ok(Corner {
            first: trim
                .then(|| line_to(la, ua, c.meet, c.meet))
                .transpose()?
                .map(Geometry2::Line),
            second: trim
                .then(|| line_to(lb, ub, c.meet, c.meet))
                .transpose()?
                .map(Geometry2::Line),
            joint: None,
        });
    }
    // Half the angle between the two kept sides.
    let half = ua.dot(ub).clamp(-1.0, 1.0).acos() / 2.0;
    if half <= 1e-9 || half >= PI / 2.0 - 1e-9 {
        return Err(not_parallel());
    }
    let along = radius / half.tan();
    let (ta, tb) = (c.meet + ua * along, c.meet + ub * along);
    let bisector = (ua + ub).normalized()?;
    let center = c.meet + bisector * (radius / half.sin());
    let joint = minor_arc(center, radius, ta, tb)?;
    let first = if trim {
        Some(Geometry2::Line(line_to(la, ua, c.meet, ta)?))
    } else {
        None
    };
    let second = if trim {
        Some(Geometry2::Line(line_to(lb, ub, c.meet, tb)?))
    } else {
        None
    };
    if !trim {
        // Without trimming the arc must still lie on both sides picked.
        line_to(la, ua, c.meet, ta)?;
        line_to(lb, ub, c.meet, tb)?;
    }
    Ok(Corner {
        first,
        second,
        joint: Some(Geometry2::Arc(joint)),
    })
}

/// The arc about `center` from `p` to `q` the short way round.
fn minor_arc(center: Point2, radius: f64, p: Point2, q: Point2) -> GeometryResult<Arc> {
    let (a1, a2) = ((p - center).angle(), (q - center).angle());
    if (a2 - a1).rem_euclid(TAU) <= PI {
        Arc::new(center, radius, a1, a2)
    } else {
        Arc::new(center, radius, a2, a1)
    }
}

fn fillet_line_arc(
    l: Line,
    pl: Point2,
    arc: Arc,
    pa: Point2,
    radius: f64,
    trim: bool,
    tol: Tolerance,
) -> GeometryResult<Corner> {
    let (c, r) = (arc.center(), arc.radius());
    let line_foot = |p: Point2| {
        let d = l.direction();
        l.start() + d * (p - l.start()).dot(d)
    };
    let on_circle = |p: Point2| -> GeometryResult<Point2> { Ok(c + (p - c).normalized()? * r) };
    let (pick_line, pick_arc) = (line_foot(pl), on_circle(pa)?);
    let normal = l.direction().perp();
    // Centers a radius away from the line (either side) and from the arc's
    // circle (outside or inside it); the one whose tangent points lie
    // nearest the picks is AutoCAD's.
    let mut best: Option<(f64, Point2, Point2, Point2)> = None;
    for side in [1.0, -1.0] {
        let start = l.start() + normal * (side * radius);
        for ring in [r + radius, r - radius] {
            if ring <= tol.linear() {
                continue;
            }
            for center in line_circle(start, l.vector(), c, ring) {
                let tl = line_foot(center);
                let ta = on_circle(center)?;
                let score = tl.distance_to(pick_line) + ta.distance_to(pick_arc);
                if best.is_none_or(|(s, ..)| score < s) {
                    best = Some((score, center, tl, ta));
                }
            }
        }
    }
    let (_, center, tl, ta) = best.ok_or(GeometryError::OutOfRange { what: "too large" })?;
    let joint = if radius <= tol.linear() {
        None
    } else {
        Some(Geometry2::Arc(minor_arc(center, radius, tl, ta)?))
    };
    if !trim {
        return Ok(Corner {
            first: None,
            second: None,
            joint,
        });
    }
    // The line from its end on the pick's side to its tangent point.
    let d = l.direction();
    let side = (pick_line - tl).dot(d);
    let far = if (l.start() - tl).dot(d) * side >= (l.end() - tl).dot(d) * side {
        l.start()
    } else {
        l.end()
    };
    let new_line = if far == l.start() {
        Line::new(far, tl)?
    } else {
        Line::new(tl, far)?
    };
    // The arc from its end on the pick's side round to its tangent point.
    let angle = |p: Point2| (p - c).angle();
    let (s, e) = (arc.start_angle(), arc.end_angle());
    let (at, ap) = (angle(ta), angle(pick_arc));
    let gone = |a: f64| (a - s).rem_euclid(TAU);
    let new_arc = if gone(at) <= arc.sweep() {
        if gone(ap) < gone(at) {
            Arc::new(c, r, s, at)?
        } else {
            Arc::new(c, r, at, e)?
        }
    } else if (at - e).rem_euclid(TAU) < (s - at).rem_euclid(TAU) {
        Arc::new(c, r, s, at)?
    } else {
        Arc::new(c, r, at, e)?
    };
    Ok(Corner {
        first: Some(Geometry2::Line(new_line)),
        second: Some(Geometry2::Arc(new_arc)),
        joint,
    })
}

/// CHAMFER: lines `a` and `b` cut across by a segment from `first` along
/// `a` to `second` along `b` from where they meet, each kept on the side
/// it was picked and, when `trim`, cut back to the segment. Distances 0
/// make a sharp corner. Refused for parallel lines and distances longer
/// than the lines.
#[allow(clippy::too_many_arguments)]
pub fn chamfer(
    a: Line,
    pa: Point2,
    b: Line,
    pb: Point2,
    first: f64,
    second: f64,
    trim: bool,
    tol: Tolerance,
) -> GeometryResult<Corner> {
    for d in [first, second] {
        if !(d.is_finite() && d >= 0.0) {
            return Err(GeometryError::NegativeOrNonFinite {
                what: "chamfer distance",
            });
        }
    }
    let c = lines_corner(a, pa, b, pb)?;
    let ((la, ua), (lb, ub)) = (c.first, c.second);
    let (ca, cb) = (c.meet + ua * first, c.meet + ub * second);
    let joint = if first <= tol.linear() && second <= tol.linear() {
        None
    } else {
        Some(Geometry2::Line(Line::new(ca, cb)?))
    };
    let new_a = line_to(la, ua, c.meet, ca)?;
    let new_b = line_to(lb, ub, c.meet, cb)?;
    Ok(Corner {
        first: trim.then_some(Geometry2::Line(new_a)),
        second: trim.then_some(Geometry2::Line(new_b)),
        joint,
    })
}

/// The second chamfer distance that makes the segment leave line `a` at
/// `angle` (radians) when it starts `length` along `a` from the corner
/// (CHAMFER's Angle method): by the triangle the two lines and the segment
/// make. Refused when the segment would never meet `b` on its side.
pub fn chamfer_second_distance(
    a: Line,
    pa: Point2,
    b: Line,
    pb: Point2,
    length: f64,
    angle: f64,
) -> GeometryResult<f64> {
    let c = lines_corner(a, pa, b, pb)?;
    let between = c.first.1.dot(c.second.1).clamp(-1.0, 1.0).acos();
    let opposite = PI - angle - between;
    if !(angle > 0.0 && opposite > 1e-12) {
        return Err(GeometryError::OutOfRange {
            what: "chamfer angle",
        });
    }
    Ok(length * angle.sin() / opposite.sin())
}

#[cfg(test)]
#[path = "fillet_tests.rs"]
mod tests;
