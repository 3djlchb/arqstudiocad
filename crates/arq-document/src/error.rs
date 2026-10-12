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
    /// No layer has this name.
    LayerNotFound(String),
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
            Self::LayerNotFound(name) => write!(f, "no layer named {name:?}"),
            Self::Geometry(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DocumentError {}

impl From<opencadcodec::DxfError> for DocumentError {
    fn from(e: opencadcodec::DxfError) -> Self {
        Self::Codec(e.to_string())
    }
}

/// Shorthand for document results.
pub type DocumentResult<T> = Result<T, DocumentError>;
