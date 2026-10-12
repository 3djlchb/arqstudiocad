//! Recording changes for transactions and undo (ADR 0011, D3.5).
//!
//! A change starts with [`Document::begin_change`] and ends with
//! [`Document::commit_change`] in the **same call on the same thread**: the
//! codec's recorder is thread-local. Commit returns an opaque [`Change`]
//! holding before/after images of every touched entity (shared `Arc`s, no
//! geometry copy) and, when tables were touched, a structure snapshot.
//! [`Document::undo_change`] and [`Document::redo_change`] move the document
//! between both sides of a change.

use std::collections::BTreeSet;
use std::sync::Arc as Shared;

use opencadcodec::document::EntityChangeRecorder;
use opencadcodec::{CadDocument, EntityType, Handle};

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::layers::LayerId;

/// What a change did, for the interface to refresh only that (D3.7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeSet {
    created: Vec<EntityId>,
    modified: Vec<EntityId>,
    deleted: Vec<EntityId>,
    layers_created: Vec<LayerId>,
    layers_modified: Vec<LayerId>,
    layers_deleted: Vec<LayerId>,
    document: bool,
}

impl ChangeSet {
    /// Entities that now exist and did not before.
    pub fn created(&self) -> &[EntityId] {
        &self.created
    }
    /// Entities whose geometry or properties changed.
    pub fn modified(&self) -> &[EntityId] {
        &self.modified
    }
    /// Entities that no longer exist.
    pub fn deleted(&self) -> &[EntityId] {
        &self.deleted
    }
    /// Layers created.
    pub fn layers_created(&self) -> &[LayerId] {
        &self.layers_created
    }
    /// Layers renamed or with changed properties.
    pub fn layers_modified(&self) -> &[LayerId] {
        &self.layers_modified
    }
    /// Layers deleted.
    pub fn layers_deleted(&self) -> &[LayerId] {
        &self.layers_deleted
    }
    /// Whether a document-level setting changed (e.g. the current layer).
    pub fn document_changed(&self) -> bool {
        self.document
    }
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The change seen backwards, as undo produces it.
    pub fn inverted(&self) -> Self {
        Self {
            created: self.deleted.clone(),
            modified: self.modified.clone(),
            deleted: self.created.clone(),
            layers_created: self.layers_deleted.clone(),
            layers_modified: self.layers_modified.clone(),
            layers_deleted: self.layers_created.clone(),
            document: self.document,
        }
    }

    /// This change followed by `next`, as one: an entity created then
    /// modified is created; created then deleted is nothing; modified then
    /// deleted is deleted.
    pub fn then(&self, next: &Self) -> Self {
        fn merge(a: (&[u64], &[u64], &[u64]), b: (&[u64], &[u64], &[u64])) -> [Vec<u64>; 3] {
            let mut created: BTreeSet<u64> = a.0.iter().copied().collect();
            let mut modified: BTreeSet<u64> = a.1.iter().copied().collect();
            let mut deleted: BTreeSet<u64> = a.2.iter().copied().collect();
            for &id in b.0 {
                if !deleted.remove(&id) {
                    created.insert(id);
                } else {
                    modified.insert(id); // deleted then re-created: changed
                }
            }
            for &id in b.1 {
                if !created.contains(&id) {
                    modified.insert(id);
                }
            }
            for &id in b.2 {
                if !created.remove(&id) {
                    modified.remove(&id);
                    deleted.insert(id);
                }
            }
            [created, modified, deleted].map(|s| s.into_iter().collect())
        }
        let [created, modified, deleted] = merge(
            (&self.created, &self.modified, &self.deleted),
            (&next.created, &next.modified, &next.deleted),
        );
        let [layers_created, layers_modified, layers_deleted] = merge(
            (
                &self.layers_created,
                &self.layers_modified,
                &self.layers_deleted,
            ),
            (
                &next.layers_created,
                &next.layers_modified,
                &next.layers_deleted,
            ),
        );
        Self {
            created,
            modified,
            deleted,
            layers_created,
            layers_modified,
            layers_deleted,
            document: self.document || next.document,
        }
    }
}

struct Image {
    handle: Handle,
    before: Option<Shared<EntityType>>,
    after: Option<Shared<EntityType>>,
}

/// One recorded change: opaque, only undone or redone by the document it
/// came from.
pub struct Change {
    images: Vec<Image>,
    /// The structure (tables, header…) of the side not currently applied.
    structure: Option<Box<CadDocument>>,
    summary: ChangeSet,
}

impl Change {
    /// What this change did when applied.
    pub fn summary(&self) -> &ChangeSet {
        &self.summary
    }
}

/// State while a change is being recorded.
pub(crate) struct Recording {
    recorder: Shared<EntityChangeRecorder>,
    structure_before: Option<Box<CadDocument>>,
    all_entities_recorded: bool,
    layers_created: BTreeSet<LayerId>,
    layers_modified: BTreeSet<LayerId>,
    layers_deleted: BTreeSet<LayerId>,
    document: bool,
}

impl Document {
    /// Whether a change is being recorded.
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Starts recording a change.
    pub fn begin_change(&mut self) -> DocumentResult<()> {
        if self.recording.is_some() {
            return Err(DocumentError::ChangeInProgress);
        }
        let recorder = self.cad_mut().begin_entity_change_recording();
        self.recording = Some(Recording {
            recorder,
            structure_before: None,
            all_entities_recorded: false,
            layers_created: BTreeSet::new(),
            layers_modified: BTreeSet::new(),
            layers_deleted: BTreeSet::new(),
            document: false,
        });
        Ok(())
    }

