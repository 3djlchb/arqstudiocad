//! Boundary representation of a solid (ADR 0024 D1): a body is lumps,
//! a lump is shells, a shell is faces; a face lies on a surface, trimmed
//! by loops of coedges, each the use of an edge in one direction; an edge
//! runs along a curve between two vertices (the same one for a closed
//! curve).
//!
//! The parts are kept in arenas and named by typed indices, so a body is
//! plain data: cheap to clone, to compare and to send between threads.
//! [`BodyBuilder::build`] checks the result, topology and geometry, before
//! a body exists.

use std::fmt;

use arq_geometry::{Bounds3, Point3, Tolerance, Transform3};

use crate::curve::Curve;
use crate::surface::Surface;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// Its position in the body's arena.
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id!(
    /// A vertex of a body.
    VertexId
);
id!(
    /// An edge of a body.
    EdgeId
);
id!(
    /// A coedge (an edge's use by a loop) of a body.
    CoedgeId
);
id!(
    /// A loop of a body.
    LoopId
);
id!(
    /// A face of a body.
    FaceId
);
id!(
    /// A shell of a body.
    ShellId
);

/// A point where edges meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Where it is.
    pub point: Point3,
}

/// A bounded piece of a curve, from `start` to `end`; `reversed` when it
/// runs against the curve's own direction. A degenerate edge (a cone's
/// apex) has no curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    /// The curve it runs along.
    pub curve: Option<Curve>,
    /// Where it starts.
    pub start: VertexId,
    /// Where it ends.
    pub end: VertexId,
    /// Whether it runs against the curve.
    pub reversed: bool,
}

/// An edge used by a loop, in the edge's direction or against it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coedge {
    /// The edge.
    pub edge: EdgeId,
    /// Whether the loop runs it from its end to its start.
    pub reversed: bool,
}

/// A closed chain of coedges bounding a face, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    /// The coedges, each starting where the previous one ends.
    pub coedges: Vec<CoedgeId>,
}

/// A piece of a surface, bounded by loops (none for a whole sphere or
/// torus); `reversed` when its outside is against the surface's normal.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// The surface it lies on.
    pub surface: Surface,
    /// Its loops.
    pub loops: Vec<LoopId>,
    /// Whether its outside faces against the surface's normal.
    pub reversed: bool,
}

/// Connected faces.
#[derive(Clone, Debug, PartialEq)]
pub struct Shell {
    /// Its faces.
    pub faces: Vec<FaceId>,
}

/// One connected piece of a body: its outer shell and any voids.
#[derive(Clone, Debug, PartialEq)]
pub struct Lump {
    /// Its shells.
    pub shells: Vec<ShellId>,
}

/// A solid, sheet or region as a boundary representation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
    coedges: Vec<Coedge>,
    loops: Vec<Loop>,
    faces: Vec<Face>,
    shells: Vec<Shell>,
    lumps: Vec<Lump>,
}

impl Body {
    /// Its lumps.
    pub fn lumps(&self) -> &[Lump] {
        &self.lumps
    }

    /// Its shells.
    pub fn shells(&self) -> &[Shell] {
        &self.shells
    }

    /// Its faces.
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// Its loops.
    pub fn loops(&self) -> &[Loop] {
        &self.loops
    }

    /// Its coedges.
    pub fn coedges(&self) -> &[Coedge] {
        &self.coedges
    }

    /// Its edges.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Its vertices.
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// The vertex `id`.
    pub fn vertex(&self, id: VertexId) -> &Vertex {
        &self.vertices[id.index()]
    }

    /// The edge `id`.
    pub fn edge(&self, id: EdgeId) -> &Edge {
        &self.edges[id.index()]
    }

    /// The box around its vertices; `None` without vertices.
    pub fn vertex_bounds(&self) -> Option<Bounds3> {
        Bounds3::from_points(self.vertices.iter().map(|v| v.point))
    }

    /// Where a loop's coedge starts and ends, in the loop's direction.
    fn coedge_ends(&self, coedge: &Coedge) -> (VertexId, VertexId) {
        let edge = &self.edges[coedge.edge.index()];
        if coedge.reversed {
            (edge.end, edge.start)
        } else {
            (edge.start, edge.end)
        }
    }

