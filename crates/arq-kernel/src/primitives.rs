//! Solids built from a few numbers (ADR 0024 step 4): the primitives the
//! 3D commands create: a box, a wedge, a cylinder, a cone, a sphere, a
//! torus, and a closed profile extruded.

use std::collections::HashMap;
use std::fmt;

use arq_geometry::{Geometry2, GeometryError, Point2, Point3, Tolerance, Vector3, intersect};

use crate::brep::{
    Body, BodyBuilder, BrepError, Coedge, Edge, EdgeId, Face, Loop, Lump, Shell, VertexId,
};
use crate::curve::{Curve, EllipseCurve, LineCurve};
use crate::surface::{Cone, Plane, Sphere, Surface, Torus};

impl Body {
    /// A box with its edges along the axes, between two opposite
    /// corners, in either order: AutoCAD's BOX. A side of no length is
    /// [`GeometryError::ZeroLength`].
    pub fn cuboid(corner: Point3, opposite: Point3) -> Result<Body, BrepError> {
        let lo = [
            corner.x().min(opposite.x()),
            corner.y().min(opposite.y()),
            corner.z().min(opposite.z()),
        ];
        let hi = [
            corner.x().max(opposite.x()),
            corner.y().max(opposite.y()),
            corner.z().max(opposite.z()),
        ];
        if (0..3).any(|i| hi[i] <= lo[i]) {
            return Err(BrepError::Geometry(GeometryError::ZeroLength));
        }
        let at = |x: usize, y: usize, z: usize| {
            Point3::new([lo[0], hi[0]][x], [lo[1], hi[1]][y], [lo[2], hi[2]][z])
                .map_err(BrepError::Geometry)
        };
        let corners = [
            at(0, 0, 0)?,
            at(1, 0, 0)?,
            at(1, 1, 0)?,
            at(0, 1, 0)?,
            at(0, 0, 1)?,
            at(1, 0, 1)?,
            at(1, 1, 1)?,
            at(0, 1, 1)?,
        ];
        // Each face's corners counterclockwise seen from outside, so its
        // outside is the plane's normal: bottom, top, then round the sides.
        let faces: [&[usize]; 6] = [
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[1, 2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 0, 4, 7],
        ];
        planar(&corners, &faces, tolerance(&corners)?)
    }

    /// A wedge in the box between two opposite corners, AutoCAD's WEDGE:
    /// its base at `first`'s height, as tall as `opposite`'s; its face at
    /// `first`'s X stands upright, and its sloped face, opposite `first`,
    /// comes down to the base at `opposite`'s X. A side of no length is
    /// [`GeometryError::ZeroLength`].
    pub fn wedge(first: Point3, opposite: Point3) -> Result<Body, BrepError> {
        let (x0, y0, z0) = (first.x(), first.y(), first.z());
        let (x1, y1, top) = (opposite.x(), opposite.y(), opposite.z());
        if x1 == x0 || y1 == y0 || top == z0 {
            return Err(BrepError::Geometry(GeometryError::ZeroLength));
        }
        let at = |x: f64, y: f64, z: f64| Point3::new(x, y, z).map_err(BrepError::Geometry);
        let corners = [
            at(x0, y0, z0)?,
            at(x1, y0, z0)?,
            at(x1, y1, z0)?,
            at(x0, y1, z0)?,
            at(x0, y0, top)?,
            at(x0, y1, top)?,
        ];
        // Counterclockwise from outside when X, Y and the height all run
        // forward: the base, the upright face, the slope, the two ends.
        let mut faces: Vec<Vec<usize>> = vec![
            vec![0, 3, 2, 1],
            vec![0, 4, 5, 3],
            vec![1, 2, 5, 4],
            vec![0, 1, 4],
            vec![3, 5, 2],
        ];
        // Each one running backward mirrors it: the corners turn the other
        // way round.
        let backward = [x1 < x0, y1 < y0, top < z0].iter().filter(|&&b| b).count();
        if backward % 2 == 1 {
            faces.iter_mut().for_each(|f| f.reverse());
        }
        let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
        planar(&corners, &faces, tolerance(&corners)?)
    }
}

impl Body {
    /// A cylinder standing on its base circle, centered at `center` with
    /// `radius`, `height` along Z (negative: downward): AutoCAD's
    /// CYLINDER.
    pub fn cylinder(center: Point3, radius: f64, height: f64) -> Result<Body, BrepError> {
        Self::cone(center, radius, radius, height)
    }

