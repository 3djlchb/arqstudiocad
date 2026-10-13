//! Tests of the own internal model (a submodule of `model`).

use acadrust::EntityType;
use arq_geometry::{Arc, Circle, Geometry2, Line, Point2, Tolerance, Transform2, Vector2};

use crate::document::Document;
use crate::entity::{Color, EntityId, PlanGeometry};
use crate::layers::LayerId;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn plan(g: impl Into<Geometry2>, z: f64) -> PlanGeometry {
    PlanGeometry::at_elevation(g.into(), z).unwrap()
}

/// A drawing built through the API: two extra layers, one of them current,
/// colored, locked and not plotted; a line, a circle and an arc on them.
fn sample() -> (Document, LayerId, Vec<EntityId>) {
    let mut doc = Document::new();
    let walls = doc.create_layer("A-MUROS").unwrap();
    let doors = doc.create_layer("A-PUERTAS").unwrap();
    doc.set_layer_color(walls, Color::Index(1)).unwrap();
    doc.set_layer_plottable(doors, false).unwrap();
    doc.set_layer_locked(doors, true).unwrap();
    doc.set_current_layer(walls).unwrap();
    let ids = vec![
        doc.add_entity(
            plan(Line::new(p(0.0, 0.0), p(10.0, 2.5)).unwrap(), 3.0),
            walls,
        )
        .unwrap(),
        doc.add_entity(
            plan(Circle::new(p(500_000.5, 9_977_000.25), 1.75).unwrap(), 0.0),
            doors,
        )
        .unwrap(),
        doc.add_entity(
            plan(Arc::new(p(4.0, 4.0), 2.0, 0.5, 2.5).unwrap(), 0.0),
            walls,
        )
        .unwrap(),
    ];
    doc.set_entity_color(
        ids[1],
        Color::Rgb {
            r: 200,
            g: 120,
            b: 40,
        },
    )
    .unwrap();
    (doc, walls, ids)
}

#[test]
fn the_model_owns_layers_and_editable_entities() {
    let (doc, walls, ids) = sample();
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
    assert_eq!(doc.model.current, walls);
    assert_eq!(doc.model.layers.len(), 3);
    assert_eq!(doc.model.entities.len(), 3);
    let line = doc.model.entities[&ids[0]];
    assert_eq!((line.layer, line.color), (walls, Color::ByLayer));
    assert_eq!(line.plan.elevation(), 3.0);
    assert_eq!(
        doc.model.entities[&ids[1]].color,
        Color::Rgb {
            r: 200,
            g: 120,
            b: 40
        }
    );
}

#[test]
fn reads_come_from_the_model() {
    let (mut doc, _, ids) = sample();
    // Change the engine's record behind the model's back.
    if let Some(EntityType::Line(l)) = doc.cad_mut().get_entity_mut(acadrust::Handle::new(ids[0])) {
        l.end.x = 99.0;
    }
    let seen = doc.entity(ids[0]).unwrap().plan().unwrap().geometry();
    assert_eq!(
        seen.end_point(),
        Some(p(10.0, 2.5)),
        "the model's line, not the engine's"
    );
    assert_eq!(
        doc.check_model(),
        [format!("entity {}: not as the engine has it", ids[0])]
    );
}

#[test]
fn layer_reads_come_from_the_model() {
    let (mut doc, walls, _) = sample();
    if let Some(layer) = doc
        .cad_mut()
        .layers
        .iter_mut()
        .find(|l| l.handle.value() == walls)
    {
        layer.flags.off = true;
    }
    doc.cad_mut().header.current_layer_name = "0".to_owned();
    assert!(doc.layer(walls).unwrap().is_visible(), "the model's layer");
    assert!(doc.layer_by_name("a-muros").unwrap().is_visible());
    assert!(
        doc.layers()
            .iter()
            .any(|l| l.id() == walls && l.is_visible())
    );
    assert_eq!(doc.current_layer().id(), walls, "the model's current layer");
    let problems = doc.check_model();
    assert!(
        problems.iter().any(|p| p.starts_with("current layer")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p == &format!("layer {walls}: not as the engine has it"))
    );
}

