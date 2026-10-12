//! Document model for ArqStudioCAD: entities, layers and blocks.
//!
//! The drawing is stored in opencadcodec's DWG database (ADR 0008, ADR 0011);
//! codec types never leave this crate. The model mirrors DWG concepts
//! (handles, symbol tables, model/paper space) so DWG/DXF load and save
//! without lossy translation.

mod document;
mod error;
mod format;

pub use document::Document;
pub use error::{DocumentError, DocumentResult};
pub use format::FileFormat;
