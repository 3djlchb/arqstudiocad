//! Seeing a body (ADR 0024 D4, step 2): its wireframe, as AutoCAD's 2D
//! wireframe shows a solid with ISOLINES set: every edge, sampled within a
//! chord tolerance, and on each curved face a few lines of its surface
//! (isolines), clipped to the face.
//!
//! Clipping uses ACIS's rule that a face lies to the left of its loops
//! seen from outside: a point of an isoline near the face's boundary is
//! inside when it lies on the left of the nearest piece of it, the outward
//! normal being up. The rule does not depend on how the surface is
//! parametrized, so it holds on periodic surfaces (cylinders, tori) as on
//! planes. Far from the boundary a curved surface may fold back and fool
//! it, so there the state is carried along the isoline instead: it changes
//! only by crossing the boundary, which happens near it. An isoline that
//! never comes near its face's boundary is left out rather than guessed.
//! Where an isoline leaves its face, the crossing is found by bisection.

use std::f64::consts::{PI, TAU};

use arq_geometry::{Bounds3, Point3, Vector3};

use crate::brep::{Body, Edge, Face};
use crate::curve::{Curve, EllipseCurve};
use crate::surface::Surface;

/// What to draw of a body: polylines through 3D points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wireframe {
    /// One polyline per edge, from its start vertex to its end vertex.
    pub edges: Vec<Vec<Point3>>,
    /// The visible pieces of the faces' isolines.
    pub isolines: Vec<Vec<Point3>>,
}

/// Points along an isoline, between the samples a crossing is looked for.
const ISOLINE_SAMPLES: usize = 48;

impl Body {
    /// The body's wireframe: each edge as a polyline that strays at most
    /// `chord` from it, and `isolines` lines on each curved face (AutoCAD's
    /// ISOLINES; 0 for none), clipped to the face.
    pub fn wireframe(&self, chord: f64, isolines: usize) -> Wireframe {
        let edges: Vec<Vec<Point3>> = self
            .edges()
            .iter()
            .map(|e| edge_polyline(self, e, chord))
            .collect();
        let mut lines = Vec::new();
        if isolines > 0 {
            for face in self.faces() {
                lines.extend(face_isolines(self, face, &edges, isolines));
            }
        }
        Wireframe {
            edges: edges.into_iter().filter(|e| e.len() >= 2).collect(),
            isolines: lines,
        }
    }
}

/// An edge from its start vertex to its end vertex; empty without a curve.
pub(crate) fn edge_polyline(body: &Body, edge: &Edge, chord: f64) -> Vec<Point3> {
    let (start, end) = (body.vertex(edge.start).point, body.vertex(edge.end).point);
    let closed = edge.start == edge.end;
    match &edge.curve {
        None => Vec::new(),
        Some(Curve::Line(_)) => vec![start, end],
        Some(Curve::Ellipse(e)) => {
            let (ts, te) = (ellipse_parameter(e, start), ellipse_parameter(e, end));
            // The curve runs counterclockwise about its normal; a reversed
            // edge runs the other way.
            let sweep = if closed {
                TAU
            } else if edge.reversed {
                (ts - te).rem_euclid(TAU)
            } else {
                (te - ts).rem_euclid(TAU)
            };
            let sign = if edge.reversed { -1.0 } else { 1.0 };
            let radius = e.major().length();
            let steps = arc_steps(sweep, radius, chord);
            let mut points: Vec<Point3> = (0..=steps)
                .map(|i| e.point_at(ts + sign * sweep * i as f64 / steps as f64))
                .collect();
            *points.first_mut().expect("two points at least") = start;
            *points.last_mut().expect("two points at least") = end;
            points
        }
        Some(Curve::Nurbs(n)) => {
            let (a, b) = n.domain();
            let (ts, te) = if closed {
                if edge.reversed { (b, a) } else { (a, b) }
            } else {
                (n.closest_parameter(start), n.closest_parameter(end))
            };
            let spans = n.points().len() - n.degree();
            let share = ((te - ts) / (b - a)).abs();
            let steps = ((16 * spans) as f64 * share).ceil().clamp(4.0, 1024.0) as usize;
            let mut points: Vec<Point3> = (0..=steps)
                .map(|i| n.point_at(ts + (te - ts) * i as f64 / steps as f64))
                .collect();
            *points.first_mut().expect("two points at least") = start;
            *points.last_mut().expect("two points at least") = end;
            points
        }
    }
}

