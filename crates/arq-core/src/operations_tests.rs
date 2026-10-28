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
    let text = doc.insert_point_for_tests(5.0, 5.0); // read-only (ADR 0022 made texts transformable)
    let mut cad = CadDocument::from_document(doc);
    let (line, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.undo().unwrap();
    cad.redo().unwrap();
    let before = line_ends(&cad, line);
    let err = cad.move_entities(&[line, text], v(1.0, 0.0)).unwrap_err();
    assert!(
        matches!(&err, CadError::InvalidOperation(m) if m.contains("cannot be transformed")),
        "{err:?}"
    );
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
fn new_geometry_of_the_same_kind_keeps_the_entity() {
    let mut doc = Document::new();
    let text = doc.insert_text_for_tests("NOTA", 5.0, 5.0);
    let mut cad = CadDocument::from_document(doc);
    let (walls, _) = cad.create_layer("A-MUROS").unwrap();
    cad.set_current_layer(walls).unwrap();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let changed = cad
        .set_entity_geometry(line, Line::new(p(0.0, 0.0), p(3.0, 4.0)).unwrap().into())
        .unwrap();
    assert_eq!(changed.modified(), [line]);
    assert!(near(line_ends(&cad, line).1, p(3.0, 4.0)));
    assert_eq!(cad.document().entity(line).unwrap().layer(), "A-MUROS");
    assert_eq!(cad.undo_name(), Some("EDIT"));
    let circle: Geometry2 = Circle::new(p(0.0, 0.0), 1.0).unwrap().into();
    // A line cannot become a circle; a text has no geometry to edit yet.
    for id in [line, text] {
        let err = cad.set_entity_geometry(id, circle).unwrap_err();
        assert!(matches!(err, CadError::InvalidOperation(_)), "{err:?}");
    }
    cad.undo().unwrap();
    assert!(near(line_ends(&cad, line).1, p(1.0, 0.0)), "one undo step");
}

#[test]
fn texts_are_written_and_edited_all_or_nothing() {
    let mut cad = CadDocument::new();
    let (a, made) = cad.add_text("A", p(0.0, 0.0), 2.0, 0.0).unwrap();
    assert_eq!(made.created(), [a]);
    assert_eq!(cad.undo_name(), Some("TEXT"));
    let (b, _) = cad.add_text("B", p(0.0, 5.0), 2.0, 0.0).unwrap();
    let changed = cad.set_text(&[a, b], None, Some(4.0), None).unwrap();
    assert_eq!(changed.modified(), [a, b]);
    assert_eq!(cad.undo_name(), Some("PROPERTIES"));
    let height =
        |cad: &CadDocument, id| cad.document().entity(id).unwrap().text().unwrap().height();
    assert!((height(&cad, b) - 4.0).abs() < 1e-12);
    let (line, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    assert!(cad.set_text(&[a, line], None, Some(9.0), None).is_err());
    assert!(
        (height(&cad, a) - 4.0).abs() < 1e-12,
        "the text edited first went back"
    );
    let moved = cad.move_entities(&[a], v(1.0, 1.0)).unwrap();
    assert_eq!(moved.modified(), [a], "texts transform as a whole");
    for _ in 0..3 {
        cad.undo().unwrap(); // MOVE, LINE, PROPERTIES
    }
    assert!((height(&cad, a) - 2.0).abs() < 1e-12);
}

#[test]
fn block_and_insert_undo_and_redo_whole() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (b, _) = cad.add_line(p(0.0, 0.0), p(0.0, 2.0)).unwrap();
    let (reference, made) = cad.create_block("ESQUINA", p(0.0, 0.0), &[a, b]).unwrap();
    assert_eq!(made.created(), [reference]);
    assert_eq!(made.deleted(), [a, b]);
    assert_eq!(cad.undo_name(), Some("BLOCK"));
    let (placed, _) = cad.insert_block("ESQUINA", p(5.0, 5.0), 1.0, 0.0).unwrap();
    assert_eq!(cad.undo_name(), Some("INSERT"));
    assert_eq!(cad.document().entities().len(), 2);

    cad.undo().unwrap();
    assert!(cad.document().entity(placed).is_none());
    cad.undo().unwrap();
    assert!(cad.document().block_names().is_empty(), "the block is gone");
    assert!(cad.document().entity(a).is_some() && cad.document().entity(b).is_some());
    cad.redo().unwrap();
    assert_eq!(cad.document().block_names(), ["ESQUINA"]);
    assert!(cad.document().entity(a).is_none());

    // Entities on a locked layer cannot go into a block.
    let (c, _) = cad.add_line(p(9.0, 9.0), p(9.0, 8.0)).unwrap();
    let zero = cad.document().current_layer().id();
    cad.set_layer_locked(zero, true).unwrap();
    assert!(matches!(
        cad.create_block("BLOQUEADO", p(0.0, 0.0), &[c]),
        Err(CadError::InvalidOperation(_))
    ));
    assert_eq!(cad.document().block_names(), ["ESQUINA"], "nothing made");
}

#[test]
fn defining_a_block_is_one_undo_step_in_every_way() {
    let definition = BlockDefinition {
        description: "Esquina".to_owned(),
        units: 6,
        scale_uniformly: false,
        explodable: true,
    };
    for objects in [
        BlockObjects::Retain,
        BlockObjects::Convert,
        BlockObjects::Delete,
    ] {
        let mut cad = CadDocument::new();
        let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
        let (placed, made) = cad
            .define_block("ESQUINA", p(0.0, 0.0), &[a, a], objects, &definition, false)
            .unwrap();
        assert_eq!(cad.undo_name(), Some("BLOCK"), "{objects:?}");
        assert_eq!(
            placed.is_some(),
            objects == BlockObjects::Convert,
            "{objects:?}"
        );
        assert_eq!(
            made.deleted().contains(&a),
            objects != BlockObjects::Retain,
            "{objects:?}"
        );
        cad.undo().unwrap();
        assert!(
            cad.document().block_names().is_empty(),
            "{objects:?}: the block is gone"
        );
        assert!(
            cad.document().entity(a).is_some(),
            "{objects:?}: the line is back"
        );
    }
    // Redefining is one step too, undone to the block as it was.
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (reference, _) = cad.create_block("ESQUINA", p(0.0, 0.0), &[a]).unwrap();
    let (b, _) = cad.add_line(p(0.0, 0.0), p(0.0, 5.0)).unwrap();
    let box_of = |c: &CadDocument| c.document().entity(reference).unwrap().bounds().unwrap();
    let before = box_of(&cad);
    let (_, redefined) = cad
        .define_block(
            "ESQUINA",
            p(0.0, 0.0),
            &[b],
            BlockObjects::Delete,
            &definition,
            true,
        )
        .unwrap();
    assert_eq!(cad.undo_name(), Some("BLOCK"));
    assert!(
        redefined.modified().contains(&reference),
        "the reference is drawn and indexed again"
    );
    assert_ne!(box_of(&cad), before, "the reference shows the new contents");
    cad.undo().unwrap();
    assert_eq!(box_of(&cad), before);
    assert_eq!(
        cad.document().block_info("ESQUINA").unwrap().description,
        ""
    );
}

#[test]
fn renaming_a_block_is_one_undo_step() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (reference, _) = cad.create_block("ESQUINA", p(0.0, 0.0), &[a]).unwrap();
    let renamed = cad.rename_block("esquina", "Rincón").unwrap();
    assert_eq!(cad.undo_name(), Some("RENAME"));
    assert_eq!(renamed.blocks_created(), ["Rincón"]);
    assert_eq!(renamed.blocks_deleted(), ["ESQUINA"]);
    assert!(renamed.modified().contains(&reference));
    cad.undo().unwrap();
    assert_eq!(cad.document().block_names(), ["ESQUINA"]);
    assert_eq!(cad.undo_name(), Some("BLOCK"));
    // A refused name changes nothing and records no step.
    assert!(matches!(
        cad.rename_block("ESQUINA", "A/B"),
        Err(CadError::InvalidOperation(_))
    ));
    assert_eq!(cad.undo_name(), Some("BLOCK"));
}

