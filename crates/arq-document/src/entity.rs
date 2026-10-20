//! `Entity`: our read-only view of one drawing entity.

use arq_geometry::{Bounds2, Geometry2, Point2};

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
            return Err(DocumentError::InvalidColorIndex(i));
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

/// A TEXT or MTEXT drawn in plan, as the editor sees it (ADR 0022).
#[derive(Clone, Debug, PartialEq)]
pub struct TextData {
    pub(crate) content: String,
    pub(crate) anchor: Point2,
    pub(crate) height: f64,
    pub(crate) rotation: f64,
    pub(crate) style: String,
    pub(crate) justification: &'static str,
    pub(crate) width_factor: f64,
    pub(crate) oblique: f64,
    pub(crate) width: f64,
    pub(crate) multiline: bool,
    pub(crate) sizes_editable: bool,
    pub(crate) content_editable: bool,
}

impl TextData {
    /// What it says: a TEXT's line; an MTEXT's paragraphs as lines, or its
    /// content as stored when it holds formatting codes.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// The point it hangs from in world coordinates (ADR 0022 D3).
    pub fn anchor(&self) -> Point2 {
        self.anchor
    }

    /// Character height.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Reading direction, radians counterclockwise from world East.
    pub fn rotation(&self) -> f64 {
        self.rotation
    }

    /// Text style name.
    pub fn style(&self) -> &str {
        &self.style
    }

    /// Justification, as AutoCAD names it: `"Left"`, `"Center"`, `"Right"`,
    /// `"Aligned"`, `"Middle"`, `"Fit"`, `"TopLeft"`… (MTEXT: its
    /// attachment point).
    pub fn justification(&self) -> &'static str {
        self.justification
    }

    /// Width factor (1 for MTEXT).
    pub fn width_factor(&self) -> f64 {
        self.width_factor
    }

    /// Oblique angle, radians (0 for MTEXT).
    pub fn oblique(&self) -> f64 {
        self.oblique
    }

    /// An MTEXT's width, the one its paragraphs wrap to (0: they do not
    /// wrap); 0 for a TEXT.
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Whether it is an MTEXT.
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// Whether its height and rotation can be set (not for Aligned and Fit
    /// texts, whose two points decide them).
    pub fn sizes_editable(&self) -> bool {
        self.sizes_editable
    }

    /// Whether its content can be set (not for an MTEXT with formatting
    /// codes, ADR 0022 D4).
    pub fn content_editable(&self) -> bool {
        self.content_editable
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
    pub(crate) linetype: String,
    pub(crate) lineweight: String,
    pub(crate) details: Vec<(String, String)>,
    pub(crate) bounds: Option<Bounds2>,
    pub(crate) plan: Option<PlanGeometry>,
    pub(crate) text: Option<TextData>,
    pub(crate) transformable: bool,
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

    /// Entity linetype, with an empty codec value represented as `ByLayer`.
    pub fn linetype(&self) -> &str {
        &self.linetype
    }

    /// Human-readable entity lineweight (millimeters or a special value).
    pub fn lineweight(&self) -> &str {
        &self.lineweight
    }

    /// Read-only, type-specific details such as block or polyline data.
    pub fn details(&self) -> &[(String, String)] {
        &self.details
    }

    /// Plan-view bounding box, when the entity has a known extent.
    pub fn bounds(&self) -> Option<Bounds2> {
        self.bounds
    }

    /// Editable plan geometry; `None` for read-only entities.
    pub fn plan(&self) -> Option<PlanGeometry> {
        self.plan
    }

    /// A TEXT or MTEXT drawn in plan; `None` for anything else.
    pub fn text(&self) -> Option<&TextData> {
        self.text.as_ref()
    }

    /// Whether its geometry can be edited in this version.
    pub fn is_editable(&self) -> bool {
        self.plan.is_some()
    }

    /// Whether it can be moved, rotated, scaled, mirrored and copied as a
    /// whole: editable entities, and blocks (INSERT) and lightweight
    /// polylines drawn in plan (ADR 0016, D1).
    pub fn is_transformable(&self) -> bool {
        self.transformable
    }
}