/// The parameter of point `p` (on or near the ellipse) as the angle from
/// its major axis in its own scaled frame.
pub(crate) fn ellipse_parameter(e: &EllipseCurve, p: Point3) -> f64 {
    let a = e.major().length();
    let u = e.major() / a;
    let v = e.normal().cross(u);
    let d = p - e.center();
    (d.dot(v) / e.ratio()).atan2(d.dot(u))
}

/// Segments for an arc of `sweep` radians and `radius` within `chord`:
/// never fewer than one per eighth of a turn.
fn arc_steps(sweep: f64, radius: f64, chord: f64) -> usize {
    let by_chord = if chord < radius {
        2.0 * (1.0 - chord / radius).acos()
    } else {
        PI / 4.0
    };
    let step = by_chord.clamp(1e-3, PI / 4.0);
    ((sweep / step).ceil() as usize).clamp(1, 4096)
}

/// The visible pieces of a face's isolines.
fn face_isolines(
    body: &Body,
    face: &Face,
    edges: &[Vec<Point3>],
    count: usize,
) -> Vec<Vec<Point3>> {
    let boundary = boundary_of(body, face, edges);
    let lines = isolines(&face.surface, &boundary, count);
    let Some((guides, reach)) = guides_of(face, &boundary) else {
        // No loops: the whole surface (a sphere, a torus).
        return lines
            .iter()
            .map(|line| sample(line).into_iter().map(line).collect())
            .collect();
    };
    let side = |p: Point3| side_of(&guides, p);
    let mut pieces = Vec::new();
    for line in lines {
        pieces.extend(clip(&line, &side, reach));
    }
    pieces
}

/// The share of a face's boundary size within which the local inside rule
/// is trusted.
const NEAR: f64 = 0.15;

/// A face's boundary as oriented segments, in loop order, from its edges'
/// polylines.
pub(crate) fn boundary_of(
    body: &Body,
    face: &Face,
    edges: &[Vec<Point3>],
) -> Vec<(Point3, Point3)> {
    let mut boundary = Vec::new();
    for lp in &face.loops {
        for &c in &body.loops()[lp.index()].coedges {
            let coedge = body.coedges()[c.index()];
            let points = &edges[coedge.edge.index()];
            let pairs = points.windows(2).map(|w| (w[0], w[1]));
            if coedge.reversed {
                boundary.extend(pairs.rev().map(|(a, b)| (b, a)));
            } else {
                boundary.extend(pairs);
            }
        }
    }
    boundary
}

/// Each boundary segment with the left direction on the face, from the
/// outward normal at its middle, and how near the boundary the local rule
/// is trusted; `None` without a boundary (the whole surface).
pub(crate) fn guides_of(face: &Face, boundary: &[(Point3, Point3)]) -> Option<(Vec<Guide>, f64)> {
    if boundary.is_empty() {
        return None;
    }
    let reach = Bounds3::from_points(boundary.iter().map(|(a, _)| *a))
        .map_or(0.0, |b| b.size().length())
        * NEAR;
    let guides = boundary
        .iter()
        .filter_map(|&(a, b)| {
            let along = b - a;
            let n = outward(face, a.midpoint(b))?;
            Some(Guide {
                a,
                along,
                left: n.cross(along),
            })
        })
        .collect();
    Some((guides, reach))
}

/// Whether the point of `line` at `t` lies in the face, by the rule
/// [`clip`] follows: its own side when near the boundary, else the state
/// carried along the line from its sample nearest the boundary; `None`
/// when the line never comes near the boundary.
pub(crate) fn inside_at(
    line: &Isoline,
    t: f64,
    side: &dyn Fn(Point3) -> (f64, bool),
    reach: f64,
) -> Option<bool> {
    let mut ts = sample(line);
    let at = ts.partition_point(|&s| s < t);
    ts.insert(at, t);
    let probes: Vec<(f64, bool)> = ts.iter().map(|&s| side(line(s))).collect();
    let seed = (0..ts.len()).min_by(|&a, &b| probes[a].0.total_cmp(&probes[b].0))?;
    if probes[seed].0 > reach {
        return None;
    }
    // From the seed toward t: the state changes only near the boundary.
    let mut state = probes[seed].1;
    let step: isize = if at >= seed { 1 } else { -1 };
    let mut i = seed;
    while i != at {
        i = i.wrapping_add_signed(step);
        if probes[i].0 <= reach {
            state = probes[i].1;
        }
    }
    Some(state)
}

/// The parameters an isoline is sampled at.
fn sample(_line: &Isoline) -> Vec<f64> {
    (0..=ISOLINE_SAMPLES)
        .map(|i| i as f64 / ISOLINE_SAMPLES as f64)
        .collect()
}

