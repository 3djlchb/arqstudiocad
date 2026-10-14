//! `CadError`: the core's error type (contract §4).

use std::fmt;

use arq_document::DocumentError;
use arq_geometry::GeometryError;

/// Why a core operation failed. Every failed operation leaves the document
/// unchanged (contract §6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CadError {
    /// A geometric value was invalid (NaN, zero radius, collinear points…).
    InvalidGeometry(String),
    /// No entity has this id.
    EntityNotFound(u64),
    /// No layer has this id.
    LayerNotFound(u64),
    /// No open document has this id.
    DocumentNotFound(u64),
    /// The operation is not allowed in the current state (locked layer,
    /// read-only entity, protected layer, an open group…).
    InvalidOperation(String),
    /// Reading or writing a file failed.
    Io(String),
    /// The file is damaged and no drawing could be recovered from it
    /// (ADR 0017 D5.1).
    DamagedFile(String),
    /// Not saved: the file would lose data its format or version cannot
    /// keep (ADR 0017 D5.3); the original is unchanged. Once the user agrees,
    /// [`CadApplication::accept_losses`](crate::CadApplication::accept_losses)
    /// with these kinds lets the save go on.
    Lossy {
        /// The English technical message.
        message: String,
        /// What would be lost, e.g. `"object TCH_WALL"`.
        kinds: Vec<String>,
        /// The same, with counts: `"object TCH_WALL: 26 → 0"`.
        losses: Vec<String>,
    },
    /// Not saved: the written file did not read back as the drawing; the
    /// original is unchanged and what was written is kept beside it
    /// (ADR 0017 D5.2).
    NotVerified(String),
    /// Saving would replace the damaged file a drawing was recovered from;
    /// the user must accept that first
    /// ([`CadApplication::allow_overwriting_damaged`](crate::CadApplication::allow_overwriting_damaged)).
    OverwritesDamaged(String),
    /// The file format is not supported.
    UnsupportedFormat(String),
}

impl fmt::Display for CadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeometry(m) => write!(f, "invalid geometry: {m}"),
            Self::EntityNotFound(id) => write!(f, "no entity with id {id}"),
            Self::LayerNotFound(id) => write!(f, "no layer with id {id}"),
            Self::DocumentNotFound(id) => write!(f, "no document with id {id}"),
            Self::InvalidOperation(m) => write!(f, "{m}"),
            Self::Io(m) | Self::DamagedFile(m) | Self::NotVerified(m) => write!(f, "{m}"),
            Self::Lossy { message, .. } => write!(f, "{message}"),
            Self::OverwritesDamaged(path) => write!(
                f,
                "{path}: the drawing was recovered from this damaged file; replacing it needs confirmation"
            ),
            Self::UnsupportedFormat(m) => write!(f, "unsupported format: {m}"),
        }
    }
}

impl std::error::Error for CadError {}

impl From<GeometryError> for CadError {
    fn from(e: GeometryError) -> Self {
        Self::InvalidGeometry(e.to_string())
    }
}

impl From<DocumentError> for CadError {
    fn from(e: DocumentError) -> Self {
        match e {
            DocumentError::Io { .. } | DocumentError::Codec(_) => Self::Io(e.to_string()),
            DocumentError::Damaged { .. } => Self::DamagedFile(e.to_string()),
            DocumentError::NotVerified { .. } => Self::NotVerified(e.to_string()),
            DocumentError::Lossy { ref losses, .. } => Self::Lossy {
                kinds: losses.iter().map(|l| l.kind().to_owned()).collect(),
                losses: losses.iter().map(ToString::to_string).collect(),
                message: e.to_string(),
            },
            DocumentError::UnsupportedFormat { path } => {
                Self::UnsupportedFormat(path.display().to_string())
            }
            DocumentError::EntityNotFound(id) => Self::EntityNotFound(id),
            DocumentError::LayerIdNotFound(id) => Self::LayerNotFound(id),
            DocumentError::Geometry(g) => Self::InvalidGeometry(g.to_string()),
            other => Self::InvalidOperation(other.to_string()),
        }
    }
}

/// Shorthand for core results.
pub type CadResult<T> = Result<T, CadError>;
