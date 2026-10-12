//! Document model for ArqStudioCAD: entities, layers and blocks.
//!
//! The drawing is stored in opencadcodec's DWG database (ADR 0008, ADR 0011);
//! codec types never leave this crate. The model mirrors DWG concepts
//! (handles, symbol tables, model/paper space) so DWG/DXF load and save
//! without lossy translation.

mod change;
mod document;
mod entities;
mod entity;
mod error;
mod format;
mod layers;
mod ocs;
mod translate;

pub use change::{Change, ChangeSet};
pub use document::Document;
pub use entity::{Color, Entity, EntityId, PlanGeometry};
pub use error::{DocumentError, DocumentResult};
pub use format::FileFormat;
pub use layers::{Layer, LayerId};
