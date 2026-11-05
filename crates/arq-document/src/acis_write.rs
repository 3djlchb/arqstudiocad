//! A body of the kernel written as ACIS (ADR 0024 D2, step 4): the records
//! a 3DSOLID keeps, for a solid made here. ACIS 7.0, as AcadRust's own
//! builders write it; the codec turns it into the binary form the drawing's
//! version expects when it saves (SAB from DWG 2004 on, ADR 0024 step 4a).
//!
//! Planes and cones (cylinders among them) bounded by straight and
//! circular edges, and a cone's apex, so far; spheres, tori and NURBS come
//! with the solids that use them. Every edge runs forward along a curve of
//! its own: a straight edge gets a line from its start to its end, a
//! circular one an ellipse turned the way it runs, its parameters the
//! angles of its ends.

use std::collections::HashMap;
use std::f64::consts::TAU;
use std::fmt;

use acadrust::EntityType;
use acadrust::entities::Solid3D;
use acadrust::entities::acis::types::{SatDocument, SatPointer, SatToken, Sense, Sidedness};
use arq_geometry::{Point3, Vector3};
use arq_kernel::{Body, BrepError, Curve, Surface};

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};

/// Why a body cannot be written as ACIS yet.
#[derive(Clone, Debug, PartialEq)]
pub enum Unwritable {
    /// It has no lump: nothing to write.
    Empty,
    /// It holds geometry not written yet, e.g. `"cone"`.
    Unsupported(&'static str),
}

impl fmt::Display for Unwritable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "an empty body"),
            Self::Unsupported(kind) => write!(f, "a {kind} is not written as ACIS yet"),
        }
    }
}

