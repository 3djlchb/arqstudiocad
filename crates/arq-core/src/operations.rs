//! The editing operations of a [`CadDocument`]. Each is one transaction:
//! one undo step, or nothing at all when it fails (contract §6).
//!
//! Operations taking ids remove duplicates; an empty list changes nothing.
//! Entities on a locked layer cannot be modified or erased; new entities may
//! still be drawn on a locked current layer, as in AutoCAD.

use arq_document::{
    ChangeSet, Color, Document, DocumentError, Entity, EntityId, LayerId, PlanGeometry,
};
use arq_geometry::{Arc, Circle, Geometry2, Line, Point2, Transform2, Vector2};

use crate::error::{CadError, CadResult};
use crate::history::CadDocument;

impl CadDocument {
    /// Draws a line on the current layer.
    pub fn add_line(&mut self, start: Point2, end: Point2) -> CadResult<(EntityId, ChangeSet)> {
        self.add_geometry(Line::new(start, end)?.into())
    }

    /// Draws a circle on the current layer.
    pub fn add_circle(&mut self, center: Point2, radius: f64) -> CadResult<(EntityId, ChangeSet)> {
        self.add_geometry(Circle::new(center, radius)?.into())
    }

    /// Draws an arc on the current layer, counterclockwise from `start_angle`
    /// to `end_angle` (radians).
    pub fn add_arc(
        &mut self,
        center: Point2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.add_geometry(Arc::new(center, radius, start_angle, end_angle)?.into())
    }

    /// Draws any plan geometry on the current layer, at elevation 0.
    pub fn add_geometry(&mut self, geometry: Geometry2) -> CadResult<(EntityId, ChangeSet)> {
        self.transact(geometry.kind().name(), |doc| {
            let layer = doc.current_layer().id();
            Ok(doc.add_entity(PlanGeometry::new(geometry), layer)?)
        })
    }

    /// Erases entities of any type (ERASE).
    pub fn delete_entities(&mut self, ids: &[EntityId]) -> CadResult<ChangeSet> {
        self.edit_entities("ERASE", ids, |doc, e| Ok(doc.delete_entity(e.id())?))
    }

    /// Moves entities by `offset` (MOVE).
    pub fn move_entities(&mut self, ids: &[EntityId], offset: Vector2) -> CadResult<ChangeSet> {
        self.transform_entities("MOVE", ids, Transform2::translation(offset))
    }

    /// Rotates entities by `angle` radians counterclockwise around `center`
    /// (ROTATE).
    pub fn rotate_entities(
        &mut self,
        ids: &[EntityId],
        center: Point2,
        angle: f64,
    ) -> CadResult<ChangeSet> {
        self.transform_entities("ROTATE", ids, Transform2::rotation(center, angle)?)
    }

    /// Scales entities by `factor` around `center` (SCALE). Elevations are
    /// kept.
    pub fn scale_entities(
        &mut self,
        ids: &[EntityId],
        center: Point2,
        factor: f64,
    ) -> CadResult<ChangeSet> {
        self.transform_entities("SCALE", ids, Transform2::scaling(center, factor)?)
    }

    /// Mirrors entities across the infinite line through `axis` (MIRROR).
    pub fn mirror_entities(&mut self, ids: &[EntityId], axis: Line) -> CadResult<ChangeSet> {
        self.transform_entities("MIRROR", ids, Transform2::mirror(axis))
    }

