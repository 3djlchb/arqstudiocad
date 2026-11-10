//! Booleans of solids with cylindrical and conical faces (ADR 0033,
//! delivery 2): planes, circular cylinders and circular cones, bounded by
//! lines, circles, ellipses, and arcs of parabolas and hyperbolas.
//!
//! # Method
//!
//! The planar booleans' (ADR 0032 D2), on curved faces:
//!
//! - **Domains:** each face is split in its own domain. A plane uses a frame
//!   in it; a cylinder or a cone its angle and height `(σθ, h)` (σ = ±1, so
//!   the domain turns about the face's outward normal), in a window of one
//!   turn that starts where nothing happens, closed by the seam where a
//!   region crosses it, and by the apex's line where a cone's region
//!   reaches it.
//! - **Sections:** the other solid's section on the face's surface is
//!   bounded by where its faces cut that surface, each kept where it lies
//!   in its own face, run with the other solid on its left. A plane cuts a
//!   plane in a line, a cylinder in an ellipse (two lines along it, a
//!   circle across it), a cone in a conic (two lines through its apex), kept
//!   exact; on a cylinder or a cone those curves are graphs `h(θ)`, rational
//!   in `cos θ` and `sin θ`, or lines.
//! - **Split and select:** [`CurvedRegion`]'s booleans split each face, and
//!   each operation keeps its parts as the planar booleans do.
//! - **Lift and assemble:** the parts are lifted back to lines, circles and
//!   ellipses, and parabolas and hyperbolas as rational quadratic NURBS
//!   (ADR 0033 D3), seams dropped, a cone's apex kept as the point its face
//!   closes on, corners welded, edges split where another face has a corner
//!   on them, shells grouped by their volume's sign, a void given to the
//!   smallest shell a ray from it crosses an odd number of times.
//!
//! Cylinders and cones meet flat faces only: two curved faces meeting are
//! delivery 4.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use arq_geometry::{CurvedRegion, Point2, Point3, Tolerance, Trim, Vector2, Vector3};

use crate::boolean::{BooleanError, Op, tolerance_of};
use crate::brep::{Body, BodyBuilder, BrepError, Coedge, Edge, Face, Loop, Lump, Shell};
use crate::chart::frame;
use crate::curve::{Curve, EllipseCurve, LineCurve};
use crate::nurbs::NurbsCurve;
use crate::surface::{Plane, Surface};
use crate::tessellate::ellipse_parameter;

/// A directed piece of an edge in space.
#[derive(Clone, Copy, Debug)]
enum Span {
    /// From `from` to `to`.
    Line { from: Point3, to: Point3 },
    /// `center + a cos t + b sin t`, `t` from `start` by `sweep`.
    Arc {
        center: Point3,
        a: Vector3,
        b: Vector3,
        start: f64,
        sweep: f64,
    },
    /// The rational quadratic arc from `from` to `to` pulled toward
    /// `control` by `weight`: a parabola's or a hyperbola's.
    Conic {
        from: Point3,
        control: Point3,
        to: Point3,
        weight: f64,
    },
}

impl Span {
    fn point(&self, t: f64) -> Point3 {
        match *self {
            Self::Line { from, to } => from + (to - from) * t,
            Self::Arc {
                center,
                a,
                b,
                start,
                sweep,
            } => {
                let (s, c) = (start + sweep * t).sin_cos();
                center + a * c + b * s
            }
            Self::Conic {
                from,
                control,
                to,
                weight,
            } => {
                let k = [(1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t) * weight, t * t];
                let d = k[0] + k[1] + k[2];
                from + ((control - from) * k[1] + (to - from) * k[2]) / d
            }
        }
    }

    fn derivative(&self, t: f64) -> Vector3 {
        match *self {
            Self::Line { from, to } => to - from,
            Self::Arc {
                a, b, start, sweep, ..
            } => {
                let (s, c) = (start + sweep * t).sin_cos();
                (b * c - a * s) * sweep
            }
            Self::Conic {
                from,
                control,
                to,
                weight,
            } => {
                // p = from + N / D, N = k₁ (C − F) + k₂ (T − F), D = Σ k.
                let k = [(1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t) * weight, t * t];
                let dk = [-2.0 * (1.0 - t), 2.0 * (1.0 - 2.0 * t) * weight, 2.0 * t];
                let (d, dd) = (k[0] + k[1] + k[2], dk[0] + dk[1] + dk[2]);
                let (c, e) = (control - from, to - from);
                let (n, dn) = (c * k[1] + e * k[2], c * dk[1] + e * dk[2]);
                (dn * d - n * dd) / (d * d)
            }
        }
    }

    fn reversed(&self) -> Self {
        match *self {
            Self::Line { from, to } => Self::Line { from: to, to: from },
            Self::Arc {
                center,
                a,
                b,
                start,
                sweep,
            } => Self::Arc {
                center,
                a,
                b,
                start: start + sweep,
                sweep: -sweep,
            },
            Self::Conic {
                from,
                control,
                to,
                weight,
            } => Self::Conic {
                from: to,
                control,
                to: from,
                weight,
            },
        }
    }

    fn part(&self, t0: f64, t1: f64) -> Self {
        match *self {
            Self::Line { .. } => Self::Line {
                from: self.point(t0),
                to: self.point(t1),
            },
            Self::Arc {
                center,
                a,
                b,
                start,
                sweep,
            } => Self::Arc {
                center,
                a,
                b,
                start: start + sweep * t0,
                sweep: sweep * (t1 - t0),
            },
            Self::Conic {
                from,
                control,
                to,
                weight,
            } => {
                // Homogeneous control points; the part's are the blossom's
                // values, its ends' weights brought back to 1.
                let h = |p: Point3, w: f64| (p - Point3::ORIGIN, w);
                let (f, c, e) = (h(from, 1.0), h(control, weight), h(to, 1.0));
                let blossom = |a: f64, b: f64| {
                    let k = [(1.0 - a) * (1.0 - b), (1.0 - a) * b + a * (1.0 - b), a * b];
                    (
                        f.0 * (k[0] * f.1) + c.0 * (k[1] * c.1) + e.0 * (k[2] * e.1),
                        k[0] * f.1 + k[1] * c.1 + k[2] * e.1,
                    )
                };
                let (p0, p1, p2) = (blossom(t0, t0), blossom(t0, t1), blossom(t1, t1));
                let point = |q: (Vector3, f64)| Point3::ORIGIN + q.0 / q.1;
                Self::Conic {
                    from: point(p0),
                    control: point(p1),
                    to: point(p2),
                    weight: p1.1 / (p0.1 * p2.1).sqrt(),
                }
            }
        }
    }

    /// The parameter of `p`, a point of the span: on an arc read from its
    /// angle (never the nearest point, which near a closed arc's start can
    /// be its end); on a conic found by Newton's method from `guess`.
    fn parameter_near(&self, p: Point3, guess: f64) -> f64 {
        match *self {
            Self::Line { .. } => self.project(p).0,
            Self::Arc {
                center,
                a,
                b,
                start,
                sweep,
            } => {
                // p − center = a cos φ + b sin φ, a and b conjugate.
                let w = p - center;
                let (aa, ab, bb) = (a.dot(a), a.dot(b), b.dot(b));
                let (wa, wb) = (w.dot(a), w.dot(b));
                let det = aa * bb - ab * ab;
                let (c, s) = ((wa * bb - wb * ab) / det, (wb * aa - wa * ab) / det);
                ((s.atan2(c) - start) / sweep).rem_euclid(TAU / sweep.abs())
            }
            Self::Conic { .. } => {
                let mut t = guess.clamp(0.0, 1.0);
                for _ in 0..50 {
                    let (q, d) = (self.point(t), self.derivative(t));
                    let dd = d.dot(d);
                    if dd <= 1e-300 {
                        break;
                    }
                    let next = (t - (q - p).dot(d) / dd).clamp(0.0, 1.0);
                    if (next - t).abs() <= 1e-16 {
                        t = next;
                        break;
                    }
                    t = next;
                }
                t
            }
        }
    }

    /// The parameter of the point of the span nearest `p`, and its distance.
    fn project(&self, p: Point3) -> (f64, f64) {
        match *self {
            Self::Line { from, to } => {
                let d = to - from;
                let s = ((p - from).dot(d) / d.dot(d)).clamp(0.0, 1.0);
                (s, (self.point(s) - p).length())
            }
            Self::Arc { .. } | Self::Conic { .. } => {
                let n = 64;
                let mut best = (0.0, f64::INFINITY);
                for i in 0..=n {
                    let s = i as f64 / n as f64;
                    let d = (self.point(s) - p).length();
                    if d < best.1 {
                        best = (s, d);
                    }
                }
                let mut s = best.0;
                for _ in 0..30 {
                    let (q, d) = (self.point(s), self.derivative(s));
                    let h = 1e-6;
                    let dd = (self.derivative((s + h).min(1.0))
                        - self.derivative((s - h).max(0.0)))
                        / ((s + h).min(1.0) - (s - h).max(0.0));
                    let w = q - p;
                    let f = w.dot(d);
                    let df = d.dot(d) + w.dot(dd);
                    if df.abs() <= 1e-300 {
                        break;
                    }
                    s = (s - f / df).clamp(0.0, 1.0);
                }
                let d = (self.point(s) - p).length();
                if d < best.1 { (s, d) } else { best }
            }
        }
    }
}

/// Where a face lives: a plane, or a circular cone or cylinder, with its
/// outward normal.
#[derive(Clone, Copy, Debug)]
enum Domain {
    /// The plane through `origin` spanned by `u` and `v`, `n = u × v`
    /// outward.
    Plane {
        origin: Point3,
        u: Vector3,
        v: Vector3,
        n: Vector3,
    },
    /// The cone round the unit `axis` through `origin`, of radius `r + k h`
    /// at height `h` along the axis (a cylinder when `k` is 0), `θ` from
    /// `e1` toward `e2 = axis × e1`; `sigma` +1 when its outside is away
    /// from the axis, −1 when toward it; the window of `x = σθ` starts at
    /// `x0`. A cone's face lies on the side of its apex where the radius is
    /// positive.
    Cone {
        origin: Point3,
        axis: Vector3,
        r: f64,
        k: f64,
        e1: Vector3,
        e2: Vector3,
        sigma: f64,
        x0: f64,
    },
}

impl Domain {
    /// The outward unit normal at `p`.
    fn normal(&self, p: Point3) -> Vector3 {
        match *self {
            Self::Plane { n, .. } => n,
            Self::Cone {
                origin,
                axis,
                k,
                sigma,
                ..
            } => {
                // Square to the side: outward from the axis, tipped back
                // along it as the radius grows.
                let d = p - origin;
                let radial = d - axis * d.dot(axis);
                radial
                    .normalized()
                    .and_then(|r| (r - axis * k).normalized())
                    .map_or(axis, |n| n * sigma)
            }
        }
    }

