//! The own internal model (ADR 0018 D2, stage 1).
//!
//! ArqStudioCAD's document model is its own; the DWG engine is an adapter.
//! In stage 1 the model owns the layers (name, color, on, frozen, locked,
//! plotted), the current layer, and every LINE, CIRCLE and ARC drawn in plan
//! (geometry, elevation, layer, color). Reads of those come from here.
//!
//! The model and the engine's database are written together (write-through):
//! every operation that changes an owned thing updates both in the same
//! step, so saving needs no conversion, and what the model does not own
//! (linetypes, thickness, the normal's direction, XDATA, every other kind of
//! entity) stays in the engine's records untouched. Undo and redo restore the
//! engine's images and refresh the model from them. [`Document::check_model`]
//! says whether both still agree.

use std::collections::BTreeMap;

use acadrust::EntityType;
use acadrust::tables::normalize_name;

use crate::document::Document;
use crate::entity::{Color, Entity, EntityId, PlanGeometry};
use crate::layers::{Layer, LayerId, read_layer};
use crate::translate::{color_from_codec, handle, plan_of};

/// What the model owns of a LINE, CIRCLE or ARC.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OwnedEntity {
    pub(crate) layer: LayerId,
    pub(crate) color: Color,
    pub(crate) plan: PlanGeometry,
}

/// The own model of one drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OwnModel {
    /// Every layer. Its linetype is a read-only mirror of the engine's: the
    /// model does not own linetypes yet.
    pub(crate) layers: BTreeMap<LayerId, Layer>,
    /// The current layer (AutoCAD's CLAYER).
    pub(crate) current: LayerId,
    /// Every LINE, CIRCLE and ARC drawn in plan, in any space.
    pub(crate) entities: BTreeMap<EntityId, OwnedEntity>,
}

impl Document {
    /// The model as the engine's database describes it now.
    fn model_from_engine(&self) -> OwnModel {
        let mut model = OwnModel {
            layers: self
                .cad()
                .layers
                .iter()
                .map(|l| (l.handle.value(), read_layer(l)))
                .collect(),
            ..OwnModel::default()
        };
        model.current = self.current_layer_from_engine();
        for e in self.cad().entities() {
            if let Some(owned) = self.owned_from_engine(e) {
                model.entities.insert(e.as_entity().handle().value(), owned);
            }
        }
        model
    }

    fn current_layer_from_engine(&self) -> LayerId {
        let cad = self.cad();
        cad.layers
            .get(&cad.header.current_layer_name)
            .or_else(|| cad.layers.get("0"))
            .map(|l| l.handle.value())
            .expect("layer 0 always exists")
    }

    /// What the model owns of `e`, if it is a LINE, CIRCLE or ARC drawn in
    /// plan on a layer of the table.
    fn owned_from_engine(&self, e: &EntityType) -> Option<OwnedEntity> {
        let plan = plan_of(e)?;
        let common = e.as_entity();
        let layer = self.cad().layers.get(common.layer())?.handle.value();
        Some(OwnedEntity {
            layer,
            color: color_from_codec(common.color()),
            plan,
        })
    }

    /// Builds the whole model from the engine (opening a drawing).
    pub(crate) fn rebuild_model(&mut self) {
        self.model = self.model_from_engine();
    }

    /// Writes the engine's present state of one entity into the model:
    /// added, changed, or gone if the entity no longer exists or is not
    /// owned.
    pub(crate) fn sync_entity(&mut self, id: EntityId) {
        let owned = self
            .cad()
            .get_entity(handle(id))
            .and_then(|e| self.owned_from_engine(e));
        match owned {
            Some(o) => {
                self.model.entities.insert(id, o);
            }
            None => {
                self.model.entities.remove(&id);
            }
        }
    }

    /// Writes the engine's layer table and current layer into the model.
    pub(crate) fn sync_layers(&mut self) {
        self.model.layers = self
            .cad()
            .layers
            .iter()
            .map(|l| (l.handle.value(), read_layer(l)))
            .collect();
        self.model.current = self.current_layer_from_engine();
    }

    /// An owned entity as the model describes it.
    pub(crate) fn owned_view(&self, id: EntityId) -> Option<Entity> {
        let owned = self.model.entities.get(&id)?;
        let layer = self.model.layers.get(&owned.layer)?;
        let codec = self.cad().get_entity(handle(id))?;
        let mut entity = crate::translate::read_entity(codec);
        entity.kind = owned.plan.geometry().kind().name();
        entity.layer = layer.name().to_owned();
        entity.color = owned.color;
        entity.bounds = Some(owned.plan.geometry().bounds());
        entity.plan = Some(owned.plan);
        entity.transformable = true;
        Some(entity)
    }

    /// The layer named `name` (ignoring case), from the model.
    pub(crate) fn model_layer_by_name(&self, name: &str) -> Option<&Layer> {
        let key = normalize_name(name);
        self.model
            .layers
            .values()
            .find(|l| normalize_name(l.name()) == key)
    }

    /// Where the own model and the engine's database disagree: an empty
    /// list when they describe the same drawing. Checked after every core
    /// transaction in debug builds, and by the round-trip corpus.
    pub fn check_model(&self) -> Vec<String> {
        let truth = self.model_from_engine();
        let mut problems = Vec::new();
        if truth.current != self.model.current {
            problems.push(format!(
                "current layer: model {}, engine {}",
                self.model.current, truth.current
            ));
        }
        for (id, layer) in &truth.layers {
            if self.model.layers.get(id) != Some(layer) {
                problems.push(format!("layer {id}: not as the engine has it"));
            }
        }
        for id in self.model.layers.keys() {
            if !truth.layers.contains_key(id) {
                problems.push(format!("layer {id}: in the model only"));
            }
        }
        for (id, owned) in &truth.entities {
            if self.model.entities.get(id) != Some(owned) {
                problems.push(format!("entity {id}: not as the engine has it"));
            }
        }
        for id in self.model.entities.keys() {
            if !truth.entities.contains_key(id) {
                problems.push(format!("entity {id}: in the model only"));
            }
        }
        problems
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