    /// Checks that the parts form a body: every index names a part, each
    /// loop closes, and each vertex lies, within `tol`, on the curves of
    /// its edges and the surfaces of its faces.
    pub fn check(&self, tol: Tolerance) -> Result<(), BrepError> {
        let limit = |what: &'static str, index: usize, len: usize| {
            if index < len {
                Ok(())
            } else {
                Err(BrepError::Dangling(what))
            }
        };
        for lump in &self.lumps {
            for s in &lump.shells {
                limit("shell", s.index(), self.shells.len())?;
            }
        }
        for shell in &self.shells {
            for f in &shell.faces {
                limit("face", f.index(), self.faces.len())?;
            }
        }
        for face in &self.faces {
            for l in &face.loops {
                limit("loop", l.index(), self.loops.len())?;
            }
        }
        for (index, lp) in self.loops.iter().enumerate() {
            if lp.coedges.is_empty() {
                return Err(BrepError::EmptyLoop(index));
            }
            for c in &lp.coedges {
                limit("coedge", c.index(), self.coedges.len())?;
            }
        }
        for c in &self.coedges {
            limit("edge", c.edge.index(), self.edges.len())?;
        }
        for e in &self.edges {
            limit("vertex", e.start.index(), self.vertices.len())?;
            limit("vertex", e.end.index(), self.vertices.len())?;
        }
        // Each loop closes: a coedge starts where the previous one ends.
        for (index, lp) in self.loops.iter().enumerate() {
            let ends: Vec<_> = lp
                .coedges
                .iter()
                .map(|c| self.coedge_ends(&self.coedges[c.index()]))
                .collect();
            for (i, &(start, _)) in ends.iter().enumerate() {
                let previous = ends[(i + ends.len() - 1) % ends.len()].1;
                if previous != start {
                    return Err(BrepError::OpenLoop(index));
                }
            }
        }
        // Each vertex on its edges' curves.
        for (index, edge) in self.edges.iter().enumerate() {
            if let Some(curve) = &edge.curve {
                for v in [edge.start, edge.end] {
                    let off = curve.distance(self.vertex(v).point);
                    if off.is_nan() || off > tol.linear() {
                        return Err(BrepError::VertexOffCurve { edge: index, off });
                    }
                }
            }
        }
        // Each vertex on its faces' surfaces.
        for (index, face) in self.faces.iter().enumerate() {
            for l in &face.loops {
                for c in &self.loops[l.index()].coedges {
                    let (start, _) = self.coedge_ends(&self.coedges[c.index()]);
                    let off = face.surface.distance(self.vertex(start).point);
                    if off.is_nan() || off > tol.linear() {
                        return Err(BrepError::VertexOffSurface { face: index, off });
                    }
                }
            }
        }
        Ok(())
    }

    /// The body moved by `t`, a rigid motion with uniform `scale`.
    pub fn transformed(&self, t: Transform3, scale: f64) -> Result<Self, BrepError> {
        let mut body = self.clone();
        for v in &mut body.vertices {
            v.point = t.apply_point(v.point);
        }
        for e in &mut body.edges {
            if let Some(curve) = &e.curve {
                e.curve = Some(curve.transformed(t).map_err(BrepError::Geometry)?);
            }
        }
        for f in &mut body.faces {
            f.surface = f
                .surface
                .transformed(t, scale)
                .map_err(BrepError::Geometry)?;
        }
        Ok(body)
    }
}

/// Builds a [`Body`] part by part, children before their owners.
#[derive(Clone, Debug, Default)]
pub struct BodyBuilder {
    body: Body,
}

/// The next id in an arena of `len` parts.
fn next_id(len: usize) -> u32 {
    u32::try_from(len).expect("a body holds fewer than 2^32 parts of a kind")
}

impl BodyBuilder {
    /// An empty body.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a vertex.
    pub fn vertex(&mut self, point: Point3) -> VertexId {
        let id = VertexId(next_id(self.body.vertices.len()));
        self.body.vertices.push(Vertex { point });
        id
    }

    /// Adds an edge.
    pub fn edge(&mut self, edge: Edge) -> EdgeId {
        let id = EdgeId(next_id(self.body.edges.len()));
        self.body.edges.push(edge);
        id
    }

    /// Adds a coedge.
    pub fn coedge(&mut self, coedge: Coedge) -> CoedgeId {
        let id = CoedgeId(next_id(self.body.coedges.len()));
        self.body.coedges.push(coedge);
        id
    }

    /// Adds a loop.
    pub fn add_loop(&mut self, lp: Loop) -> LoopId {
        let id = LoopId(next_id(self.body.loops.len()));
        self.body.loops.push(lp);
        id
    }

    /// Adds a face.
    pub fn face(&mut self, face: Face) -> FaceId {
        let id = FaceId(next_id(self.body.faces.len()));
        self.body.faces.push(face);
        id
    }

    /// Adds a shell.
    pub fn shell(&mut self, shell: Shell) -> ShellId {
        let id = ShellId(next_id(self.body.shells.len()));
        self.body.shells.push(shell);
        id
    }

    /// Adds a lump.
    pub fn lump(&mut self, lump: Lump) {
        self.body.lumps.push(lump);
    }

    /// The body, if it passes [`Body::check`] with `tol`.
    pub fn build(self, tol: Tolerance) -> Result<Body, BrepError> {
        if self.body.lumps.is_empty() {
            return Err(BrepError::Empty);
        }
        self.body.check(tol)?;
        Ok(self.body)
    }
}

/// Why parts do not form a body.
#[derive(Clone, Debug, PartialEq)]
pub enum BrepError {
    /// No lump at all.
    Empty,
    /// An index names no part of this kind.
    Dangling(&'static str),
    /// A loop without coedges.
    EmptyLoop(usize),
    /// A loop whose coedges do not join end to start.
    OpenLoop(usize),
    /// A vertex `off` from the curve of edge `edge`.
    VertexOffCurve {
        /// The edge's index.
        edge: usize,
        /// How far off.
        off: f64,
    },
    /// A vertex `off` from the surface of face `face`.
    VertexOffSurface {
        /// The face's index.
        face: usize,
        /// How far off.
        off: f64,
    },
    /// A geometric value is invalid.
    Geometry(arq_geometry::GeometryError),
}

impl fmt::Display for BrepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the body has no lump"),
            Self::Dangling(what) => write!(f, "a {what} index names nothing"),
            Self::EmptyLoop(i) => write!(f, "loop {i} has no coedge"),
            Self::OpenLoop(i) => write!(f, "loop {i} does not close"),
            Self::VertexOffCurve { edge, off } => {
                write!(f, "a vertex of edge {edge} is {off:e} off its curve")
            }
            Self::VertexOffSurface { face, off } => {
                write!(f, "a vertex of face {face} is {off:e} off its surface")
            }
            Self::Geometry(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BrepError {}

#[cfg(test)]
#[path = "brep_tests.rs"]
mod tests;