    /// Copies entities, moving the copies by `offset` (COPY); returns the
    /// copies' ids, in the order of `ids` without repeats. Lines, circles,
    /// arcs, blocks and polylines can be copied (ADR 0016, D1); any other
    /// entity fails the whole operation. The copies take the originals'
    /// layer, color and other properties.
    pub fn copy_entities(
        &mut self,
        ids: &[EntityId],
        offset: Vector2,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let ids = distinct(ids);
        let t = Transform2::translation(offset);
        self.transact("COPY", |doc| {
            let mut copies = Vec::with_capacity(ids.len());
            for id in ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
                check_transformable(&e)?;
                let copy = doc.copy_entity(id)?;
                doc.transform_entity(copy, t)?;
                copies.push(copy);
            }
            Ok(copies)
        })
    }

    /// Applies `t` to entities that transform: lines, circles and arcs
    /// through their geometry, blocks and polylines as a whole (ADR 0016,
    /// D1). Any other entity fails the whole operation.
    fn transform_entities(
        &mut self,
        name: &str,
        ids: &[EntityId],
        t: Transform2,
    ) -> CadResult<ChangeSet> {
        self.edit_entities(name, ids, |doc, e| {
            check_transformable(e)?;
            Ok(doc.transform_entity(e.id(), t)?)
        })
    }

    /// Gives a line, circle or arc new geometry of the same kind, keeping
    /// its id, layer, color and elevation (a grip stretch, a property
    /// edit). Any other entity, or geometry of another kind, fails.
    pub fn set_entity_geometry(
        &mut self,
        id: EntityId,
        geometry: Geometry2,
    ) -> CadResult<ChangeSet> {
        self.edit_entities("EDIT", &[id], |doc, e| {
            let plan = e
                .plan()
                .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?;
            Ok(doc.set_plan_geometry(id, plan.with_geometry(geometry))?)
        })
    }

    /// Moves one lightweight-polyline vertex to a plan point (grip stretch).
    pub fn set_polyline_vertex(
        &mut self,
        id: EntityId,
        vertex_index: usize,
        point: Point2,
    ) -> CadResult<ChangeSet> {
        self.edit_entities("GRIP", &[id], |doc, _| {
            Ok(doc.set_polyline_vertex(id, vertex_index, point)?)
        })
    }

    /// Explodes selected block references into their transformed contents.
    pub fn explode_entities(&mut self, ids: &[EntityId]) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let ids = distinct(ids);
        self.transact("EXPLODE", |doc| {
            let mut created = Vec::new();
            for id in ids {
                let entity = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &entity)?;
                created.extend(doc.explode_insert(id)?);
            }
            Ok(created)
        })
    }

    /// Moves entities to another layer (CHPROP).
    pub fn set_entity_layer(&mut self, ids: &[EntityId], layer: LayerId) -> CadResult<ChangeSet> {
        self.edit_entities("CHPROP", ids, |doc, e| {
            Ok(doc.set_entity_layer(e.id(), layer)?)
        })
    }

    /// Sets the color of entities (CHPROP).
    pub fn set_entity_color(&mut self, ids: &[EntityId], color: Color) -> CadResult<ChangeSet> {
        self.edit_entities("CHPROP", ids, |doc, e| {
            Ok(doc.set_entity_color(e.id(), color)?)
        })
    }

    /// Runs `edit` on each distinct entity of `ids` in one transaction,
    /// after checking it exists and is not on a locked layer.
    fn edit_entities(
        &mut self,
        name: &str,
        ids: &[EntityId],
        mut edit: impl FnMut(&mut Document, &Entity) -> CadResult<()>,
    ) -> CadResult<ChangeSet> {
        let ids = distinct(ids);
        let ((), summary) = self.transact(name, |doc| {
            for id in ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
                edit(doc, &e)?;
            }
            Ok(())
        })?;
        Ok(summary)
    }

    /// Creates a layer with AutoCAD's defaults.
    pub fn create_layer(&mut self, name: &str) -> CadResult<(LayerId, ChangeSet)> {
        self.transact("LAYER", |doc| Ok(doc.create_layer(name)?))
    }

    /// Renames a layer; its entities follow.
    pub fn rename_layer(&mut self, id: LayerId, new_name: &str) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.rename_layer(id, new_name))
    }

    /// Sets a layer's color (an ACI index or RGB).
    pub fn set_layer_color(&mut self, id: LayerId, color: Color) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_color(id, color))
    }

    /// Turns a layer on or off.
    pub fn set_layer_visible(&mut self, id: LayerId, visible: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_visible(id, visible))
    }

    /// Freezes or thaws a layer.
    pub fn set_layer_frozen(&mut self, id: LayerId, frozen: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_frozen(id, frozen))
    }

    /// Locks or unlocks a layer.
    pub fn set_layer_locked(&mut self, id: LayerId, locked: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_locked(id, locked))
    }

    /// Whether a layer is plotted.
    pub fn set_layer_plottable(&mut self, id: LayerId, plottable: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_plottable(id, plottable))
    }

    /// Deletes an empty, unprotected layer that is not current.
    pub fn delete_layer(&mut self, id: LayerId) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.delete_layer(id))
    }

    /// Makes a layer current (CLAYER).
    pub fn set_current_layer(&mut self, id: LayerId) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_current_layer(id))
    }

    fn layer_op(
        &mut self,
        f: impl FnOnce(&mut Document) -> arq_document::DocumentResult<()>,
    ) -> CadResult<ChangeSet> {
        let ((), summary) = self.transact("LAYER", |doc| Ok(f(doc)?))?;
        Ok(summary)
    }
}

/// `ids` without repeats, in first-seen order.
fn distinct(ids: &[EntityId]) -> Vec<EntityId> {
    let mut seen = std::collections::HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

fn check_transformable(e: &Entity) -> CadResult<()> {
    if e.is_transformable() {
        return Ok(());
    }
    Err(CadError::InvalidOperation(format!(
        "entity {} ({}) cannot be transformed in this version",
        e.id(),
        e.kind()
    )))
}

fn check_unlocked(doc: &Document, e: &Entity) -> CadResult<()> {
    if doc.layer_by_name(e.layer()).is_some_and(|l| l.is_locked()) {
        return Err(CadError::InvalidOperation(format!(
            "entity {} is on locked layer {}",
            e.id(),
            e.layer()
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;