    /// A cone standing on its base circle, centered at `center` with
    /// radius `base`, `height` along Z (negative: downward), its top circle
    /// of radius `top`: AutoCAD's CONE. A radius of 0 is a point (the
    /// apex), not both; equal radii make a cylinder. A radius below 0 is
    /// [`GeometryError::InvalidRadius`], a height of 0
    /// [`GeometryError::ZeroLength`].
    pub fn cone(center: Point3, base: f64, top: f64, height: f64) -> Result<Body, BrepError> {
        let radii_ok = base.is_finite() && top.is_finite() && base >= 0.0 && top >= 0.0;
        if !radii_ok || base.max(top) == 0.0 {
            return Err(BrepError::Geometry(GeometryError::InvalidRadius));
        }
        if !height.is_finite() || height == 0.0 {
            return Err(BrepError::Geometry(GeometryError::ZeroLength));
        }
        let geometry = BrepError::Geometry;
        let axis = Vector3::new(0.0, 0.0, height.signum()).map_err(geometry)?;
        let across = Vector3::new(1.0, 0.0, 0.0).map_err(geometry)?;
        let rise = height.abs();
        let top_center = center + axis * rise;
        // The side, from the larger end so its reference circle has a size:
        // widening by (top - base) per rise along the axis.
        let slant = (top - base).hypot(rise);
        let (origin, radius) = if base >= top {
            (center, base)
        } else {
            (top_center, top)
        };
        let side = Cone::new(origin, axis, radius, (top - base) / slant, rise / slant)
            .map_err(geometry)?;

        let mut b = BodyBuilder::new();
        let mut points = vec![center, top_center];
        // An end: a circle, its vertex at parameter 0, or the apex.
        let mut end = |b: &mut BodyBuilder, at: Point3, r: f64| -> Result<EdgeId, BrepError> {
            if r > 0.0 {
                let start = at + across * r;
                points.push(start);
                let vertex = b.vertex(start);
                let circle = EllipseCurve::new(at, axis, across * r, 1.0).map_err(geometry)?;
                Ok(b.edge(Edge {
                    curve: Some(Curve::Ellipse(circle)),
                    start: vertex,
                    end: vertex,
                    reversed: false,
                }))
            } else {
                let apex = b.vertex(at);
                Ok(b.edge(Edge {
                    curve: None,
                    start: apex,
                    end: apex,
                    reversed: false,
                }))
            }
        };
        let low = end(&mut b, center, base)?;
        let high = end(&mut b, top_center, top)?;
        let one = |b: &mut BodyBuilder, edge: EdgeId, reversed: bool| {
            let coedge = b.coedge(Coedge { edge, reversed });
            b.add_loop(Loop {
                coedges: vec![coedge],
            })
        };
        // Seen from outside, the base circle turns clockwise about the axis
        // and the top one counterclockwise; the side runs them the other
        // way, the base forward and the top back.
        let mut faces = Vec::new();
        if base > 0.0 {
            let lp = one(&mut b, low, true);
            let plane = Plane::new(center, -axis).map_err(geometry)?;
            faces.push(b.face(Face {
                surface: Surface::Plane(plane),
                loops: vec![lp],
                reversed: false,
            }));
        }
        if top > 0.0 {
            let lp = one(&mut b, high, false);
            let plane = Plane::new(top_center, axis).map_err(geometry)?;
            faces.push(b.face(Face {
                surface: Surface::Plane(plane),
                loops: vec![lp],
                reversed: false,
            }));
        }
        let loops = vec![one(&mut b, low, false), one(&mut b, high, true)];
        faces.push(b.face(Face {
            surface: Surface::Cone(side),
            loops,
            reversed: false,
        }));
        let shell = b.shell(Shell { faces });
        b.lump(Lump {
            shells: vec![shell],
        });
        b.build(tolerance(&points)?)
    }
}

impl Body {
    /// A sphere around `center`: AutoCAD's SPHERE. A radius not above 0 is
    /// [`GeometryError::InvalidRadius`].
    pub fn sphere(center: Point3, radius: f64) -> Result<Body, BrepError> {
        // The sphere refuses a radius not above 0 itself.
        let sphere = Sphere::new(center, radius).map_err(BrepError::Geometry)?;
        let reach = Vector3::new(radius, radius, radius).map_err(BrepError::Geometry)?;
        whole(Surface::Sphere(sphere), &[center - reach, center + reach])
    }

