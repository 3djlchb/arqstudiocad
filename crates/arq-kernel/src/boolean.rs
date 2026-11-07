//! Booleans of planar solids (ADR 0032): the union, intersection and
//! difference of bodies whose faces are all planes and whose edges are all
//! lines.
//!
//! # Method (ADR 0032 D2)
//!
//! Each face of one solid is split by the other solid's section in the
//! face's plane. The other solid's faces that the plane cuts give the
//! segments bounding the part of the plane inside that solid, and the 2D
//! booleans of [`Region`] split the face into the part inside and the part
//! outside: splitting and classifying are one step. Each operation keeps its
//! parts (union what is outside the other solid, intersection what is
//! inside, A − B the part of A outside B and the part of B inside A turned
//! over), their corners welded within the tolerance and their edges split
//! where a neighbouring face has a corner on them. The faces sharing edges
//! make shells: a shell enclosing a positive volume is the outside of a
//! piece (a lump), a negative one a void, given to the smallest piece
//! around it.
//!
//! A corner of one solid on a face's plane of the other counts as lying on
//! the plane's outer side (a symbolic perturbation), so sections stay
//! closed. This version handles solids in general position: a face lying
//! on a face of the other solid is [`BooleanError::Coplanar`], and a result
//! whose pieces touch along an edge or at a corner is
//! [`BooleanError::NonManifold`]; both cases are ADR 0032's third step.

use std::collections::HashMap;
use std::fmt;

use arq_geometry::{Point2, Point3, Region, Tolerance, Vector3};

use crate::brep::{Body, BodyBuilder, BrepError, Coedge, Edge, Face, Loop, Lump, Shell};
use crate::chart::frame;
use crate::curve::{Curve, LineCurve};
use crate::surface::{Plane, Surface};

/// Why two solids have no boolean in this version.
#[derive(Clone, Debug, PartialEq)]
pub enum BooleanError {
    /// A face is not a plane, or an edge not a line.
    Curved,
    /// A body encloses no volume: a face without area, or faces turned in.
    NotSolid,
    /// A face of one solid lies on a face of the other.
    Coplanar,
    /// The result's faces touch along an edge or at a corner, or do not
    /// close.
    NonManifold,
    /// The result's parts form no body.
    Brep(BrepError),
}

impl fmt::Display for BooleanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Curved => write!(
                f,
                "only solids of flat faces and straight edges are supported"
            ),
            Self::NotSolid => write!(f, "a body does not enclose a volume"),
            Self::Coplanar => write!(f, "a face of one solid lies on a face of the other"),
            Self::NonManifold => {
                write!(f, "the result touches itself along an edge or at a corner")
            }
            Self::Brep(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BooleanError {}

/// Which boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Union,
    Intersection,
    Difference,
}

impl Body {
    /// The solid of the points in this one or in `other`; solids apart
    /// make one body of separate lumps.
    pub fn union(&self, other: &Self) -> Result<Self, BooleanError> {
        boolean(self, other, Op::Union)?.ok_or(BooleanError::NotSolid)
    }

    /// The solid of the points in both, none if they do not overlap.
    pub fn intersection(&self, other: &Self) -> Result<Option<Self>, BooleanError> {
        boolean(self, other, Op::Intersection)
    }

    /// The solid of the points in this one and not in `other`, none if
    /// `other` covers it.
    pub fn difference(&self, other: &Self) -> Result<Option<Self>, BooleanError> {
        boolean(self, other, Op::Difference)
    }
}

/// A flat face: its outward unit normal and its loops as corner indices.
struct FlatFace {
    normal: Vector3,
    loops: Vec<Vec<usize>>,
}

/// A body read as flat faces over its corners.
struct Solid {
    points: Vec<Point3>,
    faces: Vec<FlatFace>,
}

/// A face of the result: its plane and its loops, the outer one first.
struct Piece {
    origin: Point3,
    normal: Vector3,
    loops: Vec<Vec<Point3>>,
}