    /// The radius at height `h` of a cone, its scale along the angle; 1 on
    /// a plane.
    fn radius_at(&self, h: f64) -> f64 {
        match *self {
            Self::Plane { .. } => 1.0,
            Self::Cone { r, k, .. } => r + k * h,
        }
    }

    /// A cone's apex height, none on a cylinder or a plane.
    fn apex(&self) -> Option<f64> {
        match *self {
            Self::Cone { r, k, .. } if k != 0.0 => Some(-r / k),
            _ => None,
        }
    }

    /// How far `p` lies from a cone's axis.
    fn off_axis(&self, p: Point3) -> f64 {
        match *self {
            Self::Plane { .. } => 0.0,
            Self::Cone { origin, axis, .. } => {
                let d = p - origin;
                (d - axis * d.dot(axis)).length()
            }
        }
    }

    /// `p` in the domain; on a cylinder, its `x` brought into the window.
    fn flat(&self, p: Point3) -> [f64; 2] {
        match *self {
            Self::Plane { origin, u, v, .. } => {
                let w = p - origin;
                [w.dot(u), w.dot(v)]
            }
            Self::Cone {
                origin,
                axis,
                e1,
                e2,
                sigma,
                x0,
                ..
            } => {
                let d = p - origin;
                let x = sigma * d.dot(e2).atan2(d.dot(e1));
                [x0 + (x - x0).rem_euclid(TAU), d.dot(axis)]
            }
        }
    }

    /// The point of the surface at `(x, y)` of the domain.
    fn lift(&self, q: [f64; 2]) -> Point3 {
        match *self {
            Self::Plane { origin, u, v, .. } => origin + u * q[0] + v * q[1],
            Self::Cone {
                origin,
                axis,
                r,
                k,
                e1,
                e2,
                sigma,
                ..
            } => {
                let (s, c) = (sigma * q[0]).sin_cos();
                origin + (e1 * c + e2 * s) * (r + k * q[1]) + axis * q[1]
            }
        }
    }
}

/// A face read: its surface and orientation as the body has them, its
/// domain, and its loops as spans.
#[derive(Clone, Debug)]
struct CFace {
    surface: Surface,
    reversed: bool,
    domain: Domain,
    loops: Vec<Vec<Span>>,
}

/// A body read as faces of planes and cylinders.
struct CSolid {
    faces: Vec<CFace>,
    /// Each face's region in its domain.
    regions: Vec<CurvedRegion>,
}

/// A face of the result: its surface, orientation, domain and region;
/// `flipped` when its region was turned over against its domain (a face of
/// the solid subtracted), so its outside is against the domain's normal.
struct Part {
    surface: Surface,
    reversed: bool,
    flipped: bool,
    domain: Domain,
    region: CurvedRegion,
}

/// Whether this module takes `body`: faces of planes, cylinders and cones;
/// edges of lines, ellipses and conic arcs (rational quadratic NURBS), and
/// a cone's apex.
pub(crate) fn takes(body: &Body) -> bool {
    body.faces()
        .iter()
        .all(|f| matches!(f.surface, Surface::Plane(_) | Surface::Cone(_)))
        && body.edges().iter().all(|e| match &e.curve {
            Some(Curve::Line(_) | Curve::Ellipse(_)) => true,
            Some(Curve::Nurbs(n)) => conic_of(n).is_some(),
            None => e.start == e.end,
        })
}

/// A NURBS curve that is one rational quadratic arc: its ends, its control
/// point, and the control point's weight with the ends' brought to 1.
fn conic_of(n: &NurbsCurve) -> Option<(Point3, Point3, Point3, f64)> {
    let (k, w, p) = (n.knots(), n.weights(), n.points());
    let one = n.degree() == 2 && p.len() == 3 && k.len() == 6 && k[0] == k[2] && k[3] == k[5];
    one.then(|| (p[0], p[1], p[2], w[1] / (w[0] * w[2]).sqrt()))
}

pub(crate) fn boolean(a: &Body, b: &Body, op: Op) -> Result<Option<Body>, BooleanError> {
    let points = a.vertices().iter().chain(b.vertices()).map(|v| v.point);
    let mut extent: Vec<Point3> = points.collect();
    for body in [a, b] {
        if let Some(bounds) = body.mass_properties().ok().map(|m| m.bounds) {
            extent.push(bounds.min());
            extent.push(bounds.max());
        }
    }
    let tol = tolerance_of(&extent)?;
    let (sa, sb) = (read(a, tol)?, read(b, tol)?);
    let mut parts = Vec::new();
    for (own, other, first) in [(&sa, &sb, true), (&sb, &sa, false)] {
        for (face, region) in own.faces.iter().zip(&own.regions) {
            let (face, region, inner, outer) = framed(face, region, other, tol)?;
            let (face, region) = (&face, &region);
            // The planar booleans' rules (ADR 0032 D2.4, 0033 delivery 2.3):
            // the other solid just inside the face (L) and just outside (U).
            let kept = match (op, first) {
                (Op::Union, true) => region.difference(&outer, tol),
                (Op::Intersection, true) => region.intersection(&inner, tol),
                (Op::Difference, true) => region.difference(&inner, tol),
                // L and U alike (general position) need no boolean of their own.
                (Op::Union, false) if inner == outer => region.difference(&inner, tol),
                (Op::Union, false) => region.difference(&inner.union(&outer, tol), tol),
                (Op::Intersection | Op::Difference, false) if inner == outer => {
                    region.intersection(&inner, tol)
                }
                (Op::Intersection | Op::Difference, false) => {
                    region.intersection(&inner.intersection(&outer, tol), tol)
                }
            };
            let flip = op == Op::Difference && !first;
            for piece in across_seam(&face.domain, kept.pieces(tol), tol) {
                parts.push(Part {
                    surface: face.surface.clone(),
                    reversed: face.reversed ^ flip,
                    flipped: flip,
                    domain: face.domain,
                    region: if flip {
                        reverse_region(&piece, tol)?
                    } else {
                        piece
                    },
                });
            }
        }
    }
    let parts = merge_coplanar(parts, tol)?;
    if parts.is_empty() {
        return Ok(None);
    }
    assemble(&parts, tol).map(Some)
}

