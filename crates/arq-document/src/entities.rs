//! Entity access on [`Document`].

use acadrust::entities::LwPolyline;
use acadrust::types::{Transform, Vector2, Vector3};
use acadrust::{EntityType, Handle};
use arq_geometry::{Bounds2, GeometryError, Point2, Transform2};

use crate::document::Document;
use crate::entity::Color;
use crate::entity::{Entity, EntityId, PlanGeometry};
use crate::error::{DocumentError, DocumentResult};
use crate::layers::LayerId;
use crate::ocs::plan_normal;
use crate::translate::{
    attachment_named, attribute_as_text, bounds_of, color_to_codec, edit_text, handle,
    is_whole_transformable, new_entity, new_mtext, new_text, read_entity, transform_whole,
    write_plan,
};

impl Document {
    /// Replaces one block reference with its transformed contents. Its
    /// visible attributes, and the definition's constant ones, become
    /// single-line texts showing their values, as AutoCAD Express's BURST
    /// (ADR 0023 D1); the other attribute definitions are not copied.
    pub fn explode_insert(&mut self, id: EntityId) -> DocumentResult<Vec<EntityId>> {
        let insert = match self.cad().get_entity(handle(id)) {
            Some(EntityType::Insert(insert)) => insert.clone(),
            Some(entity) => {
                return Err(DocumentError::NotEditable {
                    id,
                    kind: entity.as_entity().entity_type(),
                });
            }
            None => return Err(DocumentError::EntityNotFound(id)),
        };
        let record = self
            .cad()
            .block_records
            .get(&insert.block_name)
            .ok_or_else(|| {
                DocumentError::UnsupportedOperation("the block definition is missing".to_owned())
            })?;
        // INSERT transforms contents around the block's base point. The codec's
        // explode helper omits this adjustment, so normalize the cloned source
        // entities first to match the document's drawing transform.
        let base = record.base_point;
        let base_offset = Transform::from_translation(Vector3::new(-base.x, -base.y, -base.z));
        let mut contents: Vec<_> = self
            .cad()
            .entities_in_block(&insert.block_name)
            .filter_map(|entity| match entity {
                EntityType::AttributeDefinition(_) => attribute_as_text(entity),
                other => Some(other.clone()),
            })
            .collect();
        for entity in &mut contents {
            entity.apply_transform(&base_offset);
        }
        let mut parts = insert.explode(&contents);
        // The reference's own attributes already sit where they are drawn.
        parts.extend(
            insert
                .attributes
                .iter()
                .filter_map(|a| attribute_as_text(&EntityType::AttributeEntity(a.clone()))),
        );
        if parts.is_empty() {
            return Err(DocumentError::UnsupportedOperation(
                "the block has no contents that can be exploded".to_owned(),
            ));
        }
        let mut ids = Vec::with_capacity(parts.len());
        for mut part in parts.drain(..) {
            let common = part.common_mut();
            common.handle = Handle::NULL;
            common.owner_handle = Handle::NULL;
            common.raw_record = None;
            common.reactors.clear();
            common.xdictionary_handle = None;
            common.graphic_data = None;
            let handle = self.cad_mut().add_entity(part)?;
            ids.push(handle.value());
            self.sync_entity(handle.value());
        }
        self.delete_entity(id)?;
        Ok(ids)
    }

    /// Moves one vertex of a lightweight polyline in plan, preserving its
    /// bulge, widths, elevation and entity properties.
    pub fn set_polyline_vertex(
        &mut self,
        id: EntityId,
        vertex_index: usize,
        point: arq_geometry::Point2,
    ) -> DocumentResult<()> {
        let raw = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let EntityType::LwPolyline(polyline) = raw else {
            return Err(DocumentError::NotEditable {
                id,
                kind: raw.as_entity().entity_type(),
            });
        };
        let normal = plan_normal(polyline.normal.x, polyline.normal.y, polyline.normal.z).ok_or(
            DocumentError::NotEditable {
                id,
                kind: "LWPOLYLINE",
            },
        )?;
        let vertex =
            polyline
                .vertices
                .get_mut(vertex_index)
                .ok_or(DocumentError::InvalidVertexIndex {
                    id,
                    index: vertex_index,
                })?;
        let (x, y, _) = normal.map_point(point.x(), point.y(), polyline.elevation);
        vertex.location = acadrust::types::Vector2::new(x, y);
        Ok(())
    }

