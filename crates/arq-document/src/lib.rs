//! Document model for ArqStudioCAD: entities, layers and blocks.
//!
//! The drawing has an own internal model (ADR 0018 D2, stage 1: layers, the
//! current layer, LINE, CIRCLE, ARC) written through to AcadRust's DWG
//! database (ADR 0011, ADR 0017), which keeps everything the model does not
//! own; codec types never leave this crate. Both mirror DWG concepts
//! (handles, symbol tables, model/paper space) so DWG/DXF load and save
//! without lossy translation.
//!
//! Applications use it through `arq-core`, which adds transactions, undo and
//! the rules for editing; this crate is the drawing and its change
//! recording.
//!
//! | Need | Use |
//! |---|---|
//! | Open, save, snapshot a drawing | [`Document`], [`FileFormat`] |
//! | What survives a save (checked by every save) | [`Document::validate_roundtrip`] |
//! | Damaged files: recovered or refused | [`Document::recovery`] → [`Recovery`], [`DocumentError::Damaged`] |
//! | Read entities | [`Document::entity`], [`Document::entities`] → [`Entity`] |
//! | Plan geometry of LINE, CIRCLE, ARC | [`PlanGeometry`] |
//! | Layers | [`Document::layers`] → [`Layer`], [`Document::create_layer`]… |
//! | Record, undo and redo a change | [`Document::begin_change`] → [`Change`], [`ChangeSet`] |
//! | Check the own model against the engine | [`Document::check_model`] |
//! | Errors | [`DocumentError`] |

mod change;
mod document;
mod drawing;
mod entities;
mod entity;
mod error;
mod format;
mod layers;
mod model;
mod ocs;
mod reading;
#[cfg(test)]
mod test_dxf;
mod translate;
mod verify;

pub use change::{Change, ChangeSet};
pub use document::Document;
pub use drawing::{DrawItem, Shape};
pub use entity::{Color, Entity, EntityId, PlanGeometry};
pub use error::{DocumentError, DocumentResult};
pub use format::FileFormat;
pub use layers::{Layer, LayerId};
pub use reading::Recovery;
