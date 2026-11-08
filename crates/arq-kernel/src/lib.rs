//! ArqStudioCAD's own geometry kernel (ADR 0020), grown capability by
//! capability; first, the boundary representation of solids (ADR 0024 D1).
//!
//! # Map
//!
//! | Need | Use |
//! |---|---|
//! | A solid, sheet or region | [`Body`], built with [`BodyBuilder`] |
//! | A box or a wedge between two corners | [`Body::cuboid`], [`Body::wedge`] |
//! | A cylinder or a cone on its base | [`Body::cylinder`], [`Body::cone`] |
//! | A sphere or a torus round its center | [`Body::sphere`], [`Body::torus`] |
//! | A closed profile extruded | [`Body::extrusion`] of [`ProfilePiece`]s |
//! | A closed profile turned about an axis | [`Body::revolution`] in a [`RevolutionFrame`] |
//! | Its parts | [`Vertex`], [`Edge`], [`Coedge`], [`Loop`], [`Face`], [`Shell`], [`Lump`] |
//! | An edge's curve | [`Curve`]: [`LineCurve`], [`EllipseCurve`], [`NurbsCurve`] |
//! | A face's surface | [`Surface`]: [`Plane`], [`Cone`], [`EllipticCylinder`], [`Sphere`], [`Torus`], [`NurbsSurface`] |
//! | Why parts are no body | [`BrepError`] |
//! | What to draw of a body | [`Body::wireframe`] → [`Wireframe`] |
//! | Union, intersection, difference of planar solids | [`Body::union`], [`Body::intersection`], [`Body::difference`] → [`BooleanError`] |
//! | Its volume, area, centroid and bounds | [`Body::mass_properties`] → [`MassProperties`] |
//!
//! Like `arq-geometry`, this crate depends on no document, Python or GUI;
//! no engine or backend type appears in it (ADR 0020 D4).

mod boolean;
mod boolean_curved;
mod brep;
mod chart;
mod curve;
mod measure;
mod nurbs;
mod primitives;
mod surface;
mod tessellate;

pub use boolean::BooleanError;
pub use brep::{
    Body, BodyBuilder, BrepError, Coedge, CoedgeId, Edge, EdgeId, Face, FaceId, Loop, LoopId, Lump,
    Shell, ShellId, Vertex, VertexId,
};
pub use curve::{Curve, EllipseCurve, LineCurve};
pub use measure::{MassProperties, MeasureError};
pub use nurbs::{NurbsCurve, NurbsSurface};
pub use primitives::{
    ExtrusionError, ProfileError, ProfilePiece, RevolutionError, RevolutionFrame,
};
pub use surface::{Cone, EllipticCylinder, Plane, Sphere, Surface, Torus};
pub use tessellate::Wireframe;