#[test]
fn purging_blocks_is_one_undo_step() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (reference, _) = cad.create_block("ESQUINA", p(0.0, 0.0), &[a]).unwrap();
    cad.delete_entities(&[reference]).unwrap();
    let (purged, changes) = cad.purge_blocks(&["esquina".to_owned()], true).unwrap();
    assert_eq!(purged, ["ESQUINA"]);
    assert_eq!(cad.undo_name(), Some("PURGE"));
    assert_eq!(changes.blocks_deleted(), ["ESQUINA"]);
    assert!(cad.document().block_names().is_empty());
    cad.undo().unwrap();
    assert_eq!(cad.document().block_names(), ["ESQUINA"]);
    // A block in use is refused and records no step.
    assert!(matches!(
        cad.purge_blocks(&["NADA".to_owned()], true),
        Err(CadError::InvalidOperation(_))
    ));
    assert_eq!(cad.undo_name(), Some("ERASE"));
}

#[test]
fn block_data_and_currencies_are_one_undo_step_each() {
    use arq_document::{Currency, Money};
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    cad.create_block("MESA", p(0.0, 0.0), &[a]).unwrap();
    let mut register = Currencies::default();
    register.list.push(Currency {
        code: "PEN".to_owned(),
        name: "Peruvian sol".to_owned(),
        symbol: "S/".to_owned(),
        decimals: 2,
        active: true,
        format: None,
    });
    let changes = cad.set_currencies(&register).unwrap();
    assert_eq!(cad.undo_name(), Some("CURRENCIES"));
    assert!(changes.document_changed());
    let data = BlockData {
        category: "Mobiliario".to_owned(),
        cost: Some(Money::parse("450", 2, "PEN").unwrap()),
        ..BlockData::default()
    };
    let (name, changes) = cad.set_block_data("mesa", &data).unwrap();
    assert_eq!(name, "MESA");
    assert_eq!(cad.undo_name(), Some("BLOCKDATA"));
    assert_eq!(changes.blocks_modified(), ["MESA"]);
    assert_eq!(cad.document().block_data("MESA").unwrap(), Some(data));
    // Refused: PEN is in use; no step is recorded.
    assert!(matches!(
        cad.set_currencies(&Currencies::default()),
        Err(CadError::InvalidOperation(m)) if m.starts_with("PEN is used by 1 block")
    ));
    assert_eq!(cad.undo_name(), Some("BLOCKDATA"));
    cad.undo().unwrap();
    assert_eq!(cad.document().block_data("MESA").unwrap(), None);
    cad.undo().unwrap();
    assert_eq!(cad.document().currencies().unwrap(), Currencies::default());
}

