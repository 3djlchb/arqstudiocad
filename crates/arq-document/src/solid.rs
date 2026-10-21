//! An ACIS body (3DSOLID, SURFACE, REGION, BODY) read into the kernel's
//! own boundary representation (ADR 0024 D2, step 1 of D4): planes,
//! circular cones and cylinders, spheres and tori, bounded by straight and
//! elliptic edges. A body with anything else (spline surfaces and curves
//! first) is not translated yet; its ACIS data stays kept as it is, so
//! nothing is lost either way.

use std::collections::HashMap;
use std::fmt;

use acadrust::entities::acis::types::{
    SatBody, SatCoedge, SatConeSurface, SatDocument, SatEdge, SatEllipseCurve, SatFace, SatLoop,
    SatLump, SatPlaneSurface, SatPoint, SatPointer, SatRecord, SatShell, SatSphereSurface,
    SatStraightCurve, SatTorusSurface, SatTransform, SatVertex, Sense,
};
use arq_geometry::{Point3, Tolerance, Transform3, Vector3};
use arq_kernel::{
    Body, BodyBuilder, Coedge, Cone, Curve, Edge, EdgeId, EllipseCurve, EllipticCylinder, Face,
    LineCurve, Loop, Lump, Plane, Shell, Sphere, Surface, Torus, VertexId,
};

use crate::document::Document;
use crate::entity::EntityId;
use crate::translate::handle;

/// Why an ACIS body is not in the kernel's representation yet.
#[derive(Clone, Debug, PartialEq)]
pub enum Untranslated {
    /// Its ACIS data does not parse.
    Unreadable,
    /// It holds a kind of record not translated yet, e.g. `spline-surface`.
    Unsupported(String),
    /// What it holds does not make a body: a misread or a damaged body.
    Invalid(String),
}

impl fmt::Display for Untranslated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable => write!(f, "its ACIS data does not parse"),
            Self::Unsupported(kind) => write!(f, "{kind} is not translated yet"),
            Self::Invalid(why) => write!(f, "not a valid body: {why}"),
        }
    }
}

impl Document {
    /// Entity `id` as a body of the kernel, if it is an ACIS body (a
    /// 3DSOLID, SURFACE, REGION or BODY); `None` for anything else.
    pub fn solid(&self, id: EntityId) -> Option<Result<Body, Untranslated>> {
        let e = self.cad().get_entity(handle(id))?;
        let data = crate::acis::acis_data(e)?;
        let doc = if data.is_binary {
            acadrust::SabReader::read(&data.sab_data).ok()
        } else {
            SatDocument::parse(&data.sat_data).ok()
        };
        Some(
            doc.ok_or(Untranslated::Unreadable)
                .and_then(|d| translate(&d)),
        )
    }
}

/// Every body of an ACIS document, as one body of the kernel.
fn translate(doc: &SatDocument) -> Result<Body, Untranslated> {
    let mut reader = Reader {
        doc,
        builder: BodyBuilder::new(),
        vertices: HashMap::new(),
        edges: HashMap::new(),
        points: Vec::new(),
    };
    let mut placements = Vec::new();
    for body in doc.bodies() {
        if !body.wire_body().is_null() {
            return Err(Untranslated::Unsupported("wire".into()));
        }
        placements.push(placement(doc, &body)?);
        let mut lump = body.lump();
        while let Some(record) = reader.resolve(lump) {
            let l = SatLump::from_record(record).ok_or_else(|| invalid("a lump"))?;
            let mut shells = Vec::new();
            let mut shell = l.shell();
            while let Some(record) = reader.resolve(shell) {
                let s = SatShell::from_record(record).ok_or_else(|| invalid("a shell"))?;
                shells.push(reader.shell(&s, record)?);
                shell = s.next_shell();
            }
            reader.builder.lump(Lump { shells });
            lump = l.next_lump();
        }
    }
    // One placement for the whole document: AutoCAD writes one body.
    let placement = match placements.as_slice() {
        [] => None,
        [first, rest @ ..] if rest.iter().all(|p| p == first) => *first,
        _ => return Err(Untranslated::Unsupported("bodies placed apart".into())),
    };
    let tolerance = tolerance(&reader.points);
    let body = reader
        .builder
        .build(tolerance)
        .map_err(|e| Untranslated::Invalid(e.to_string()))?;
    match placement {
        None => Ok(body),
        Some((t, scale)) => body
            .transformed(t, scale)
            .map_err(|e| Untranslated::Invalid(e.to_string())),
    }
}