fn boolean(a: &Body, b: &Body, op: Op) -> Result<Option<Body>, BooleanError> {
    let (sa, sb) = (solid(a)?, solid(b)?);
    let tol = tolerance(sa.points.iter().chain(&sb.points))?;
    let mut pieces = Vec::new();
    for (own, other, first) in [(&sa, &sb, true), (&sb, &sa, false)] {
        for face in &own.faces {
            let origin = own.points[face.loops[0][0]];
            let (u, v) = frame(face.normal);
            let flat = |p: Point3| {
                let w = p - origin;
                Point2::new(w.dot(u), w.dot(v))
            };
            let contours = face
                .loops
                .iter()
                .map(|l| l.iter().map(|&i| flat(own.points[i])).collect())
                .collect::<Result<Vec<Vec<Point2>>, _>>()
                .map_err(BrepError::Geometry)
                .map_err(BooleanError::Brep)?;
            let region = Region::new(contours).map_err(|_| BooleanError::NotSolid)?;
            let segments = section(other, &region, origin, face.normal, (u, v), tol)?;
            let section = Region::from_edges(&segments, tol);
            let kept = match (op, first) {
                (Op::Union, _) | (Op::Difference, true) => region.difference(&section, tol),
                (Op::Intersection, _) | (Op::Difference, false) => {
                    region.intersection(&section, tol)
                }
            };
            let flip = op == Op::Difference && !first;
            for part in kept.pieces(tol) {
                let lift = |q: &Point2| origin + u * q.x() + v * q.y();
                let mut loops: Vec<Vec<Point3>> = part
                    .contours()
                    .iter()
                    .map(|c| c.iter().map(lift).collect())
                    .collect();
                if flip {
                    loops.iter_mut().for_each(|l| l.reverse());
                }
                pieces.push(Piece {
                    origin,
                    normal: if flip { -face.normal } else { face.normal },
                    loops,
                });
            }
        }
    }
    if pieces.is_empty() {
        return Ok(None);
    }
    assemble(&pieces, tol).map(Some)
}

/// The body as flat faces, if all its faces are planes and edges lines and
/// it encloses a volume.
fn solid(body: &Body) -> Result<Solid, BooleanError> {
    let mut ids: Vec<usize> = body
        .lumps()
        .iter()
        .flat_map(|l| &l.shells)
        .flat_map(|s| &body.shells()[s.index()].faces)
        .map(|f| f.index())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let points: Vec<Point3> = body.vertices().iter().map(|v| v.point).collect();
    let mut faces = Vec::with_capacity(ids.len());
    for i in ids {
        let face = &body.faces()[i];
        let Surface::Plane(plane) = &face.surface else {
            return Err(BooleanError::Curved);
        };
        if face.loops.is_empty() {
            return Err(BooleanError::NotSolid);
        }
        let mut loops = Vec::with_capacity(face.loops.len());
        for l in &face.loops {
            let mut corners = Vec::new();
            for c in &body.loops()[l.index()].coedges {
                let coedge = &body.coedges()[c.index()];
                let edge = body.edge(coedge.edge);
                if !matches!(edge.curve, Some(Curve::Line(_))) {
                    return Err(BooleanError::Curved);
                }
                corners.push(
                    if coedge.reversed {
                        edge.end
                    } else {
                        edge.start
                    }
                    .index(),
                );
            }
            if corners.len() < 3 {
                return Err(BooleanError::NotSolid);
            }
            loops.push(corners);
        }
        let normal = if face.reversed {
            -plane.normal()
        } else {
            plane.normal()
        };
        // The loops turn counterclockwise round the outward normal.
        let area: f64 = loops
            .iter()
            .map(|l| newell(l.iter().map(|&i| points[i])).dot(normal))
            .sum();
        if area <= 0.0 {
            return Err(BooleanError::NotSolid);
        }
        faces.push(FlatFace { normal, loops });
    }
    let solid = Solid { points, faces };
    let center = solid.points.first().copied().unwrap_or(Point3::ORIGIN);
    let volume: f64 = solid
        .faces
        .iter()
        .flat_map(|f| &f.loops)
        .map(|l| fan_volume(l.iter().map(|&i| solid.points[i]), center))
        .sum();
    if volume <= 0.0 {
        return Err(BooleanError::NotSolid);
    }
    Ok(solid)
}

/// As the primitives': a billionth of the farthest coordinate, at least of
/// the size of both solids together.
fn tolerance<'a>(
    points: impl Iterator<Item = &'a Point3> + Clone,
) -> Result<Tolerance, BooleanError> {
    let reach = points
        .clone()
        .flat_map(|p| [p.x().abs(), p.y().abs(), p.z().abs()])
        .fold(0.0, f64::max);
    let size =
        arq_geometry::Bounds3::from_points(points.copied()).map_or(0.0, |b| b.size().length());
    Tolerance::new(1e-9 * reach.max(size), Tolerance::DEFAULT.angular())
        .map_err(BrepError::Geometry)
        .map_err(BooleanError::Brep)
}