#[test]
fn saving_a_block_editor_is_one_step_that_redraws_the_references() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (reference, _) = cad.create_block("ESQUINA", p(0.0, 0.0), &[a]).unwrap();
    let (loose, _) = cad.add_line(p(5.0, 5.0), p(6.0, 5.0)).unwrap();
    let mut editor = cad.block_editor("ESQUINA").unwrap();
    assert!(!editor.is_modified(), "a fresh editor");
    editor.add_line(p(0.0, 0.0), p(0.0, 3.0)).unwrap();
    let box_of = |c: &CadDocument| c.document().entity(reference).unwrap().bounds().unwrap();
    let before = box_of(&cad);
    let changes = cad
        .replace_block_contents("ESQUINA", editor.document())
        .unwrap();
    assert_eq!(
        changes.modified(),
        [reference],
        "the reference only, not {loose}"
    );
    assert!(changes.created().is_empty() && changes.deleted().is_empty());
    assert_eq!(cad.undo_name(), Some("BEDIT"));
    assert!(
        near(box_of(&cad).max(), p(2.0, 3.0)),
        "it draws the new line"
    );
    cad.undo().unwrap();
    assert_eq!(box_of(&cad), before, "undone whole");
    cad.redo().unwrap();
    assert!(near(box_of(&cad).max(), p(2.0, 3.0)));
}

#[test]
fn a_block_drawn_through_another_redraws_that_reference_too() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (inner, _) = cad.create_block("ESQUINA", p(0.0, 0.0), &[a]).unwrap();
    let (outer, _) = cad.create_block("MARCO", p(0.0, 0.0), &[inner]).unwrap();
    let (_, _) = cad.add_line(p(5.0, 5.0), p(6.0, 5.0)).unwrap();
    let mut editor = cad.block_editor("ESQUINA").unwrap();
    editor.add_line(p(0.0, 0.0), p(0.0, 3.0)).unwrap();
    let changes = cad
        .replace_block_contents("ESQUINA", editor.document())
        .unwrap();
    assert_eq!(changes.modified(), [outer], "MARCO draws ESQUINA");
}