#[test]
fn copies_join_the_model() {
    let (mut doc, walls, ids) = sample();
    let copy = doc.copy_entity(ids[2]).unwrap();
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
    let (a, b) = (doc.model.entities[&ids[2]], doc.model.entities[&copy]);
    assert_eq!((b.layer, b.plan), (walls, a.plan));
}

#[test]
fn renaming_a_layer_renames_it_for_its_entities() {
    let (mut doc, walls, ids) = sample();
    doc.rename_layer(walls, "A-MUROS-EXT").unwrap();
    assert_eq!(doc.entity(ids[0]).unwrap().layer(), "A-MUROS-EXT");
    assert_eq!(doc.current_layer().name(), "A-MUROS-EXT");
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
}

#[test]
fn undo_and_redo_keep_the_model_in_step() {
    let (mut doc, walls, ids) = sample();
    let moved = Transform2::translation(Vector2::new(1.0, 1.0).unwrap());
    doc.begin_change().unwrap();
    doc.transform_entity(ids[0], moved).unwrap();
    doc.delete_entity(ids[2]).unwrap();
    doc.set_layer_color(walls, Color::Index(5)).unwrap();
    let extra = doc.create_layer("EXTRA").unwrap();
    doc.set_current_layer(extra).unwrap();
    let mut change = doc.commit_change().unwrap();
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());

    doc.undo_change(&mut change).unwrap();
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
    let (fresh, _, _) = sample();
    assert_eq!(doc.model, fresh.model, "undo restores the model exactly");

    doc.redo_change(&mut change).unwrap();
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
    assert!(!doc.model.entities.contains_key(&ids[2]));
    assert_eq!(doc.model.current, extra);
    assert_eq!(doc.model.layers[&walls].color(), Color::Index(5));
}

#[test]
fn what_the_model_does_not_own_stays_in_the_engine() {
    let (mut doc, _, ids) = sample();
    // Linetype and thickness are not owned: they survive an edit untouched.
    if let Some(EntityType::Line(l)) = doc.cad_mut().get_entity_mut(acadrust::Handle::new(ids[0])) {
        l.common.linetype = "DASHED".to_owned();
        l.thickness = 0.25;
    }
    let shifted = Line::new(p(1.0, 1.0), p(11.0, 3.5)).unwrap();
    doc.set_plan_geometry(ids[0], plan(shifted, 3.0)).unwrap();
    let Some(EntityType::Line(l)) = doc.cad().get_entity(acadrust::Handle::new(ids[0])) else {
        panic!("not a line")
    };
    assert_eq!((l.common.linetype.as_str(), l.thickness), ("DASHED", 0.25));
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
}

/// The technical MVP's "save and recover the working document from the own
/// model" (master plan §15, ADR 0018).
#[test]
fn the_working_document_is_saved_and_recovered_from_the_own_model() {
    let (doc, _, ids) = sample();
    let dir = tempfile::tempdir().unwrap();
    let tol = Tolerance::new(1e-9, 1e-9).unwrap();
    for name in ["casa.dwg", "casa.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        assert!(
            back.check_model().is_empty(),
            "{name}: {:?}",
            back.check_model()
        );
        assert_eq!(back.model.current, doc.model.current, "{name}");
        assert_eq!(back.model.layers, doc.model.layers, "{name}");
        for id in &ids {
            let (a, b) = (doc.model.entities[id], back.model.entities[id]);
            assert_eq!(
                (a.layer, a.color),
                (b.layer, b.color),
                "{name}: entity {id}"
            );
            assert!(
                (a.plan.elevation() - b.plan.elevation()).abs() < 1e-9,
                "{name}"
            );
            for t in [0.0, 0.5, 1.0] {
                let (pa, pb) = (a.plan.geometry().point_at(t), b.plan.geometry().point_at(t));
                assert!(pa.approx_eq(pb, tol), "{name}: entity {id} at {t}");
            }
        }
        assert_eq!(
            back.model.entities.len(),
            doc.model.entities.len(),
            "{name}"
        );
    }
}
