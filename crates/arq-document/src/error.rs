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
