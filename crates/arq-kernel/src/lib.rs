//! ArqStudioCAD's own geometry kernel (ADR 0020), grown capability by
//! capability; first, the boundary representation of solids (ADR 0024 D1).
//!
//! # Map
//!
//! | Need | Use |
//! |---|---|
//! | A solid, sheet or region | [`Body`], built with [`BodyBuilder`] |
//! | Its parts | [`Vertex`], [`Edge`], [`Coedge`], [`Loop`], [`Face`], [`Shell`], [`Lump`] |
//! | An edge's curve | [`Curve`]: [`LineCurve`], [`EllipseCurve`], [`NurbsCurve`] |
//! | A face's surface | [`Surface`]: [`Plane`], [`Cone`], [`EllipticCylinder`], [`Sphere`], [`Torus`], [`NurbsSurface`] |
//! | Why parts are no body | [`BrepError`] |
//! | What to draw of a body | [`Body::wireframe`] → [`Wireframe`] |
//!
//! Like `arq-geometry`, this crate depends on no document, Python or GUI;
//! no engine or backend type appears in it (ADR 0020 D4).

mod brep;
mod curve;
mod nurbs;
mod surface;
mod tessellate;

pub use brep::{
    Body, BodyBuilder, BrepError, Coedge, CoedgeId, Edge, EdgeId, Face, FaceId, Loop, LoopId, Lump,
    Shell, ShellId, Vertex, VertexId,
};
pub use curve::{Curve, EllipseCurve, LineCurve};
pub use nurbs::{NurbsCurve, NurbsSurface};
pub use surface::{Cone, EllipticCylinder, Plane, Sphere, Surface, Torus};
pub use tessellate::Wireframe;