#[test]
fn a_polyline_is_drawn_with_its_arcs_in_one_step() {
    let mut cad = CadDocument::new();
    // (0,0) to (2,0) with bulge 1: a half circle around (1,0), run
    // counterclockwise, so through (1,-1); then straight up to (2,3).
    let (id, made) = cad
        .add_polyline(
            &[(p(0.0, 0.0), 1.0), (p(2.0, 0.0), 0.0), (p(2.0, 3.0), 0.0)],
            false,
        )
        .unwrap();
    assert_eq!(made.created(), [id]);
    assert_eq!(cad.undo_name(), Some("PLINE"));
    let e = cad.document().entity(id).unwrap();
    assert_eq!(e.kind(), "LWPOLYLINE");
    let b = e.bounds().unwrap();
    assert!(
        near(b.min(), p(0.0, -1.0)) && near(b.max(), p(2.0, 3.0)),
        "{b:?}"
    );
    // Closed, the last vertex joins the first.
    let (square, _) = cad
        .add_polyline(
            &[(p(0.0, 0.0), 0.0), (p(1.0, 0.0), 0.0), (p(1.0, 1.0), 0.0)],
            true,
        )
        .unwrap();
    assert_eq!(
        cad.pick(p(0.5, 0.5), 0.05).unwrap(),
        Some(square),
        "the closing segment, (1,1) back to (0,0)"
    );
    // Refused, and nothing drawn: one vertex, a zero-length segment (the
    // closing one too), a bulge that is not a number.
    let before = cad.document().entities().len();
    for (vertices, closed) in [
        (vec![(p(0.0, 0.0), 0.0)], false),
        (vec![(p(0.0, 0.0), 0.0), (p(0.0, 0.0), 0.0)], false),
        (
            vec![(p(0.0, 0.0), 0.0), (p(1.0, 0.0), 0.0), (p(0.0, 0.0), 0.0)],
            true,
        ),
        (vec![(p(0.0, 0.0), f64::NAN), (p(1.0, 0.0), 0.0)], false),
    ] {
        assert!(cad.add_polyline(&vertices, closed).is_err(), "{vertices:?}");
    }
    assert_eq!(cad.document().entities().len(), before);
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

/// A drawing with block PUERTA, base (1,1): one LINE (1,1)-(2,1); an INSERT
/// of it at (10,5); and a polyline (0,0)-(4,0)-(4,2).
fn blocks_and_polylines() -> (CadDocument, EntityId, EntityId) {
    let codes = [
        "0",
        "SECTION",
        "2",
        "HEADER",
        "9",
        "$ACADVER",
        "1",
        "AC1009",
        "0",
        "ENDSEC", //
        "0",
        "SECTION",
        "2",
        "TABLES",
        "0",
        "TABLE",
        "2",
        "LAYER",
        "70",
        "1", //
        "0",
        "LAYER",
        "2",
        "0",
        "70",
        "0",
        "62",
        "7",
        "6",
        "CONTINUOUS", //
        "0",
        "ENDTAB",
        "0",
        "ENDSEC", //
        "0",
        "SECTION",
        "2",
        "BLOCKS", //
        "0",
        "BLOCK",
        "8",
        "0",
        "2",
        "PUERTA",
        "70",
        "0",
        "10",
        "1",
        "20",
        "1",
        "30",
        "0",
        "3",
        "PUERTA", //
        "0",
        "LINE",
        "8",
        "0",
        "10",
        "1",
        "20",
        "1",
        "30",
        "0",
        "11",
        "2",
        "21",
        "1",
        "31",
        "0", //
        "0",
        "ENDBLK",
        "8",
        "0",
        "0",
        "ENDSEC", //
        "0",
        "SECTION",
        "2",
        "ENTITIES", //
        "0",
        "INSERT",
        "8",
        "0",
        "2",
        "PUERTA",
        "10",
        "10",
        "20",
        "5",
        "30",
        "0", //
        "0",
        "LWPOLYLINE",
        "8",
        "0",
        "90",
        "3",
        "70",
        "0",
        "10",
        "0",
        "20",
        "0",
        "10",
        "4",
        "20",
        "0",
        "10",
        "4",
        "20",
        "2", //
        "0",
        "ENDSEC",
        "0",
        "EOF",
        "",
    ];
    let doc = Document::from_dxf_bytes(codes.join("\n").into_bytes()).unwrap();
    let ids: Vec<_> = doc.entities().iter().map(|e| e.id()).collect();
    (CadDocument::from_document(doc), ids[0], ids[1])
}

/// The ends of every segment drawn for `id`, sorted for comparison.
fn drawn_ends(cad: &CadDocument, id: EntityId) -> Vec<(i64, i64)> {
    let r = |x: f64| (x * 1e6).round() as i64;
    let mut ends: Vec<_> = cad
        .document()
        .drawing(id)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item.shape {
            arq_document::Shape::Line(l) => Some([l.start(), l.end()]),
            _ => None,
        })
        .flatten()
        .map(|q| (r(q.x()), r(q.y())))
        .collect();
    ends.sort_unstable();
    ends.dedup();
    ends
}

