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
//! | Data a save would lose (stops it unless accepted) | [`Document::losses_in`] → [`Loss`], [`Document::save_accepting`] |
//! | Damaged files: recovered or refused | [`Document::recovery`] → [`Recovery`], [`DocumentError::Damaged`] |
//! | Read entities | [`Document::entity`], [`Document::entities`] → [`Entity`] |
//! | Pick, window/crossing selection, object snaps | [`Document::pick`], [`Document::select_window`], [`Document::snap`] |
//! | Plan geometry of LINE, CIRCLE, ARC | [`PlanGeometry`] |
//! | Layers | [`Document::layers`] → [`Layer`], [`Document::create_layer`]… |
//! | Record, undo and redo a change | [`Document::begin_change`] → [`Change`], [`ChangeSet`] |
//! | Check the own model against the engine | [`Document::check_model`] |
//! | Errors | [`DocumentError`] |

mod acis;
mod block_copy;
mod block_editor;
mod blocks;
mod change;
mod document;
mod drawing;
mod entities;
mod entity;
mod error;
mod format;
mod grips;
mod inventory;
mod layers;
mod model;
mod ocs;
mod pick;
mod reading;
mod solid;
mod table_style_verify;
#[cfg(test)]
mod test_dxf;
mod text_layout;
mod translate;
mod verify;

pub use block_copy::ClashChoice;
pub use blocks::{BlockDefinition, BlockInfo, BlockObjects};
pub use change::{Change, ChangeSet};
pub use document::Document;
pub use drawing::{DrawItem, Shape};
pub use entity::{Color, Entity, EntityId, PlanGeometry, TextData};
pub use error::{DocumentError, DocumentResult};
pub use format::FileFormat;
pub use grips::{Grip, GripKind};
pub use inventory::Loss;
pub use layers::{Layer, LayerId};
pub use reading::Recovery;
pub use solid::Untranslated;
