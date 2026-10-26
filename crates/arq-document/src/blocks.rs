//! Making and placing blocks (ADR 0023 D2): BLOCK turns entities into a new
//! block definition and puts a reference to it where they were; INSERT
//! places a reference to a block, its attributes taking their default
//! values. The Blocks palette reads what it shows of each block here
//! (ADR 0025).

use std::collections::HashMap;

use acadrust::entities::{AttributeEntity, Insert};
use acadrust::tables::{BlockRecord, TableEntry};
use acadrust::types::{Transform, Vector3};
use acadrust::{EntityType, Handle};
use arq_geometry::Point2;

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::translate::handle;

/// A block as the Blocks palette describes it (ADR 0025 D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockInfo {
    /// Its name, spelled as the block table spells it.
    pub name: String,
    /// Its description; empty when it has none.
    pub description: String,
    /// Its insertion units, as DWG's INSUNITS code: 0 unitless, 1 inches,
    /// 2 feet, 4 millimeters, 5 centimeters, 6 meters…
    pub units: i16,
    /// The attribute definitions it holds, constant ones included.
    pub attributes: usize,
    /// Whether it is a dynamic block (ADR 0023 D3a).
    pub dynamic: bool,
    /// A dynamic block's parameters the user sees, by name, in the file's
    /// order; empty for a static block or when the engine cannot read them.
    pub parameters: Vec<String>,
    /// Whether its references may be exploded (ADR 0023 D2a).
    pub explodable: bool,
    /// Whether its references must be scaled the same in X and Y.
    pub scale_uniformly: bool,
}

/// What BLOCK does with the objects it makes a block from (ADR 0023 D2a):
/// leaves them as they are, replaces them by a reference to the block at
/// its base point, or deletes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockObjects {
    /// The objects stay as they are.
    Retain,
    /// A reference to the block takes their place.
    Convert,
    /// The objects are deleted.
    Delete,
}

/// What a block's record holds besides its contents (ADR 0023 D2a): its
/// description, insertion units (DWG's INSUNITS code) and two flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDefinition {
    /// Its description; empty for none.
    pub description: String,
    /// Its insertion units, as DWG's INSUNITS code (0 unitless).
    pub units: i16,
    /// Whether its references must be scaled the same in X and Y.
    pub scale_uniformly: bool,
    /// Whether its references may be exploded.
    pub explodable: bool,
}