fn ends(points: &[(f64, f64)]) -> Vec<(i64, i64)> {
    let r = |x: f64| (x * 1e6).round() as i64;
    let mut out: Vec<_> = points.iter().map(|(x, y)| (r(*x), r(*y))).collect();
    out.sort_unstable();
    out
}

#[test]
fn blocks_and_polylines_transform_as_a_whole_and_undo() {
    let (mut cad, door, wall) = blocks_and_polylines();
    // The door is drawn (10,5)-(11,5); the wall (0,0)-(4,0)-(4,2).
    assert_eq!(drawn_ends(&cad, door), ends(&[(10.0, 5.0), (11.0, 5.0)]));
    let moved = cad.move_entities(&[door, wall], v(1.0, 2.0)).unwrap();
    assert_eq!(moved.modified(), [door, wall]);
    assert_eq!(drawn_ends(&cad, door), ends(&[(11.0, 7.0), (12.0, 7.0)]));
    assert_eq!(
        drawn_ends(&cad, wall),
        ends(&[(1.0, 2.0), (5.0, 2.0), (5.0, 4.0)])
    );
    // Rotating 90° around (11,7): (12,7) -> (11,8).
    cad.rotate_entities(&[door], p(11.0, 7.0), FRAC_PI_2)
        .unwrap();
    assert_eq!(drawn_ends(&cad, door), ends(&[(11.0, 7.0), (11.0, 8.0)]));
    // Scaling by 2 around (0,0): (11,7)-(11,8) -> (22,14)-(22,16).
    cad.scale_entities(&[door], p(0.0, 0.0), 2.0).unwrap();
    assert_eq!(drawn_ends(&cad, door), ends(&[(22.0, 14.0), (22.0, 16.0)]));
    // Mirroring the wall across x = 0: (1,2)-(5,2)-(5,4) -> (-1,2)-(-5,2)-(-5,4).
    let axis = Line::new(p(0.0, 0.0), p(0.0, 1.0)).unwrap();
    cad.mirror_entities(&[wall], axis).unwrap();
    assert_eq!(
        drawn_ends(&cad, wall),
        ends(&[(-1.0, 2.0), (-5.0, 2.0), (-5.0, 4.0)])
    );
    for _ in 0..4 {
        cad.undo().unwrap();
    }
    assert_eq!(drawn_ends(&cad, door), ends(&[(10.0, 5.0), (11.0, 5.0)]));
    assert_eq!(
        drawn_ends(&cad, wall),
        ends(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0)])
    );
}

#[test]
fn stretching_one_polyline_vertex_is_transactional_and_undoable() {
    let (mut cad, _, wall) = blocks_and_polylines();
    let changes = cad.set_polyline_vertex(wall, 1, p(6.0, 3.0)).unwrap();
    assert_eq!(changes.modified(), [wall]);
    assert_eq!(
        drawn_ends(&cad, wall),
        ends(&[(0.0, 0.0), (6.0, 3.0), (4.0, 2.0)])
    );
    assert_eq!(cad.undo_name(), Some("GRIP"));
    let failed = cad.set_polyline_vertex(wall, 9, p(1.0, 1.0));
    assert!(matches!(failed, Err(CadError::InvalidOperation(_))));
    assert_eq!(cad.undo_name(), Some("GRIP"), "failure adds no undo step");
    cad.undo().unwrap();
    assert_eq!(
        drawn_ends(&cad, wall),
        ends(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0)])
    );
    cad.redo().unwrap();
    assert_eq!(
        drawn_ends(&cad, wall),
        ends(&[(0.0, 0.0), (6.0, 3.0), (4.0, 2.0)])
    );
}

