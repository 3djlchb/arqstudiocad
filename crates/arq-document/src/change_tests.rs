//! Tests of change recording, undo and redo (a submodule of `change`).

use arq_geometry::{Circle, Line, Point2};

use super::*;
use crate::entity::{Color, PlanGeometry};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(a: (f64, f64), b: (f64, f64)) -> PlanGeometry {
    PlanGeometry::new(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap().into())
}

fn zero(doc: &Document) -> LayerId {
    doc.layer_by_name("0").unwrap().id()
}

/// Records `f` as one change.
fn record(doc: &mut Document, f: impl FnOnce(&mut Document)) -> Change {
    doc.begin_change().unwrap();
    f(doc);
    doc.commit_change().unwrap()
}

/// Everything a user can observe, for "nothing changed" checks.
fn state(doc: &Document) -> (Vec<crate::Entity>, Vec<crate::Layer>, LayerId) {
    (doc.entities(), doc.layers(), doc.current_layer().id())
}

#[test]
fn modify_undo_redo() {
    let mut doc = Document::new();
    let id = doc
        .add_entity(line((0.0, 0.0), (1.0, 0.0)), zero(&doc))
        .unwrap();
    let before = doc.entity(id).unwrap();
    let mut c = record(&mut doc, |d| {
        d.set_plan_geometry(id, line((5.0, 5.0), (9.0, 5.0)))
            .unwrap()
    });
    assert_eq!(c.summary().modified(), [id]);
    let after = doc.entity(id).unwrap();
    assert_eq!(doc.undo_change(&mut c).unwrap().modified(), [id]);
    assert_eq!(doc.entity(id).unwrap(), before);
    doc.redo_change(&mut c).unwrap();
    assert_eq!(doc.entity(id).unwrap(), after);
}

#[test]
fn create_and_delete_undo_redo() {
    let mut doc = Document::new();
    let l0 = zero(&doc);
    let mut id = 0;
    let mut add = record(&mut doc, |d| {
        id = d.add_entity(line((0.0, 0.0), (1.0, 1.0)), l0).unwrap()
    });
    assert_eq!(add.summary().created(), [id]);
    assert_eq!(doc.undo_change(&mut add).unwrap().deleted(), [id]);
    assert!(doc.entity(id).is_none() && doc.entities().is_empty());
    doc.redo_change(&mut add).unwrap();
    assert!(doc.entity(id).is_some(), "same id after redo");
    let mut del = record(&mut doc, |d| d.delete_entity(id).unwrap());
    assert_eq!(del.summary().deleted(), [id]);
    assert!(doc.entity(id).is_none());
    doc.undo_change(&mut del).unwrap();
    assert_eq!(doc.entities().len(), 1, "back in model space");
}

#[test]
fn ids_are_never_reused_after_undo() {
    let mut doc = Document::new();
    let l0 = zero(&doc);
    let mut first = 0;
    let mut c = record(&mut doc, |d| {
        first = d.add_entity(line((0.0, 0.0), (1.0, 0.0)), l0).unwrap();
        d.create_layer("NUEVA").unwrap(); // structure snapshot too
    });
    doc.undo_change(&mut c).unwrap();
    let second = doc.add_entity(line((0.0, 0.0), (2.0, 0.0)), l0).unwrap();
    assert_ne!(first, second);
    let layer = doc.create_layer("OTRA").unwrap();
    assert_ne!(layer, first);
}

#[test]
fn layers_undo_redo() {
    let mut doc = Document::new();
    let mut id = 0;
    let mut c = record(&mut doc, |d| {
        id = d.create_layer("A-MUROS").unwrap();
        d.set_layer_color(id, Color::Index(1)).unwrap();
    });
    assert_eq!(c.summary().layers_created(), [id]);
    assert!(
        c.summary().layers_modified().is_empty(),
        "created wins over modified"
    );
    doc.undo_change(&mut c).unwrap();
    assert!(doc.layer(id).is_none());
    doc.redo_change(&mut c).unwrap();
    assert_eq!(doc.layer(id).unwrap().color(), Color::Index(1));
}