/// An isoline: its point at a parameter from 0 to 1.
pub(crate) type Isoline = Box<dyn Fn(f64) -> Point3>;

/// The isolines of a surface: none on a plane; on a cylinder or cone,
/// lines along the axis; on a sphere, meridians and the equator; on a
/// torus, circles both ways; on a NURBS surface, curves at even steps of
/// each parameter. The extent along an unbounded direction is the
/// boundary's.
fn isolines(surface: &Surface, boundary: &[(Point3, Point3)], count: usize) -> Vec<Isoline> {
    let mut out: Vec<Isoline> = Vec::new();
    let angles = move |k: usize| TAU * k as f64 / count as f64;
    match *surface {
        Surface::Plane(_) => {}
        Surface::Cone(cone) => {
            let (o, a) = (cone.origin(), cone.axis());
            let Some((h0, h1)) = height_range(boundary, o, a) else {
                return out;
            };
            let (e1, e2) = frame(a);
            let slope = cone.sin_half_angle() / cone.cos_half_angle();
            let r0 = cone.radius();
            for k in 0..count {
                let dir = e1 * angles(k).cos() + e2 * angles(k).sin();
                out.push(Box::new(move |t| {
                    let h = h0 + (h1 - h0) * t;
                    o + a * h + dir * (r0 + h * slope)
                }));
            }
        }
        Surface::EllipticCylinder(cylinder) => {
            let section = cylinder.section();
            let (o, a) = (section.center(), section.normal());
            let Some((h0, h1)) = height_range(boundary, o, a) else {
                return out;
            };
            for k in 0..count {
                let base = section.point_at(angles(k));
                out.push(Box::new(move |t| base + a * (h0 + (h1 - h0) * t)));
            }
        }
        Surface::Sphere(sphere) => {
            let (c, r) = (sphere.center(), sphere.radius());
            let z = Vector3::new(0.0, 0.0, 1.0).expect("finite");
            let (e1, e2) = frame(z);
            for k in 0..count {
                let dir = e1 * angles(k).cos() + e2 * angles(k).sin();
                out.push(Box::new(move |t| {
                    let lat = -PI / 2.0 + PI * t;
                    c + (dir * lat.cos() + z * lat.sin()) * r
                }));
            }
            out.push(Box::new(move |t| {
                let lon = TAU * t;
                c + (e1 * lon.cos() + e2 * lon.sin()) * r
            }));
        }
        Surface::Torus(torus) => {
            let (c, a) = (torus.center(), torus.axis());
            let (big, small) = (torus.major_radius(), torus.minor_radius());
            let (e1, e2) = frame(a);
            for k in 0..count {
                // Around the tube at an angle about the axis.
                let dir = e1 * angles(k).cos() + e2 * angles(k).sin();
                out.push(Box::new(move |t| {
                    let v = TAU * t;
                    c + dir * (big + small * v.cos()) + a * (small * v.sin())
                }));
                // Around the axis at an angle about the tube.
                let v = angles(k);
                out.push(Box::new(move |t| {
                    let u = TAU * t;
                    let dir = e1 * u.cos() + e2 * u.sin();
                    c + dir * (big + small * v.cos()) + a * (small * v.sin())
                }));
            }
        }
        Surface::Nurbs(ref nurbs) => {
            let ((u0, u1), (v0, v1)) = nurbs.domain();
            for k in 1..count {
                let f = k as f64 / count as f64;
                let (u, n) = (u0 + (u1 - u0) * f, nurbs.clone());
                out.push(Box::new(move |t| n.point_at(u, v0 + (v1 - v0) * t)));
                let (v, n) = (v0 + (v1 - v0) * f, nurbs.clone());
                out.push(Box::new(move |t| n.point_at(u0 + (u1 - u0) * t, v)));
            }
        }
    }
    out
}

/// Two unit vectors perpendicular to unit `axis` and to each other.
fn frame(axis: Vector3) -> (Vector3, Vector3) {
    let helper = if axis.x().abs() < 0.9 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    }
    .expect("finite");
    let e1 = axis.cross(helper).normalized().expect("not parallel");
    (e1, axis.cross(e1))
}

/// How far along `axis` from `origin` the boundary reaches; `None` without
/// a boundary (the surface is unbounded that way).
fn height_range(
    boundary: &[(Point3, Point3)],
    origin: Point3,
    axis: Vector3,
) -> Option<(f64, f64)> {
    let heights = boundary.iter().map(|(p, _)| (*p - origin).dot(axis));
    let (lo, hi) = heights.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| {
        (lo.min(h), hi.max(h))
    });
    (lo < hi).then_some((lo, hi))
}

