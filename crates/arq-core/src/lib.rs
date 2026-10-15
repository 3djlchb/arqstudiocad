//! Core facade of ArqStudioCAD: operations, transactions, undo/redo and queries.
//!
//! Must build and pass its tests without Python, Qt or a graphics system.
//! Decisions are in [ADR 0011].
//!
//! # Rules
//!
//! - Every operation is **one transaction**: one undo step, or, when it
//!   fails, no change at all. A group makes one step of a command's
//!   operations and is rolled back as a whole on failure.
//! - Every change returns a [`CadChangeSet`]: entities and layers created,
//!   modified and deleted, and whether a document setting changed.
//! - Ids are DWG handles: stable across save and open, never reused.
//! - LINE, CIRCLE and ARC drawn in plan are editable; every other entity is
//!   kept as read, can be erased, and makes an edit of it fail.
//! - Entities on a locked layer cannot be modified or erased.
//!
//! # Map
//!
//! | Need | Use |
//! |---|---|
//! | Open documents, one lock each | [`CadApplication`] |
//! | One drawing with its history | [`CadDocument`] |
//! | Draw, erase, move, rotate, scale, mirror, change properties | [`CadDocument`] operations |
//! | Layers | [`CadDocument::create_layer`] and the `set_layer_*` operations |
//! | Undo, redo, groups | [`CadDocument::undo`], [`CadDocument::begin_group`] |
//! | Extents and window queries | [`CadDocument::bounds`], [`CadDocument::query_entities`] |
//! | Reading the drawing | [`CadDocument::document`] → [`Document`], [`Entity`], [`Layer`] |
//! | Saving away from the lock | [`CadDocument::snapshot`] → [`Snapshot`] |
//! | Errors | [`CadError`] |
//!
//! # Example
//!
//! ```
//! use arq_core::{CadApplication, Color};
//! use arq_geometry::{Point2, Vector2};
//!
//! let p = |x, y| Point2::new(x, y);
//! let app = CadApplication::new();
//! let id = app.create_document("Plano1");
//!
//! app.edit(id, |cad| {
//!     let (walls, _) = cad.create_layer("A-MUROS")?;
//!     cad.set_current_layer(walls)?;
//!
//!     // A command: two walls, undone as one step.
//!     cad.begin_group("WALLS");
//!     let (a, _) = cad.add_line(p(0.0, 0.0)?, p(10.0, 0.0)?)?;
//!     let (b, _) = cad.add_line(p(10.0, 0.0)?, p(10.0, 8.0)?)?;
//!     let changes = cad.end_group()?;
//!     assert_eq!(changes.created(), [a, b]);
//!
//!     cad.move_entities(&[a, b], Vector2::new(5.0, 0.0)?)?;
//!     cad.set_entity_color(&[a], Color::Index(1))?;
//!     assert_eq!(cad.query_entities(arq_geometry::Bounds2::new(p(14.0, 7.0)?, p(16.0, 9.0)?)), [b]);
//!
//!     cad.undo()?; // the color
//!     cad.undo()?; // the move
//!     cad.undo()?; // both walls at once
//!     assert!(cad.document().entities().is_empty());
//!     Ok(())
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [ADR 0011]: https://github.com/3djlchb/arqstudiocad/blob/main/docs/architecture/adr/0011-document-model-and-transactions.md

mod app;
mod error;
mod history;
mod operations;
mod queries;

pub use app::{CadApplication, DocumentId};
pub use arq_document::ChangeSet as CadChangeSet;
pub use arq_document::{
    Color, Document, DrawItem, Entity, EntityId, FileFormat, Grip, GripKind, Layer, LayerId,
    PlanGeometry, Shape,
};
pub use arq_geometry::snap::{SnapKind, SnapPoint};
pub use error::{CadError, CadResult};
pub use history::{CadDocument, Snapshot};

/// Version of the CAD core, taken from the workspace version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_workspace() {
        assert_eq!(version(), "0.1.0");
    }
}
