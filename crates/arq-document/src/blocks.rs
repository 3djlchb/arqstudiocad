//! Making and placing blocks (ADR 0023 D2): BLOCK turns entities into a new
//! block definition and puts a reference to it where they were; INSERT
//! places a reference to a block, its attributes taking their default
//! values.

use acadrust::entities::{AttributeEntity, Insert};
use acadrust::tables::{BlockRecord, TableEntry};
use acadrust::types::{Transform, Vector3};
use acadrust::{EntityType, Handle};
use arq_geometry::Point2;

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::translate::handle;

/// Characters a DWG block name cannot hold.
const FORBIDDEN: &[char] = &[
    '<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', ',', '=', '`',
];

impl Document {
    /// The blocks INSERT can place, sorted ignoring case: not anonymous
    /// (`*U…`, `*D…`), not a layout, not an external reference.
    pub fn block_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .cad()
            .block_records
            .iter()
            .filter(|b| !b.is_anonymous() && !b.is_layout() && b.xref_path.is_empty())
            .filter(|b| !b.is_model_space() && !b.is_paper_space())
            .map(|b| b.name.clone())
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    /// Makes block `name` from `ids` with its base point at `base`, and
    /// replaces them with a reference to it at `base` on the current layer,
    /// as AutoCAD's BLOCK does by default; returns the reference's id. The
    /// entities keep their coordinates, layers and properties inside the
    /// block.
    pub fn create_block(
        &mut self,
        name: &str,
        base: Point2,
        ids: &[EntityId],
    ) -> DocumentResult<EntityId> {
        // A name in use is refused by the block table itself ("… exists").
        check_block_name(name)?;
        // In drawing order, each once.
        let chosen: Vec<EntityId> = self
            .entities()
            .iter()
            .map(|e| e.id())
            .filter(|id| ids.contains(id))
            .collect();
        if chosen.is_empty() || ids.iter().any(|id| !chosen.contains(id)) {
            let missing = ids.iter().find(|id| !chosen.contains(id));
            return Err(match missing {
                Some(&id) => DocumentError::EntityNotFound(id),
                None => DocumentError::InvalidBlockName("a block needs objects".to_owned()),
            });
        }
        self.touch_structure();
        let mut record = BlockRecord::new(name);
        let h = self.cad_mut().allocate_handle();
        record.set_handle(h);
        record.block_entity_handle = self.cad_mut().allocate_handle();
        record.block_end_handle = self.cad_mut().allocate_handle();
        record.base_point = Vector3::new(base.x(), base.y(), 0.0);
        self.cad_mut()
            .block_records
            .add(record)
            .map_err(DocumentError::InvalidBlockName)?;
        for id in chosen {
            self.clone_entity(id, h)?;
            self.delete_entity(id)?;
        }
        self.insert_block(name, base, 1.0, 0.0)
    }

    /// Places block `name` at `point`, scaled by `scale` and turned
    /// `rotation` radians, on the current layer; its attributes take their
    /// definitions' default values. Returns the reference's id.
    pub fn insert_block(
        &mut self,
        name: &str,
        point: Point2,
        scale: f64,
        rotation: f64,
    ) -> DocumentResult<EntityId> {
        if !(scale.is_finite() && scale > 0.0) {
            return Err(DocumentError::InvalidBlockName(
                "a block's scale must be a finite number greater than zero".to_owned(),
            ));
        }
        if !rotation.is_finite() {
            return Err(DocumentError::InvalidBlockName(
                "a block's rotation must be a finite number".to_owned(),
            ));
        }
        let record = self
            .cad()
            .block_records
            .get(name)
            .filter(|_| {
                self.block_names()
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(name))
            })
            .ok_or_else(|| DocumentError::InvalidBlockName(format!("no block named {name:?}")))?;
        let block = record.name.clone();
        let base = record.base_point;
        let at = Vector3::new(point.x(), point.y(), 0.0);
        let mut insert = Insert::new(block.clone(), at);
        insert.set_x_scale(scale);
        insert.set_y_scale(scale);
        insert.set_z_scale(scale);
        insert.rotation = rotation;
        insert.common.layer = self.current_layer().name().to_owned();
        // Attributes from the definition, placed as the reference places
        // the block's contents.
        let placed = Transform::from_translation(Vector3::new(-base.x, -base.y, -base.z))
            .then(&Transform::from_scaling(Vector3::new(scale, scale, scale)))
            .then(&Transform::from_rotation(
                Vector3::new(0.0, 0.0, 1.0),
                rotation,
            ))
            .then(&Transform::from_translation(at));
        let definitions: Vec<_> = self
            .cad()
            .entities_in_block(&block)
            .filter_map(|e| match e {
                EntityType::AttributeDefinition(d) if !d.flags.constant => Some(d.clone()),
                _ => None,
            })
            .collect();
        for definition in &definitions {
            let mut attribute =
                EntityType::AttributeEntity(AttributeEntity::from_definition(definition, None));
            attribute.apply_transform(&placed);
            if let EntityType::AttributeEntity(mut attribute) = attribute {
                attribute.common.handle = self.cad_mut().allocate_handle();
                insert.attributes.push(attribute);
            }
        }
        let h = self.cad_mut().add_entity(EntityType::Insert(insert))?;
        if let Some(EntityType::Insert(insert)) = self.cad_mut().get_entity_mut(h) {
            for attribute in &mut insert.attributes {
                attribute.common.owner_handle = h;
            }
        }
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Adds a copy of entity `id` to the block record `owner` (model space
    /// when it is null), without the links a copy cannot share (reactors,
    /// an extension dictionary, cached proxy graphics); a block reference's
    /// attributes get handles of their own. Returns the copy's id.
    pub(crate) fn clone_entity(&mut self, id: EntityId, owner: Handle) -> DocumentResult<EntityId> {
        let mut copy = self
            .cad()
            .get_entity(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?
            .clone();
        let common = copy.common_mut();
        common.handle = Handle::NULL;
        common.owner_handle = owner;
        common.raw_record = None;
        common.reactors.clear();
        common.xdictionary_handle = None;
        common.graphic_data = None;
        if let EntityType::Insert(insert) = &mut copy {
            for attribute in &mut insert.attributes {
                let h = self.cad_mut().allocate_handle();
                let common = &mut attribute.common;
                common.handle = h;
                common.raw_record = None;
                common.reactors.clear();
                common.xdictionary_handle = None;
            }
            insert.seqend_handle = None;
        }
        let h = self.cad_mut().add_entity(copy)?;
        if let Some(EntityType::Insert(insert)) = self.cad_mut().get_entity_mut(h) {
            for attribute in &mut insert.attributes {
                attribute.common.owner_handle = h;
            }
        }
        self.sync_entity(h.value());
        Ok(h.value())
    }
}

/// A block name AutoCAD accepts: not empty, at most 255 characters, no
/// surrounding spaces, none of the characters DWG forbids, not anonymous.
fn check_block_name(name: &str) -> DocumentResult<()> {
    let bad = name.is_empty()
        || name.chars().count() > 255
        || name.trim() != name
        || name.contains(FORBIDDEN)
        || name.starts_with('*');
    if bad {
        return Err(DocumentError::InvalidBlockName(format!(
            "{name:?} is not a valid block name"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "blocks_tests.rs"]
mod tests;