/// How close a vertex must be to its curves and surfaces: ACIS's 1e-6
/// model units, relative for large bodies and far from the origin.
fn tolerance(points: &[Point3]) -> Tolerance {
    let reach = points
        .iter()
        .flat_map(|p| [p.x().abs(), p.y().abs(), p.z().abs()])
        .fold(1.0_f64, f64::max);
    Tolerance::new(1e-6 * reach, Tolerance::DEFAULT.angular()).expect("finite and positive")
}

/// A body's placement: its transform, rows applied to row vectors as ACIS
/// does, and uniform scale; `None` without one.
fn placement(doc: &SatDocument, body: &SatBody) -> Result<Option<(Transform3, f64)>, Untranslated> {
    let Some(record) = doc.resolve(body.transform()) else {
        return Ok(None);
    };
    let t = SatTransform::from_record(record).ok_or_else(|| invalid("a transform"))?;
    let scale = t.scale();
    let rows = [t.row0(), t.row1(), t.row2()];
    let column = |r: (f64, f64, f64), i: usize| [r.0, r.1, r.2][i] * scale;
    // p' = p · R · s + t, so the matrix acting on column vectors is Rᵀ · s.
    let linear = [
        [column(rows[0], 0), column(rows[1], 0), column(rows[2], 0)],
        [column(rows[0], 1), column(rows[1], 1), column(rows[2], 1)],
        [column(rows[0], 2), column(rows[1], 2), column(rows[2], 2)],
    ];
    let (x, y, z) = t.translation();
    let offset = Vector3::new(x, y, z).map_err(|_| invalid("a transform"))?;
    let transform =
        Transform3::new(linear, Point3::ORIGIN, offset).map_err(|_| invalid("a transform"))?;
    Ok(Some((transform, scale)))
}

/// The walk through one ACIS document, record by record.
struct Reader<'a> {
    doc: &'a SatDocument,
    builder: BodyBuilder,
    vertices: HashMap<i32, VertexId>,
    edges: HashMap<i32, EdgeId>,
    points: Vec<Point3>,
}

impl<'a> Reader<'a> {
    fn resolve(&self, ptr: SatPointer) -> Option<&'a SatRecord> {
        self.doc.resolve(ptr)
    }

    fn shell(
        &mut self,
        shell: &SatShell,
        record: &SatRecord,
    ) -> Result<arq_kernel::ShellId, Untranslated> {
        if !shell.subshell().is_null() {
            return Err(Untranslated::Unsupported("subshell".into()));
        }
        // ACIS 1.x shells have no wire pointer: the lump follows the face.
        let has_wire = record.tokens.len() > 5;
        if has_wire && !shell.wire().is_null() {
            return Err(Untranslated::Unsupported("wire".into()));
        }
        let mut faces = Vec::new();
        let mut face = shell.face();
        while let Some(record) = self.resolve(face) {
            let f = SatFace::from_record(record).ok_or_else(|| invalid("a face"))?;
            faces.push(self.face(&f)?);
            face = f.next_face();
        }
        Ok(self.builder.shell(Shell { faces }))
    }

    fn face(&mut self, face: &SatFace) -> Result<arq_kernel::FaceId, Untranslated> {
        let record = self
            .resolve(face.surface())
            .ok_or_else(|| invalid("a face's surface"))?;
        let (surface, inward) = surface(record)?;
        let mut loops = Vec::new();
        let mut lp = face.first_loop();
        while let Some(record) = self.resolve(lp) {
            let l = SatLoop::from_record(record).ok_or_else(|| invalid("a loop"))?;
            loops.push(self.add_loop(&l)?);
            lp = l.next_loop();
        }
        let reversed = matches!(face.sense(), Sense::Reversed) != inward;
        Ok(self.builder.face(Face {
            surface,
            loops,
            reversed,
        }))
    }

    fn add_loop(&mut self, lp: &SatLoop) -> Result<arq_kernel::LoopId, Untranslated> {
        let first = lp.first_coedge();
        let mut coedges = Vec::new();
        let mut at = first;
        while let Some(record) = self.resolve(at) {
            let c = SatCoedge::from_record(record).ok_or_else(|| invalid("a coedge"))?;
            let edge = self.edge(c.edge())?;
            coedges.push(self.builder.coedge(Coedge {
                edge,
                reversed: matches!(c.sense(), Sense::Reversed),
            }));
            at = c.next();
            if at == first {
                break;
            }
            if coedges.len() > self.doc.records.len() {
                return Err(invalid("a loop that never closes"));
            }
        }
        Ok(self.builder.add_loop(Loop { coedges }))
    }

    fn edge(&mut self, ptr: SatPointer) -> Result<EdgeId, Untranslated> {
        if let Some(&id) = self.edges.get(&ptr.0) {
            return Ok(id);
        }
        let record = self
            .resolve(ptr)
            .ok_or_else(|| invalid("a coedge's edge"))?;
        let e = SatEdge::from_record(record).ok_or_else(|| invalid("an edge"))?;
        let start = self.vertex(e.start_vertex())?;
        let end = self.vertex(e.end_vertex())?;
        let curve = match self.resolve(e.curve()) {
            Some(record) => Some(curve(record)?),
            None => None,
        };
        let id = self.builder.edge(Edge {
            curve,
            start,
            end,
            reversed: matches!(e.sense(), Sense::Reversed),
        });
        self.edges.insert(ptr.0, id);
        Ok(id)
    }

    fn vertex(&mut self, ptr: SatPointer) -> Result<VertexId, Untranslated> {
        if let Some(&id) = self.vertices.get(&ptr.0) {
            return Ok(id);
        }
        let record = self
            .resolve(ptr)
            .ok_or_else(|| invalid("an edge's vertex"))?;
        let v = SatVertex::from_record(record).ok_or_else(|| invalid("a vertex"))?;
        let point = self
            .resolve(v.point())
            .and_then(SatPoint::from_record)
            .ok_or_else(|| invalid("a vertex's point"))?;
        let point = point3(point.position())?;
        self.points.push(point);
        let id = self.builder.vertex(point);
        self.vertices.insert(ptr.0, id);
        Ok(id)
    }
}