    /// A torus around the Z axis through `center`: a tube of radius `tube`
    /// swept round at `radius` from it, AutoCAD's TORUS. The tube must be
    /// thinner than the torus is wide, a ring with a hole: a self-crossing
    /// torus, which AutoCAD also makes, waits. Otherwise
    /// [`GeometryError::InvalidRadius`].
    pub fn torus(center: Point3, radius: f64, tube: f64) -> Result<Body, BrepError> {
        let ring = radius.is_finite() && tube.is_finite() && tube > 0.0 && radius > tube;
        if !ring {
            return Err(BrepError::Geometry(GeometryError::InvalidRadius));
        }
        let axis = Vector3::new(0.0, 0.0, 1.0).map_err(BrepError::Geometry)?;
        let torus = Torus::new(center, axis, radius, tube).map_err(BrepError::Geometry)?;
        let out = radius + tube;
        let reach = Vector3::new(out, out, tube).map_err(BrepError::Geometry)?;
        whole(Surface::Torus(torus), &[center - reach, center + reach])
    }
}

/// A body of one face covering a whole closed surface, without loops (a
/// sphere, a torus); `extent` the corners of its box, for the tolerance.
fn whole(surface: Surface, extent: &[Point3]) -> Result<Body, BrepError> {
    let mut b = BodyBuilder::new();
    let face = b.face(Face {
        surface,
        loops: Vec::new(),
        reversed: false,
    });
    let shell = b.shell(Shell { faces: vec![face] });
    b.lump(Lump {
        shells: vec![shell],
    });
    b.build(tolerance(extent)?)
}

/// A piece of a closed profile in plan: a segment or an arc (stored
/// counterclockwise, as [`Arc`] is), run from its start to its end, or
/// `backward`, from its end to its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePiece {
    /// The segment or arc.
    pub curve: Geometry2,
    /// Whether the profile runs it from its end to its start.
    pub backward: bool,
}

impl ProfilePiece {
    /// Where the profile enters it.
    pub fn start(&self) -> Point2 {
        let (a, b) = ends(self.curve);
        if self.backward { b } else { a }
    }

    /// Where the profile leaves it.
    pub fn end(&self) -> Point2 {
        let (a, b) = ends(self.curve);
        if self.backward { a } else { b }
    }

    /// The same piece run the other way.
    fn reversed(self) -> Self {
        Self {
            backward: !self.backward,
            ..self
        }
    }

    /// Twice the area it sweeps about the origin, counterclockwise
    /// positive, with an arc's bulge beyond its chord.
    fn swept(&self) -> f64 {
        let (p, q) = (self.start(), self.end());
        let chord = p.x() * q.y() - q.x() * p.y();
        let bulge = match self.curve {
            Geometry2::Arc(a) => {
                let t = a.sweep();
                let segment = a.radius() * a.radius() * (t - t.sin());
                if self.backward { -segment } else { segment }
            }
            _ => 0.0,
        };
        chord + bulge
    }
}

/// Why a profile cannot be extruded.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileError {
    /// Fewer than two pieces, or one not starting where the last ends.
    Open,
    /// It crosses or touches itself away from its corners.
    Crossing,
    /// It encloses no area.
    Flat,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "the profile does not close"),
            Self::Crossing => write!(f, "the profile crosses itself"),
            Self::Flat => write!(f, "the profile encloses no area"),
        }
    }
}

impl std::error::Error for ProfileError {}