impl Document {
    /// Adds `body` as a 3DSOLID on the current layer, its ACIS written
    /// from it (ADR 0024 step 4); returns its id. A body not written as
    /// ACIS yet is [`DocumentError::UnsupportedOperation`].
    pub fn add_solid(&mut self, body: &Body) -> DocumentResult<EntityId> {
        let sat =
            sat_of(body).map_err(|why| DocumentError::UnsupportedOperation(why.to_string()))?;
        let mut solid = Solid3D::from_sat(&sat.to_sat_string());
        solid.common.layer = self.current_layer().name().to_owned();
        let h = self.cad_mut().add_entity(EntityType::Solid3D(solid))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Adds a box between two opposite corners, its sides along the axes,
    /// on the current layer (BOX, ADR 0024 step 4a); returns its id.
    pub fn add_box(&mut self, corner: Point3, opposite: Point3) -> DocumentResult<EntityId> {
        let body = Body::cuboid(corner, opposite).map_err(|e| match e {
            BrepError::Geometry(g) => DocumentError::Geometry(g),
            other => DocumentError::UnsupportedOperation(other.to_string()),
        })?;
        self.add_solid(&body)
    }

    /// Adds a cone standing on its base circle at `center`, of radius
    /// `base`, `height` along Z (negative: downward), its top of radius
    /// `top` (0: a point), on the current layer (CONE, and CYLINDER with
    /// equal radii; ADR 0024 step 4b); returns its id.
    pub fn add_cone(
        &mut self,
        center: Point3,
        base: f64,
        top: f64,
        height: f64,
    ) -> DocumentResult<EntityId> {
        let body = Body::cone(center, base, top, height).map_err(|e| match e {
            BrepError::Geometry(g) => DocumentError::Geometry(g),
            other => DocumentError::UnsupportedOperation(other.to_string()),
        })?;
        self.add_solid(&body)
    }

    /// Adds a wedge in the box between two opposite corners, upright at
    /// `first`'s X and sloping down toward `opposite`'s, on the current
    /// layer (WEDGE, ADR 0024 step 4b); returns its id.
    pub fn add_wedge(&mut self, first: Point3, opposite: Point3) -> DocumentResult<EntityId> {
        let body = Body::wedge(first, opposite).map_err(|e| match e {
            BrepError::Geometry(g) => DocumentError::Geometry(g),
            other => DocumentError::UnsupportedOperation(other.to_string()),
        })?;
        self.add_solid(&body)
    }
}

/// The parameter range of an edge that is a point, a cone's apex: empty,
/// from 1 to 0, as AcadRust's own cone writes it.
const APEX_RANGE: (f64, f64) = (1.0, 0.0);

/// Record indices laid out before writing, so every pointer is known when
/// its record is written: geometry, vertices and edges first, then
/// coedges, loops, faces, shells and lumps, each in the body's order.
struct Layout {
    coedges: i32,
    loops: i32,
    faces: i32,
    shells: i32,
    lumps: i32,
}

fn ptr(index: i32) -> SatPointer {
    SatPointer::new(index)
}

fn index(i: usize) -> i32 {
    i32::try_from(i).expect("an ACIS document has fewer than 2³¹ records")
}

fn xyz(p: Point3) -> [f64; 3] {
    [p.x(), p.y(), p.z()]
}

fn components(v: Vector3) -> [f64; 3] {
    [v.x(), v.y(), v.z()]
}

/// `body` as an ACIS document of one body.
pub(crate) fn sat_of(body: &Body) -> Result<SatDocument, Unwritable> {
    if body.lumps().is_empty() {
        return Err(Unwritable::Empty);
    }
    let mut sat = SatDocument::new_body();

    // Faces' surfaces.
    let mut surfaces = Vec::with_capacity(body.faces().len());
    for face in body.faces() {
        surfaces.push(match &face.surface {
            Surface::Plane(plane) => {
                let normal = components(plane.normal());
                sat.add_plane_surface(xyz(plane.origin()), normal, across(normal))
            }
            Surface::Cone(cone) => {
                let axis = components(cone.axis());
                let major = across(axis).map(|c| c * cone.radius());
                // AcadRust takes the cosine first and writes the sine first.
                sat.add_cone_surface(
                    xyz(cone.origin()),
                    axis,
                    major,
                    1.0,
                    cone.cos_half_angle(),
                    cone.sin_half_angle(),
                )
            }
            Surface::EllipticCylinder(_) => {
                return Err(Unwritable::Unsupported("elliptic cylinder"));
            }
            Surface::Sphere(_) => return Err(Unwritable::Unsupported("sphere")),
            Surface::Torus(_) => return Err(Unwritable::Unsupported("torus")),
            Surface::Nurbs(_) => return Err(Unwritable::Unsupported("NURBS surface")),
        });
    }

    // Vertices, each on a point of its own; their edge is linked at the end.
    let mut vertices = Vec::with_capacity(body.vertices().len());
    for vertex in body.vertices() {
        let [x, y, z] = xyz(vertex.point);
        let point = sat.add_point(x, y, z);
        vertices.push(sat.add_vertex(SatPointer::NULL, ptr(point)));
    }

    // Edges, each on a line of its own from its start to its end.
    let mut edges = Vec::with_capacity(body.edges().len());
    for edge in body.edges() {
        let start = body.vertex(edge.start).point;
        let end = body.vertex(edge.end).point;
        let (curve, from, to) = match &edge.curve {
            Some(Curve::Line(_)) => {
                let run = [
                    end.x() - start.x(),
                    end.y() - start.y(),
                    end.z() - start.z(),
                ];
                let length = (run[0] * run[0] + run[1] * run[1] + run[2] * run[2]).sqrt();
                let direction = run.map(|c| c / length);
                (
                    ptr(sat.add_straight_curve(xyz(start), direction)),
                    0.0,
                    length,
                )
            }
            Some(Curve::Ellipse(e)) => {
                // Turned over, an ellipse runs the other way round.
                let normal = if edge.reversed {
                    -e.normal()
                } else {
                    e.normal()
                };
                let from = angle_on(e.center(), normal, e.major(), e.ratio(), start);
                let mut to = angle_on(e.center(), normal, e.major(), e.ratio(), end);
                while to <= from {
                    to += TAU;
                }
                let curve = sat.add_ellipse_curve(
                    xyz(e.center()),
                    components(normal),
                    components(e.major()),
                    e.ratio(),
                );
                (ptr(curve), from, to)
            }
            Some(Curve::Nurbs(_)) => return Err(Unwritable::Unsupported("NURBS curve")),
            // A point the face closes on, a cone's apex: no curve.
            None => (SatPointer::NULL, APEX_RANGE.0, APEX_RANGE.1),
        };
        edges.push(sat.add_edge(
            ptr(vertices[edge.start.index()]),
            from,
            ptr(vertices[edge.end.index()]),
            to,
            SatPointer::NULL,
            curve,
            Sense::Forward,
        ));
    }

    let base = index(sat.records.len());
    let layout = Layout {
        coedges: base,
        loops: base + index(body.coedges().len()),
        faces: base + index(body.coedges().len() + body.loops().len()),
        shells: base + index(body.coedges().len() + body.loops().len() + body.faces().len()),
        lumps: base
            + index(
                body.coedges().len()
                    + body.loops().len()
                    + body.faces().len()
                    + body.shells().len(),
            ),
    };

    // Which loop each coedge is in, and the coedges round each edge.
    let mut owner = vec![None; body.coedges().len()];
    for (l, lp) in body.loops().iter().enumerate() {
        for c in &lp.coedges {
            owner[c.index()] = Some(l);
        }
    }
    let mut around: HashMap<usize, Vec<usize>> = HashMap::new();
    for (c, coedge) in body.coedges().iter().enumerate() {
        around.entry(coedge.edge.index()).or_default().push(c);
    }

    // Coedges, next and previous round their loop, partner round their edge.
    let mut links = vec![(SatPointer::NULL, SatPointer::NULL); body.coedges().len()];
    for lp in body.loops() {
        let n = lp.coedges.len();
        for (k, c) in lp.coedges.iter().enumerate() {
            let next = lp.coedges[(k + 1) % n].index();
            let prev = lp.coedges[(k + n - 1) % n].index();
            links[c.index()] = (
                ptr(layout.coedges + index(next)),
                ptr(layout.coedges + index(prev)),
            );
        }
    }
    for (c, coedge) in body.coedges().iter().enumerate() {
        let ring = &around[&coedge.edge.index()];
        let partner = if ring.len() > 1 {
            let k = ring.iter().position(|&o| o == c).expect("in its own ring");
            ptr(layout.coedges + index(ring[(k + 1) % ring.len()]))
        } else {
            SatPointer::NULL
        };
        let (next, prev) = links[c];
        let in_loop = owner[c].map_or(SatPointer::NULL, |l| ptr(layout.loops + index(l)));
        let sense = if coedge.reversed {
            Sense::Reversed
        } else {
            Sense::Forward
        };
        sat.add_coedge(
            next,
            prev,
            partner,
            ptr(edges[coedge.edge.index()]),
            sense,
            in_loop,
        );
    }

    // Loops, chained within their face.
    let mut face_of_loop = vec![0; body.loops().len()];
    let mut next_loop = vec![SatPointer::NULL; body.loops().len()];
    for (f, face) in body.faces().iter().enumerate() {
        for (k, l) in face.loops.iter().enumerate() {
            face_of_loop[l.index()] = f;
            if let Some(next) = face.loops.get(k + 1) {
                next_loop[l.index()] = ptr(layout.loops + index(next.index()));
            }
        }
    }
    for (l, lp) in body.loops().iter().enumerate() {
        let first = lp
            .coedges
            .first()
            .map_or(SatPointer::NULL, |c| ptr(layout.coedges + index(c.index())));
        sat.add_loop(
            next_loop[l],
            first,
            ptr(layout.faces + index(face_of_loop[l])),
        );
    }

    // Faces, chained within their shell.
    let mut shell_of_face = vec![0; body.faces().len()];
    let mut next_face = vec![SatPointer::NULL; body.faces().len()];
    for (s, shell) in body.shells().iter().enumerate() {
        for (k, f) in shell.faces.iter().enumerate() {
            shell_of_face[f.index()] = s;
            if let Some(next) = shell.faces.get(k + 1) {
                next_face[f.index()] = ptr(layout.faces + index(next.index()));
            }
        }
    }
    for (f, face) in body.faces().iter().enumerate() {
        let first = face
            .loops
            .first()
            .map_or(SatPointer::NULL, |l| ptr(layout.loops + index(l.index())));
        let sense = if face.reversed {
            Sense::Reversed
        } else {
            Sense::Forward
        };
        sat.add_face(
            next_face[f],
            first,
            ptr(layout.shells + index(shell_of_face[f])),
            ptr(surfaces[f]),
            sense,
            Sidedness::Single,
        );
    }

    // Shells, chained within their lump; lumps, within the body.
    let mut lump_of_shell = vec![0; body.shells().len()];
    let mut next_shell = vec![SatPointer::NULL; body.shells().len()];
    for (u, lump) in body.lumps().iter().enumerate() {
        for (k, s) in lump.shells.iter().enumerate() {
            lump_of_shell[s.index()] = u;
            if let Some(next) = lump.shells.get(k + 1) {
                next_shell[s.index()] = ptr(layout.shells + index(next.index()));
            }
        }
    }
    for (s, shell) in body.shells().iter().enumerate() {
        let first = shell
            .faces
            .first()
            .map_or(SatPointer::NULL, |f| ptr(layout.faces + index(f.index())));
        let at = sat.add_shell(first, ptr(layout.lumps + index(lump_of_shell[s])));
        set_pointer(&mut sat, at, 1, next_shell[s]);
    }
    for (u, lump) in body.lumps().iter().enumerate() {
        let first = lump
            .shells
            .first()
            .map_or(SatPointer::NULL, |s| ptr(layout.shells + index(s.index())));
        let at = sat.add_lump(first, ptr(0));
        if u + 1 < body.lumps().len() {
            set_pointer(&mut sat, at, 1, ptr(layout.lumps + index(u + 1)));
        }
    }
    // The body's first lump.
    set_pointer(&mut sat, 0, 1, ptr(layout.lumps));
    sat.complete_brep_links();
    Ok(sat)
}

/// Where `p` lies on the ellipse round `center` with unit `normal`,
/// semi-major axis `major` and `ratio`: its angle from the major axis,
/// toward `normal × major`.
fn angle_on(center: Point3, normal: Vector3, major: Vector3, ratio: f64, p: Point3) -> f64 {
    let d = p - center;
    let minor = normal.cross(major) * ratio;
    (d.dot(minor) / minor.length_squared()).atan2(d.dot(major) / major.length_squared())
}

/// Sets pointer `token` of record `at`.
fn set_pointer(sat: &mut SatDocument, at: i32, token: usize, to: SatPointer) {
    if let Some(record) = usize::try_from(at).ok().and_then(|i| sat.record_mut(i)) {
        record.tokens[token] = SatToken::Pointer(to);
    }
}

/// A unit direction square to `normal`: a plane's u direction.
fn across(normal: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = normal;
    // Crossed with the axis it is least along, then made unit.
    let v = if x.abs() <= y.abs() && x.abs() <= z.abs() {
        [0.0, z, -y]
    } else if y.abs() <= z.abs() {
        [-z, 0.0, x]
    } else {
        [y, -x, 0.0]
    };
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|c| c / length)
}

#[cfg(test)]
#[path = "acis_write_tests.rs"]
mod tests;