/// A face's surface, and whether ACIS's own normal points inward (a
/// sphere or torus of negative radius).
fn surface(record: &SatRecord) -> Result<(Surface, bool), Untranslated> {
    let bad = |_| invalid(&record.entity_type);
    if let Some(s) = SatPlaneSurface::from_record(record) {
        let plane = Plane::new(point3(s.root_point())?, vector3(s.normal())?).map_err(bad)?;
        return Ok((Surface::Plane(plane), false));
    }
    if let Some(s) = SatConeSurface::from_record(record) {
        let center = point3(s.center())?;
        let axis = vector3(s.axis())?;
        let major = vector3(s.major_axis())?;
        // A negative cosine turns the normal toward the axis.
        let inward = s.cos_half_angle() < 0.0;
        let sign = if inward { -1.0 } else { 1.0 };
        let (sin, cos) = (s.sin_half_angle() * sign, s.cos_half_angle() * sign);
        if (s.ratio() - 1.0).abs() > 1e-9 {
            if sin != 0.0 {
                return Err(Untranslated::Unsupported("elliptic cone-surface".into()));
            }
            let section = EllipseCurve::new(center, axis, major, s.ratio()).map_err(bad)?;
            return Ok((
                Surface::EllipticCylinder(EllipticCylinder::new(section)),
                inward,
            ));
        }
        let cone = Cone::new(center, axis, major.length(), sin, cos).map_err(bad)?;
        return Ok((Surface::Cone(cone), inward));
    }
    if let Some(s) = SatSphereSurface::from_record(record) {
        let radius = s.radius();
        let sphere = Sphere::new(point3(s.center())?, radius.abs()).map_err(bad)?;
        return Ok((Surface::Sphere(sphere), radius < 0.0));
    }
    if let Some(s) = SatTorusSurface::from_record(record) {
        let minor = s.minor_radius();
        let torus = Torus::new(
            point3(s.center())?,
            vector3(s.normal())?,
            s.major_radius(),
            minor.abs(),
        )
        .map_err(bad)?;
        return Ok((Surface::Torus(torus), minor < 0.0));
    }
    Err(Untranslated::Unsupported(record.entity_type.clone()))
}

/// An edge's curve.
fn curve(record: &SatRecord) -> Result<Curve, Untranslated> {
    let bad = |_| invalid(&record.entity_type);
    if let Some(c) = SatStraightCurve::from_record(record) {
        return Ok(Curve::Line(
            LineCurve::new(point3(c.root_point())?, vector3(c.direction())?).map_err(bad)?,
        ));
    }
    if let Some(c) = SatEllipseCurve::from_record(record) {
        return Ok(Curve::Ellipse(
            EllipseCurve::new(
                point3(c.center())?,
                vector3(c.normal())?,
                vector3(c.major_axis())?,
                c.ratio(),
            )
            .map_err(bad)?,
        ));
    }
    Err(Untranslated::Unsupported(record.entity_type.clone()))
}

fn point3((x, y, z): (f64, f64, f64)) -> Result<Point3, Untranslated> {
    Point3::new(x, y, z).map_err(|_| invalid("a point"))
}

fn vector3((x, y, z): (f64, f64, f64)) -> Result<Vector3, Untranslated> {
    Vector3::new(x, y, z).map_err(|_| invalid("a vector"))
}

fn invalid(what: &str) -> Untranslated {
    Untranslated::Invalid(format!("{what} does not read"))
}

#[cfg(test)]
#[path = "solid_tests.rs"]
mod tests;