/// The directed segments, in the frame `(u, v)` at `origin`, bounding the
/// part of the plane through `origin` with unit `normal` inside `solid`, the
/// inside on their left; [`BooleanError::Coplanar`] if a face of the solid
/// lies on the plane where `region` is.
fn section(
    solid: &Solid,
    region: &Region,
    origin: Point3,
    normal: Vector3,
    (u, v): (Vector3, Vector3),
    tol: Tolerance,
) -> Result<Vec<(Point2, Point2)>, BooleanError> {
    let t = tol.linear();
    let flat = |p: Point3| {
        let w = p - origin;
        Point2::new(w.dot(u), w.dot(v))
            .map_err(BrepError::Geometry)
            .map_err(BooleanError::Brep)
    };
    let height: Vec<f64> = solid
        .points
        .iter()
        .map(|&p| (p - origin).dot(normal))
        .collect();
    // On the plane counts as above it.
    let above: Vec<bool> = height.iter().map(|&h| h >= -t).collect();
    let mut crossings: HashMap<(usize, usize), Point3> = HashMap::new();
    let mut segments = Vec::new();
    for face in &solid.faces {
        let corners = || face.loops.iter().flatten().copied();
        if corners().all(|i| height[i].abs() <= t) {
            // A face on the plane matters only near the region.
            let flats = corners()
                .map(|i| flat(solid.points[i]))
                .collect::<Result<Vec<_>, _>>()?;
            if overlap(&flats, region.contours().iter().flatten().copied(), t) {
                return Err(BooleanError::Coplanar);
            }
            continue;
        }
        let along = normal.cross(face.normal);
        if along.length() <= tol.angular() {
            continue;
        }
        let mut hits = Vec::new();
        for l in &face.loops {
            for k in 0..l.len() {
                let (i, j) = (l[k], l[(k + 1) % l.len()]);
                if above[i] == above[j] {
                    continue;
                }
                let key = (i.min(j), i.max(j));
                // Computed once per edge, so both its faces share it.
                let p = *crossings.entry(key).or_insert_with(|| {
                    let (a, b) = key;
                    let (pa, pb) = (solid.points[a], solid.points[b]);
                    pa + (pb - pa) * (height[a] / (height[a] - height[b]))
                });
                hits.push(((p - origin).dot(along), p));
            }
        }
        if hits.len() % 2 == 1 {
            return Err(BooleanError::NotSolid);
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in hits.chunks(2) {
            segments.push((flat(pair[0].1)?, flat(pair[1].1)?));
        }
    }
    Ok(segments)
}

/// Whether the boxes around two sets of points overlap within `t`.
fn overlap(a: &[Point2], b: impl Iterator<Item = Point2>, t: f64) -> bool {
    let bounds = |points: &mut dyn Iterator<Item = Point2>| {
        points.fold(None, |acc: Option<[f64; 4]>, p| {
            Some(acc.map_or([p.x(), p.y(), p.x(), p.y()], |b| {
                [
                    b[0].min(p.x()),
                    b[1].min(p.y()),
                    b[2].max(p.x()),
                    b[3].max(p.y()),
                ]
            }))
        })
    };
    match (bounds(&mut a.iter().copied()), bounds(&mut b.into_iter())) {
        (Some(p), Some(q)) => {
            p[0] <= q[2] + t && q[0] <= p[2] + t && p[1] <= q[3] + t && q[1] <= p[3] + t
        }
        _ => false,
    }
}

/// The body of the result's faces, if they close into shells.
fn assemble(pieces: &[Piece], tol: Tolerance) -> Result<Body, BooleanError> {
    let t = tol.linear();
    let mut pool = Pool::new(t);
    // Each face's loops as welded corners, without repeats.
    let mut faces: Vec<(usize, Vec<Vec<usize>>)> = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let mut loops = Vec::new();
        for (k, l) in piece.loops.iter().enumerate() {
            let mut ids: Vec<usize> = l.iter().map(|&p| pool.weld(p)).collect();
            ids.dedup();
            while ids.len() > 1 && ids.first() == ids.last() {
                ids.pop();
            }
            if ids.len() >= 3 {
                loops.push(ids);
            } else if k == 0 {
                break;
            }
        }
        if !loops.is_empty() {
            faces.push((index, loops));
        }
    }
    // An edge split where another face has a corner on it.
    let points = pool.points.clone();
    for (_, loops) in &mut faces {
        for l in loops.iter_mut() {
            let mut out = Vec::with_capacity(l.len());
            for k in 0..l.len() {
                let (i, j) = (l[k], l[(k + 1) % l.len()]);
                out.push(i);
                let (a, b) = (points[i], points[j]);
                let d = b - a;
                let len2 = d.dot(d);
                let mut on: Vec<(f64, usize)> = (0..points.len())
                    .filter(|&m| m != i && m != j)
                    .filter_map(|m| {
                        let w = points[m] - a;
                        let s = w.dot(d) / len2;
                        let len = len2.sqrt();
                        let inner = s * len > t && (1.0 - s) * len > t;
                        (inner && (w - d * s).length() <= t).then_some((s, m))
                    })
                    .collect();
                on.sort_by(|x, y| x.0.total_cmp(&y.0));
                out.extend(on.into_iter().map(|(_, m)| m));
            }
            *l = out;
        }
    }
    // Each edge once each way: the faces close without touching.
    let mut uses: HashMap<(usize, usize), (usize, u32)> = HashMap::new();
    for (f, (_, loops)) in faces.iter().enumerate() {
        for l in loops {
            for k in 0..l.len() {
                uses.entry((l[k], l[(k + 1) % l.len()])).or_insert((f, 0)).1 += 1;
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
    for (&(i, j), &(f, count)) in &uses {
        match uses.get(&(j, i)) {
            Some(&(g, 1)) if count == 1 => {
                let (rf, rg) = (root(&mut parent, f), root(&mut parent, g));
                parent[rf] = rg;
            }
            _ => return Err(BooleanError::NonManifold),
        }
    }
    // Shells, their volumes, and the lumps.
    let mut shells: HashMap<usize, Vec<usize>> = HashMap::new();
    for f in 0..faces.len() {
        let r = root(&mut parent, f);
        shells.entry(r).or_default().push(f);
    }
    let mut shells: Vec<Vec<usize>> = shells.into_values().collect();
    shells.sort();
    let center = points[0];
    let corners = |l: &Vec<usize>| l.iter().map(|&i| points[i]).collect::<Vec<_>>();
    let volume = |shell: &[usize]| -> f64 {
        shell
            .iter()
            .flat_map(|&f| &faces[f].1)
            .map(|l| fan_volume(corners(l).into_iter(), center))
            .sum()
    };
    let volumes: Vec<f64> = shells.iter().map(|s| volume(s)).collect();
    let outsides: Vec<usize> = (0..shells.len()).filter(|&s| volumes[s] > 0.0).collect();
    let mut lumps: Vec<Vec<usize>> = outsides.iter().map(|&s| vec![s]).collect();
    for void in (0..shells.len()).filter(|&s| volumes[s] <= 0.0) {
        let probe = points[faces[shells[void][0]].1[0][0]];
        let around = outsides
            .iter()
            .enumerate()
            .filter(|&(_, &o)| {
                let w: f64 = shells[o]
                    .iter()
                    .flat_map(|&f| &faces[f].1)
                    .map(|l| solid_angle(&corners(l), probe))
                    .sum();
                w > 2.0 * std::f64::consts::PI
            })
            .min_by(|x, y| volumes[*x.1].total_cmp(&volumes[*y.1]));
        match around {
            Some((k, _)) => lumps[k].push(void),
            None => return Err(BooleanError::NonManifold),
        }
    }
    // The body.
    let mut b = BodyBuilder::new();
    let mut vertices = HashMap::new();
    let mut edges = HashMap::new();
    let mut face_ids = Vec::with_capacity(faces.len());
    for (index, loops) in &faces {
        let piece = &pieces[*index];
        let mut loop_ids = Vec::with_capacity(loops.len());
        for l in loops {
            let mut coedges = Vec::with_capacity(l.len());
            for k in 0..l.len() {
                let (i, j) = (l[k], l[(k + 1) % l.len()]);
                let key = (i.min(j), i.max(j));
                let edge = match edges.get(&key) {
                    Some(&edge) => edge,
                    None => {
                        let mut vertex =
                            |m: usize| *vertices.entry(m).or_insert_with(|| b.vertex(points[m]));
                        let (start, end) = (vertex(key.0), vertex(key.1));
                        let (from, to) = (points[key.0], points[key.1]);
                        let line = LineCurve::new(from, to - from)
                            .map_err(BrepError::Geometry)
                            .map_err(BooleanError::Brep)?;
                        let edge = b.edge(Edge {
                            curve: Some(Curve::Line(line)),
                            start,
                            end,
                            reversed: false,
                        });
                        edges.insert(key, edge);
                        edge
                    }
                };
                coedges.push(b.coedge(Coedge {
                    edge,
                    reversed: i > j,
                }));
            }
            loop_ids.push(b.add_loop(Loop { coedges }));
        }
        let plane = Plane::new(piece.origin, piece.normal)
            .map_err(BrepError::Geometry)
            .map_err(BooleanError::Brep)?;
        face_ids.push(b.face(Face {
            surface: Surface::Plane(plane),
            loops: loop_ids,
            reversed: false,
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
    for lump in lumps {
        b.lump(Lump {
            shells: lump.into_iter().map(|s| shell_ids[s]).collect(),
        });
    }
    // Welding moves a corner up to the tolerance from its own plane.
    let check = Tolerance::new(16.0 * t, tol.angular())
        .map_err(BrepError::Geometry)
        .map_err(BooleanError::Brep)?;
    b.build(check).map_err(BooleanError::Brep)
}

/// Points welded within a tolerance.
struct Pool {
    tol: f64,
    points: Vec<Point3>,
    cells: HashMap<(i64, i64, i64), Vec<usize>>,
}

impl Pool {
    fn new(tol: f64) -> Self {
        Self {
            tol,
            points: Vec::new(),
            cells: HashMap::new(),
        }
    }

    /// The index of the point within the tolerance of `p`, a new one if
    /// none is.
    fn weld(&mut self, p: Point3) -> usize {
        // Cells twice the tolerance wide: a point within it is in one of the
        // 27 around `p`'s. (Casts saturate.)
        let size = 2.0 * self.tol;
        let cell = |x: f64| (x / size).floor() as i64;
        let key = (cell(p.x()), cell(p.y()), cell(p.z()));
        for dx in -1..=1i64 {
            for dy in -1..=1i64 {
                for dz in -1..=1i64 {
                    let near = (
                        key.0.saturating_add(dx),
                        key.1.saturating_add(dy),
                        key.2.saturating_add(dz),
                    );
                    if let Some(found) = self.cells.get(&near) {
                        for &i in found {
                            if (self.points[i] - p).length() <= self.tol {
                                return i;
                            }
                        }
                    }
                }
            }
        }
        self.points.push(p);
        let index = self.points.len() - 1;
        self.cells.entry(key).or_default().push(index);
        index
    }
}

/// Newell's area vector of a closed polygon: its normal times its area.
fn newell(corners: impl Iterator<Item = Point3> + Clone) -> Vector3 {
    let first = corners.clone().next();
    let mut sum = Vector3::new(0.0, 0.0, 0.0).expect("finite");
    let mut previous = None;
    for p in corners.chain(first) {
        if let (Some(q), Some(o)) = (previous, first) {
            let (a, b): (Vector3, Vector3) = (q - o, p - o);
            sum = sum + a.cross(b) * 0.5;
        }
        previous = Some(p);
    }
    sum
}

/// The signed volume of the cone from `center` to a closed polygon: summed
/// over a closed surface's loops, the volume it encloses.
fn fan_volume(corners: impl Iterator<Item = Point3>, center: Point3) -> f64 {
    let c: Vec<Vector3> = corners.map(|p| p - center).collect();
    (1..c.len().saturating_sub(1))
        .map(|k| c[0].dot(c[k].cross(c[k + 1])) / 6.0)
        .sum()
}

/// The signed solid angle a closed planar polygon subtends at `p`, by the
/// triangles of a fan (Van Oosterom and Strackee).
fn solid_angle(corners: &[Point3], p: Point3) -> f64 {
    let c: Vec<Vector3> = corners.iter().map(|&q| q - p).collect();
    (1..c.len().saturating_sub(1))
        .map(|k| {
            let (a, b, d) = (c[0], c[k], c[k + 1]);
            let (la, lb, ld) = (a.length(), b.length(), d.length());
            let num = a.dot(b.cross(d));
            let den = la * lb * ld + a.dot(b) * ld + a.dot(d) * lb + b.dot(d) * la;
            2.0 * num.atan2(den)
        })
        .sum()
}

#[cfg(test)]
#[path = "boolean_tests.rs"]
mod tests;
