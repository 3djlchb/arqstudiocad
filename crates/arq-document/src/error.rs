//! Errors of the document layer. Codec errors are converted here and never
//! leave this crate (ADR 0008).

use std::fmt;
use std::path::PathBuf;

/// Why a document operation failed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentError {
    /// Reading or writing a file failed at the operating-system level.
    Io {
        /// The file involved.
        path: PathBuf,
        /// What the operating system reported.
        message: String,
    },
    /// The file extension is not one we read or write (`.dwg`, `.dxf`).
    UnsupportedFormat {
        /// The file involved.
        path: PathBuf,
    },
    /// The DWG/DXF engine could not read or write the drawing.
    Codec(String),
    /// Not saved: the file written did not read back as the drawing
    /// (ADR 0017 D5.2). The target is untouched; what was written is kept
    /// beside it, if that was possible.
    NotVerified {
        /// The file that was to be saved.
        path: PathBuf,
        /// Where what was written is kept (`<name>.unverified.<ext>`).
        kept: Option<PathBuf>,
        /// How it differed, at least one line.
        differences: Vec<String>,
    },
    /// Not saved: the file would hold less than the drawing, data its format
    /// or version cannot keep (ADR 0017 D5.3). The target is untouched.
    Lossy {
        /// The file that was to be saved.
        path: PathBuf,
        /// What would be lost, at least one.
        losses: Vec<crate::inventory::Loss>,
    },
    /// The file is damaged and no drawing data could be recovered from it
    /// (ADR 0017 D5.1).
    Damaged {
        /// The file involved.
        path: PathBuf,
        /// What the reader found wrong, at least one line.
        problems: Vec<String>,
    },
    /// No entity has this id.
    EntityNotFound(u64),
    /// The entity exists but its geometry cannot be edited yet: an
    /// unsupported type, or drawn out of plan (ADR 0011, D3.3–D3.4).
    NotEditable {
        /// The entity.
        id: u64,
        /// Its DXF type name, e.g. `"TEXT"`.
        kind: &'static str,
    },
    /// New geometry of a different kind than the entity (a LINE cannot
    /// become a CIRCLE: that is a delete plus a create).
    KindMismatch {
        /// The entity.
        id: u64,
        /// Its DXF type name.
        kind: &'static str,
    },
    /// A vertex index is outside a lightweight polyline's vertices.
    InvalidVertexIndex {
        /// The polyline entity.
        id: u64,
        /// The requested zero-based vertex.
        index: usize,
    },
    /// No layer has this name.
    LayerNotFound(String),
    /// No layer has this id.
    LayerIdNotFound(u64),
    /// A layer with this name already exists (names ignore case).
    LayerExists(String),
    /// Not a valid layer name (empty, too long, surrounding spaces, or one
    /// of `< > / \ " : ; ? * | , = `` and control characters).
    InvalidName(String),
    /// Layers "0" and "Defpoints" and external-reference layers cannot be
    /// renamed or deleted; the current layer cannot be deleted.
    ProtectedLayer(String),
    /// The layer still has entities, so it cannot be deleted.
    LayerInUse(String),
    /// A layer's own color must be an ACI index or RGB, not ByLayer/ByBlock.
    InvalidLayerColor,
    /// An ACI color index must be 1 to 255 (0 means ByBlock).
    InvalidColorIndex(u8),
    /// A frozen layer cannot be the current layer.
    FrozenLayer(String),
    /// A change is already being recorded (or undo/redo was asked during one).
    ChangeInProgress,
    /// Commit or rollback without a change being recorded.
    NoChangeInProgress,
    /// A geometric value is invalid.
    Geometry(arq_geometry::GeometryError),
}

impl From<arq_geometry::GeometryError> for DocumentError {
    fn from(e: arq_geometry::GeometryError) -> Self {
        Self::Geometry(e)
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::UnsupportedFormat { path } => {
                write!(
                    f,
                    "{}: only .dwg and .dxf files are supported",
                    path.display()
                )
            }
            Self::Codec(message) => write!(f, "drawing could not be processed: {message}"),
            Self::NotVerified {
                path,
                kept,
                differences,
            } => {
                write!(
                    f,
                    "{}: not saved, the written file does not read back as the drawing ({});                      the original is unchanged",
                    path.display(),
                    differences.join("; ")
                )?;
                match kept {
                    Some(kept) => write!(f, "; what was written is kept as {}", kept.display()),
                    None => Ok(()),
                }
            }
            Self::Lossy { path, losses } => write!(
                f,
                "{}: not saved, the file would lose data ({}); the original is unchanged",
                path.display(),
                crate::inventory::describe(losses)
            ),
            Self::Damaged { path, problems } => write!(
                f,
                "{}: the file is damaged and no drawing could be recovered from it: {}",
                path.display(),
                problems.join("; ")
            ),
            Self::EntityNotFound(id) => write!(f, "no entity with id {id}"),
            Self::NotEditable { id, kind } => {
                write!(f, "entity {id} ({kind}) cannot be edited yet")
            }
            Self::KindMismatch { id, kind } => {
                write!(
                    f,
                    "entity {id} is a {kind}; its geometry cannot change kind"
                )
            }
            Self::InvalidVertexIndex { id, index } => {
                write!(f, "polyline {id} has no vertex {index}")
            }
            Self::LayerNotFound(name) => write!(f, "no layer named {name:?}"),
            Self::LayerIdNotFound(id) => write!(f, "no layer with id {id}"),
            Self::LayerExists(name) => write!(f, "a layer named {name:?} already exists"),
            Self::InvalidName(name) => write!(f, "{name:?} is not a valid layer name"),
            Self::ProtectedLayer(name) => write!(f, "layer {name:?} cannot be changed that way"),
            Self::LayerInUse(name) => write!(f, "layer {name:?} still has entities"),
            Self::InvalidLayerColor => {
                write!(f, "a layer color must be a color index or RGB")
            }
            Self::InvalidColorIndex(i) => {
                write!(f, "color index {i} is not 1 to 255 (0 means ByBlock)")
            }
            Self::FrozenLayer(name) => write!(f, "layer {name:?} is frozen"),
            Self::ChangeInProgress => write!(f, "a change is already in progress"),
            Self::NoChangeInProgress => write!(f, "no change is in progress"),
            Self::Geometry(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DocumentError {}

impl From<acadrust::DxfError> for DocumentError {
    fn from(e: acadrust::DxfError) -> Self {
        Self::Codec(e.to_string())
    }
}

/// Shorthand for document results.
pub type DocumentResult<T> = Result<T, DocumentError>;
