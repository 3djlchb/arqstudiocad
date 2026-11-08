//! Booleans of solids with cylindrical faces (ADR 0033, delivery 2):
//! planes and circular cylinders, bounded by lines, circles and ellipses.
//!
//! # Method
//!
//! The planar booleans' (ADR 0032 D2), on curved faces:
//!
//! - **Domains:** each face is split in its own domain. A plane uses a frame
//!   in it; a cylinder its angle and height `(σθ, h)` (σ = ±1, so the
//!   domain turns about the face's outward normal), in a window of one turn
//!   that starts where nothing happens, closed by the seam where a region
//!   crosses it.
//! - **Sections:** the other solid's section on the face's surface is
//!   bounded by where its faces cut that surface, each kept where it lies
//!   in its own face, run with the other solid on its left. A plane cuts a
//!   plane in a line, a cylinder in an ellipse (two lines along it, a
//!   circle across it), kept exact; on a cylinder those curves are
//!   graphs `h(θ)` or lines.
//! - **Split and select:** [`CurvedRegion`]'s booleans split each face, and
//!   each operation keeps its parts as the planar booleans do.
//! - **Lift and assemble:** the parts are lifted back to lines, circles and
//!   ellipses, seams dropped, corners welded, edges split where another
//!   face has a corner on them, shells grouped by their volume's sign, a
//!   void given to the smallest shell a ray from it crosses an odd number of
//!   times.
//!
//! This version handles solids in general position, and cylinders with flat
//! faces only: two cylindrical faces meeting are delivery 4, coincident
//! faces delivery 2.3.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use arq_geometry::{CurvedRegion, Point2, Point3, Tolerance, Trim, Vector2, Vector3};

