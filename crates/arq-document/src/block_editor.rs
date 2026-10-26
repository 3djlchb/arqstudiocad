//! The block editor (ADR 0023 D3): a block's definition, alone, as the
//! model space of a drawing of its own, where every tool works; saving puts
//! that model space back as the definition's contents.

use std::collections::BTreeSet;

use acadrust::EntityType;
use acadrust::Handle;
use acadrust::tables::TableEntry;

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};

impl Document {
    /// Whether block `name` is dynamic: its parameters wait for the
    /// parametric engine, so it cannot be edited yet (ADR 0023 D3a).
    pub fn is_dynamic_block(&self, name: &str) -> bool {
        self.cad()
            .block_records
            .get(name)
            .is_some_and(|r| self.cad().is_dynamic_block(r.handle()))
    }

    /// A drawing holding only block `name`'s contents in its model space,
    /// in the block's own coordinates, with this drawing's layers, styles
    /// and blocks: what the block editor edits. Only blocks INSERT can place
    /// are edited, and not dynamic ones.
    pub fn block_editor(&self, name: &str) -> DocumentResult<Document> {
        let block = self.editable_block(name)?;
        let contents: Vec<EntityType> = self.cad().entities_in_block(&block).cloned().collect();
        let mut editor = Document::from_engine(self.cad().clone());
        let model: Vec<EntityId> = editor
            .cad()
            .model_space_entities()
            .map(|e| e.as_entity().handle().value())
            .collect();
        for id in model {
            editor.delete_entity(id)?;
        }
        for entity in contents {
            editor.add_copy(entity, Handle::NULL)?;
        }
        Ok(editor)
    }

    /// Makes what `editor`'s model space holds the contents of block
    /// `name`, as the block editor saves it; every reference to the block
    /// is redrawn. Layers and blocks made in the editor are added to this
    /// drawing; changes to layers it already has are not carried. A block
    /// that would hold itself is refused.
    pub fn replace_block_contents(&mut self, name: &str, editor: &Document) -> DocumentResult<()> {
        let block = self.editable_block(name)?;
        let contents: Vec<EntityType> = editor.cad().model_space_entities().cloned().collect();
        let nested = editor.blocks_used(&contents);
        if nested.iter().any(|n| n.eq_ignore_ascii_case(&block)) {
            return Err(DocumentError::InvalidBlockName(format!(
                "block {block} cannot hold itself"
            )));
        }
        self.touch_structure();
        self.add_missing_layers(editor)?;
        self.add_missing_blocks(editor, &nested)?;
        let owner = self.clear_block(&block)?;
        for entity in contents {
            self.add_copy(entity, owner)?;
        }
        self.redraw_references(&block);
        Ok(())
    }

    /// Deletes what block `block` holds, leaving it empty; returns its
    /// record's handle. The caller has touched the structure.
    pub(crate) fn clear_block(&mut self, block: &str) -> DocumentResult<Handle> {
        let record =
            self.cad().block_records.get(block).ok_or_else(|| {
                DocumentError::InvalidBlockName(format!("no block named {block:?}"))
            })?;
        let owner = record.handle();
        let old: Vec<EntityId> = record.entity_handles.iter().map(|h| h.value()).collect();
        for id in old {
            self.delete_entity(id)?;
        }
        // Deleting leaves the handles in the record's list.
        let cad = self.cad_mut();
        let gone: BTreeSet<Handle> = cad
            .block_records
            .get(block)
            .map(|r| r.entity_handles.iter().copied())
            .into_iter()
            .flatten()
            .filter(|h| cad.get_entity(*h).is_none())
            .collect();
        if let Some(record) = cad.block_records.get_mut(block) {
            record.entity_handles.retain(|h| !gone.contains(h));
        }
        Ok(owner)
    }

    /// The name of block `name` as the drawing spells it, if INSERT could
    /// place it and it is not dynamic.
    pub(crate) fn editable_block(&self, name: &str) -> DocumentResult<String> {
        let block = self
            .block_names()
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| DocumentError::InvalidBlockName(format!("no block named {name:?}")))?;
        if self.is_dynamic_block(&block) {
            return Err(DocumentError::InvalidBlockName(format!(
                "{block} is a dynamic block"
            )));
        }
        Ok(block)
    }

    /// Every block `entities` reference, directly or through other blocks
    /// of this drawing, by name.
    pub(crate) fn blocks_used(&self, entities: &[EntityType]) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut pending: Vec<String> = entities.iter().filter_map(block_of).collect();
        while let Some(name) = pending.pop() {
            if found.insert(name.clone()) {
                pending.extend(self.cad().entities_in_block(&name).filter_map(block_of));
            }
        }
        found
    }

    /// Adds the layers of `editor` this drawing lacks, with their colors
    /// and states.
    fn add_missing_layers(&mut self, editor: &Document) -> DocumentResult<()> {
        for layer in editor.layers() {
            if self.layer_by_name(layer.name()).is_some() {
                continue;
            }
            let id = self.create_layer(layer.name())?;
            self.set_layer_color(id, layer.color())?;
            self.set_layer_visible(id, layer.is_visible())?;
            self.set_layer_frozen(id, layer.is_frozen())?;
            self.set_layer_locked(id, layer.is_locked())?;
            self.set_layer_plottable(id, layer.is_plottable())?;
        }
        Ok(())
    }

    /// Copies the blocks among `names` this drawing lacks from `editor`,
    /// with their base points and contents.
    fn add_missing_blocks(
        &mut self,
        editor: &Document,
        names: &BTreeSet<String>,
    ) -> DocumentResult<()> {
        for name in names {
            if self.cad().block_records.get(name).is_some() {
                continue;
            }
            let Some(source) = editor.cad().block_records.get(name) else {
                continue;
            };
            let base = source.base_point;
            let owner = self.add_block_record(name, base)?;
            let contents: Vec<EntityType> = editor.cad().entities_in_block(name).cloned().collect();
            for entity in contents {
                self.add_copy(entity, owner)?;
            }
        }
        Ok(())
    }

    /// Marks every model-space reference that draws block `name`, directly
    /// or nested, as changed, so it is drawn and indexed again.
    pub(crate) fn redraw_references(&mut self, name: &str) {
        let references: Vec<Handle> = self
            .cad()
            .model_space_entities()
            .filter(|e| {
                block_of(e).is_some_and(|b| {
                    b.eq_ignore_ascii_case(name)
                        || self
                            .blocks_used(std::slice::from_ref(e))
                            .iter()
                            .any(|n| n.eq_ignore_ascii_case(name))
                })
            })
            .map(|e| e.as_entity().handle())
            .collect();
        for h in references {
            // Taking it mutably records it as changed.
            let _ = self.cad_mut().get_entity_mut(h);
        }
    }
}

/// The block a reference draws.
pub(crate) fn block_of(e: &EntityType) -> Option<String> {
    match e {
        EntityType::Insert(i) => Some(i.block_name.clone()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "block_editor_tests.rs"]
mod tests;