    /// Ends recording and returns the change.
    pub fn commit_change(&mut self) -> DocumentResult<Change> {
        let rec = self
            .recording
            .take()
            .ok_or(DocumentError::NoChangeInProgress)?;
        self.cad_mut().end_entity_change_recording();
        let mut images = Vec::new();
        let mut summary = ChangeSet::default();
        for (handle, before) in rec.recorder.take_before_images() {
            let after = self.cad().get_entity_arc(handle);
            let id = handle.value();
            match (&before, &after) {
                (None, None) => continue, // created and deleted in the same change
                (None, Some(_)) => summary.created.push(id),
                (Some(_), None) => summary.deleted.push(id),
                (Some(b), Some(a)) if Shared::ptr_eq(b, a) => continue, // untouched
                (Some(_), Some(_)) => summary.modified.push(id),
            }
            images.push(Image {
                handle,
                before,
                after,
            });
        }
        let structure = rec.structure_before.map(|mut before| {
            let added: Vec<Handle> = summary.created.iter().map(|&id| Handle::new(id)).collect();
            // Undo must not roll back the handle counter: ids are never reused.
            CadDocument::align_added_entity_structure(&mut before, self.cad(), &added);
            before
        });
        summary.layers_created = rec.layers_created.into_iter().collect();
        summary.layers_modified = rec
            .layers_modified
            .into_iter()
            .filter(|id| !summary.layers_created.contains(id) && !rec.layers_deleted.contains(id))
            .collect();
        summary.layers_deleted = rec.layers_deleted.into_iter().collect();
        summary.document = rec.document;
        Ok(Change {
            images,
            structure,
            summary,
        })
    }

    /// Ends recording and puts everything back as it was before the change.
    pub fn rollback_change(&mut self) -> DocumentResult<()> {
        let mut change = self.commit_change()?;
        self.undo_change(&mut change)?;
        Ok(())
    }

    /// Reverts a committed change; returns what that did.
    pub fn undo_change(&mut self, change: &mut Change) -> DocumentResult<ChangeSet> {
        if self.recording.is_some() {
            return Err(DocumentError::ChangeInProgress);
        }
        for img in change.images.iter().rev() {
            match (&img.before, &img.after) {
                (Some(b), Some(_)) => {
                    self.cad_mut()
                        .replace_entity_arc(img.handle, Shared::clone(b));
                }
                (None, Some(_)) => {
                    self.cad_mut().remove_entity_arc(img.handle);
                }
                (Some(b), None) => {
                    self.cad_mut().restore_entity_arc(Shared::clone(b));
                }
                (None, None) => {}
            }
        }
        self.swap_structure(change);
        Ok(change.summary.inverted())
    }

    /// Applies again a change that was undone; returns what that did.
    pub fn redo_change(&mut self, change: &mut Change) -> DocumentResult<ChangeSet> {
        if self.recording.is_some() {
            return Err(DocumentError::ChangeInProgress);
        }
        self.swap_structure(change);
        for img in &change.images {
            match (&img.before, &img.after) {
                (_, Some(a)) if img.before.is_some() => {
                    self.cad_mut()
                        .replace_entity_arc(img.handle, Shared::clone(a));
                }
                (None, Some(a)) => {
                    self.cad_mut().restore_entity_arc(Shared::clone(a));
                }
                (Some(_), None) => {
                    self.cad_mut().remove_entity_arc(img.handle);
                }
                _ => {}
            }
        }
        Ok(change.summary.clone())
    }

    fn swap_structure(&mut self, change: &mut Change) {
        if let Some(other) = change.structure.take() {
            let displaced = self.cad_mut().swap_structure(*other);
            change.structure = Some(Box::new(displaced));
        }
    }

    /// Takes the structure snapshot before the first table change of the
    /// current recording.
    pub(crate) fn touch_structure(&mut self) {
        if self
            .recording
            .as_ref()
            .is_some_and(|r| r.structure_before.is_none())
        {
            let snapshot = self.cad_mut().snapshot_structure();
            if let Some(rec) = self.recording.as_mut() {
                rec.structure_before = Some(Box::new(snapshot));
            }
        }
    }

    /// Records every entity's image before an operation that may rewrite
    /// many entities internally (renaming a layer).
    pub(crate) fn record_all_entities(&mut self) {
        if self
            .recording
            .as_ref()
            .is_some_and(|r| !r.all_entities_recorded)
        {
            self.cad().record_all_entities_for_transaction();
            if let Some(rec) = self.recording.as_mut() {
                rec.all_entities_recorded = true;
            }
        }
    }

    pub(crate) fn note_layer(&mut self, id: LayerId, what: LayerNote) {
        if let Some(rec) = self.recording.as_mut() {
            match what {
                LayerNote::Created => rec.layers_created.insert(id),
                LayerNote::Modified => rec.layers_modified.insert(id),
                LayerNote::Deleted => rec.layers_deleted.insert(id),
            };
        }
    }

    pub(crate) fn note_document(&mut self) {
        if let Some(rec) = self.recording.as_mut() {
            rec.document = true;
        }
    }
}

/// How a layer was touched within a change.
#[derive(Clone, Copy)]
pub(crate) enum LayerNote {
    Created,
    Modified,
    Deleted,
}

#[cfg(test)]
#[path = "change_tests.rs"]
mod tests;