#[test]
fn exploding_a_block_replaces_it_atomically_and_is_undoable() {
    let (mut cad, door, wall) = blocks_and_polylines();
    let failed = cad.explode_entities(&[door, wall]);
    assert!(matches!(failed, Err(CadError::InvalidOperation(_))));
    assert_eq!(
        cad.document().entities().len(),
        2,
        "a failed mixed explode rolls back"
    );

    let (created, changes) = cad.explode_entities(&[door]).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(changes.created(), created);
    assert_eq!(changes.deleted(), [door]);
    assert_eq!(
        drawn_ends(&cad, created[0]),
        ends(&[(10.0, 5.0), (11.0, 5.0)])
    );
    assert_eq!(cad.undo_name(), Some("EXPLODE"));

    cad.undo().unwrap();
    assert_eq!(drawn_ends(&cad, door), ends(&[(10.0, 5.0), (11.0, 5.0)]));
    assert_eq!(cad.document().entities().len(), 2);
    cad.redo().unwrap();
    assert_eq!(
        drawn_ends(&cad, created[0]),
        ends(&[(10.0, 5.0), (11.0, 5.0)])
    );
}

#[test]
fn copying_makes_one_undo_step() {
    let (mut cad, door, wall) = blocks_and_polylines();
    let (line, _) = cad.add_line(p(0.0, 10.0), p(1.0, 10.0)).unwrap();
    let (copies, changes) = cad
        .copy_entities(&[door, wall, line, door], v(0.0, -5.0))
        .unwrap();
    assert_eq!(copies.len(), 3, "repeated ids are copied once");
    assert_eq!(changes.created(), copies.as_slice());
    assert!(changes.modified().is_empty(), "the originals do not change");
    assert_eq!(
        drawn_ends(&cad, copies[0]),
        ends(&[(10.0, 0.0), (11.0, 0.0)])
    );
    assert_eq!(drawn_ends(&cad, door), ends(&[(10.0, 5.0), (11.0, 5.0)]));
    assert_eq!(
        drawn_ends(&cad, copies[1]),
        ends(&[(0.0, -5.0), (4.0, -5.0), (4.0, -3.0)])
    );
    assert_eq!(line_ends(&cad, copies[2]), (p(0.0, 5.0), p(1.0, 5.0)));
    assert_eq!(cad.undo_name(), Some("COPY"));
    assert_eq!(cad.document().entities().len(), 6);
    cad.undo().unwrap();
    assert_eq!(
        cad.document().entities().len(),
        3,
        "one undo removes every copy"
    );
    let (none, nothing) = cad.copy_entities(&[], v(1.0, 0.0)).unwrap();
    assert!(none.is_empty() && nothing.created().is_empty());
}

#[test]
fn copying_fails_whole_on_what_cannot_be_copied() {
    let mut doc = Document::new();
    let text = doc.insert_point_for_tests(5.0, 5.0); // read-only (ADR 0022 made texts transformable)
    let mut cad = CadDocument::from_document(doc);
    let (line, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let refused = cad.copy_entities(&[line, text], v(1.0, 0.0)).unwrap_err();
    assert!(
        matches!(&refused, CadError::InvalidOperation(m) if m.contains("cannot be transformed")),
        "{refused:?}"
    );
    for ids in [vec![line, text], vec![line, 999_999]] {
        assert!(cad.copy_entities(&ids, v(1.0, 0.0)).is_err());
        assert_eq!(cad.document().entities().len(), 2, "nothing was copied");
        assert_eq!(cad.undo_name(), Some("LINE"));
    }
    let zero = cad.document().current_layer().id();
    cad.set_layer_locked(zero, true).unwrap();
    let locked = cad.copy_entities(&[line], v(1.0, 0.0)).unwrap_err();
    assert!(
        matches!(locked, CadError::InvalidOperation(_)),
        "{locked:?}"
    );
}
