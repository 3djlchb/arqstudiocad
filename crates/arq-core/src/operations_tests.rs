//! Tests of the editing operations (a submodule of `operations`).
//!
//! Expected geometry is worked out by hand, not with `Transform2`, so a bug
//! shared by the transform and its use here cannot hide.

use std::f64::consts::FRAC_PI_2;

use arq_geometry::{Geometry2, Tolerance};

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

fn geometry(cad: &CadDocument, id: EntityId) -> Geometry2 {
    cad.document()
        .entity(id)
        .and_then(|e| e.plan())
        .unwrap()
        .geometry()
}

fn near(a: Point2, b: Point2) -> bool {
    a.approx_eq(b, Tolerance::default())
}

fn line_ends(cad: &CadDocument, id: EntityId) -> (Point2, Point2) {
    let g = geometry(cad, id);
    (g.start_point().unwrap(), g.end_point().unwrap())
}

#[test]
fn new_entities_go_to_the_current_layer() {
    let mut cad = CadDocument::new();
    let (walls, _) = cad.create_layer("A-MUROS").unwrap();
    cad.set_current_layer(walls).unwrap();
    let (l, made) = cad.add_line(p(0.0, 0.0), p(3.0, 4.0)).unwrap();
    let (c, _) = cad.add_circle(p(1.0, 1.0), 2.0).unwrap();
    let (a, _) = cad.add_arc(p(0.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
    assert_eq!(made.created(), [l]);
    for (id, kind) in [(l, "LINE"), (c, "CIRCLE"), (a, "ARC")] {
        let e = cad.document().entity(id).unwrap();
        assert_eq!((e.kind(), e.layer()), (kind, "A-MUROS"));
    }
    assert_eq!(cad.undo_name(), Some("ARC"));
}

#[test]
fn invalid_geometry_is_rejected_before_anything_changes() {
    let mut cad = CadDocument::new();
    assert!(matches!(
        cad.add_line(p(1.0, 1.0), p(1.0, 1.0)),
        Err(CadError::InvalidGeometry(_))
    ));
    assert!(matches!(
        cad.add_circle(p(0.0, 0.0), -1.0),
        Err(CadError::InvalidGeometry(_))
    ));
    assert!(matches!(
        cad.rotate_entities(&[], p(0.0, 0.0), f64::NAN),
        Err(CadError::InvalidGeometry(_))
    ));
    assert!(cad.document().entities().is_empty() && !cad.can_undo());
}

#[test]
fn move_rotate_scale_and_mirror_land_where_expected() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(1.0, 0.0), p(2.0, 0.0)).unwrap();

    cad.move_entities(&[id], v(1.0, 2.0)).unwrap();
    let (s, e) = line_ends(&cad, id);
    assert!(near(s, p(2.0, 2.0)) && near(e, p(3.0, 2.0)));

    // 90° counterclockwise around (2, 2): (x, y) → (2 − (y − 2), 2 + (x − 2)).
    cad.rotate_entities(&[id], p(2.0, 2.0), FRAC_PI_2).unwrap();
    let (s, e) = line_ends(&cad, id);
    assert!(near(s, p(2.0, 2.0)) && near(e, p(2.0, 3.0)), "{s:?} {e:?}");

    cad.scale_entities(&[id], p(2.0, 0.0), 2.0).unwrap();
    let (s, e) = line_ends(&cad, id);
    assert!(near(s, p(2.0, 4.0)) && near(e, p(2.0, 6.0)), "{s:?} {e:?}");

    // Across the X axis: y → −y.
    let axis = Line::new(p(-1.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.mirror_entities(&[id], axis).unwrap();
    let (s, e) = line_ends(&cad, id);
    assert!(
        near(s, p(2.0, -4.0)) && near(e, p(2.0, -6.0)),
        "{s:?} {e:?}"
    );
}

#[test]
fn mirroring_an_arc_keeps_it_counterclockwise() {
    let mut cad = CadDocument::new();
    // Quarter arc from East (1, 0) to North (0, 1).
    let (id, _) = cad.add_arc(p(0.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
    let axis = Line::new(p(0.0, -1.0), p(0.0, 1.0)).unwrap(); // x → −x
    cad.mirror_entities(&[id], axis).unwrap();
    // The image runs from North (0, 1) to West (−1, 0), counterclockwise.
    let (s, e) = line_ends(&cad, id);
    assert!(near(s, p(0.0, 1.0)) && near(e, p(-1.0, 0.0)), "{s:?} {e:?}");
}

#[test]
fn editing_a_read_only_entity_fails_the_whole_operation() {
    let mut doc = Document::new();
    let text = doc.insert_text_for_tests("NOTA", 5.0, 5.0);
    let mut cad = CadDocument::from_document(doc);
    let (line, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.undo().unwrap();
    cad.redo().unwrap();
    let before = line_ends(&cad, line);
    let err = cad.move_entities(&[line, text], v(1.0, 0.0)).unwrap_err();
    assert!(matches!(err, CadError::InvalidOperation(_)), "{err:?}");
    assert_eq!(line_ends(&cad, line), before, "the line moved back");
    assert_eq!(cad.undo_name(), Some("LINE"), "no step for the failed MOVE");
    // Erasing needs no geometry, so it works on any type.
    assert_eq!(cad.delete_entities(&[text]).unwrap().deleted(), [text]);
}

#[test]
fn locked_layers_protect_their_entities() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let zero = cad.document().current_layer().id();
    cad.set_layer_locked(zero, true).unwrap();
    for result in [
        cad.move_entities(&[id], v(1.0, 0.0)),
        cad.delete_entities(&[id]),
        cad.set_entity_color(&[id], Color::Index(1)),
    ] {
        assert!(matches!(result, Err(CadError::InvalidOperation(_))));
    }
    assert!(cad.document().entity(id).is_some());
    // Drawing on a locked current layer is allowed.
    cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    cad.set_layer_locked(zero, false).unwrap();
    cad.move_entities(&[id], v(1.0, 0.0)).unwrap();
}

#[test]
fn repeated_ids_are_edited_once() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let moved = cad.move_entities(&[id, id, id], v(1.0, 0.0)).unwrap();
    assert_eq!(moved.modified(), [id]);
    assert!(near(line_ends(&cad, id).0, p(1.0, 0.0)), "moved once");
    assert_eq!(cad.delete_entities(&[id, id]).unwrap().deleted(), [id]);
}

#[test]
fn properties_change_and_undo() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (b, _) = cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    let (walls, _) = cad.create_layer("A-MUROS").unwrap();
    let set = cad.set_entity_layer(&[a, b], walls).unwrap();
    assert_eq!(set.modified(), [a, b]);
    cad.set_entity_color(&[a], Color::Rgb { r: 1, g: 2, b: 3 })
        .unwrap();
    assert_eq!(cad.undo_name(), Some("CHPROP"));
    let doc = cad.document();
    assert_eq!(doc.entity(b).unwrap().layer(), "A-MUROS");
    assert_eq!(
        doc.entity(a).unwrap().color(),
        Color::Rgb { r: 1, g: 2, b: 3 }
    );
    cad.undo().unwrap();
    cad.undo().unwrap();
    let doc = cad.document();
    assert_eq!(doc.entity(a).unwrap().color(), Color::ByLayer);
    assert_eq!(doc.entity(a).unwrap().layer(), "0");
    assert_eq!(
        cad.set_entity_layer(&[a], 777_777).unwrap_err(),
        CadError::LayerNotFound(777_777)
    );
}

#[test]
fn layer_operations_report_and_undo() {
    let mut cad = CadDocument::new();
    let (l, created) = cad.create_layer("A-PUERTAS").unwrap();
    assert_eq!(created.layers_created(), [l]);
    assert_eq!(
        cad.rename_layer(l, "A-PUERTA").unwrap().layers_modified(),
        [l]
    );
    cad.set_layer_color(l, Color::Index(3)).unwrap();
    cad.set_layer_visible(l, false).unwrap();
    cad.set_layer_plottable(l, false).unwrap();
    cad.set_layer_frozen(l, true).unwrap();
    assert!(matches!(
        cad.set_current_layer(l),
        Err(CadError::InvalidOperation(_))
    ));
    cad.set_layer_frozen(l, false).unwrap();
    assert!(cad.set_current_layer(l).unwrap().document_changed());
    let layer = cad.document().layer(l).unwrap();
    assert_eq!(
        (
            layer.name(),
            layer.color(),
            layer.is_visible(),
            layer.is_plottable()
        ),
        ("A-PUERTA", Color::Index(3), false, false)
    );
    assert!(cad.delete_layer(l).is_err(), "it is current");
    let zero = cad.document().layer_by_name("0").unwrap().id();
    cad.set_current_layer(zero).unwrap();
    assert_eq!(cad.delete_layer(l).unwrap().layers_deleted(), [l]);
    assert_eq!(cad.undo_name(), Some("LAYER"));
    while cad.can_undo() {
        cad.undo().unwrap();
    }
    assert!(cad.document().layer(l).is_none());
    assert_eq!(cad.delete_layer(l).unwrap_err(), CadError::LayerNotFound(l));
}

#[test]
fn color_index_zero_is_an_invalid_operation() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let err = cad.set_entity_color(&[id], Color::Index(0)).unwrap_err();
    assert!(matches!(err, CadError::InvalidOperation(_)), "{err:?}");
    let err = CadError::from(Color::index(0).unwrap_err());
    assert!(matches!(err, CadError::InvalidOperation(_)), "{err:?}");
    assert_eq!(Color::index(255).unwrap(), Color::Index(255));
    assert_eq!(
        cad.undo_name(),
        Some("LINE"),
        "no step for the failed CHPROP"
    );
}
