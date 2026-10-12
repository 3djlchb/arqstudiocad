//! `Entity`: our read-only view of one drawing entity.

use arq_geometry::{Bounds2, Geometry2};

use crate::error::{DocumentError, DocumentResult};

/// Identity of an entity: its DWG handle (ADR 0011, D3.6).
pub type EntityId = u64;

/// Entity or layer color, as DWG stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    /// Takes the color of the entity's layer.
    ByLayer,
    /// Takes the color of the block it is inserted in.
    ByBlock,
    /// AutoCAD Color Index, 1 to 255.
    Index(u8),
    /// True color.
    Rgb {
        /// Red.
        r: u8,
        /// Green.
        g: u8,
        /// Blue.
        b: u8,
    },
    /// No color assigned (rare; kept for round trip).
    None,
}

impl Color {
    /// An ACI color; the index must be 1 to 255.
    pub fn index(i: u8) -> DocumentResult<Self> {
        if i == 0 {
            return Err(DocumentError::Codec(
                "color index 0 is BYBLOCK; use Color::ByBlock".into(),
            ));
        }
        Ok(Self::Index(i))
    }
}

/// The editable part of an entity drawn in plan: its 2D geometry in world
/// coordinates, plus its elevation (Z), which edits preserve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanGeometry {
    geometry: Geometry2,
    elevation: f64,
}

impl PlanGeometry {
    /// Geometry at elevation 0.
    pub fn new(geometry: Geometry2) -> Self {
        Self {
            geometry,
            elevation: 0.0,
        }
    }

    /// Geometry at the given elevation (Z, model units).
    pub fn at_elevation(geometry: Geometry2, elevation: f64) -> DocumentResult<Self> {
        if !elevation.is_finite() {
            return Err(arq_geometry::GeometryError::NonFinite { what: "elevation" }.into());
        }
        Ok(Self {
            geometry,
            elevation,
        })
    }

    /// The 2D geometry in world coordinates.
    pub fn geometry(self) -> Geometry2 {
        self.geometry
    }

    /// Elevation (Z).
    pub fn elevation(self) -> f64 {
        self.elevation
    }

    /// Same elevation, new geometry.
    pub fn with_geometry(self, geometry: Geometry2) -> Self {
        Self { geometry, ..self }
    }
}

/// A snapshot of one entity, as the core and Python see it.
///
/// Every entity type is listed (ADR 0011, D3.4); only LINE, CIRCLE and ARC
/// drawn in plan carry editable [`PlanGeometry`].
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub(crate) id: EntityId,
    pub(crate) kind: &'static str,
    pub(crate) layer: String,
    pub(crate) color: Color,
    pub(crate) bounds: Option<Bounds2>,
    pub(crate) plan: Option<PlanGeometry>,
}

impl Entity {
    /// Its id (DWG handle).
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// DXF type name: `"LINE"`, `"CIRCLE"`, `"TEXT"`, `"HATCH"`…
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// Name of its layer.
    pub fn layer(&self) -> &str {
        &self.layer
    }

    /// Its own color (often [`Color::ByLayer`]).
    pub fn color(&self) -> Color {
        self.color
    }

    /// Plan-view bounding box, when the entity has a known extent.
    pub fn bounds(&self) -> Option<Bounds2> {
        self.bounds
    }

    /// Editable plan geometry; `None` for read-only entities.
    pub fn plan(&self) -> Option<PlanGeometry> {
        self.plan
    }

    /// Whether its geometry can be edited in this version.
    pub fn is_editable(&self) -> bool {
        self.plan.is_some()
    }
}