#[test]
fn renaming_a_layer_is_undone_with_its_entities() {
    let mut doc = Document::new();
    let walls = doc.create_layer("muros").unwrap();
    let e = doc.add_entity(line((0.0, 0.0), (1.0, 0.0)), walls).unwrap();
    let other = doc
        .add_entity(line((0.0, 1.0), (1.0, 1.0)), zero(&doc))
        .unwrap();
    let mut c = record(&mut doc, |d| d.rename_layer(walls, "A-MUROS").unwrap());
    assert_eq!(c.summary().layers_modified(), [walls]);
    assert_eq!(c.summary().modified(), [e], "only the entity on that layer");
    doc.undo_change(&mut c).unwrap();
    assert_eq!(doc.layer(walls).unwrap().name(), "muros");
    assert_eq!(doc.entity(e).unwrap().layer(), "muros");
    assert_eq!(doc.entity(other).unwrap().layer(), "0");
}

#[test]
fn current_layer_is_a_document_change() {
    let mut doc = Document::new();
    let work = doc.create_layer("TRABAJO").unwrap();
    let mut c = record(&mut doc, |d| d.set_current_layer(work).unwrap());
    assert!(c.summary().document_changed());
    doc.undo_change(&mut c).unwrap();
    assert_eq!(doc.current_layer().name(), "0");
}

#[test]
fn rollback_restores_everything() {
    let mut doc = Document::new();
    let id = doc
        .add_entity(line((0.0, 0.0), (1.0, 0.0)), zero(&doc))
        .unwrap();
    let before = state(&doc);
    doc.begin_change().unwrap();
    doc.set_plan_geometry(id, line((3.0, 3.0), (4.0, 3.0)))
        .unwrap();
    let l = doc.create_layer("TEMP").unwrap();
    let circle = PlanGeometry::new(Circle::new(p(0.0, 0.0), 2.0).unwrap().into());
    doc.add_entity(circle, l).unwrap();
    doc.set_entity_color(id, Color::Index(5)).unwrap();
    doc.delete_entity(id).unwrap();
    doc.rollback_change().unwrap();
    assert_eq!(state(&doc), before);
}

#[test]
fn files_stay_valid_after_undo_and_redo() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::new();
    let l0 = zero(&doc);
    let keep = doc.add_entity(line((0.0, 0.0), (1.0, 0.0)), l0).unwrap();
    let mut add = record(&mut doc, |d| {
        d.add_entity(line((0.0, 2.0), (1.0, 2.0)), l0).unwrap();
    });
    let mut del = record(&mut doc, |d| d.delete_entity(keep).unwrap());
    doc.undo_change(&mut del).unwrap();
    doc.undo_change(&mut add).unwrap();
    for (step, expected) in [("undone", 1), ("redone", 2)] {
        if step == "redone" {
            doc.redo_change(&mut add).unwrap();
        }
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("{step}.{ext}"));
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            assert_eq!(back.entities().len(), expected, "{step}.{ext}");
            assert!(back.entity(keep).is_some(), "{step}.{ext}");
        }
    }
}

#[test]
fn misuse_is_an_error() {
    let mut doc = Document::new();
    assert!(matches!(
        doc.commit_change(),
        Err(DocumentError::NoChangeInProgress)
    ));
    doc.begin_change().unwrap();
    assert!(matches!(
        doc.begin_change(),
        Err(DocumentError::ChangeInProgress)
    ));
    let mut empty = Change {
        images: vec![],
        structure: None,
        summary: ChangeSet::default(),
    };
    assert!(matches!(
        doc.undo_change(&mut empty),
        Err(DocumentError::ChangeInProgress)
    ));
    assert!(doc.commit_change().unwrap().summary().is_empty());
}

#[test]
fn merging_change_sets() {
    let set = |c: &[u64], m: &[u64], d: &[u64]| ChangeSet {
        created: c.to_vec(),
        modified: m.to_vec(),
        deleted: d.to_vec(),
        ..ChangeSet::default()
    };
    // created then modified → created; created then deleted → nothing;
    // modified then deleted → deleted.
    let a = set(&[1, 2], &[3], &[]);
    let b = set(&[], &[1, 3], &[2, 3]);
    assert_eq!(a.then(&b), set(&[1], &[], &[3]));
    assert_eq!(set(&[], &[], &[7]).inverted(), set(&[7], &[], &[]));
}