impl Body {
    /// A closed profile in plan, at height `elevation`, extruded `height`
    /// along Z (negative: downward): AutoCAD's EXTRUDE of a closed
    /// polyline. Each segment makes a flat side, each arc a piece of a
    /// cylinder. Its pieces must chain end to start, back to the first, and
    /// not cross; a height of 0 is [`GeometryError::ZeroLength`].
    pub fn extrusion(
        pieces: &[ProfilePiece],
        elevation: f64,
        height: f64,
    ) -> Result<Body, ExtrusionError> {
        let tol = Tolerance::DEFAULT;
        let profile = closed_profile(pieces, tol)?;
        if !(height.is_finite() && height != 0.0 && elevation.is_finite()) {
            return Err(ExtrusionError::Geometry(BrepError::Geometry(
                GeometryError::ZeroLength,
            )));
        }
        // Extruded upward from the lower of the two levels.
        let (base, top) = if height > 0.0 {
            (elevation, elevation + height)
        } else {
            (elevation + height, elevation)
        };
        let geometry = |e| ExtrusionError::Geometry(BrepError::Geometry(e));
        let at = |p: Point2, z: f64| Point3::new(p.x(), p.y(), z).map_err(geometry);
        let up = Vector3::new(0.0, 0.0, 1.0).map_err(geometry)?;
        let n = profile.len();
        let mut b = BodyBuilder::new();
        let mut points = Vec::with_capacity(2 * n);
        let (mut low, mut high) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for piece in &profile {
            let (a, t) = (at(piece.start(), base)?, at(piece.start(), top)?);
            points.extend([a, t]);
            low.push(b.vertex(a));
            high.push(b.vertex(t));
        }
        // The profile's edges at both levels, and the upright ones at its
        // corners, each running up.
        let along = |b: &mut BodyBuilder,
                     z: f64,
                     ring: &[VertexId]|
         -> Result<Vec<EdgeId>, ExtrusionError> {
            let mut edges = Vec::with_capacity(n);
            for (i, piece) in profile.iter().enumerate() {
                let (start, end) = (ring[i], ring[(i + 1) % n]);
                let (curve, reversed) = match piece.curve {
                    Geometry2::Arc(a) => {
                        let center = at(a.center(), z)?;
                        let major = at(a.start_point(), z)? - center;
                        let circle = EllipseCurve::new(center, up, major, 1.0).map_err(geometry)?;
                        (Curve::Ellipse(circle), piece.backward)
                    }
                    _ => {
                        let (p, q) = (at(piece.start(), z)?, at(piece.end(), z)?);
                        (
                            Curve::Line(LineCurve::new(p, q - p).map_err(geometry)?),
                            false,
                        )
                    }
                };
                edges.push(b.edge(Edge {
                    curve: Some(curve),
                    start,
                    end,
                    reversed,
                }));
            }
            Ok(edges)
        };
        let bottom = along(&mut b, base, &low)?;
        let roof = along(&mut b, top, &high)?;
        let mut uprights = Vec::with_capacity(n);
        for i in 0..n {
            let (p, q) = (points[2 * i], points[2 * i + 1]);
            let line = LineCurve::new(p, q - p).map_err(geometry)?;
            uprights.push(b.edge(Edge {
                curve: Some(Curve::Line(line)),
                start: low[i],
                end: high[i],
                reversed: false,
            }));
        }
        let ring = |b: &mut BodyBuilder, uses: &[(EdgeId, bool)]| {
            let coedges = uses
                .iter()
                .map(|&(edge, reversed)| b.coedge(Coedge { edge, reversed }))
                .collect();
            b.add_loop(Loop { coedges })
        };
        // Seen from outside: the base clockwise about Z, the top
        // counterclockwise; each side along the base, up, back along the
        // top and down.
        let under: Vec<_> = bottom.iter().rev().map(|&e| (e, true)).collect();
        let over: Vec<_> = roof.iter().map(|&e| (e, false)).collect();
        let mut faces = Vec::with_capacity(n + 2);
        let base_loop = ring(&mut b, &under);
        let floor = Plane::new(points[0], -up).map_err(geometry)?;
        faces.push(b.face(Face {
            surface: Surface::Plane(floor),
            loops: vec![base_loop],
            reversed: false,
        }));
        let top_loop = ring(&mut b, &over);
        let ceiling = Plane::new(points[1], up).map_err(geometry)?;
        faces.push(b.face(Face {
            surface: Surface::Plane(ceiling),
            loops: vec![top_loop],
            reversed: false,
        }));
        for (i, piece) in profile.iter().enumerate() {
            let next = (i + 1) % n;
            let side = ring(
                &mut b,
                &[
                    (bottom[i], false),
                    (uprights[next], false),
                    (roof[i], true),
                    (uprights[i], true),
                ],
            );
            let (surface, reversed) = match piece.curve {
                // Outside a convex arc lies away from its axis; outside a
                // concave one (run clockwise), toward it.
                Geometry2::Arc(a) => {
                    let cylinder = Cone::new(at(a.center(), base)?, up, a.radius(), 0.0, 1.0)
                        .map_err(geometry)?;
                    (Surface::Cone(cylinder), piece.backward)
                }
                _ => {
                    let d = piece.end() - piece.start();
                    let outward = Vector3::new(d.y(), -d.x(), 0.0).map_err(geometry)?;
                    let wall = Plane::new(points[2 * i], outward).map_err(geometry)?;
                    (Surface::Plane(wall), false)
                }
            };
            faces.push(b.face(Face {
                surface,
                loops: vec![side],
                reversed,
            }));
        }
        let shell = b.shell(Shell { faces });
        b.lump(Lump {
            shells: vec![shell],
        });
        b.build(tolerance(&points).map_err(ExtrusionError::Geometry)?)
            .map_err(ExtrusionError::Geometry)
    }
}

/// Why an extrusion cannot be made.
#[derive(Clone, Debug, PartialEq)]
pub enum ExtrusionError {
    /// The profile is not a closed, simple loop.
    Profile(ProfileError),
    /// A size or a value is invalid.
    Geometry(BrepError),
}