/// The kept flat parts lying in one plane and facing the same way, merged:
/// lifted, brought into one frame of that plane and united, so the edges
/// they shared are gone (ADR 0032 D2.5).
fn merge_coplanar(parts: Vec<Part>, tol: Tolerance) -> Result<Vec<Part>, BooleanError> {
    let t = tol.linear();
    let outward = |p: &Part| match p.domain {
        Domain::Plane { n, origin, .. } => Some((if p.flipped { -n } else { n }, origin)),
        Domain::Cone { .. } => None,
    };
    let mut groups: Vec<Vec<Part>> = Vec::new();
    let mut out = Vec::new();
    for part in parts {
        let Some((n, o)) = outward(&part) else {
            out.push(part);
            continue;
        };
        let same = groups.iter_mut().find(|g| {
            let (m, q) = outward(&g[0]).expect("flat");
            (m - n).length() <= tol.angular() && (o - q).dot(m).abs() <= t
        });
        match same {
            Some(g) => g.push(part),
            None => groups.push(vec![part]),
        }
    }
    for group in groups {
        if group.len() == 1 {
            out.extend(group);
            continue;
        }
        let (n, origin) = outward(&group[0]).expect("flat");
        let (u, v) = frame(n);
        let domain = Domain::Plane { origin, u, v, n };
        let mut merged = CurvedRegion::default();
        for part in &group {
            let mut contours = Vec::new();
            for contour in part.region.contours() {
                let spans = lift_contour(&part.domain, contour, t)?;
                contours.push(
                    spans
                        .iter()
                        .map(|s| trim_on(&domain, s))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            let region = CurvedRegion::new(contours, tol).map_err(|_| BooleanError::NonManifold)?;
            merged = merged.union(&region, tol);
        }
        let plane = Plane::new(origin, n).map_err(geometry)?;
        for piece in merged.pieces(tol) {
            out.push(Part {
                surface: Surface::Plane(plane),
                reversed: false,
                flipped: false,
                domain,
                region: piece,
            });
        }
    }
    Ok(out)
}

/// A cylinder's pieces joined again where the window's seam cut one face
/// in two: a piece meeting the window's right edge over some heights is one
/// face with the piece meeting its left edge over the same heights.
fn across_seam(domain: &Domain, pieces: Vec<CurvedRegion>, tol: Tolerance) -> Vec<CurvedRegion> {
    let Domain::Cone { x0, .. } = *domain else {
        return pieces;
    };
    let t = tol.linear();
    // Each piece's stretches of seam, left and right, as height ranges.
    let seams = |piece: &CurvedRegion, x: f64| -> Vec<(f64, f64)> {
        piece
            .contours()
            .iter()
            .flatten()
            .filter_map(|trim| match trim {
                Trim::Segment { from, to }
                    if (from.x() - x).abs() <= t && (to.x() - x).abs() <= t =>
                {
                    Some((from.y().min(to.y()), from.y().max(to.y())))
                }
                _ => None,
            })
            .collect()
    };
    let left: Vec<Vec<(f64, f64)>> = pieces.iter().map(|p| seams(p, x0)).collect();
    let right: Vec<Vec<(f64, f64)>> = pieces.iter().map(|p| seams(p, x0 + TAU)).collect();
    let mut parent: Vec<usize> = (0..pieces.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (i, ri) in right.iter().enumerate() {
        for (j, lj) in left.iter().enumerate() {
            let meets = ri
                .iter()
                .any(|&(a, b)| lj.iter().any(|&(c, d)| a.max(c) < b.min(d) - t));
            if meets {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut groups: Vec<(usize, Vec<Vec<Trim>>)> = Vec::new();
    for (i, piece) in pieces.iter().enumerate() {
        let r = root(&mut parent, i);
        match groups.iter_mut().find(|g| g.0 == r) {
            Some(g) => g.1.extend(piece.contours().iter().cloned()),
            None => groups.push((r, piece.contours().to_vec())),
        }
    }
    groups
        .into_iter()
        .filter_map(|(_, contours)| CurvedRegion::new(contours, tol).ok())
        .collect()
}

/// `region` with every contour run the other way: seen from the other side.
fn reverse_region(region: &CurvedRegion, tol: Tolerance) -> Result<CurvedRegion, BooleanError> {
    let contours = region
        .contours()
        .iter()
        .map(|c| c.iter().rev().map(Trim::reversed).collect())
        .collect();
    CurvedRegion::new(contours, tol).map_err(|_| BooleanError::NonManifold)
}

// ---------------------------------------------------------------- reading

/// The body as faces of planes, cylinders and cones, each with its region.
fn read(body: &Body, tol: Tolerance) -> Result<CSolid, BooleanError> {
    let mut ids: Vec<usize> = body
        .lumps()
        .iter()
        .flat_map(|l| &l.shells)
        .flat_map(|s| &body.shells()[s.index()].faces)
        .map(|f| f.index())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut faces = Vec::new();
    for i in ids {
        let face = &body.faces()[i];
        let mut loops = Vec::new();
        for l in &face.loops {
            let mut spans = Vec::new();
            for c in &body.loops()[l.index()].coedges {
                let coedge = &body.coedges()[c.index()];
                let edge = body.edge(coedge.edge);
                if edge.curve.is_none() {
                    continue; // a cone's apex: its line closes the region
                }
                let span = span_of(body, edge)?;
                spans.push(if coedge.reversed {
                    span.reversed()
                } else {
                    span
                });
            }
            if !spans.is_empty() {
                loops.push(spans);
            }
        }
        if loops.is_empty() {
            return Err(BooleanError::NotSolid);
        }
        let domain = match &face.surface {
            Surface::Plane(p) => {
                let n = if face.reversed {
                    -p.normal()
                } else {
                    p.normal()
                };
                let (u, v) = frame(n);
                Domain::Plane {
                    origin: loops[0][0].point(0.0),
                    u,
                    v,
                    n,
                }
            }
            Surface::Cone(c) => {
                let axis = c.axis();
                let e1 = c.reference();
                Domain::Cone {
                    origin: c.origin(),
                    axis,
                    r: c.radius(),
                    k: c.sin_half_angle() / c.cos_half_angle(),
                    e1,
                    e2: axis.cross(e1),
                    sigma: if face.reversed { -1.0 } else { 1.0 },
                    x0: 0.0,
                }
            }
            _ => return Err(BooleanError::Curved),
        };
        faces.push(CFace {
            surface: face.surface.clone(),
            reversed: face.reversed,
            domain,
            loops,
        });
    }
    let mut regions = Vec::with_capacity(faces.len());
    for face in &mut faces {
        if let Domain::Cone { .. } = face.domain {
            let trims: Vec<Vec<Trim>> = face
                .loops
                .iter()
                .map(|l| {
                    l.iter()
                        .map(|s| trim_on(&face.domain, s))
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?;
            let x0 = quiet_start(trims.iter().flatten());
            if let Domain::Cone { x0: start, .. } = &mut face.domain {
                *start = x0;
            }
        }
        let trims: Vec<Trim> = face
            .loops
            .iter()
            .flatten()
            .map(|s| trim_on(&face.domain, s))
            .collect::<Result<_, _>>()?;
        regions.push(close(&face.domain, trims, tol)?);
    }
    Ok(CSolid { faces, regions })
}

/// An edge as a span, run its own way.
fn span_of(body: &Body, edge: &Edge) -> Result<Span, BooleanError> {
    let (start, end) = (body.vertex(edge.start).point, body.vertex(edge.end).point);
    match &edge.curve {
        Some(Curve::Line(_)) => Ok(Span::Line {
            from: start,
            to: end,
        }),
        Some(Curve::Ellipse(e)) => {
            let (ts, te) = (ellipse_parameter(e, start), ellipse_parameter(e, end));
            let sweep = if edge.start == edge.end {
                TAU
            } else if edge.reversed {
                (ts - te).rem_euclid(TAU)
            } else {
                (te - ts).rem_euclid(TAU)
            };
            let minor = e.normal().cross(e.major()) * e.ratio();
            Ok(Span::Arc {
                center: e.center(),
                a: e.major(),
                b: minor,
                start: ts,
                sweep: if edge.reversed { -sweep } else { sweep },
            })
        }
        Some(Curve::Nurbs(n)) => {
            let (from, control, to, weight) = conic_of(n).ok_or(BooleanError::Curved)?;
            let span = Span::Conic {
                from,
                control,
                to,
                weight,
            };
            Ok(if edge.reversed { span.reversed() } else { span })
        }
        None => Err(BooleanError::Curved),
    }
}

// ---------------------------------------------------------------- domains

/// `span`, which lies on the face, in the face's domain; on a cylinder or
/// a cone unwrapped from its start, not yet brought into the window.
fn trim_on(domain: &Domain, span: &Span) -> Result<Trim, BooleanError> {
    let pt = |q: [f64; 2]| Point2::new(q[0], q[1]).map_err(geometry);
    match (*domain, *span) {
        (Domain::Plane { origin, u, v, .. }, Span::Line { from, to }) => {
            let f = |p: Point3| [(p - origin).dot(u), (p - origin).dot(v)];
            Ok(Trim::Segment {
                from: pt(f(from))?,
                to: pt(f(to))?,
            })
        }
        (
            Domain::Plane { origin, u, v, .. },
            Span::Arc {
                center,
                a,
                b,
                start,
                sweep,
            },
        ) => {
            let c = center - origin;
            Ok(Trim::Ellipse {
                center: pt([c.dot(u), c.dot(v)])?,
                a: Vector2::new(a.dot(u), a.dot(v)).map_err(geometry)?,
                b: Vector2::new(b.dot(u), b.dot(v)).map_err(geometry)?,
                start,
                sweep,
            })
        }
        (
            Domain::Plane { origin, u, v, .. },
            Span::Conic {
                from,
                control,
                to,
                weight,
            },
        ) => {
            // An affine map keeps a rational arc's weights.
            let f = |p: Point3| pt([(p - origin).dot(u), (p - origin).dot(v)]);
            Ok(Trim::Conic {
                from: f(from)?,
                control: f(control)?,
                to: f(to)?,
                weight,
            })
        }
        (Domain::Cone { .. }, Span::Line { from, to }) => {
            let (p, q) = (domain.flat(from), domain.flat(to));
            // A line on a cylinder or a cone runs along it: one angle, read
            // at the end off the axis (a cone's apex has none).
            let x = if domain.off_axis(from) >= domain.off_axis(to) {
                p[0]
            } else {
                q[0]
            };
            Ok(Trim::Segment {
                from: pt([x, p[1]])?,
                to: pt([x, q[1]])?,
            })
        }
        (
            Domain::Cone {
                origin,
                axis,
                r,
                k,
                e1,
                e2,
                sigma,
                ..
            },
            Span::Arc { .. } | Span::Conic { .. },
        ) => {
            // The arc's points turn round the axis: θ at its start, and how
            // far it turns as the arc does.
            let theta = |p: Point3| {
                let d = p - origin;
                d.dot(e2).atan2(d.dot(e1))
            };
            let mut turned = 0.0;
            let mut last = theta(span.point(0.0));
            let n = 64;
            for i in 1..=n {
                let now = theta(span.point(i as f64 / n as f64));
                let mut d = now - last;
                if d > PI {
                    d -= TAU;
                } else if d < -PI {
                    d += TAU;
                }
                turned += d;
                last = now;
            }
            let x_from = sigma * theta(span.point(0.0));
            let x_to = x_from + sigma * turned;
            // The plane the curve lies in, and a point of it.
            let (normal, on) = match *span {
                Span::Arc { center, a, b, .. } => (a.cross(b), center),
                Span::Conic {
                    from, control, to, ..
                } => ((control - from).cross(to - from), from),
                Span::Line { .. } => return Err(BooleanError::NotSolid),
            };
            let along = normal.normalized().map_or(0.0, |m| m.dot(axis).abs());
            let rise = |p: Point3| (p - origin).dot(axis);
            if (along - 1.0).abs() <= 1e-9 {
                // A circle across the axis: one height.
                let h = rise(on);
                Ok(Trim::Segment {
                    from: pt([x_from, h])?,
                    to: pt([x_to, h])?,
                })
            } else {
                // Where a plane m·p = c cuts the side, its height over each
                // angle: (r + k h) m·e(θ) + h m·axis = c, so
                // h = (c − r m·e(θ)) / (m·axis + k m·e(θ)).
                let m = normal.normalized().map_err(geometry)?;
                let c = m.dot(on - origin);
                let den = m.dot(axis);
                if k == 0.0 && den.abs() <= 1e-12 {
                    return Err(BooleanError::NotSolid);
                }
                let (ce, se) = (m.dot(e1), m.dot(e2));
                // In x = σθ: cos θ = cos x, sin θ = σ sin x.
                Ok(Trim::Graph {
                    num: [c, -r * ce, -sigma * r * se],
                    den: [den, k * ce, sigma * k * se],
                    from: x_from,
                    to: x_to,
                })
            }
        }
    }
}

/// The angles in `[0, 2π)` where `a + b cos x + c sin x = 0`.
fn trig_roots(a: f64, b: f64, c: f64) -> Vec<f64> {
    let r = b.hypot(c);
    if r <= 1e-300 {
        return Vec::new();
    }
    let v = -a / r;
    if v.abs() > 1.0 {
        return Vec::new();
    }
    // b cos x + c sin x = r cos(x − φ).
    let (phi, spread) = (c.atan2(b), v.acos());
    let mut out = vec![
        (phi - spread).rem_euclid(TAU),
        (phi + spread).rem_euclid(TAU),
    ];
    out.sort_by(f64::total_cmp);
    out.dedup_by(|x, y| (*x - *y).abs() < 1e-15);
    out
}

/// A window start away from every corner and every turn of the trims: the
/// middle of the widest gap between them.
fn quiet_start<'a>(trims: impl Iterator<Item = &'a Trim>) -> f64 {
    let mut events: Vec<f64> = Vec::new();
    for t in trims {
        events.push(t.start().x().rem_euclid(TAU));
        events.push(t.end().x().rem_euclid(TAU));
        if let Trim::Graph { num: n, den: d, .. } = t {
            // Where h = n / d turns: n′d − n d′ = 0, which is
            // (n₂d₁ − n₁d₂) + (d₀n₂ − n₀d₂) cos x + (n₀d₁ − d₀n₁) sin x.
            events.extend(trig_roots(
                n[2] * d[1] - n[1] * d[2],
                d[0] * n[2] - n[0] * d[2],
                n[0] * d[1] - d[0] * n[1],
            ));
        }
    }
    events.sort_by(f64::total_cmp);
    events.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    if events.is_empty() {
        return 0.0;
    }
    let mut best = (
        events[0] + TAU - events[events.len() - 1],
        events[events.len() - 1],
    );
    for w in events.windows(2) {
        if w[1] - w[0] > best.0 {
            best = (w[1] - w[0], w[0]);
        }
    }
    (best.1 + best.0 / 2.0).rem_euclid(TAU)
}

/// The region bounded by `trims` in `domain`: on a cylinder or a cone,
/// brought into the window and closed by the seam, and on a cone by its
/// apex's line where the region reaches it.
fn close(domain: &Domain, trims: Vec<Trim>, tol: Tolerance) -> Result<CurvedRegion, BooleanError> {
    let Domain::Cone { x0, .. } = *domain else {
        return Ok(CurvedRegion::from_trims(&trims, tol));
    };
    let t = tol.linear();
    let mut inside: Vec<Trim> = Vec::new();
    for trim in trims {
        inside.extend(into_window(&trim, x0, t)?);
    }
    if let Some(apex) = domain.apex() {
        inside.extend(along_apex(domain, &inside, x0, apex, t)?);
    }
    // Where the region's boundary meets the seam, from below: outside, then
    // inside and outside in turn.
    let mut heights: Vec<f64> = Vec::new();
    for trim in &inside {
        for p in [trim.start(), trim.end()] {
            if (p.x() - x0).abs() <= t || (p.x() - x0 - TAU).abs() <= t {
                heights.push(p.y());
            }
        }
    }
    heights.sort_by(f64::total_cmp);
    heights.dedup_by(|a, b| (*a - *b).abs() <= t);
    let point = |x: f64, y: f64| Point2::new(x, y).map_err(geometry);
    if heights.len() % 2 == 1 {
        // A cone's region reaching its apex: the apex's line closes it,
        // run with the region, above it or below it, on its left.
        let Some(apex) = domain.apex() else {
            return Err(BooleanError::NonManifold);
        };
        let (from, to) = if heights.iter().all(|&h| h > apex) {
            (x0, x0 + TAU)
        } else if heights.iter().all(|&h| h < apex) {
            (x0 + TAU, x0)
        } else {
            return Err(BooleanError::NonManifold);
        };
        inside.push(Trim::Segment {
            from: point(from, apex)?,
            to: point(to, apex)?,
        });
        heights.push(apex);
        heights.sort_by(f64::total_cmp);
    }
    for pair in heights.chunks(2) {
        let (low, high) = (pair[0], pair[1]);
        inside.push(Trim::Segment {
            from: point(x0, high)?,
            to: point(x0, low)?,
        });
        inside.push(Trim::Segment {
            from: point(x0 + TAU, low)?,
            to: point(x0 + TAU, high)?,
        });
    }
    Ok(CurvedRegion::from_trims(&inside, tol))
}

/// Where a cone's region runs into its apex along one curve and out along
/// another (lines through the apex, the apex a corner): the apex's line
/// between them, in the window from `x0`, run with the region on its left,
/// which lies on the side of the apex where the radius is positive.
fn along_apex(
    domain: &Domain,
    trims: &[Trim],
    x0: f64,
    apex: f64,
    t: f64,
) -> Result<Vec<Trim>, BooleanError> {
    let Domain::Cone { k, .. } = *domain else {
        return Ok(Vec::new());
    };
    let at_apex = |p: Point2| (p.y() - apex).abs() <= t;
    let flat = |trim: &Trim| at_apex(trim.start()) && at_apex(trim.end());
    let ends: Vec<f64> = trims
        .iter()
        .filter(|tr| at_apex(tr.end()) && !flat(tr))
        .map(|tr| tr.end().x())
        .collect();
    let starts: Vec<f64> = trims
        .iter()
        .filter(|tr| at_apex(tr.start()) && !flat(tr))
        .map(|tr| tr.start().x())
        .collect();
    // Above the apex the region's left is ahead in x, below it behind.
    let ahead = k > 0.0;
    let point = |x: f64| Point2::new(x, apex).map_err(geometry);
    let mut out = Vec::new();
    for &xe in &ends {
        let gap = |xs: f64| {
            if ahead {
                (xs - xe).rem_euclid(TAU)
            } else {
                (xe - xs).rem_euclid(TAU)
            }
        };
        let Some(d) = starts.iter().map(|&xs| gap(xs)).min_by(f64::total_cmp) else {
            return Err(BooleanError::NonManifold);
        };
        if d <= 1e-12 {
            continue; // out where it came in: a corner, nothing between
        }
        // Into the window: across the seam in two.
        let xs = if ahead { xe + d } else { xe - d };
        if xs > x0 + TAU + 1e-12 {
            out.push(Trim::Segment {
                from: point(xe)?,
                to: point(x0 + TAU)?,
            });
            out.push(Trim::Segment {
                from: point(x0)?,
                to: point(xs - TAU)?,
            });
        } else if xs < x0 - 1e-12 {
            out.push(Trim::Segment {
                from: point(xe)?,
                to: point(x0)?,
            });
            out.push(Trim::Segment {
                from: point(x0 + TAU)?,
                to: point(xs + TAU)?,
            });
        } else {
            out.push(Trim::Segment {
                from: point(xe)?,
                to: point(xs)?,
            });
        }
    }
    Ok(out)
}

/// The parts of `trim` in the window from `x0`, each shifted into it.
fn into_window(trim: &Trim, x0: f64, t: f64) -> Result<Vec<Trim>, BooleanError> {
    let (xa, xb) = (trim.start().x(), trim.end().x());
    let (lo, hi) = (xa.min(xb), xa.max(xb));
    // The seams the trim crosses, x0 + 2πk strictly inside.
    let k0 = ((lo - x0) / TAU).floor() as i64;
    let k1 = ((hi - x0) / TAU).ceil() as i64;
    let mut cuts: Vec<f64> = (k0..=k1)
        .map(|k| x0 + TAU * k as f64)
        .filter(|&x| x > lo + t && x < hi - t)
        .map(|x| (x - xa) / (xb - xa))
        .collect();
    cuts.sort_by(f64::total_cmp);
    let mut out = Vec::new();
    let mut from = 0.0;
    for s in cuts.into_iter().chain([1.0]) {
        let part = trim.part(from, s);
        let mid = part.point(0.5).x();
        let shift = TAU * ((mid - x0) / TAU).floor();
        out.push(shifted(&part, -shift)?);
        from = s;
    }
    Ok(out)
}

/// `trim` moved `dx` along x; a graph's function moved with it.
fn shifted(trim: &Trim, dx: f64) -> Result<Trim, BooleanError> {
    let pt = |p: Point2| Point2::new(p.x() + dx, p.y()).map_err(geometry);
    Ok(match *trim {
        Trim::Segment { from, to } => Trim::Segment {
            from: pt(from)?,
            to: pt(to)?,
        },
        Trim::Graph { num, den, from, to } => {
            // A whole number of turns: cos and sin are unchanged.
            Trim::Graph {
                num,
                den,
                from: from + dx,
                to: to + dx,
            }
        }
        other => other,
    })
}

// ---------------------------------------------------------------- sections

/// The part of `face`'s surface inside `other`, as a region of the face's
/// domain.
/// The face, its region and the other solid's section on it. On a cylinder
/// the window starts away from the section's curves too, not only from the
/// face's own edges: a curve lying where the seam is would be taken for it.
fn framed(
    face: &CFace,
    region: &CurvedRegion,
    other: &CSolid,
    tol: Tolerance,
) -> Result<(CFace, CurvedRegion, CurvedRegion, CurvedRegion), BooleanError> {
    let mut cuts = Vec::new();
    for (g, r) in other.faces.iter().zip(&other.regions) {
        cuts.extend(cut(face, g, r, tol)?);
    }
    let Domain::Cone { .. } = face.domain else {
        let (inner, outer) = sides(&face.domain, &cuts, tol)?;
        return Ok((face.clone(), region.clone(), inner, outer));
    };
    let own: Vec<Trim> = face
        .loops
        .iter()
        .flatten()
        .map(|s| trim_on(&face.domain, s))
        .collect::<Result<_, _>>()?;
    let mut moved = face.clone();
    if let Domain::Cone { x0, .. } = &mut moved.domain {
        *x0 = quiet_start(own.iter().chain(cuts.iter().map(|(t, _)| t)));
    }
    let region = close(&moved.domain, own, tol)?;
    let (inner, outer) = sides(&moved.domain, &cuts, tol)?;
    Ok((moved, region, inner, outer))
}

/// The other solid just inside a face (L) and just outside it (U), from the
/// section's pieces and the sides they bound.
fn sides(
    domain: &Domain,
    cuts: &[(Trim, Reach)],
    tol: Tolerance,
) -> Result<(CurvedRegion, CurvedRegion), BooleanError> {
    let pick = |want: Reach| {
        cuts.iter()
            .filter(|(_, r)| *r == Reach::Both || *r == want)
            .map(|(t, _)| *t)
            .collect::<Vec<_>>()
    };
    // In general position every piece bounds both sides: one section.
    if cuts.iter().all(|(_, r)| *r == Reach::Both) {
        let both = close(domain, pick(Reach::Both), tol)?;
        return Ok((both.clone(), both));
    }
    Ok((
        close(domain, pick(Reach::Inner), tol)?,
        close(domain, pick(Reach::Outer), tol)?,
    ))
}

/// Which side of a face a piece of the other solid's section bounds: a
/// curve through the inside of its own face, both; one along its own
/// face's edge, only the side that face goes on to (ADR 0033 delivery 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reach {
    Both,
    Inner,
    Outer,
}

/// How far `p` lies outside the face's surface: along its outward normal.
fn height(domain: &Domain, p: Point3) -> f64 {
    match *domain {
        Domain::Plane { origin, n, .. } => (p - origin).dot(n),
        Domain::Cone {
            origin,
            axis,
            r,
            k,
            sigma,
            ..
        } => {
            let d = p - origin;
            let h = d.dot(axis);
            sigma * ((d - axis * h).length() - (r + k * h)) / k.hypot(1.0)
        }
    }
}

/// Where `g`'s face cuts `face`'s surface, the parts in `g` (its `region`),
/// as trims of `face`'s domain run with `g`'s solid on their left.
fn cut(
    face: &CFace,
    g: &CFace,
    region: &CurvedRegion,
    tol: Tolerance,
) -> Result<Vec<(Trim, Reach)>, BooleanError> {
    let t = tol.linear();
    // The curves, as spans in space, before keeping their parts in g.
    let curves: Vec<Span> = match (face.domain, g.domain) {
        (
            Domain::Plane { origin, n, .. },
            Domain::Plane {
                origin: og, n: ng, ..
            },
        ) => plane_plane(origin, n, og, ng, &g.domain, region, t),
        (Domain::Plane { origin, n, .. }, Domain::Cone { .. }) => {
            plane_cone(origin, n, &g.domain, heights(&g.domain, [face, g]), t)?
        }
        (Domain::Cone { .. }, Domain::Plane { origin, n, .. }) => {
            plane_cone(origin, n, &face.domain, heights(&face.domain, [face, g]), t)?
        }
        (Domain::Cone { .. }, Domain::Cone { .. }) => {
            return if apart(face, g, t) {
                Ok(Vec::new())
            } else {
                Err(BooleanError::Curved)
            };
        }
    };
    let mut out = Vec::new();
    for curve in curves {
        for (part, reach) in inside_parts(&curve, face, g, region, tol)? {
            // Run with g's solid on the left: along n_face × n_g.
            let p = part.point(0.5);
            let want = face.domain.normal(p).cross(g.domain.normal(p));
            let part = if part.derivative(0.5).dot(want) < 0.0 {
                part.reversed()
            } else {
                part
            };
            let trim = trim_on(&face.domain, &part)?;
            out.push((trim, reach));
        }
    }
    Ok(out)
}

/// The line where two planes meet, as a long span across `g`'s region.
fn plane_plane(
    o: Point3,
    n: Vector3,
    og: Point3,
    ng: Vector3,
    g: &Domain,
    region: &CurvedRegion,
    _t: f64,
) -> Vec<Span> {
    let d = n.cross(ng);
    // Parallel, to rounding: a box turned a right angle has faces whose
    // normals cross in a vector of 1e-17.
    if d.length() <= 1e-12 {
        return Vec::new();
    }
    let Ok(dir) = d.normalized() else {
        return Vec::new();
    };
    // A point on both: o + α n + β ng with n·p = n·o, ng·p = ng·og.
    let (c1, c2) = (n.dot(o - Point3::ORIGIN), ng.dot(og - Point3::ORIGIN));
    let nn = n.dot(ng);
    let det = 1.0 - nn * nn;
    let (alpha, beta) = ((c1 - c2 * nn) / det, (c2 - c1 * nn) / det);
    let base = Point3::ORIGIN + n * alpha + ng * beta;
    // Long enough to cross g's whole region.
    let reach = region_reach(g, region);
    let mid = base + dir * dir.dot(reach.0 - base);
    vec![Span::Line {
        from: mid - dir * reach.1,
        to: mid + dir * reach.1,
    }]
}

/// A point of `g`'s region and a length that covers it.
fn region_reach(g: &Domain, region: &CurvedRegion) -> (Point3, f64) {
    let points: Vec<Point3> = region
        .contours()
        .iter()
        .flatten()
        .flat_map(|t| (0..=8).map(move |i| t.point(i as f64 / 8.0)))
        .map(|q| g.lift([q.x(), q.y()]))
        .collect();
    let c = points
        .iter()
        .fold(Vector3::new(0.0, 0.0, 0.0).expect("finite"), |s, p| {
            s + (*p - Point3::ORIGIN)
        })
        / points.len().max(1) as f64;
    let center = Point3::ORIGIN + c;
    let reach = points
        .iter()
        .map(|p| (*p - center).length())
        .fold(0.0, f64::max);
    (center, 2.0 * reach + 1.0)
}

/// Where the plane through `o` with normal `n` cuts the cylinder `cyl`: an
/// ellipse (a circle across it), or two lines along it.
fn plane_cylinder(o: Point3, n: Vector3, cyl: &Domain) -> Vec<Span> {
    let Domain::Cone {
        origin,
        axis,
        r,
        e1,
        e2,
        ..
    } = *cyl
    else {
        return Vec::new();
    };
    let den = n.dot(axis);
    let k = n.dot(o - origin);
    if den.abs() > 1e-12 {
        // h(θ) = (k − r n·e(θ)) / n·axis: an ellipse round the turn.
        let center = origin + axis * (k / den);
        let a = e1 * r - axis * (r * n.dot(e1) / den);
        let b = e2 * r - axis * (r * n.dot(e2) / den);
        return vec![Span::Arc {
            center,
            a,
            b,
            start: 0.0,
            sweep: TAU,
        }];
    }
    // Along the axis: the plane is k from the axis, across e(θ)·n r = k.
    let m = n - axis * n.dot(axis);
    let Ok(mu) = m.normalized() else {
        return Vec::new();
    };
    let dist = k / m.length();
    if dist.abs() >= r {
        return Vec::new();
    }
    let side = axis.cross(mu);
    let off = (r * r - dist * dist).sqrt();
    // Long lines, cut to the face later.
    let long = 1e6 * r.max(1.0);
    [off, -off]
        .iter()
        .map(|&s| {
            let foot = origin + mu * dist + side * s;
            Span::Line {
                from: foot - axis * long,
                to: foot + axis * long,
            }
        })
        .collect()
}

/// The heights along a cone's axis that two faces span, its apex among
/// them, widened: what a section curve on it must cover.
fn heights(cone: &Domain, faces: [&CFace; 2]) -> (f64, f64) {
    let Domain::Cone { origin, axis, .. } = *cone else {
        return (0.0, 0.0);
    };
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let points = faces
        .iter()
        .flat_map(|f| f.loops.iter().flatten())
        .flat_map(|s| (0..=16).map(move |i| s.point(i as f64 / 16.0)));
    for h in points.map(|p| (p - origin).dot(axis)).chain(cone.apex()) {
        lo = lo.min(h);
        hi = hi.max(h);
    }
    let pad = 0.25 * (hi - lo);
    (lo - pad, hi + pad)
}

/// Where the plane through `o` with normal `n` cuts the cone `cone` between
/// the heights `lo` and `hi`, on its face's nappe: a circle across the
/// axis, the lines along the side through the apex, or arcs of a conic
/// (ADR 0033 D3). On a cylinder, [`plane_cylinder`].
fn plane_cone(
    o: Point3,
    n: Vector3,
    cone: &Domain,
    (lo, hi): (f64, f64),
    t: f64,
) -> Result<Vec<Span>, BooleanError> {
    let Domain::Cone {
        origin,
        axis,
        r,
        k,
        e1,
        e2,
        ..
    } = *cone
    else {
        return Ok(Vec::new());
    };
    if k == 0.0 {
        return Ok(plane_cylinder(o, n, cone));
    }
    let apex = -r / k;
    // The face's nappe: where the radius r + k h is positive.
    let (lo, hi) = if k > 0.0 {
        (lo.max(apex), hi)
    } else {
        (lo, hi.min(apex))
    };
    if lo >= hi {
        return Ok(Vec::new());
    }
    // (r + k h) n·e(θ) + h n·axis = c: h = (c − r n·e) / (n·axis + k n·e).
    let (c, na, n1, n2) = (n.dot(o - origin), n.dot(axis), n.dot(e1), n.dot(e2));
    let num = [c, -r * n1, -r * n2];
    let den = [na, k * n1, k * n2];
    let rho = |h: f64| r + k * h;
    if n1.hypot(n2) <= 1e-12 {
        // Square to the axis: a circle.
        let h = c / na;
        if h < lo || h > hi || rho(h) <= t {
            return Ok(Vec::new());
        }
        return Ok(vec![Span::Arc {
            center: origin + axis * h,
            a: e1 * rho(h),
            b: e2 * rho(h),
            start: 0.0,
            sweep: TAU,
        }]);
    }
    if (c - apex * na).abs() <= t {
        // Through the apex: the lines along the side where the plane holds
        // them, n·(axis + k e(θ)) = 0.
        let far = if k > 0.0 { hi } else { lo };
        let tip = origin + axis * apex;
        return Ok(trig_roots(den[0], den[1], den[2])
            .into_iter()
            .map(|th| {
                let (s, co) = th.sin_cos();
                Span::Line {
                    from: tip,
                    to: tip + (axis + (e1 * co + e2 * s) * k) * (far - apex),
                }
            })
            .collect());
    }
    let value = |c: [f64; 3], th: f64| {
        let (s, co) = th.sin_cos();
        c[0] + c[1] * co + c[2] * s
    };
    let keep = |th: f64| {
        let d = value(den, th);
        let h = value(num, th) / d;
        d != 0.0 && h >= lo && h <= hi
    };
    // Where the curve runs off to infinity or leaves the heights.
    let level = |v: f64| {
        trig_roots(
            num[0] - v * den[0],
            num[1] - v * den[1],
            num[2] - v * den[2],
        )
    };
    let mut cuts = trig_roots(den[0], den[1], den[2]);
    cuts.extend(level(lo));
    cuts.extend(level(hi));
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    if cuts.is_empty() {
        return Ok(if keep(0.0) {
            vec![section(cone, num, den, 0.0, TAU)?]
        } else {
            Vec::new()
        });
    }
    // The stretches between the cuts round the turn where the curve is
    // kept; two meeting where it only touches a height are one.
    let mut runs: Vec<(f64, f64)> = Vec::new();
    for i in 0..cuts.len() {
        let (a, b) = (cuts[i], cuts.get(i + 1).copied().unwrap_or(cuts[0] + TAU));
        if !keep((a + b) / 2.0) {
            continue;
        }
        match runs.last_mut() {
            Some(last) if (last.1 - a).abs() < 1e-12 => last.1 = b,
            _ => runs.push((a, b)),
        }
    }
    if runs.len() > 1 {
        let (first, last) = (runs[0], runs[runs.len() - 1]);
        if (last.1 - (first.0 + TAU)).abs() < 1e-12 {
            runs[0] = (last.0, first.1 + TAU);
            runs.pop();
        }
    }
    runs.into_iter()
        .map(|(a, b)| section(cone, num, den, a, b))
        .collect()
}

/// The arc of a cone's section by a plane from the angle `ta` to `tb`, its
/// height `h(θ) = num / den` in `cos θ` and `sin θ` (θ from `e1` toward
/// `e2`, not the window's x): a circle's or an ellipse's arc where the
/// plane cuts the cone all round, else a parabola's or a hyperbola's as a
/// rational quadratic (ADR 0033 D3).
fn section(
    cone: &Domain,
    num: [f64; 3],
    den: [f64; 3],
    ta: f64,
    tb: f64,
) -> Result<Span, BooleanError> {
    let Domain::Cone {
        origin,
        axis,
        r,
        k,
        e1,
        e2,
        ..
    } = *cone
    else {
        return Err(BooleanError::NotSolid);
    };
    // The point at θ and its derivative.
    let at = |th: f64| -> (Point3, Vector3) {
        let (s, c) = th.sin_cos();
        let (n, dn) = (num[0] + num[1] * c + num[2] * s, num[2] * c - num[1] * s);
        let (d, dd) = (den[0] + den[1] * c + den[2] * s, den[2] * c - den[1] * s);
        let (h, dh) = (n / d, (dn * d - n * dd) / (d * d));
        let (e, de) = (e1 * c + e2 * s, e2 * c - e1 * s);
        let rho = r + k * h;
        (
            origin + e * rho + axis * h,
            de * rho + e * (k * dh) + axis * dh,
        )
    };
    if den[1].hypot(den[2]) <= 1e-12 * den[0].abs() {
        // Square to the axis: a circle, its angle the cone's.
        let h = num[0] / den[0];
        return Ok(Span::Arc {
            center: origin + axis * h,
            a: e1 * (r + k * h),
            b: e2 * (r + k * h),
            start: ta,
            sweep: tb - ta,
        });
    }
    // The plane: m·(p − origin) = num₀ with m = den₀ axis + (den₁ e1 +
    // den₂ e2) / k.
    let m = axis * den[0] + (e1 * den[1] + e2 * den[2]) / k;
    let (p0, d0) = at(ta);
    let (p2, d2) = at(tb);
    if den[0].abs() > den[1].hypot(den[2]) * (1.0 + 1e-9) {
        // All round, on one nappe: an ellipse (a parabola, to rounding, is
        // the rational quadratic's). Its major axis lies in the
        // plane of the cone's axis and the plane's slope; its minor axis
        // ends square to it through the center, on the side.
        let tm = den[2].atan2(den[1]);
        let (v1, v2) = (at(tm).0, at(tm + PI).0);
        let center = v1 + (v2 - v1) * 0.5;
        let major = v1 - center;
        let u = m.cross(major).normalized().map_err(geometry)?;
        let (dc, ua) = (center - origin, u.dot(axis));
        let hc = dc.dot(axis);
        let (rc, ru) = (dc - axis * hc, u - axis * ua);
        let rho = r + k * hc;
        // |rc + s ru|² = (rho + k ua s)².
        let qa = ru.dot(ru) - k * k * ua * ua;
        let qb = 2.0 * (rc.dot(ru) - rho * k * ua);
        let qc = rc.dot(rc) - rho * rho;
        let disc = qb * qb - 4.0 * qa * qc;
        if disc <= 0.0 || qa.abs() <= 1e-300 {
            return Err(BooleanError::NotSolid);
        }
        let minor = u * (disc.sqrt() / (2.0 * qa.abs()));
        let phase = |p: Point3| {
            let w = p - center;
            (w.dot(minor) / minor.dot(minor)).atan2(w.dot(major) / major.dot(major))
        };
        let fa = phase(p0);
        let ahead = minor * fa.cos() - major * fa.sin();
        let forward = ahead.dot(d0) * (tb - ta) > 0.0;
        let sweep = if (tb - ta).abs() >= TAU - 1e-9 {
            TAU
        } else if forward {
            (phase(p2) - fa).rem_euclid(TAU)
        } else {
            (fa - phase(p2)).rem_euclid(TAU)
        };
        return Ok(Span::Arc {
            center,
            a: major,
            b: minor,
            start: fa,
            sweep: if forward { sweep } else { -sweep },
        });
    }
    // A parabola or a hyperbola: the rational quadratic from its ends, with
    // their tangents, through its middle, worked out in its plane.
    let dir = (tb - ta).signum();
    let x = (p2 - p0).normalized().map_err(geometry)?;
    let y = m.normalized().map_err(geometry)?.cross(x);
    let flat = |p: Point3| [(p - p0).dot(x), (p - p0).dot(y)];
    let ahead = |v: Vector3| [v.dot(x) * dir, v.dot(y) * dir];
    let middle = at((ta + tb) / 2.0).0;
    let Some(Trim::Conic {
        control, weight, ..
    }) = Trim::conic_through(flat(p0), ahead(d0), flat(p2), ahead(d2), flat(middle))
    else {
        return Err(BooleanError::NonManifold);
    };
    Ok(Span::Conic {
        from: p0,
        control: p0 + x * control.x() + y * control.y(),
        to: p2,
        weight,
    })
}

/// Whether two cylindrical faces lie apart: their regions' boxes in space.
fn apart(a: &CFace, b: &CFace, t: f64) -> bool {
    let bounds = |f: &CFace| {
        let pts: Vec<Point3> = f
            .loops
            .iter()
            .flatten()
            .flat_map(|s| (0..=16).map(move |i| s.point(i as f64 / 16.0)))
            .collect();
        arq_geometry::Bounds3::from_points(pts)
    };
    match (bounds(a), bounds(b)) {
        (Some(p), Some(q)) => {
            // A cylinder's or a cone's side bulges beyond its edges' box by
            // at most its widest radius; take the boxes widened by it.
            let grow = |f: &CFace| {
                f.loops
                    .iter()
                    .flatten()
                    .flat_map(|s| (0..=16).map(move |i| s.point(i as f64 / 16.0)))
                    .map(|p| f.domain.off_axis(p))
                    .fold(0.0, f64::max)
            };
            let (gp, gq) = (grow(a) + t, grow(b) + t);
            let (pl, ph, ql, qh) = (p.min(), p.max(), q.min(), q.max());
            pl.x() - gp > qh.x() + gq
                || ql.x() - gq > ph.x() + gp
                || pl.y() - gp > qh.y() + gq
                || ql.y() - gq > ph.y() + gp
                || pl.z() - gp > qh.z() + gq
                || ql.z() - gq > ph.z() + gp
        }
        _ => true,
    }
}

/// The parts of `curve` lying in face `g`: where it crosses g's boundary in
/// g's domain, the pieces between whose middles are inside.
fn inside_parts(
    curve: &Span,
    face: &CFace,
    g: &CFace,
    region: &CurvedRegion,
    tol: Tolerance,
) -> Result<Vec<(Span, Reach)>, BooleanError> {
    let t = tol.linear();
    // The curve in g's domain, as one trim (into g's window if a cylinder
    // or a cone).
    let image = trim_on(&g.domain, curve)?;
    // A conic's own parameter runs unevenly round a cone: there a point of
    // the image is found on the curve itself.
    let uneven = matches!(g.domain, Domain::Cone { k, .. } if k != 0.0)
        && matches!(image, Trim::Graph { .. });
    let on_curve =
        |q: Point2, guess: f64| curve.parameter_near(g.domain.lift([q.x(), q.y()]), guess);
    let images: Vec<(Trim, f64, f64)> = match g.domain {
        Domain::Cone { x0, .. } => {
            // Split as into_window does, remembering each piece's share.
            let pieces = into_window(&image, x0, t)?;
            let (xa, xb) = (image.start().x(), image.end().x());
            let mut out = Vec::new();
            let mut from = 0.0;
            let last = pieces.len().saturating_sub(1);
            for (i, p) in pieces.into_iter().enumerate() {
                let even = from + (p.end().x() - p.start().x()) / (xb - xa);
                let to = if i == last || (xb - xa).abs() == 0.0 {
                    1.0
                } else if uneven {
                    on_curve(p.end(), even)
                } else {
                    even
                };
                out.push((p, from, to));
                from = to;
            }
            out
        }
        Domain::Plane { .. } => vec![(image, 0.0, 1.0)],
    };
    // The curve's parameter at `w` of a piece of its image, from `s0` to `s1`.
    let along = |trim: &Trim, w: f64, s0: f64, s1: f64| {
        let even = s0 + (s1 - s0) * w;
        if uneven && w > 0.0 && w < 1.0 {
            on_curve(trim.point(w), even)
        } else {
            even
        }
    };
    let boundary: Vec<Trim> = region.contours().iter().flatten().copied().collect();
    let mut parts = Vec::new();
    for (trim, s0, s1) in images {
        let mut cuts: Vec<f64> = vec![0.0, 1.0];
        for b in &boundary {
            for (s, _) in trim.crossings(b, tol) {
                cuts.push(s);
            }
            for q in [b.start(), b.end()] {
                let (s, d) = trim.closest(q);
                if d <= t {
                    cuts.push(s);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
        for w in cuts.windows(2) {
            let at = (w[0] + w[1]) / 2.0;
            let mid = trim.point(at);
            let reach = if inside_face(&g.domain, region, mid, tol) {
                Reach::Both
            } else if region.contains(mid, tol) == arq_geometry::Containment::Boundary {
                // Along g's edge: g goes on to one side of the face only.
                match side_of_edge(face, g, region, &trim, at, tol) {
                    Some(reach) => reach,
                    None => continue,
                }
            } else {
                continue;
            };
            let (a, b) = (along(&trim, w[0], s0, s1), along(&trim, w[1], s0, s1));
            // Longer than the tolerance (a closed curve ends where it
            // starts: its middle tells).
            if (curve.point(a) - curve.point((a + b) / 2.0)).length() > t {
                parts.push((curve.part(a, b), reach));
            }
        }
    }
    // Joined again where the window cut them, so one piece runs on.
    Ok(parts)
}

/// Which side of `face` the face `g` goes on to from its edge, where the
/// curve `trim` (in g's domain) runs along it at `at`: a hair into g's
/// region, how far above `face` it is.
fn side_of_edge(
    face: &CFace,
    g: &CFace,
    region: &CurvedRegion,
    trim: &Trim,
    at: f64,
    tol: Tolerance,
) -> Option<Reach> {
    let t = tol.linear();
    let (p, q) = (trim.point(at), trim.point((at + 1e-3).min(1.0)));
    let (dx, dy) = (q.x() - p.x(), q.y() - p.y());
    // On a cylinder or a cone x is an angle: steps measured along the
    // surface, at its radius there.
    let scale = g.domain.radius_at(p.y());
    let len = (dx * scale).hypot(dy);
    if len == 0.0 || scale <= 0.0 {
        return None;
    }
    let step = 1e3 * t;
    for sign in [1.0, -1.0] {
        let probe = Point2::new(
            p.x() - sign * dy / len * step / scale,
            p.y() + sign * dx * scale / len * step,
        )
        .ok()?;
        if inside_face(&g.domain, region, probe, tol) {
            let h = height(&face.domain, g.domain.lift([probe.x(), probe.y()]));
            return Some(if h < 0.0 { Reach::Inner } else { Reach::Outer });
        }
    }
    None
}

/// Whether the point `q` of a face's domain lies inside the face's region.
/// On a cylinder's seam the window's region has its edge, but the face
/// goes on across it: there the point is inside if it is so a hair to
/// either side.
fn inside_face(domain: &Domain, region: &CurvedRegion, q: Point2, tol: Tolerance) -> bool {
    let inside = |p: Point2| region.contains(p, tol) == arq_geometry::Containment::Inside;
    let Domain::Cone { x0, .. } = *domain else {
        return inside(q);
    };
    let t = tol.linear();
    let seam = (q.x() - x0).abs() <= t || (q.x() - x0 - TAU).abs() <= t;
    if !seam {
        return inside(q);
    }
    // A hair: a hundred tolerances along the circle there.
    let hair = (100.0 * t / domain.radius_at(q.y()).max(t)).min(0.1);
    [x0 + hair, x0 + TAU - hair]
        .into_iter()
        .filter_map(|x| Point2::new(x, q.y()).ok())
        .any(inside)
}

// ---------------------------------------------------------------- assembly

/// A welded edge use: its span, its ends' and middle's indices.
#[derive(Clone, Copy, Debug)]
struct Use {
    span: Span,
    from: usize,
    to: usize,
    middle: usize,
}

fn assemble(parts: &[Part], tol: Tolerance) -> Result<Body, BooleanError> {
    let t = tol.linear();
    let mut pool = Pool::new(t);
    let mut middles = Pool::new(t);
    // Each part's loops as spans in space, seams dropped and chains joined;
    // and where a cone's part closes on its apex, the apex.
    let mut faces: Vec<(usize, Vec<Vec<Use>>)> = Vec::new();
    let mut apexes: Vec<Option<usize>> = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let mut loops = Vec::new();
        // All its contours at once: on a cylinder they close across the
        // seam, one contour into another.
        let mut spans = Vec::new();
        for contour in part.region.contours() {
            spans.extend(lift_contour(&part.domain, contour, t)?);
        }
        for chain in chain_up(spans, t)? {
            let uses: Vec<Use> = rejoined(chain, t)
                .into_iter()
                .map(|span| Use {
                    span,
                    from: pool.weld(span.point(0.0)),
                    to: pool.weld(span.point(1.0)),
                    middle: middles.weld(span.point(0.5)),
                })
                .collect();
            if !uses.is_empty() {
                loops.push(uses);
            }
        }
        if !loops.is_empty() {
            faces.push((index, loops));
            apexes.push(apex_of(part, t).map(|p| pool.weld(p)));
        }
    }
    // Edges split where another face has a corner on them.
    let corners = pool.points.clone();
    for (_, loops) in &mut faces {
        for l in loops.iter_mut() {
            let mut out = Vec::new();
            for u in l.iter() {
                let mut on: Vec<(f64, usize)> = Vec::new();
                for (m, &q) in corners.iter().enumerate() {
                    if m == u.from || m == u.to {
                        continue;
                    }
                    let (s, d) = u.span.project(q);
                    let inner = (u.span.point(s) - u.span.point(0.0)).length() > t
                        && (u.span.point(s) - u.span.point(1.0)).length() > t;
                    if d <= t && inner {
                        on.push((s, m));
                    }
                }
                on.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut from = (0.0, u.from);
                for (s, m) in on.into_iter().chain([(1.0, u.to)]) {
                    let span = u.span.part(from.0, s);
                    out.push(Use {
                        span,
                        from: from.1,
                        to: m,
                        middle: middles.weld(span.point(0.5)),
                    });
                    from = (s, m);
                }
            }
            *l = out;
        }
    }
    // Each edge used once each way.
    let key = |u: &Use| (u.from.min(u.to), u.from.max(u.to), u.middle);
    let mut by_edge: HashMap<(usize, usize, usize), Vec<(usize, bool)>> = HashMap::new();
    for (f, (_, loops)) in faces.iter().enumerate() {
        for l in loops {
            for u in l {
                let forward = if u.from == u.to {
                    arc_turns_forward(&u.span)
                } else {
                    u.from < u.to
                };
                by_edge.entry(key(u)).or_default().push((f, forward));
            }
        }
    }
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    fn root(parent: &mut [usize], mut f: usize) -> usize {
        while parent[f] != f {
            parent[f] = parent[parent[f]];
            f = parent[f];
        }
        f
    }
    for uses in by_edge.values() {
        let ok = uses.len() == 2 && uses[0].1 != uses[1].1;
        if !ok {
            return Err(BooleanError::NonManifold);
        }
        let (ra, rb) = (root(&mut parent, uses[0].0), root(&mut parent, uses[1].0));
        parent[ra] = rb;
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for f in 0..faces.len() {
        let r = root(&mut parent, f);
        groups.entry(r).or_default().push(f);
    }
    let mut shells: Vec<Vec<usize>> = groups.into_values().collect();
    shells.sort();
    // The body, each shell its own lump for now, to measure them.
    let built = build(parts, &faces, &apexes, &shells, &[], &pool.points, tol)?;
    let volumes: Vec<f64> = (0..shells.len())
        .map(|s| shell_volume(&built, s))
        .collect::<Result<_, _>>()?;
    let outsides: Vec<usize> = (0..shells.len()).filter(|&s| volumes[s] > 0.0).collect();
    let mut lumps: Vec<Vec<usize>> = outsides.iter().map(|&s| vec![s]).collect();
    for void in (0..shells.len()).filter(|&s| volumes[s] <= 0.0) {
        let probe = probe_of(parts, &faces, &shells[void]);
        let around = outsides
            .iter()
            .enumerate()
            .filter(|&(_, &o)| inside_shell(parts, &faces, &shells[o], probe, t))
            .min_by(|x, y| volumes[*x.1].total_cmp(&volumes[*y.1]));
        match around {
            Some((k, _)) => lumps[k].push(void),
            None => return Err(BooleanError::NonManifold),
        }
    }
    build(parts, &faces, &apexes, &shells, &lumps, &pool.points, tol)
}

/// The apex a cone's part closes on: where its region runs a whole turn
/// along the apex's line.
fn apex_of(part: &Part, t: f64) -> Option<Point3> {
    let apex = part.domain.apex()?;
    let along: f64 = part
        .region
        .contours()
        .iter()
        .flatten()
        .filter_map(|trim| match trim {
            Trim::Segment { from, to }
                if (from.y() - apex).abs() <= t && (to.y() - apex).abs() <= t =>
            {
                Some((to.x() - from.x()).abs())
            }
            _ => None,
        })
        .sum();
    (along >= TAU - 1e-9).then(|| part.domain.lift([0.0, apex]))
}

/// Whether a closed arc turns forward: about one fixed direction, whatever
/// diameters it is written with.
fn arc_turns_forward(span: &Span) -> bool {
    match span {
        Span::Arc { a, b, sweep, .. } => {
            let k = Vector3::new(0.316_227_766, 0.547_722_557_5, 0.774_596_669_2).expect("finite");
            (a.cross(*b).dot(k) > 0.0) == (*sweep > 0.0)
        }
        Span::Line { .. } | Span::Conic { .. } => true,
    }
}

/// A contour of a part's region lifted into space: lines, circles and
/// ellipses; seam segments dropped.
fn lift_contour(domain: &Domain, contour: &[Trim], t: f64) -> Result<Vec<Span>, BooleanError> {
    let mut out = Vec::new();
    for trim in contour {
        match (*domain, *trim) {
            (Domain::Plane { origin, u, v, .. }, Trim::Segment { from, to }) => {
                out.push(Span::Line {
                    from: origin + u * from.x() + v * from.y(),
                    to: origin + u * to.x() + v * to.y(),
                })
            }
            (
                Domain::Plane { origin, u, v, .. },
                Trim::Ellipse {
                    center,
                    a,
                    b,
                    start,
                    sweep,
                },
            ) => out.push(Span::Arc {
                center: origin + u * center.x() + v * center.y(),
                a: u * a.x() + v * a.y(),
                b: u * b.x() + v * b.y(),
                start,
                sweep,
            }),
            (
                Domain::Cone {
                    origin,
                    axis,
                    r,
                    k,
                    e1,
                    e2,
                    sigma,
                    x0,
                },
                Trim::Segment { from, to },
            ) => {
                let on_seam = |x: f64| (x - x0).abs() <= t || (x - x0 - TAU).abs() <= t;
                if (from.x() - to.x()).abs() <= t {
                    if on_seam(from.x()) {
                        continue; // the seam joins the face to itself
                    }
                    out.push(Span::Line {
                        from: domain.lift([from.x(), from.y()]),
                        to: domain.lift([to.x(), to.y()]),
                    });
                } else if (from.y() - to.y()).abs() <= t {
                    // Round the axis at one height; at a cone's apex, a point.
                    let rho = r + k * from.y();
                    if rho <= t {
                        continue;
                    }
                    out.push(Span::Arc {
                        center: origin + axis * from.y(),
                        a: e1 * rho,
                        b: e2 * rho,
                        start: sigma * from.x(),
                        sweep: sigma * (to.x() - from.x()),
                    });
                } else {
                    return Err(BooleanError::NonManifold);
                }
            }
            (
                Domain::Cone {
                    origin,
                    axis,
                    r,
                    k: slope,
                    e1,
                    e2,
                    sigma,
                    ..
                },
                Trim::Graph { num, den, from, to },
            ) => {
                if slope != 0.0 {
                    // A conic's arc; in θ = σx, cos θ = cos x and
                    // sin θ = σ sin x.
                    let turn = |c: [f64; 3]| [c[0], c[1], sigma * c[2]];
                    out.push(section(
                        domain,
                        turn(num),
                        turn(den),
                        sigma * from,
                        sigma * to,
                    )?);
                    continue;
                }
                // h = (k + c cos x + s sin x) / d, with θ = σx.
                let d = den[0];
                let (k, c, s) = (num[0] / d, num[1] / d, sigma * num[2] / d);
                out.push(Span::Arc {
                    center: origin + axis * k,
                    a: e1 * r + axis * c,
                    b: e2 * r + axis * s,
                    start: sigma * from,
                    sweep: sigma * (to - from),
                });
            }
            (
                Domain::Plane { origin, u, v, .. },
                Trim::Conic {
                    from,
                    control,
                    to,
                    weight,
                },
            ) => {
                let lift = |q: Point2| origin + u * q.x() + v * q.y();
                out.push(Span::Conic {
                    from: lift(from),
                    control: lift(control),
                    to: lift(to),
                    weight,
                });
            }
            _ => return Err(BooleanError::NonManifold),
        }
    }
    Ok(out)
}

/// A closed chain with consecutive pieces of one curve joined again: a
/// curve the window's seam cut in two is one edge.
fn rejoined(mut chain: Vec<Span>, t: f64) -> Vec<Span> {
    let mut changed = true;
    while changed && chain.len() > 1 {
        changed = false;
        for i in 0..chain.len() {
            let next = (i + 1) % chain.len();
            if let Some(one) = joined(&chain[i], &chain[next], t) {
                chain[i] = one;
                chain.remove(next);
                changed = true;
                break;
            }
        }
    }
    chain
}

/// `a` and `b`, which starts where it ends, as one span when they run along
/// one curve the same way.
fn joined(a: &Span, b: &Span, t: f64) -> Option<Span> {
    match (*a, *b) {
        (Span::Line { from, to }, Span::Line { to: end, .. }) => {
            let (d, e) = (to - from, end - from);
            let straight = d.cross(e).length() <= t * e.length() && d.dot(e) > 0.0;
            (straight && d.dot(e) < e.dot(e)).then_some(Span::Line { from, to: end })
        }
        (
            Span::Arc {
                center,
                a: ma,
                b: mb,
                start,
                sweep,
            },
            Span::Arc {
                center: c2,
                a: a2,
                b: b2,
                start: s2,
                sweep: w2,
            },
        ) => {
            let same =
                (center - c2).length() <= t && (ma - a2).length() <= t && (mb - b2).length() <= t;
            let size = ma.length().max(mb.length()).max(f64::MIN_POSITIVE);
            let gap = (start + sweep - s2).rem_euclid(TAU);
            let meets = gap.min(TAU - gap) * size <= t;
            let total = sweep + w2;
            (same && meets && sweep * w2 > 0.0 && total.abs() <= TAU + t / size).then_some(
                Span::Arc {
                    center,
                    a: ma,
                    b: mb,
                    start,
                    sweep: total,
                },
            )
        }
        (Span::Conic { from, control, .. }, Span::Conic { to: end, .. }) => {
            // In the arcs' plane: the conic through both far ends with their
            // tangents and through the corner, if both lie on it.
            let x = (end - from).normalized().ok()?;
            let y = (control - from)
                .cross(end - from)
                .normalized()
                .ok()?
                .cross(x);
            let flat = |p: Point3| [(p - from).dot(x), (p - from).dot(y)];
            let ahead = |v: Vector3| [v.dot(x), v.dot(y)];
            let Trim::Conic {
                control: c, weight, ..
            } = Trim::conic_through(
                flat(from),
                ahead(a.derivative(0.0)),
                flat(end),
                ahead(b.derivative(1.0)),
                flat(a.point(1.0)),
            )?
            else {
                return None;
            };
            let one = Span::Conic {
                from,
                control: from + x * c.x() + y * c.y(),
                to: end,
                weight,
            };
            let on = |piece: &Span| (1..8).all(|i| one.project(piece.point(i as f64 / 8.0)).1 <= t);
            (on(a) && on(b)).then_some(one)
        }
        _ => None,
    }
}

/// Spans joined into closed chains by where they start and end in space:
/// a contour cut by the seam closes across it.
fn chain_up(spans: Vec<Span>, t: f64) -> Result<Vec<Vec<Span>>, BooleanError> {
    let mut left: Vec<Span> = spans;
    let mut chains = Vec::new();
    while let Some(first) = left.pop() {
        let mut chain = vec![first];
        loop {
            let end = chain.last().expect("one").point(1.0);
            let start = chain[0].point(0.0);
            if (end - start).length() <= t {
                break;
            }
            let Some(k) = left.iter().position(|s| (s.point(0.0) - end).length() <= t) else {
                return Err(BooleanError::NonManifold);
            };
            chain.push(left.remove(k));
        }
        chains.push(chain);
    }
    Ok(chains)
}

/// A shell's signed volume: the body's lump of it, measured.
fn shell_volume(body: &Body, shell: usize) -> Result<f64, BooleanError> {
    let single = sub_body(body, shell)?;
    single
        .mass_properties()
        .map(|m| m.volume)
        .map_err(|_| BooleanError::NonManifold)
}

/// The body of one of `body`'s shells (built one per lump).
fn sub_body(body: &Body, shell: usize) -> Result<Body, BooleanError> {
    let mut b = BodyBuilder::new();
    for v in body.vertices() {
        b.vertex(v.point);
    }
    for e in body.edges() {
        b.edge(e.clone());
    }
    for c in body.coedges() {
        b.coedge(*c);
    }
    for l in body.loops() {
        b.add_loop(l.clone());
    }
    for f in body.faces() {
        b.face(f.clone());
    }
    for s in body.shells() {
        b.shell(s.clone());
    }
    let id = body.lumps()[shell].shells[0];
    b.lump(Lump { shells: vec![id] });
    b.build(Tolerance::new(1e300, 1.0).map_err(geometry)?)
        .map_err(BooleanError::Brep)
}

/// A point inside a face of a shell, beside an edge's middle.
fn probe_of(parts: &[Part], faces: &[(usize, Vec<Vec<Use>>)], shell: &[usize]) -> Point3 {
    let (index, _) = &faces[shell[0]];
    let part = &parts[*index];
    let trim = part.region.contours()[0][0];
    let (p, d) = (trim.point(0.5), trim.point(0.51));
    let side = [-(d.y() - p.y()), d.x() - p.x()];
    let len = side[0].hypot(side[1]).max(f64::MIN_POSITIVE);
    let q = [p.x() + side[0] / len * 1e-3, p.y() + side[1] / len * 1e-3];
    part.domain.lift(q)
}

/// Whether `p` lies inside a closed shell: a ray from it crosses its faces
/// an odd number of times.
fn inside_shell(
    parts: &[Part],
    faces: &[(usize, Vec<Vec<Use>>)],
    shell: &[usize],
    p: Point3,
    t: f64,
) -> bool {
    let w = Vector3::new(0.531_234_567_8, 0.312_345_678_9, 0.758_946_638_4)
        .and_then(|w| w.normalized())
        .expect("finite");
    let mut crossings = 0;
    for &f in shell {
        let part = &parts[faces[f].0];
        let hits = ray_hits(&part.domain, p, w);
        for s in hits {
            if s <= t {
                continue;
            }
            let q = part.domain.flat(p + w * s);
            let at = Point2::new(q[0], q[1]).unwrap_or(Point2::ORIGIN);
            let tol = Tolerance::new(t, 1e-10).unwrap_or(Tolerance::DEFAULT);
            if inside_face(&part.domain, &part.region, at, tol) {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

/// Where the ray from `p` along `w` meets a face's surface, as distances
/// along it: a plane once, a cylinder or a cone (its face's nappe) up to
/// twice.
fn ray_hits(domain: &Domain, p: Point3, w: Vector3) -> Vec<f64> {
    match *domain {
        Domain::Plane { origin, n, .. } => {
            let den = w.dot(n);
            if den.abs() <= 1e-15 {
                Vec::new()
            } else {
                vec![(origin - p).dot(n) / den]
            }
        }
        Domain::Cone {
            origin, axis, r, k, ..
        } => {
            // |(q − origin) ⟂ axis| = r + k h at q = p + s w.
            let d = p - origin;
            let (hp, wa) = (d.dot(axis), w.dot(axis));
            let (dp, wp) = (d - axis * hp, w - axis * wa);
            let rho = r + k * hp;
            let qa = wp.dot(wp) - k * k * wa * wa;
            let qb = 2.0 * (dp.dot(wp) - rho * k * wa);
            let qc = dp.dot(dp) - rho * rho;
            let roots = if qa.abs() <= 1e-30 {
                if qb.abs() <= 1e-300 {
                    Vec::new()
                } else {
                    vec![-qc / qb]
                }
            } else {
                let disc = qb * qb - 4.0 * qa * qc;
                if disc < 0.0 {
                    Vec::new()
                } else {
                    let root = disc.sqrt();
                    vec![(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
                }
            };
            roots
                .into_iter()
                .filter(|s| rho + k * wa * s >= 0.0)
                .collect()
        }
    }
}

/// The body of the faces, edges once per pair of uses, a cone's apex a
/// loop of its own where its face closes on it; `lumps` of shells, or each
/// shell its own lump when empty.
fn build(
    parts: &[Part],
    faces: &[(usize, Vec<Vec<Use>>)],
    apexes: &[Option<usize>],
    shells: &[Vec<usize>],
    lumps: &[Vec<usize>],
    points: &[Point3],
    tol: Tolerance,
) -> Result<Body, BooleanError> {
    let mut b = BodyBuilder::new();
    let vertices: Vec<_> = points.iter().map(|&p| b.vertex(p)).collect();
    // Each edge, with the end its first use starts at and the way it turned.
    let mut edges: HashMap<(usize, usize, usize), (crate::brep::EdgeId, usize, bool)> =
        HashMap::new();
    let mut face_ids = Vec::with_capacity(faces.len());
    for ((index, loops), apex) in faces.iter().zip(apexes) {
        let part = &parts[*index];
        let mut loop_ids = Vec::new();
        for l in loops {
            let mut coedges = Vec::new();
            for u in l {
                let key = (u.from.min(u.to), u.from.max(u.to), u.middle);
                let turns = arc_turns_forward(&u.span);
                let (edge, from, first_turns) = match edges.get(&key) {
                    Some(&found) => found,
                    None => {
                        let edge = b.edge(edge_of(&u.span, vertices[u.from], vertices[u.to])?);
                        edges.insert(key, (edge, u.from, turns));
                        (edge, u.from, turns)
                    }
                };
                // The use runs the edge's way when it starts where the edge
                // was first run from; a closed edge, when it turns as it did.
                let reversed = if u.from == u.to {
                    turns != first_turns
                } else {
                    u.from != from
                };
                coedges.push(b.coedge(Coedge { edge, reversed }));
            }
            loop_ids.push(b.add_loop(Loop { coedges }));
        }
        if let Some(v) = *apex {
            // A point the face closes on: an edge without a curve.
            let edge = b.edge(Edge {
                curve: None,
                start: vertices[v],
                end: vertices[v],
                reversed: false,
            });
            let coedge = b.coedge(Coedge {
                edge,
                reversed: false,
            });
            loop_ids.push(b.add_loop(Loop {
                coedges: vec![coedge],
            }));
        }
        face_ids.push(b.face(Face {
            surface: part.surface.clone(),
            loops: loop_ids,
            reversed: part.reversed,
        }));
    }
    let shell_ids: Vec<_> = shells
        .iter()
        .map(|s| {
            b.shell(Shell {
                faces: s.iter().map(|&f| face_ids[f]).collect(),
            })
        })
        .collect();
    if lumps.is_empty() {
        for s in &shell_ids {
            b.lump(Lump { shells: vec![*s] });
        }
    } else {
        for lump in lumps {
            b.lump(Lump {
                shells: lump.iter().map(|&s| shell_ids[s]).collect(),
            });
        }
    }
    let check = Tolerance::new(16.0 * tol.linear(), tol.angular()).map_err(geometry)?;
    b.build(check).map_err(BooleanError::Brep)
}

/// An edge from `start` to `end` along `span`'s curve: a line; an ellipse
/// from the span's conjugate diameters, run against it when the span turns
/// backward; or a conic's arc as a rational quadratic NURBS curve.
fn edge_of(
    span: &Span,
    start: crate::brep::VertexId,
    end: crate::brep::VertexId,
) -> Result<Edge, BooleanError> {
    match *span {
        Span::Line { from, to } => Ok(Edge {
            curve: Some(Curve::Line(
                LineCurve::new(from, to - from).map_err(geometry)?,
            )),
            start,
            end,
            reversed: false,
        }),
        Span::Arc {
            center,
            a,
            b,
            sweep,
            ..
        } => {
            // Principal axes of c + a cos t + b sin t.
            let t0 = 0.5 * (2.0 * a.dot(b)).atan2(a.dot(a) - b.dot(b));
            let (s0, c0) = t0.sin_cos();
            let mut major = a * c0 + b * s0;
            let mut minor = b * c0 - a * s0;
            if minor.length() > major.length() {
                // A quarter turn on: the old minor leads, the old major,
                // turned over, follows.
                std::mem::swap(&mut major, &mut minor);
                minor = -minor;
            }
            let normal = major.cross(minor);
            let ratio = minor.length() / major.length();
            let ellipse = EllipseCurve::new(center, normal, major, ratio).map_err(geometry)?;
            Ok(Edge {
                curve: Some(Curve::Ellipse(ellipse)),
                start,
                end,
                reversed: sweep < 0.0,
            })
        }
        Span::Conic {
            from,
            control,
            to,
            weight,
        } => {
            let curve = NurbsCurve::new(
                2,
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                vec![from, control, to],
                vec![1.0, weight, 1.0],
            )
            .map_err(geometry)?;
            Ok(Edge {
                curve: Some(Curve::Nurbs(curve)),
                start,
                end,
                reversed: false,
            })
        }
    }
}

/// Points welded within a tolerance.
struct Pool {
    tol: f64,
    points: Vec<Point3>,
}

impl Pool {
    fn new(tol: f64) -> Self {
        Self {
            tol,
            points: Vec::new(),
        }
    }

    fn weld(&mut self, p: Point3) -> usize {
        if let Some(i) = self
            .points
            .iter()
            .position(|q| (*q - p).length() <= self.tol)
        {
            return i;
        }
        self.points.push(p);
        self.points.len() - 1
    }
}

fn geometry(e: arq_geometry::GeometryError) -> BooleanError {
    BooleanError::Brep(BrepError::Geometry(e))
}

#[cfg(test)]
#[path = "boolean_curved_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "boolean_cone_tests.rs"]
mod cone_tests;