impl Default for BlockDefinition {
    /// No description, unitless, scaled freely, explodable: what BLOCK
    /// made before the Block Definition dialog.
    fn default() -> Self {
        Self {
            description: String::new(),
            units: 0,
            scale_uniformly: false,
            explodable: true,
        }
    }
}

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

    /// What the Blocks palette shows of block `name`, found ignoring case;
    /// `None` unless INSERT can place it (see `block_names`).
    pub fn block_info(&self, name: &str) -> Option<BlockInfo> {
        if !self
            .block_names()
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
        {
            return None;
        }
        let record = self.cad().block_records.get(name)?;
        let attributes = self
            .cad()
            .entities_in_block(&record.name)
            .filter(|e| matches!(e, EntityType::AttributeDefinition(_)))
            .count();
        let dynamic = self.is_dynamic_block(&record.name);
        let mut parameters: Vec<String> = Vec::new();
        if dynamic {
            let shown = self
                .cad()
                .dynamic_block_properties(record.handle(), None)
                .unwrap_or_default()
                .into_iter()
                .filter(|p| p.show && !p.name.is_empty());
            for property in shown {
                if !parameters.contains(&property.name) {
                    parameters.push(property.name);
                }
            }
        }
        Some(BlockInfo {
            name: record.name.clone(),
            description: record.description.clone(),
            units: record.units,
            attributes,
            dynamic,
            parameters,
            explodable: record.explodable,
            scale_uniformly: record.scale_uniformly,
        })
    }

    /// How many references each block has anywhere in the drawing: model
    /// and paper space, and inside other blocks. A dynamic block's
    /// references are drawn through anonymous copies (`*U…`); they count
    /// for the dynamic block. Blocks without references are left out.
    pub fn block_references(&self) -> HashMap<String, usize> {
        let cad = self.cad();
        let names: HashMap<Handle, &str> = cad
            .block_records
            .iter()
            .map(|b| (b.handle(), b.name.as_str()))
            .collect();
        let mut counts = HashMap::new();
        for e in cad.entities() {
            if let EntityType::Insert(insert) = e {
                // The reference's own link to its dynamic block, else the
                // anonymous block's (AcadRust reads both kinds of file).
                let definition = cad
                    .dynamic_definition_for_insert(insert.common.handle)
                    .map(|h| cad.dynamic_block_definition(h).unwrap_or(h));
                if let Some(name) = definition.and_then(|h| names.get(&h)) {
                    *counts.entry((*name).to_owned()).or_insert(0) += 1;
                }
            }
        }
        counts
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
        let placed = self.define_block(
            name,
            base,
            ids,
            BlockObjects::Convert,
            &BlockDefinition::default(),
            false,
        )?;
        placed.ok_or_else(|| DocumentError::UnsupportedOperation("no reference placed".to_owned()))
    }

    /// Makes block `name` from `ids`, based at `base`, with `definition`'s
    /// description, units and flags, and does with the objects what
    /// `objects` says (BLOCK's dialog, ADR 0023 D2a). With `redefine`, block
    /// `name` exists and its contents, base point and record are replaced,
    /// every reference redrawn; a dynamic block, or one that would hold
    /// itself, is refused. Returns the reference placed by Convert.
    pub fn define_block(
        &mut self,
        name: &str,
        base: Point2,
        ids: &[EntityId],
        objects: BlockObjects,
        definition: &BlockDefinition,
        redefine: bool,
    ) -> DocumentResult<Option<EntityId>> {
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
        let point = Vector3::new(base.x(), base.y(), 0.0);
        let block = if redefine {
            let block = self.editable_block(name)?;
            let contents: Vec<EntityType> = chosen
                .iter()
                .filter_map(|&id| self.cad().get_entity(handle(id)).cloned())
                .collect();
            if self
                .blocks_used(&contents)
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&block))
            {
                return Err(DocumentError::InvalidBlockName(format!(
                    "block {block} cannot hold itself"
                )));
            }
            self.touch_structure();
            self.clear_block(&block)?;
            block
        } else {
            // A name in use is refused by the block table itself ("… exists").
            check_block_name(name)?;
            self.touch_structure();
            self.add_block_record(name, point)?;
            name.to_owned()
        };
        let owner = {
            let record = self
                .cad_mut()
                .block_records
                .get_mut(&block)
                .ok_or_else(|| {
                    DocumentError::InvalidBlockName(format!("no block named {block:?}"))
                })?;
            record.base_point = point;
            record.description = definition.description.clone();
            record.units = definition.units;
            record.scale_uniformly = definition.scale_uniformly;
            record.explodable = definition.explodable;
            record.handle()
        };
        for &id in &chosen {
            self.clone_entity(id, owner)?;
            if objects != BlockObjects::Retain {
                self.delete_entity(id)?;
            }
        }
        if redefine {
            self.redraw_references(&block);
        }
        match objects {
            BlockObjects::Convert => self.insert_block(&block, base, 1.0, 0.0).map(Some),
            BlockObjects::Retain | BlockObjects::Delete => Ok(None),
        }
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

    /// A new, empty block record `name` with its base point at `base`;
    /// returns its handle. The caller has touched the structure.
    pub(crate) fn add_block_record(&mut self, name: &str, base: Vector3) -> DocumentResult<Handle> {
        let mut record = BlockRecord::new(name);
        let h = self.cad_mut().allocate_handle();
        record.set_handle(h);
        record.block_entity_handle = self.cad_mut().allocate_handle();
        record.block_end_handle = self.cad_mut().allocate_handle();
        record.base_point = base;
        self.cad_mut()
            .block_records
            .add(record)
            .map_err(|e| DocumentError::InvalidBlockName(e.to_string()))?;
        Ok(h)
    }

    /// Adds a copy of entity `id` to the block record `owner` (model space
    /// when it is null); see [`add_copy`](Self::add_copy). Returns the
    /// copy's id.
    pub(crate) fn clone_entity(&mut self, id: EntityId, owner: Handle) -> DocumentResult<EntityId> {
        let copy = self
            .cad()
            .get_entity(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?
            .clone();
        self.add_copy(copy, owner)
    }

    /// Adds `copy`, an entity of this drawing or another, to the block
    /// record `owner` (model space when it is null), without the links a
    /// copy cannot share (reactors, an extension dictionary, cached proxy
    /// graphics); a block reference's attributes get handles of their own.
    /// Returns its id.
    pub(crate) fn add_copy(
        &mut self,
        mut copy: EntityType,
        owner: Handle,
    ) -> DocumentResult<EntityId> {
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
pub(crate) fn check_block_name(name: &str) -> DocumentResult<()> {
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