impl fmt::Display for ExtrusionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(e) => write!(f, "{e}"),
            Self::Geometry(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ExtrusionError {}

/// `pieces` as a closed, simple profile running counterclockwise.
fn closed_profile(
    pieces: &[ProfilePiece],
    tol: Tolerance,
) -> Result<Vec<ProfilePiece>, ExtrusionError> {
    let n = pieces.len();
    let open = || ExtrusionError::Profile(ProfileError::Open);
    if n < 2 {
        return Err(open());
    }
    for i in 0..n {
        if !pieces[i].end().approx_eq(pieces[(i + 1) % n].start(), tol) {
            return Err(open());
        }
    }
    // Neighbours meet only at their shared corners; others, nowhere.
    for i in 0..n {
        for j in i + 1..n {
            let meet = intersect(pieces[i].curve, pieces[j].curve, tol);
            if !meet.overlaps().is_empty() {
                return Err(ExtrusionError::Profile(ProfileError::Crossing));
            }
            let mut corners = Vec::new();
            if j == i + 1 {
                corners.push(pieces[i].end());
            }
            if (j + 1) % n == i {
                corners.push(pieces[j].end());
            }
            let stray = meet
                .points()
                .iter()
                .any(|p| !corners.iter().any(|c| c.approx_eq(*p, tol)));
            if stray {
                return Err(ExtrusionError::Profile(ProfileError::Crossing));
            }
        }
    }
    // A closed profile that does not cross itself encloses some area.
    let area: f64 = pieces.iter().map(ProfilePiece::swept).sum();
    // Clockwise: run it the other way round.
    Ok(if area > 0.0 {
        pieces.to_vec()
    } else {
        pieces.iter().rev().map(|p| p.reversed()).collect()
    })
}

/// A segment's or an arc's two ends, as stored.
fn ends(curve: Geometry2) -> (Point2, Point2) {
    match curve {
        Geometry2::Line(l) => (l.start(), l.end()),
        Geometry2::Arc(a) => (a.start_point(), a.end_point()),
        Geometry2::Circle(c) => {
            let p = c.point_at_angle(0.0);
            (p, p)
        }
    }
}

/// How close a vertex must be to its curves and surfaces: a billionth of
/// the farthest coordinate, at least of the body's size.
fn tolerance(points: &[Point3]) -> Result<Tolerance, BrepError> {
    let reach = points
        .iter()
        .flat_map(|p| [p.x().abs(), p.y().abs(), p.z().abs()])
        .fold(0.0, f64::max);
    let size = points
        .iter()
        .flat_map(|a| points.iter().map(move |b| (*a - *b).length()))
        .fold(0.0, f64::max);
    Tolerance::new(1e-9 * reach.max(size), Tolerance::DEFAULT.angular())
        .map_err(BrepError::Geometry)
}

/// A closed body of flat faces, each listing its corners counterclockwise
/// seen from outside; each pair of neighbours shares one straight edge.
fn planar(points: &[Point3], faces: &[&[usize]], tolerance: Tolerance) -> Result<Body, BrepError> {
    let mut b = BodyBuilder::new();
    let vertices: Vec<_> = points.iter().map(|&p| b.vertex(p)).collect();
    let mut edges = HashMap::new();
    let mut ids = Vec::with_capacity(faces.len());
    for &corners in faces {
        let mut coedges = Vec::with_capacity(corners.len());
        for k in 0..corners.len() {
            let (i, j) = (corners[k], corners[(k + 1) % corners.len()]);
            let key = (i.min(j), i.max(j));
            let edge = match edges.get(&key) {
                Some(&edge) => edge,
                None => {
                    let (from, to) = (points[key.0], points[key.1]);
                    let line = LineCurve::new(from, to - from).map_err(BrepError::Geometry)?;
                    let edge = b.edge(Edge {
                        curve: Some(Curve::Line(line)),
                        start: vertices[key.0],
                        end: vertices[key.1],
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
        let lp = b.add_loop(Loop { coedges });
        let (a, c, d) = (points[corners[0]], points[corners[1]], points[corners[2]]);
        let plane = Plane::new(a, (c - a).cross(d - a)).map_err(BrepError::Geometry)?;
        ids.push(b.face(Face {
            surface: Surface::Plane(plane),
            loops: vec![lp],
            reversed: false,
        }));
    }
    let shell = b.shell(Shell { faces: ids });
    b.lump(Lump {
        shells: vec![shell],
    });
    b.build(tolerance)
}

#[cfg(test)]
#[path = "primitives_tests.rs"]
mod tests;