/// The face's outward unit normal near `p`: the surface's, turned when
/// the face is reversed.
pub(crate) fn outward(face: &Face, p: Point3) -> Option<Vector3> {
    let n = match face.surface {
        Surface::Plane(s) => Some(s.normal()),
        Surface::Cone(s) => {
            let d = p - s.origin();
            let h = d.dot(s.axis());
            let radial = (d - s.axis() * h).normalized().ok()?;
            Some(radial * s.cos_half_angle() - s.axis() * s.sin_half_angle())
        }
        Surface::EllipticCylinder(s) => {
            let e = s.section();
            let a = e.major().length();
            let b = a * e.ratio();
            let u = e.major() / a;
            let v = e.normal().cross(u);
            let d = p - e.center();
            (u * (d.dot(u) / (a * a)) + v * (d.dot(v) / (b * b)))
                .normalized()
                .ok()
        }
        Surface::Sphere(s) => (p - s.center()).normalized().ok(),
        Surface::Torus(s) => {
            let d = p - s.center();
            let h = d.dot(s.axis());
            let radial = (d - s.axis() * h).normalized().ok()?;
            let tube = s.center() + radial * s.major_radius();
            (p - tube).normalized().ok()
        }
        Surface::Nurbs(ref s) => s.normal_near(p),
    }?;
    Some(if face.reversed { -n } else { n })
}

/// A piece of a face's boundary: from `a` along `along`, with the
/// direction `left` pointing into the face beside it.
pub(crate) struct Guide {
    a: Point3,
    along: Vector3,
    left: Vector3,
}

/// How far `p`, on the face's surface, lies from the face's boundary, and
/// whether it is on the left of the nearest boundary segment seen from
/// outside: inside, when near.
pub(crate) fn side_of(guides: &[Guide], p: Point3) -> (f64, bool) {
    let mut nearest: Option<(f64, Point3, &Guide)> = None;
    for g in guides {
        let len2 = g.along.dot(g.along);
        if len2 == 0.0 {
            continue;
        }
        let t = ((p - g.a).dot(g.along) / len2).clamp(0.0, 1.0);
        let q = g.a + g.along * t;
        let d = q.distance_squared_to(p);
        if nearest.is_none_or(|(best, _, _)| d < best) {
            nearest = Some((d, q, g));
        }
    }
    let Some((d2, q, g)) = nearest else {
        return (0.0, true);
    };
    (d2.sqrt(), g.left.dot(p - q) > 0.0)
}

/// The pieces of an isoline inside its face. Near the boundary (within
/// `reach`) a sample's state is its side; farther, the state of the
/// sample before it, starting from the sample nearest the boundary. Each
/// change of state is located by bisection on the side.
fn clip(line: &Isoline, side: &dyn Fn(Point3) -> (f64, bool), reach: f64) -> Vec<Vec<Point3>> {
    let ts = sample(line);
    let probes: Vec<(f64, bool)> = ts.iter().map(|&t| side(line(t))).collect();
    let Some(seed) = (0..ts.len()).min_by(|&a, &b| probes[a].0.total_cmp(&probes[b].0)) else {
        return Vec::new();
    };
    if probes[seed].0 > reach {
        return Vec::new(); // never near its face: not guessed
    }
    let mut states = vec![false; ts.len()];
    states[seed] = probes[seed].1;
    let near = |i: usize| probes[i].0 <= reach;
    for i in seed + 1..ts.len() {
        states[i] = if near(i) { probes[i].1 } else { states[i - 1] };
    }
    for i in (0..seed).rev() {
        states[i] = if near(i) { probes[i].1 } else { states[i + 1] };
    }
    let crossing = |mut a: f64, mut b: f64, from: bool| {
        for _ in 0..40 {
            let m = (a + b) / 2.0;
            if side(line(m)).1 == from {
                a = m;
            } else {
                b = m;
            }
        }
        (a + b) / 2.0
    };
    let mut pieces = Vec::new();
    let mut current: Vec<Point3> = Vec::new();
    for i in 0..ts.len() {
        if states[i] {
            if current.is_empty() && i > 0 {
                current.push(line(crossing(ts[i - 1], ts[i], false)));
            }
            current.push(line(ts[i]));
            if i + 1 < ts.len() && !states[i + 1] {
                current.push(line(crossing(ts[i], ts[i + 1], true)));
                pieces.push(std::mem::take(&mut current));
            }
        }
    }
    if current.len() >= 2 {
        pieces.push(current);
    }
    pieces.retain(|p| p.len() >= 2);
    pieces
}

#[cfg(test)]
#[path = "tessellate_tests.rs"]
mod tests;