use crate::boolean::{BooleanError, Op, tolerance_of};
use crate::brep::{Body, BodyBuilder, BrepError, Coedge, Edge, Face, Loop, Lump, Shell};
use crate::chart::frame;
use crate::curve::{Curve, EllipseCurve, LineCurve};
use crate::surface::Surface;
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
            Self::Arc { .. } => {
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

/// Where a face lives: a plane or a circular cylinder, with its outward
/// normal.
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
    /// The cylinder of radius `r` round the unit `axis` through `origin`,
    /// `θ` from `e1` toward `e2 = axis × e1`; `sigma` +1 when its outside
    /// is away from the axis, −1 when toward it; the window of `x = σθ`
    /// starts at `x0`.
    Cylinder {
        origin: Point3,
        axis: Vector3,
        r: f64,
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
            Self::Cylinder {
                origin,
                axis,
                sigma,
                ..
            } => {
                let d = p - origin;
                let radial = d - axis * d.dot(axis);
                radial.normalized().map_or(axis, |r| r * sigma)
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
            Self::Cylinder {
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
            Self::Cylinder {
                origin,
                axis,
                r,
                e1,
                e2,
                sigma,
                ..
            } => {
                let (s, c) = (sigma * q[0]).sin_cos();
                origin + (e1 * c + e2 * s) * r + axis * q[1]
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

/// A face of the result: its surface, orientation, domain and region.
struct Part {
    surface: Surface,
    reversed: bool,
    domain: Domain,
    region: CurvedRegion,
}

/// Whether `body` has a face this module needs (a cylinder) and only faces
/// it can take (planes and cylinders, edges lines and ellipses).
pub(crate) fn takes(body: &Body) -> bool {
    body.faces().iter().all(|f| match &f.surface {
        Surface::Plane(_) => true,
        Surface::Cone(c) => c.is_cylinder(),
        _ => false,
    }) && body
        .edges()
        .iter()
        .all(|e| matches!(e.curve, Some(Curve::Line(_) | Curve::Ellipse(_))))
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
            let (face, region, section) = framed(face, region, other, tol)?;
            let (face, region) = (&face, &region);
            let kept = match (op, first) {
                (Op::Union, _) | (Op::Difference, true) => region.difference(&section, tol),
                (Op::Intersection, _) | (Op::Difference, false) => {
                    region.intersection(&section, tol)
                }
            };
            let flip = op == Op::Difference && !first;
            for piece in across_seam(&face.domain, kept.pieces(tol), tol) {
                parts.push(Part {
                    surface: face.surface.clone(),
                    reversed: face.reversed ^ flip,
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
    if parts.is_empty() {
        return Ok(None);
    }
    assemble(&parts, tol).map(Some)
}

/// A cylinder's pieces joined again where the window's seam cut one face
/// in two: a piece meeting the window's right edge over some heights is one
/// face with the piece meeting its left edge over the same heights.
fn across_seam(domain: &Domain, pieces: Vec<CurvedRegion>, tol: Tolerance) -> Vec<CurvedRegion> {
    let Domain::Cylinder { x0, .. } = *domain else {
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

/// The body as faces of planes and cylinders, each with its region.
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
                let span = span_of(body, edge)?;
                spans.push(if coedge.reversed {
                    span.reversed()
                } else {
                    span
                });
            }
            loops.push(spans);
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
            Surface::Cone(c) if c.is_cylinder() => {
                let axis = c.axis();
                let e1 = c.reference();
                Domain::Cylinder {
                    origin: c.origin(),
                    axis,
                    r: c.radius(),
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
        if let Domain::Cylinder { .. } = face.domain {
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
            if let Domain::Cylinder { x0: start, .. } = &mut face.domain {
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
        _ => Err(BooleanError::Curved),
    }
}

// ---------------------------------------------------------------- domains

/// `span`, which lies on the face, in the face's domain; on a cylinder
/// unwrapped from its start, not yet brought into the window.
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
        (Domain::Cylinder { .. }, Span::Line { from, to }) => {
            let (p, q) = (domain.flat(from), domain.flat(to));
            // A line on a cylinder runs along it: one angle.
            Ok(Trim::Segment {
                from: pt(p)?,
                to: pt([p[0], q[1]])?,
            })
        }
        (
            Domain::Cylinder {
                origin,
                axis,
                r,
                e1,
                e2,
                sigma,
                ..
            },
            Span::Arc { center, a, b, .. },
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
            let normal = a.cross(b);
            let along = normal.normalized().map_or(0.0, |m| m.dot(axis).abs());
            let rise = |p: Point3| (p - origin).dot(axis);
            if (along - 1.0).abs() <= 1e-9 {
                // A circle across the axis: one height.
                let h = rise(center);
                Ok(Trim::Segment {
                    from: pt([x_from, h])?,
                    to: pt([x_to, h])?,
                })
            } else {
                // An ellipse where a plane cuts the cylinder: the plane's
                // height over each angle, h = (k − r m·e(θ)) / m·axis.
                let m = normal.normalized().map_err(geometry)?;
                let k = m.dot(center - origin);
                let den = m.dot(axis);
                if den.abs() <= 1e-12 {
                    return Err(BooleanError::NotSolid);
                }
                let (ce, se) = (-r * m.dot(e1), -r * m.dot(e2));
                // In x = σθ: cos θ = cos x, sin θ = σ sin x.
                Ok(Trim::Graph {
                    num: [k, ce, sigma * se],
                    den: [den, 0.0, 0.0],
                    from: x_from,
                    to: x_to,
                })
            }
        }
    }
}

/// A window start away from every corner and every turn of the trims: the
/// middle of the widest gap between them.
fn quiet_start<'a>(trims: impl Iterator<Item = &'a Trim>) -> f64 {
    let mut events: Vec<f64> = Vec::new();
    for t in trims {
        events.push(t.start().x().rem_euclid(TAU));
        events.push(t.end().x().rem_euclid(TAU));
        if let Trim::Graph { num, .. } = t {
            // Where h(x) turns: n₂ cos x = n₁ sin x.
            let x = num[2].atan2(num[1]);
            events.push(x.rem_euclid(TAU));
            events.push((x + PI).rem_euclid(TAU));
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

/// The region bounded by `trims` in `domain`: on a cylinder, brought into
/// the window and closed by the seam.
fn close(domain: &Domain, trims: Vec<Trim>, tol: Tolerance) -> Result<CurvedRegion, BooleanError> {
    let Domain::Cylinder { x0, .. } = *domain else {
        return Ok(CurvedRegion::from_trims(&trims, tol));
    };
    let t = tol.linear();
    let mut inside: Vec<Trim> = Vec::new();
    for trim in trims {
        inside.extend(into_window(&trim, x0, t)?);
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
    if heights.len() % 2 == 1 {
        return Err(BooleanError::NonManifold);
    }
    let point = |x: f64, y: f64| Point2::new(x, y).map_err(geometry);
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
        let part = sub_trim(trim, from, s);
        let mid = part.point(0.5).x();
        let shift = TAU * ((mid - x0) / TAU).floor();
        out.push(shifted(&part, -shift)?);
        from = s;
    }
    Ok(out)
}

/// The part of `trim` from `t0` to `t1`, for segments and graphs, whose
/// parameter runs evenly in x.
fn sub_trim(trim: &Trim, t0: f64, t1: f64) -> Trim {
    match *trim {
        Trim::Segment { .. } => Trim::Segment {
            from: trim.point(t0),
            to: trim.point(t1),
        },
        Trim::Graph { num, den, from, to } => Trim::Graph {
            num,
            den,
            from: from + (to - from) * t0,
            to: from + (to - from) * t1,
        },
        Trim::Ellipse {
            center,
            a,
            b,
            start,
            sweep,
        } => Trim::Ellipse {
            center,
            a,
            b,
            start: start + sweep * t0,
            sweep: sweep * (t1 - t0),
        },
    }
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
) -> Result<(CFace, CurvedRegion, CurvedRegion), BooleanError> {
    let mut cuts = Vec::new();
    for (g, r) in other.faces.iter().zip(&other.regions) {
        cuts.extend(cut(face, g, r, tol)?);
    }
    let Domain::Cylinder { .. } = face.domain else {
        let section = close(&face.domain, cuts, tol)?;
        return Ok((face.clone(), region.clone(), section));
    };
    let own: Vec<Trim> = face
        .loops
        .iter()
        .flatten()
        .map(|s| trim_on(&face.domain, s))
        .collect::<Result<_, _>>()?;
    let mut moved = face.clone();
    if let Domain::Cylinder { x0, .. } = &mut moved.domain {
        *x0 = quiet_start(own.iter().chain(&cuts));
    }
    let region = close(&moved.domain, own, tol)?;
    let section = close(&moved.domain, cuts, tol)?;
    Ok((moved, region, section))
}

/// Where `g`'s face cuts `face`'s surface, the parts in `g` (its `region`),
/// as trims of `face`'s domain run with `g`'s solid on their left.
fn cut(
    face: &CFace,
    g: &CFace,
    region: &CurvedRegion,
    tol: Tolerance,
) -> Result<Vec<Trim>, BooleanError> {
    let t = tol.linear();
    // The curves, as spans in space, before keeping their parts in g.
    let curves: Vec<Span> = match (face.domain, g.domain) {
        (
            Domain::Plane { origin, n, .. },
            Domain::Plane {
                origin: og, n: ng, ..
            },
        ) => plane_plane(origin, n, og, ng, &g.domain, region, t),
        (Domain::Plane { origin, n, .. }, Domain::Cylinder { .. }) => {
            plane_cylinder(origin, n, &g.domain)
        }
        (Domain::Cylinder { .. }, Domain::Plane { origin, n, .. }) => {
            plane_cylinder(origin, n, &face.domain)
        }
        (Domain::Cylinder { .. }, Domain::Cylinder { .. }) => {
            return if apart(face, g, t) {
                Ok(Vec::new())
            } else {
                Err(BooleanError::Curved)
            };
        }
    };
    let mut out = Vec::new();
    for curve in curves {
        for part in inside_parts(&curve, g, region, tol)? {
            // Run with g's solid on the left: along n_face × n_g.
            let p = part.point(0.5);
            let want = face.domain.normal(p).cross(g.domain.normal(p));
            let part = if part.derivative(0.5).dot(want) < 0.0 {
                part.reversed()
            } else {
                part
            };
            let trim = trim_on(&face.domain, &part)?;
            out.push(trim);
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
    let Ok(dir) = d.normalized() else {
        return Vec::new(); // parallel
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
    let Domain::Cylinder {
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
            // A cylinder's side bulges beyond its edges' box by at most its
            // radius; take the boxes widened by it.
            let grow = |f: &CFace| match f.domain {
                Domain::Cylinder { r, .. } => r,
                Domain::Plane { .. } => 0.0,
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
    g: &CFace,
    region: &CurvedRegion,
    tol: Tolerance,
) -> Result<Vec<Span>, BooleanError> {
    let t = tol.linear();
    // The curve in g's domain, as one trim (into g's window if a cylinder).
    let image = trim_on(&g.domain, curve)?;
    let images: Vec<(Trim, f64, f64)> = match g.domain {
        Domain::Cylinder { x0, .. } => {
            // Split as into_window does, remembering each piece's share.
            let pieces = into_window(&image, x0, t)?;
            let (xa, xb) = (image.start().x(), image.end().x());
            let mut out = Vec::new();
            let mut from = 0.0;
            for p in pieces {
                let span = (p.end().x() - p.start().x()) / (xb - xa);
                let to = if (xb - xa).abs() > 0.0 {
                    from + span
                } else {
                    1.0
                };
                out.push((p, from, to));
                from = to;
            }
            out
        }
        Domain::Plane { .. } => vec![(image, 0.0, 1.0)],
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
            let mid = trim.point((w[0] + w[1]) / 2.0);
            if inside_face(&g.domain, region, mid, tol) {
                let (a, b) = (s0 + (s1 - s0) * w[0], s0 + (s1 - s0) * w[1]);
                // Longer than the tolerance (a closed curve ends where it
                // starts: its middle tells).
                if (curve.point(a) - curve.point((a + b) / 2.0)).length() > t {
                    parts.push(curve.part(a, b));
                }
            }
        }
    }
    // Joined again where the window cut them, so one piece runs on.
    Ok(parts)
}

/// Whether the point `q` of a face's domain lies inside the face's region.
/// On a cylinder's seam the window's region has its edge, but the face
/// goes on across it: there the point is inside if it is so a hair to
/// either side.
fn inside_face(domain: &Domain, region: &CurvedRegion, q: Point2, tol: Tolerance) -> bool {
    let inside = |p: Point2| region.contains(p, tol) == arq_geometry::Containment::Inside;
    let Domain::Cylinder { x0, r, .. } = *domain else {
        return inside(q);
    };
    let t = tol.linear();
    let seam = (q.x() - x0).abs() <= t || (q.x() - x0 - TAU).abs() <= t;
    if !seam {
        return inside(q);
    }
    // A hair: a hundred tolerances along the circle.
    let hair = 100.0 * t / r.max(f64::MIN_POSITIVE);
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
    // Each part's loops as spans in space, seams dropped and chains joined.
    let mut faces: Vec<(usize, Vec<Vec<Use>>)> = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let mut loops = Vec::new();
        // All its contours at once: on a cylinder they close across the
        // seam, one contour into another.
        let mut spans = Vec::new();
        for contour in part.region.contours() {
            spans.extend(lift_contour(&part.domain, contour, t)?);
        }
        for chain in chain_up(spans, t)? {
            let uses: Vec<Use> = chain
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
    let built = build(parts, &faces, &shells, &[], &pool.points, tol)?;
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
    build(parts, &faces, &shells, &lumps, &pool.points, tol)
}

/// Whether a closed arc turns forward: about one fixed direction, whatever
/// diameters it is written with.
fn arc_turns_forward(span: &Span) -> bool {
    match span {
        Span::Arc { a, b, sweep, .. } => {
            let k = Vector3::new(0.316_227_766, 0.547_722_557_5, 0.774_596_669_2).expect("finite");
            (a.cross(*b).dot(k) > 0.0) == (*sweep > 0.0)
        }
        Span::Line { .. } => true,
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
                Domain::Cylinder {
                    origin,
                    axis,
                    r,
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
                    // Round the axis at one height.
                    out.push(Span::Arc {
                        center: origin + axis * from.y(),
                        a: e1 * r,
                        b: e2 * r,
                        start: sigma * from.x(),
                        sweep: sigma * (to.x() - from.x()),
                    });
                } else {
                    return Err(BooleanError::NonManifold);
                }
            }
            (
                Domain::Cylinder {
                    origin,
                    axis,
                    r,
                    e1,
                    e2,
                    sigma,
                    ..
                },
                Trim::Graph { num, den, from, to },
            ) => {
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
            _ => return Err(BooleanError::NonManifold),
        }
    }
    Ok(out)
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
        let hits: Vec<f64> = match part.domain {
            Domain::Plane { origin, n, .. } => {
                let den = w.dot(n);
                if den.abs() <= 1e-15 {
                    Vec::new()
                } else {
                    vec![(origin - p).dot(n) / den]
                }
            }
            Domain::Cylinder {
                origin, axis, r, ..
            } => {
                // |(p + s w − origin) ⟂ axis| = r.
                let d = p - origin;
                let (dp, wp) = (d - axis * d.dot(axis), w - axis * w.dot(axis));
                let (qa, qb, qc) = (wp.dot(wp), 2.0 * dp.dot(wp), dp.dot(dp) - r * r);
                let disc = qb * qb - 4.0 * qa * qc;
                if qa <= 1e-30 || disc < 0.0 {
                    Vec::new()
                } else {
                    let root = disc.sqrt();
                    vec![(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
                }
            }
        };
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

/// The body of the faces, edges once per pair of uses; `lumps` of shells,
/// or each shell its own lump when empty.
fn build(
    parts: &[Part],
    faces: &[(usize, Vec<Vec<Use>>)],
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
    for (index, loops) in faces {
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

/// An edge from `start` to `end` along `span`'s curve: a line, or an
/// ellipse from the span's conjugate diameters, run against it when the
/// span turns backward.
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
