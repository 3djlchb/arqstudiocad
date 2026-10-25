//! The editing operations of a [`CadDocument`]. Each is one transaction:
//! one undo step, or nothing at all when it fails (contract §6).
//!
//! Operations taking ids remove duplicates; an empty list changes nothing.
//! Entities on a locked layer cannot be modified or erased; new entities may
//! still be drawn on a locked current layer, as in AutoCAD.

use std::collections::BTreeMap;

use arq_document::{
    ChangeSet, ClashChoice, Color, Document, DocumentError, Entity, EntityId, LayerId, PlanGeometry,
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

    /// Writes a single-line text on the current layer (TEXT, ADR 0022).
    pub fn add_text(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("TEXT", |doc| {
            Ok(doc.add_text(content, anchor, height, rotation)?)
        })
    }

    /// Draws a polyline on the current layer (PLINE): vertices with the
    /// bulge of the segment leaving each, open or `closed`.
    pub fn add_polyline(
        &mut self,
        vertices: &[(Point2, f64)],
        closed: bool,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("PLINE", |doc| Ok(doc.add_polyline(vertices, closed)?))
    }

    /// Makes block `name` from `ids`, base point `base`, and puts a
    /// reference to it in their place (BLOCK, ADR 0023 D2).
    pub fn create_block(
        &mut self,
        name: &str,
        base: Point2,
        ids: &[EntityId],
    ) -> CadResult<(EntityId, ChangeSet)> {
        let ids = distinct(ids);
        self.transact("BLOCK", |doc| {
            for &id in &ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
            }
            Ok(doc.create_block(name, base, &ids)?)
        })
    }

    /// A new drawing holding block `name`'s contents alone, for the block
    /// editor (BEDIT, ADR 0023 D3); this drawing is unchanged.
    pub fn block_editor(&self, name: &str) -> CadResult<CadDocument> {
        Ok(CadDocument::from_document(
            self.document().block_editor(name)?,
        ))
    }

    /// Makes `contents`' model space the contents of block `name`, as the
    /// block editor saves it: one undo step (BEDIT, ADR 0023 D3).
    pub fn replace_block_contents(
        &mut self,
        name: &str,
        contents: &Document,
    ) -> CadResult<ChangeSet> {
        Ok(self
            .transact("BEDIT", |doc| {
                Ok(doc.replace_block_contents(name, contents)?)
            })?
            .1)
    }

    /// Places block `name` (INSERT, ADR 0023 D2).
    pub fn insert_block(
        &mut self,
        name: &str,
        point: Point2,
        scale: f64,
        rotation: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("INSERT", |doc| {
            Ok(doc.insert_block(name, point, scale, rotation)?)
        })
    }

    /// Copies block `name` of `source` here, with what it needs, choosing
    /// by `choices` for each block whose name this drawing uses for other
    /// contents (ADR 0025 D6); returns its name here. One undo step, or
    /// part of INSERT's when grouped with it.
    pub fn copy_block(
        &mut self,
        source: &Document,
        name: &str,
        choices: &BTreeMap<String, ClashChoice>,
    ) -> CadResult<(String, ChangeSet)> {
        self.transact("INSERT", |doc| Ok(doc.copy_block(source, name, choices)?))
    }

    /// Writes a multiline text on the current layer (MTEXT, ADR 0022 D6).
    #[allow(clippy::too_many_arguments)]
    pub fn add_mtext(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
        width: f64,
        justification: &str,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("MTEXT", |doc| {
            Ok(doc.add_mtext(content, anchor, height, rotation, width, justification)?)
        })
    }

    /// Sets what is given of texts' content, height and rotation (radians),
    /// all or none (ADR 0022).
    pub fn set_text(
        &mut self,
        ids: &[EntityId],
        content: Option<&str>,
        height: Option<f64>,
        rotation: Option<f64>,
    ) -> CadResult<ChangeSet> {
        self.edit_entities("PROPERTIES", ids, |doc, e| {
            Ok(doc.set_text(e.id(), content, height, rotation)?)
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
