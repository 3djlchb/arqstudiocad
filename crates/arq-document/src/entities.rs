//! Entity access on [`Document`].

use acadrust::{EntityType, Handle};
use arq_geometry::{Bounds2, Transform2};

use crate::document::Document;
use crate::entity::Color;
use crate::entity::{Entity, EntityId, PlanGeometry};
use crate::error::{DocumentError, DocumentResult};
use crate::layers::LayerId;
use crate::translate::{
    bounds_of, color_to_codec, handle, is_whole_transformable, new_entity, read_entity,
    transform_whole, write_plan,
};

impl Document {
    /// A snapshot of the entity with this id, in any space.
    pub fn entity(&self, id: EntityId) -> Option<Entity> {
        self.cad().get_entity(handle(id)).map(read_entity)
    }

    /// Snapshots of every entity in model space, in drawing order.
    pub fn entities(&self) -> Vec<Entity> {
        self.cad().model_space_entities().map(read_entity).collect()
    }

    /// The plan bounds of every model-space entity that has them, in drawing
    /// order: a cheap pass for spatial queries (no layer names copied).
    pub fn entity_bounds(&self) -> impl Iterator<Item = (EntityId, Bounds2)> + '_ {
        self.cad()
            .model_space_entities()
            .filter_map(|e| Some((e.as_entity().handle().value(), bounds_of(e)?)))
    }

    /// Adds plan geometry to model space on `layer`; returns its id.
    pub fn add_entity(&mut self, plan: PlanGeometry, layer: LayerId) -> DocumentResult<EntityId> {
        let name = self
            .layer(layer)
            .ok_or(DocumentError::LayerIdNotFound(layer))?
            .name()
            .to_owned();
        let h = self.cad_mut().add_entity(new_entity(plan, &name))?;
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
        write_plan(e, plan)
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
        let mut copy = original.clone();
        let common = copy.common_mut();
        common.handle = Handle::NULL;
        common.owner_handle = Handle::NULL;
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
        Ok(h.value())
    }

    /// Deletes an entity of any type (erasing needs no geometry, D3.4).
    ///
    /// Uses the codec's shared-image removal, which keeps block membership so
    /// undo can restore it exactly.
    pub fn delete_entity(&mut self, id: EntityId) -> DocumentResult<()> {
        self.cad_mut()
            .remove_entity_arc(handle(id))
            .map(|_| ())
            .ok_or(DocumentError::EntityNotFound(id))
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
        Ok(())
    }
}

/// Test helpers for other crates (feature `test-support`).
#[cfg(feature = "test-support")]
impl Document {
    /// Inserts a TEXT entity on layer "0", as a drawing from another CAD
    /// would contain; TEXT is read-only in this version.
    pub fn insert_text_for_tests(&mut self, value: &str, x: f64, y: f64) -> EntityId {
        let text = acadrust::Text::with_value(value, acadrust::types::Vector3::new(x, y, 0.0));
        self.cad_mut()
            .add_entity(acadrust::EntityType::Text(text))
            .expect("adding a text")
            .value()
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