    /// A snapshot of the entity with this id, in any space: from the own
    /// model when it owns the entity, else from the engine.
    pub fn entity(&self, id: EntityId) -> Option<Entity> {
        self.owned_view(id).or_else(|| {
            self.cad()
                .get_entity(handle(id))
                .map(|e| self.read_with_extent(e))
        })
    }

    /// Snapshots of every entity in model space, in drawing order.
    pub fn entities(&self) -> Vec<Entity> {
        self.cad()
            .model_space_entities()
            .map(|e| {
                let id = e.as_entity().handle().value();
                self.owned_view(id)
                    .unwrap_or_else(|| self.read_with_extent(e))
            })
            .collect()
    }

    /// An engine entity as the core sees it; a block reference or a
    /// dimension boxed by what it draws, which the engine does not know.
    fn read_with_extent(&self, e: &EntityType) -> Entity {
        let mut entity = read_entity(e);
        if let Some(extent) = self.block_extent(e) {
            entity.bounds = Some(extent);
        }
        entity
    }

    /// The drawn box of a block reference or a dimension, and of a
    /// polyline, whose arcs the engine's box (its vertices) leaves out;
    /// `None` for other kinds, whose own box is right.
    fn block_extent(&self, e: &EntityType) -> Option<Bounds2> {
        match e {
            EntityType::Insert(_)
            | EntityType::Dimension(_)
            | EntityType::LwPolyline(_)
            | EntityType::Polyline2D(_)
            | EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_) => self.drawn_extent(e),
            _ => None,
        }
    }

    /// The plan bounds of every model-space entity that has them, in drawing
    /// order: a cheap pass for spatial queries (no layer names copied).
    pub fn entity_bounds(&self) -> impl Iterator<Item = (EntityId, Bounds2)> + '_ {
        self.cad().model_space_entities().filter_map(|e| {
            let bounds = self.block_extent(e).or_else(|| bounds_of(e))?;
            Some((e.as_entity().handle().value(), bounds))
        })
    }

    /// Adds plan geometry to model space on `layer`; returns its id.
    pub fn add_entity(&mut self, plan: PlanGeometry, layer: LayerId) -> DocumentResult<EntityId> {
        let name = self
            .layer(layer)
            .ok_or(DocumentError::LayerIdNotFound(layer))?
            .name()
            .to_owned();
        let h = self.cad_mut().add_entity(new_entity(plan, &name))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Replaces the plan geometry of an editable entity of the same kind,
    /// keeping its id, layer, color, thickness, normal direction and XDATA.
    pub fn set_plan_geometry(&mut self, id: EntityId, plan: PlanGeometry) -> DocumentResult<()> {
        let editable = self
            .entity(id)
            .ok_or(DocumentError::EntityNotFound(id))?
            .is_editable();
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        if !editable {
            let kind = e.as_entity().entity_type();
            return Err(DocumentError::NotEditable { id, kind });
        }
        write_plan(e, plan)?;
        self.sync_entity(id);
        Ok(())
    }

    /// Moves, rotates, scales or mirrors an entity that
    /// [`Entity::is_transformable`]: editable ones through their plan
    /// geometry, blocks and polylines as a whole (ADR 0016, D1). Anything
    /// else is [`DocumentError::NotEditable`].
    pub fn transform_entity(&mut self, id: EntityId, t: Transform2) -> DocumentResult<()> {
        let e = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        if let Some(plan) = e.plan() {
            let moved = plan.with_geometry(t.apply(plan.geometry())?);
            return self.set_plan_geometry(id, moved);
        }
        let raw = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        if !is_whole_transformable(raw) {
            let kind = raw.as_entity().entity_type();
            return Err(DocumentError::NotEditable { id, kind });
        }
        // Kinds transformed whole are not owned by the model: nothing to sync.
        transform_whole(raw, t);
        Ok(())
    }

    /// Adds a copy of a transformable entity to model space, with the same
    /// layer, color and other properties, where the original is; returns
    /// its id. A block's attributes get handles of their own; links the
    /// copy cannot share (reactors, an extension dictionary, cached proxy
    /// graphics) are left out, as copying drops them in AutoCAD too.
    pub fn copy_entity(&mut self, id: EntityId) -> DocumentResult<EntityId> {
        let original = self
            .cad()
            .get_entity(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        if !read_entity(original).is_transformable() {
            let kind = original.as_entity().entity_type();
            return Err(DocumentError::NotEditable { id, kind });
        }
        self.clone_entity(id, Handle::NULL)
    }

    /// Deletes an entity of any type (erasing needs no geometry, D3.4).
    ///
    /// Uses the codec's shared-image removal, which keeps block membership so
    /// undo can restore it exactly.
    pub fn delete_entity(&mut self, id: EntityId) -> DocumentResult<()> {
        self.cad_mut()
            .remove_entity_arc(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        self.model.entities.remove(&id);
        Ok(())
    }

    /// Draws a lightweight polyline on the current layer (PLINE): each
    /// vertex with the bulge of the segment leaving it (0 for a straight
    /// segment, the tangent of a quarter of an arc's angle otherwise,
    /// positive counterclockwise); `closed` joins the last vertex to the
    /// first. Two vertices at least, no segment of zero length.
    pub fn add_polyline(
        &mut self,
        vertices: &[(Point2, f64)],
        closed: bool,
    ) -> DocumentResult<EntityId> {
        if vertices.len() < 2 {
            return Err(DocumentError::Geometry(GeometryError::OutOfRange {
                what: "polyline vertices",
            }));
        }
        let segments = if closed {
            vertices.len()
        } else {
            vertices.len() - 1
        };
        for i in 0..segments {
            let (a, bulge) = vertices[i];
            let b = vertices[(i + 1) % vertices.len()].0;
            if !bulge.is_finite() {
                return Err(DocumentError::Geometry(GeometryError::NonFinite {
                    what: "bulge",
                }));
            }
            if a == b {
                return Err(DocumentError::Geometry(GeometryError::ZeroLength));
            }
        }
        let mut polyline = LwPolyline::new();
        for &(point, bulge) in vertices {
            polyline.add_point_with_bulge(Vector2::new(point.x(), point.y()), bulge);
        }
        polyline.is_closed = closed;
        polyline.common.layer = self.current_layer().name().to_owned();
        let h = self
            .cad_mut()
            .add_entity(EntityType::LwPolyline(polyline))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// The drawing's default text height (TEXTSIZE), 2.5 if it holds none.
    pub fn text_size(&self) -> f64 {
        let size = self.cad().header.text_height;
        if size.is_finite() && size > 0.0 {
            size
        } else {
            2.5
        }
    }

    /// Writes a single-line TEXT, left-justified on the baseline at
    /// `anchor`, `rotation` radians from East, on the current layer with
    /// the current text style (ADR 0022 D1); returns its id.
    pub fn add_text(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
    ) -> DocumentResult<EntityId> {
        check_single_line(content)?;
        check_text_sizes(Some(height), Some(rotation))?;
        let layer = self.current_layer().name().to_owned();
        let style = self.current_text_style();
        let text = new_text(content, anchor, height, rotation, &layer, &style);
        let h = self.cad_mut().add_entity(text)?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Writes an MTEXT on the current layer with the current text style
    /// (ADR 0022 D6): `content` is plain text whose line breaks are
    /// paragraphs, wrapped to `width` (0: no wrapping), hanging from
    /// `anchor` as `justification` says (`"TopLeft"`…); returns its id.
    pub fn add_mtext(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
        width: f64,
        justification: &str,
    ) -> DocumentResult<EntityId> {
        if content.trim().is_empty() {
            return Err(DocumentError::InvalidText(
                "a text needs at least one character",
            ));
        }
        check_text_sizes(Some(height), Some(rotation))?;
        if !(width.is_finite() && width >= 0.0) {
            return Err(DocumentError::InvalidText(
                "a text width must be a finite number, zero or greater",
            ));
        }
        let attachment = attachment_named(justification).ok_or(DocumentError::InvalidText(
            "unknown multiline text justification",
        ))?;
        let layer = self.current_layer().name().to_owned();
        let style = self.current_text_style();
        let text = new_mtext(
            content, anchor, height, rotation, width, attachment, &layer, &style,
        );
        let h = self.cad_mut().add_entity(text)?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// The current text style (TEXTSTYLE), or "Standard" when the drawing
    /// lacks it.
    fn current_text_style(&self) -> String {
        let wanted = &self.cad().header.current_text_style_name;
        if self.cad().text_styles.get(wanted).is_some() {
            wanted.clone()
        } else {
            "Standard".to_owned()
        }
    }

    /// Sets what is given of a text's content, height and rotation (world
    /// radians). A single-line text takes one non-empty line; an MTEXT with
    /// formatting codes keeps its content, and Aligned and Fit texts their
    /// height and rotation (ADR 0022 D3, D4).
    pub fn set_text(
        &mut self,
        id: EntityId,
        content: Option<&str>,
        height: Option<f64>,
        rotation: Option<f64>,
    ) -> DocumentResult<()> {
        let e = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        let text = e
            .text()
            .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?;
        if let Some(content) = content {
            if !text.content_editable() {
                return Err(DocumentError::UnsupportedOperation(
                    "this MTEXT holds formatting codes; its content is not edited yet".to_owned(),
                ));
            }
            if text.is_multiline() {
                if content.trim().is_empty() {
                    return Err(DocumentError::InvalidText(
                        "a text needs at least one character",
                    ));
                }
            } else {
                check_single_line(content)?;
            }
        }
        if (height.is_some() || rotation.is_some()) && !text.sizes_editable() {
            return Err(DocumentError::UnsupportedOperation(
                "an aligned or fit text takes its height and rotation from its points".to_owned(),
            ));
        }
        check_text_sizes(height, rotation)?;
        let raw = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        edit_text(raw, content, height, rotation);
        Ok(())
    }

    /// Moves an entity of any type to another layer.
    pub fn set_entity_layer(&mut self, id: EntityId, layer: LayerId) -> DocumentResult<()> {
        let name = self
            .layer(layer)
            .ok_or(DocumentError::LayerIdNotFound(layer))?
            .name()
            .to_owned();
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let common = e.common_mut();
        common.layer = name;
        common.layer_handle = Some(acadrust::Handle::new(layer));
        self.sync_entity(id);
        Ok(())
    }

    /// Sets the color of an entity of any type.
    pub fn set_entity_color(&mut self, id: EntityId, color: Color) -> DocumentResult<()> {
        if color == Color::Index(0) {
            return Err(DocumentError::InvalidColorIndex(0));
        }
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        e.common_mut().color = color_to_codec(color);
        self.sync_entity(id);
        Ok(())
    }
}

fn check_single_line(content: &str) -> DocumentResult<()> {
    if content.is_empty() {
        return Err(DocumentError::InvalidText(
            "a text needs at least one character",
        ));
    }
    if content.contains(['\n', '\r']) {
        return Err(DocumentError::InvalidText(
            "a single-line text holds one line",
        ));
    }
    Ok(())
}

fn check_text_sizes(height: Option<f64>, rotation: Option<f64>) -> DocumentResult<()> {
    if height.is_some_and(|h| !(h.is_finite() && h > 0.0)) {
        return Err(DocumentError::InvalidText(
            "a text height must be a finite number greater than zero",
        ));
    }
    if rotation.is_some_and(|r| !r.is_finite()) {
        return Err(DocumentError::InvalidText(
            "a text rotation must be a finite number",
        ));
    }
    Ok(())
}

/// Test helpers for other crates (feature `test-support`).
#[cfg(feature = "test-support")]
impl Document {
    /// Inserts a POINT on layer "0": an entity that is read-only in this
    /// version (it cannot be transformed or copied).
    pub fn insert_point_for_tests(&mut self, x: f64, y: f64) -> EntityId {
        let point = acadrust::Point::from_coords(x, y, 0.0);
        self.cad_mut()
            .add_entity(acadrust::EntityType::Point(point))
            .expect("adding a point")
            .value()
    }

    /// Inserts a single-line TEXT on layer "0", as a drawing from another
    /// CAD would contain.
    pub fn insert_text_for_tests(&mut self, value: &str, x: f64, y: f64) -> EntityId {
        let text = acadrust::Text::with_value(value, acadrust::types::Vector3::new(x, y, 0.0));
        self.cad_mut()
            .add_entity(acadrust::EntityType::Text(text))
            .expect("adding a text")
            .value()
    }

    /// Attaches XDATA of application `app` to an entity: data the platform
    /// does not show, which only the inventory of a save sees.
    pub fn attach_xdata_for_tests(&mut self, id: EntityId, app: &str) {
        let _ = self
            .cad_mut()
            .app_ids
            .add(acadrust::tables::AppId::new(app));
        let mut record = acadrust::xdata::ExtendedDataRecord::new(app);
        record.add_value(acadrust::xdata::XDataValue::String("kept?".to_owned()));
        self.cad_mut()
            .get_entity_mut(acadrust::Handle::new(id))
            .expect("an entity")
            .common_mut()
            .extended_data
            .add_record(record);
    }

    /// A fault for [`damage_next_save_for_tests`](Self::damage_next_save_for_tests):
    /// drops every XDATA group (codes 1000 to 1071) from a DXF.
    pub fn drop_dxf_xdata_for_tests(bytes: &mut Vec<u8>) {
        let text = String::from_utf8(bytes.clone()).expect("a DXF");
        let lines: Vec<&str> = text.split('\n').collect();
        let mut kept = Vec::new();
        let mut i = 0;
        while i + 1 < lines.len() {
            let code: i32 = lines[i].trim().parse().unwrap_or(0);
            if !(1000..=1071).contains(&code) {
                kept.push(lines[i]);
                kept.push(lines[i + 1]);
            }
            i += 2;
        }
        kept.extend(&lines[i..]);
        *bytes = kept.join("\n").into_bytes();
    }

    /// Sets the file-format version the next save writes, by code
    /// (`"AC1015"`…); `false` for an unknown code. Choosing the version is
    /// not a user feature yet; the round-trip corpus needs it.
    pub fn set_version_for_tests(&mut self, code: &str) -> bool {
        match acadrust::types::DxfVersion::parse(code) {
            Some(version) => {
                self.cad_mut().version = version;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
#[path = "whole_tests.rs"]
mod whole_tests;

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use acadrust::types::Vector3;
    use acadrust::{EntityType, Handle};
    use arq_geometry::{Arc, Circle, Geometry2, Line, Point2, Tolerance, Transform2, Vector2};

    use super::*;
    use crate::entity::Color;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn plan(g: impl Into<Geometry2>, z: f64) -> PlanGeometry {
        PlanGeometry::at_elevation(g.into(), z).unwrap()
    }

    fn layer0(doc: &Document) -> LayerId {
        doc.layer_by_name("0").unwrap().id()
    }

    /// Adds a raw codec entity, as a file written by another CAD would contain.
    fn add_raw(doc: &mut Document, e: EntityType) -> EntityId {
        doc.cad_mut().add_entity(e).unwrap().value()
    }

    fn codec_arc(center: (f64, f64, f64), r: f64, s: f64, e: f64, normal_z: f64) -> EntityType {
        let mut a = acadrust::Arc::new();
        a.center = Vector3::new(center.0, center.1, center.2);
        a.radius = r;
        a.start_angle = s;
        a.end_angle = e;
        a.normal = Vector3::new(0.0, 0.0, normal_z);
        EntityType::Arc(a)
    }

    #[test]
    fn added_entities_read_back_exactly() {
        let mut doc = Document::new();
        let shapes: [Geometry2; 3] = [
            Line::new(p(0.0, 0.0), p(10.0, 5.0)).unwrap().into(),
            Circle::new(p(3.0, 4.0), 2.5).unwrap().into(),
            Arc::new(p(1.0, 1.0), 4.0, 0.25, 2.0).unwrap().into(),
        ];
        let ids: Vec<_> = shapes
            .iter()
            .map(|g| doc.add_entity(plan(*g, 7.5), layer0(&doc)).unwrap())
            .collect();
        assert_eq!(
            ids.len(),
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            "unique ids"
        );
        for (id, g) in ids.iter().zip(shapes) {
            let e = doc.entity(*id).unwrap();
            assert_eq!(e.plan().unwrap().geometry(), g);
            assert_eq!(e.plan().unwrap().elevation(), 7.5);
            assert_eq!((e.layer(), e.color()), ("0", Color::ByLayer));
            assert_eq!(e.bounds(), Some(g.bounds()));
            assert!(e.is_editable());
        }
        let kinds: Vec<_> = doc.entities().iter().map(|e| e.kind()).collect();
        assert_eq!(kinds, ["LINE", "CIRCLE", "ARC"]);
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let line = Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
        let r = Document::new().add_entity(plan(line, 0.0), 424_242);
        assert_eq!(r, Err(DocumentError::LayerIdNotFound(424_242)));
    }

    #[test]
    fn dwg_and_dxf_round_trip_keep_ids_and_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = Document::new();
        let (x, y) = (500_000.0, 9_977_000.0); // UTM 17S
        let ids = [
            doc.add_entity(
                plan(Line::new(p(x, y), p(x + 12.0, y + 3.0)).unwrap(), 2_850.0),
                layer0(&doc),
            )
            .unwrap(),
            doc.add_entity(
                plan(Circle::new(p(x + 5.0, y), 1.25).unwrap(), 2_850.0),
                layer0(&doc),
            )
            .unwrap(),
            doc.add_entity(
                plan(Arc::new(p(x, y + 8.0), 3.0, 5.5, 0.4).unwrap(), 2_850.0),
                layer0(&doc),
            )
            .unwrap(),
        ];
        for name in ["r.dwg", "r.dxf"] {
            let path = dir.path().join(name);
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            for id in ids {
                let (a, b) = (doc.entity(id).unwrap(), back.entity(id).unwrap());
                let (ga, gb) = (a.plan().unwrap().geometry(), b.plan().unwrap().geometry());
                assert_eq!(a.kind(), b.kind(), "{name}");
                assert!(
                    (b.plan().unwrap().elevation() - 2_850.0).abs() < 1e-9,
                    "{name}"
                );
                for t in [0.0, 0.5, 1.0] {
                    assert!(
                        ga.point_at(t).approx_eq(gb.point_at(t), TOL),
                        "{name}: {id} at {t}"
                    );
                }
            }
        }
    }

    #[test]
    fn mirrored_ocs_arc_is_read_in_world_coordinates() {
        // Written by another CAD with normal (0, 0, -1): OCS center (2, 3, 4),
        // radius 5, from 0 to 90°.
        let mut doc = Document::new();
        let id = add_raw(
            &mut doc,
            codec_arc((2.0, 3.0, 4.0), 5.0, 0.0, FRAC_PI_2, -1.0),
        );
        let e = doc.entity(id).unwrap();
        let pl = e.plan().expect("a mirrored-OCS arc is still in plan");
        let Geometry2::Arc(a) = pl.geometry() else {
            panic!("not an arc")
        };
        assert_eq!(pl.elevation(), -4.0);
        assert!(a.center().approx_eq(p(-2.0, 3.0), TOL));
        // OCS end (2, 8) becomes the world start; OCS start (7, 3) the world end.
        assert!(a.start_point().approx_eq(p(-2.0, 8.0), TOL));
        assert!(a.end_point().approx_eq(p(-7.0, 3.0), TOL));
        assert!((a.start_angle() - FRAC_PI_2).abs() < 1e-12 && (a.end_angle() - PI).abs() < 1e-12);
    }

    #[test]
    fn editing_a_mirrored_arc_keeps_its_ocs() {
        let mut doc = Document::new();
        let id = add_raw(
            &mut doc,
            codec_arc((2.0, 3.0, 4.0), 5.0, 0.0, FRAC_PI_2, -1.0),
        );
        let before = doc.entity(id).unwrap().plan().unwrap();
        let moved = Transform2::translation(Vector2::new(10.0, -1.0).unwrap())
            .apply(before.geometry())
            .unwrap();
        doc.set_plan_geometry(id, before.with_geometry(moved))
            .unwrap();
        let after = doc.entity(id).unwrap().plan().unwrap();
        for t in [0.0, 0.3, 1.0] {
            assert!(
                after
                    .geometry()
                    .point_at(t)
                    .approx_eq(moved.point_at(t), TOL)
            );
        }
        assert_eq!(after.elevation(), -4.0);
        let Some(EntityType::Arc(raw)) = doc.cad().get_entity(Handle::new(id)) else {
            panic!()
        };
        assert_eq!(raw.normal.z, -1.0, "still stored as the other CAD wrote it");
    }

    #[test]
    fn edits_keep_color_thickness_and_layer() {
        let mut doc = Document::new();
        let mut c = acadrust::Circle::new();
        c.radius = 2.0;
        c.thickness = 3.5;
        c.common.color = acadrust::Color::Index(1);
        let id = add_raw(&mut doc, EntityType::Circle(c));
        let bigger = Circle::new(p(1.0, 1.0), 4.0).unwrap();
        doc.set_plan_geometry(id, plan(bigger, 0.0)).unwrap();
        let e = doc.entity(id).unwrap();
        assert_eq!(e.color(), Color::Index(1));
        assert_eq!(e.plan().unwrap().geometry(), bigger.into());
        let Some(EntityType::Circle(raw)) = doc.cad().get_entity(Handle::new(id)) else {
            panic!()
        };
        assert_eq!(raw.thickness, 3.5);
    }

    #[test]
    fn tilted_sloped_and_unsupported_entities_are_read_only() {
        let mut doc = Document::new();
        let mut tilted = acadrust::Circle::new();
        tilted.radius = 1.0;
        tilted.normal = Vector3::new(1.0, 0.0, 0.0);
        let tilted = add_raw(&mut doc, EntityType::Circle(tilted));
        let sloped = add_raw(
            &mut doc,
            EntityType::Line(acadrust::Line::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, 2.0)),
        );
        let text = add_raw(
            &mut doc,
            EntityType::Text(acadrust::Text::with_value(
                "PLANTA",
                Vector3::new(1.0, 2.0, 0.0),
            )),
        );
        for (id, kind) in [(tilted, "CIRCLE"), (sloped, "LINE"), (text, "TEXT")] {
            let e = doc.entity(id).unwrap();
            assert_eq!(e.kind(), kind);
            assert!(!e.is_editable(), "{kind} must be read-only");
            let any = plan(Circle::new(p(0.0, 0.0), 1.0).unwrap(), 0.0);
            assert_eq!(
                doc.set_plan_geometry(id, any),
                Err(DocumentError::NotEditable { id, kind })
            );
        }
        assert_eq!(
            doc.entities().len(),
            3,
            "read-only entities are still listed"
        );
    }

    #[test]
    fn kind_mismatch_and_missing_entity() {
        let mut doc = Document::new();
        let id = doc
            .add_entity(
                plan(Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap(), 0.0),
                layer0(&doc),
            )
            .unwrap();
        let circle = plan(Circle::new(p(0.0, 0.0), 1.0).unwrap(), 0.0);
        assert_eq!(
            doc.set_plan_geometry(id, circle),
            Err(DocumentError::KindMismatch { id, kind: "LINE" })
        );
        assert_eq!(
            doc.set_plan_geometry(999_999, circle),
            Err(DocumentError::EntityNotFound(999_999))
        );
        assert!(doc.entity(999_999).is_none());
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;
        use std::f64::consts::TAU;

        fn geometry() -> impl Strategy<Value = Geometry2> {
            let c = -1e6..1e6f64;
            prop_oneof![
                (c.clone(), c.clone(), c.clone(), c.clone())
                    .prop_filter("length", |(a, b, x, y)| (a - x).hypot(b - y) > 1e-3)
                    .prop_map(|(a, b, x, y)| Line::new(p(a, b), p(x, y)).unwrap().into()),
                (c.clone(), c.clone(), 1e-3..1e5f64)
                    .prop_map(|(x, y, r)| Circle::new(p(x, y), r).unwrap().into()),
                (c.clone(), c, 1e-3..1e5f64, 0.0..TAU, 0.01..(TAU - 0.01))
                    .prop_map(|(x, y, r, s, w)| Arc::new(p(x, y), r, s, s + w).unwrap().into()),
            ]
        }

        proptest! {
            /// Whatever plan geometry is written, through either OCS
            /// orientation, the same geometry is read back.
            #[test]
            fn write_then_read_returns_the_geometry(
                g in geometry(), z in -1e4..1e4f64, mirrored in any::<bool>(),
            ) {
                let mut doc = Document::new();
                let nz = if mirrored { -1.0 } else { 1.0 };
                let raw = match g {
                    Geometry2::Line(_) => EntityType::Line(acadrust::Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0)),
                    Geometry2::Circle(_) => { let mut c = acadrust::Circle::new(); c.radius = 1.0; c.normal = Vector3::new(0.0, 0.0, nz); EntityType::Circle(c) }
                    Geometry2::Arc(_) => codec_arc((0.0, 0.0, 0.0), 1.0, 0.0, 1.0, nz),
                };
                let id = add_raw(&mut doc, raw);
                doc.set_plan_geometry(id, plan(g, z)).unwrap();
                let back = doc.entity(id).unwrap().plan().unwrap();
                let tol = Tolerance::new(1e-12 * (1.0 + 2e6), 1e-9).unwrap();
                for t in [0.0, 0.37, 1.0] {
                    prop_assert!(back.geometry().point_at(t).approx_eq(g.point_at(t), tol));
                }
                prop_assert!((back.elevation() - z).abs() <= 1e-9 * (1.0 + z.abs()));
                prop_assert_eq!(back.geometry().kind(), g.kind());
            }
        }
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;
