//! Tests of the editing operations (a submodule of `operations`).
//!
//! Expected geometry is worked out by hand, not with `Transform2`, so a bug
//! shared by the transform and its use here cannot hide.

use std::f64::consts::{FRAC_PI_2, PI};

use arq_geometry::{Ellipse, Geometry2, Tolerance};

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
        let err = cad.set_entity_geometry(id, circle.clone()).unwrap_err();
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
fn a_polyline_keeps_its_elevation_thickness_and_width() {
    let mut cad = CadDocument::new();
    let square = [
        (p(0.0, 0.0), 0.0),
        (p(4.0, 0.0), 0.0),
        (p(4.0, 2.0), 0.0),
        (p(0.0, 2.0), 0.0),
    ];
    let style = PolylineStyle {
        elevation: -2.5,
        thickness: 3.0,
        width: 0.25,
    };
    let (id, _) = cad.add_polyline_with(&square, true, style).unwrap();
    let e = cad.document().entity(id).unwrap();
    let details: Vec<_> = e.details().to_vec();
    for (name, value) in [
        ("Elevation", "-2.500000"),
        ("Thickness", "3.000000"),
        ("Constant width", "0.250000"),
        ("Closed", "true"),
    ] {
        assert!(
            details.contains(&(name.to_owned(), value.to_owned())),
            "{name}: {details:?}"
        );
    }
    // A plain polyline holds none of them.
    let (plain, _) = cad.add_polyline(&square, true).unwrap();
    let plain = cad.document().entity(plain).unwrap();
    assert!(
        plain
            .details()
            .contains(&("Thickness".to_owned(), "0.000000".to_owned()))
    );
    // Refused, and nothing drawn.
    let before = cad.document().entities().len();
    for bad in [
        PolylineStyle {
            elevation: f64::NAN,
            ..style
        },
        PolylineStyle {
            thickness: f64::INFINITY,
            ..style
        },
        PolylineStyle {
            width: -0.1,
            ..style
        },
    ] {
        assert!(
            cad.add_polyline_with(&square, true, bad).is_err(),
            "{bad:?}"
        );
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

// TRIM and EXTEND (milestone 02.14).

fn curve_of(cad: &CadDocument, id: EntityId) -> Geometry2 {
    cad.document()
        .entity(id)
        .unwrap()
        .plan()
        .unwrap()
        .geometry()
}

fn curve_ends(g: Geometry2) -> ((f64, f64), (f64, f64)) {
    let r = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
    let (a, b) = (g.start_point().unwrap(), g.end_point().unwrap());
    ((r(a.x()), r(a.y())), (r(b.x()), r(b.y())))
}

/// A line along x from 0 to 10, crossed by two uprights at x = 3 and 7.
fn crossed() -> (CadDocument, EntityId, EntityId, EntityId) {
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let (a, _) = cad.add_line(p(3.0, -1.0), p(3.0, 1.0)).unwrap();
    let (b, _) = cad.add_line(p(7.0, -1.0), p(7.0, 1.0)).unwrap();
    (cad, line, a, b)
}

#[test]
fn trimming_the_middle_splits_a_line_in_one_step() {
    let (mut cad, line, _, _) = crossed();
    cad.set_entity_color(&[line], Color::Index(3)).unwrap();
    let (left, changes) = cad.trim_entity(line, p(5.0, 0.0), None, true).unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!(left[0], line, "the first piece keeps its id");
    assert_eq!(
        curve_ends(curve_of(&cad, left[0])),
        ((0.0, 0.0), (3.0, 0.0))
    );
    assert_eq!(
        curve_ends(curve_of(&cad, left[1])),
        ((7.0, 0.0), (10.0, 0.0))
    );
    assert_eq!(
        cad.document().entity(left[1]).unwrap().color(),
        Color::Index(3)
    );
    assert_eq!(changes.created(), [left[1]]);
    assert_eq!(cad.undo_name(), Some("TRIM"));
    cad.undo().unwrap();
    assert_eq!(curve_ends(curve_of(&cad, line)), ((0.0, 0.0), (10.0, 0.0)));
    assert!(cad.document().entity(left[1]).is_none());
}

#[test]
fn with_no_edge_quick_erases_and_standard_refuses() {
    let mut cad = CadDocument::new();
    let (alone, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let before = cad.undo_names().len();
    assert!(
        cad.trim_entity(alone, p(5.0, 0.0), Some(&[]), false)
            .is_err()
    );
    assert_eq!(cad.undo_names().len(), before, "nothing changed");
    let (left, _) = cad.trim_entity(alone, p(5.0, 0.0), None, true).unwrap();
    assert!(left.is_empty() && cad.document().entity(alone).is_none());
}

#[test]
fn standard_mode_cuts_at_the_edges_chosen_only() {
    let (mut cad, line, a, _) = crossed();
    // Only the upright at 3: picked at 5, the part from 3 to the end goes.
    let (left, _) = cad
        .trim_entity(line, p(5.0, 0.0), Some(&[a]), false)
        .unwrap();
    assert_eq!(left, [line]);
    assert_eq!(curve_ends(curve_of(&cad, line)), ((0.0, 0.0), (3.0, 0.0)));
}

#[test]
fn a_circle_cut_twice_becomes_an_arc_with_its_properties() {
    let mut cad = CadDocument::new();
    let (circle, _) = cad.add_circle(p(0.0, 0.0), 5.0).unwrap();
    cad.set_entity_color(&[circle], Color::Index(5)).unwrap();
    cad.add_line(p(-10.0, 0.0), p(10.0, 0.0)).unwrap();
    let (left, _) = cad.trim_entity(circle, p(0.0, 5.0), None, true).unwrap();
    let [arc] = left.as_slice() else {
        panic!("{left:?}")
    };
    assert!(
        cad.document().entity(circle).is_none(),
        "the circle is replaced"
    );
    let e = cad.document().entity(*arc).unwrap();
    assert_eq!((e.kind(), e.color()), ("ARC", Color::Index(5)));
    let Geometry2::Arc(a) = e.plan().unwrap().geometry() else {
        panic!()
    };
    assert!(
        (a.sweep() - std::f64::consts::PI).abs() < 1e-12,
        "the lower half"
    );
    assert_eq!(cad.undo_name(), Some("TRIM"));
    cad.undo().unwrap();
    assert!(cad.document().entity(circle).is_some());
}

#[test]
fn texts_are_not_cutting_edges() {
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    // A text whose letters cross the line.
    cad.add_text("IIII", p(4.0, -1.0), 3.0, 0.0).unwrap();
    let (left, _) = cad.trim_entity(line, p(5.0, 0.0), None, true).unwrap();
    assert!(left.is_empty(), "no edge: Quick erases it");
}

#[test]
fn extending_reaches_the_nearest_boundary_in_one_step() {
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(5.0, 0.0)).unwrap();
    let (near, _) = cad.add_line(p(8.0, -1.0), p(8.0, 1.0)).unwrap();
    cad.add_line(p(12.0, -1.0), p(12.0, 1.0)).unwrap();
    cad.extend_entity(line, p(4.0, 0.0), None).unwrap();
    assert_eq!(curve_ends(curve_of(&cad, line)), ((0.0, 0.0), (8.0, 0.0)));
    assert_eq!(cad.undo_name(), Some("EXTEND"));
    // Again: on to the next.
    cad.extend_entity(line, p(7.0, 0.0), None).unwrap();
    assert_eq!(curve_ends(curve_of(&cad, line)), ((0.0, 0.0), (12.0, 0.0)));
    // Standard, toward an edge it never reaches: refused, nothing changed.
    let before = cad.undo_names().len();
    assert!(
        cad.extend_entity(line, p(11.0, 0.0), Some(&[near]))
            .is_err()
    );
    assert_eq!(cad.undo_names().len(), before);
}

#[test]
fn locked_layers_are_neither_trimmed_nor_extended() {
    let (mut cad, line, _, _) = crossed();
    let layer = cad.document().entity(line).unwrap().layer().to_owned();
    let id = cad.document().layer_by_name(&layer).unwrap().id();
    cad.set_layer_locked(id, true).unwrap();
    assert!(cad.trim_entity(line, p(5.0, 0.0), None, true).is_err());
    assert!(cad.extend_entity(line, p(9.0, 0.0), None).is_err());
    assert_eq!(curve_ends(curve_of(&cad, line)), ((0.0, 0.0), (10.0, 0.0)));
}

/// A polyline's vertices as the entity's grips show them, in the world.
fn polyline_points(cad: &CadDocument, id: EntityId) -> Vec<(f64, f64)> {
    let r = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
    let mut grips: Vec<_> = cad
        .document()
        .grips(&[id])
        .into_iter()
        .filter(|g| g.vertex_index.is_some())
        .collect();
    grips.sort_by_key(|g| g.vertex_index);
    grips
        .iter()
        .map(|g| (r(g.point.x()), r(g.point.y())))
        .collect()
}

fn detail(cad: &CadDocument, id: EntityId, name: &str) -> String {
    let e = cad.document().entity(id).unwrap();
    e.details()
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap()
}

#[test]
fn a_wide_polyline_trimmed_in_the_middle_becomes_two_alike() {
    let mut cad = CadDocument::new();
    let ell = [
        (p(0.0, 0.0), 0.0),
        (p(10.0, 0.0), 0.0),
        (p(10.0, 10.0), 0.0),
    ];
    let style = PolylineStyle {
        width: 0.25,
        ..PolylineStyle::default()
    };
    let (pl, _) = cad.add_polyline_with(&ell, false, style).unwrap();
    cad.add_line(p(3.0, -1.0), p(3.0, 1.0)).unwrap();
    cad.add_line(p(9.0, 5.0), p(11.0, 5.0)).unwrap();
    let (left, _) = cad.trim_entity(pl, p(5.0, 0.0), None, true).unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!(polyline_points(&cad, left[0]), [(0.0, 0.0), (3.0, 0.0)]);
    assert_eq!(polyline_points(&cad, left[1]), [(10.0, 5.0), (10.0, 10.0)]);
    assert_eq!(detail(&cad, left[1], "Constant width"), "0.250000");
    assert_eq!(cad.undo_name(), Some("TRIM"));
    cad.undo().unwrap();
    assert_eq!(
        polyline_points(&cad, pl),
        [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]
    );
}

#[test]
fn a_closed_polyline_trimmed_opens() {
    let mut cad = CadDocument::new();
    let square = [
        (p(0.0, 0.0), 0.0),
        (p(4.0, 0.0), 0.0),
        (p(4.0, 2.0), 0.0),
        (p(0.0, 2.0), 0.0),
    ];
    let (pl, _) = cad.add_polyline(&square, true).unwrap();
    cad.add_line(p(2.0, -1.0), p(2.0, 3.0)).unwrap();
    let (left, _) = cad.trim_entity(pl, p(4.0, 1.0), None, true).unwrap();
    assert_eq!(left, [pl]);
    assert_eq!(
        polyline_points(&cad, pl),
        [(2.0, 2.0), (0.0, 2.0), (0.0, 0.0), (2.0, 0.0)]
    );
    assert_eq!(detail(&cad, pl, "Closed"), "false");
}

#[test]
fn a_polyline_extends_at_its_end_and_a_closed_one_cannot() {
    let mut cad = CadDocument::new();
    let ell = [
        (p(0.0, 0.0), 0.0),
        (p(10.0, 0.0), 0.0),
        (p(10.0, 10.0), 0.0),
    ];
    let (pl, _) = cad.add_polyline(&ell, false).unwrap();
    cad.add_line(p(5.0, 15.0), p(15.0, 15.0)).unwrap();
    cad.extend_entity(pl, p(10.0, 9.0), None).unwrap();
    assert_eq!(
        polyline_points(&cad, pl),
        [(0.0, 0.0), (10.0, 0.0), (10.0, 15.0)]
    );
    assert_eq!(cad.undo_name(), Some("EXTEND"));
    let (closed, _) = cad.add_polyline(&ell, true).unwrap();
    assert!(cad.extend_entity(closed, p(10.0, 9.0), None).is_err());
}

// FILLET and CHAMFER (milestone 02.14).

/// Two lines crossing at the origin.
fn corner_lines() -> (CadDocument, EntityId, EntityId) {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(-10.0, 0.0), p(10.0, 0.0)).unwrap();
    let (b, _) = cad.add_line(p(0.0, -10.0), p(0.0, 10.0)).unwrap();
    (cad, a, b)
}

#[test]
fn a_fillet_trims_both_and_adds_an_arc_like_the_first_in_one_step() {
    let (mut cad, a, b) = corner_lines();
    cad.set_entity_color(&[a], Color::Index(2)).unwrap();
    let (arc, changes) = cad
        .fillet_entities(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, true)
        .unwrap();
    let arc = arc.expect("an arc");
    assert_eq!(changes.created(), [arc]);
    let e = cad.document().entity(arc).unwrap();
    assert_eq!((e.kind(), e.color()), ("ARC", Color::Index(2)));
    assert_eq!(curve_ends(curve_of(&cad, a)), ((2.0, 0.0), (10.0, 0.0)));
    assert_eq!(curve_ends(curve_of(&cad, b)), ((0.0, 2.0), (0.0, 10.0)));
    assert_eq!(cad.undo_name(), Some("FILLET"));
    cad.undo().unwrap();
    assert!(cad.document().entity(arc).is_none());
    assert_eq!(curve_ends(curve_of(&cad, a)), ((-10.0, 0.0), (10.0, 0.0)));
}

#[test]
fn a_chamfer_cuts_the_corner_and_a_zero_one_meets_the_lines() {
    let (mut cad, a, b) = corner_lines();
    let (cut, _) = cad
        .chamfer_entities(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, 3.0, true)
        .unwrap();
    assert_eq!(
        curve_ends(curve_of(&cad, cut.unwrap())),
        ((2.0, 0.0), (0.0, 3.0))
    );
    assert_eq!(cad.undo_name(), Some("CHAMFER"));
    let (mut cad, a, b) = corner_lines();
    let (none, _) = cad
        .fillet_entities(a, p(5.0, 0.0), b, p(0.0, 5.0), 0.0, true)
        .unwrap();
    assert!(none.is_none());
    assert_eq!(curve_ends(curve_of(&cad, a)), ((0.0, 0.0), (10.0, 0.0)));
}

#[test]
fn no_trim_leaves_the_objects_as_they_were() {
    let (mut cad, a, b) = corner_lines();
    let (arc, _) = cad
        .fillet_entities(a, p(5.0, 0.0), b, p(0.0, 5.0), 2.0, false)
        .unwrap();
    assert!(arc.is_some());
    assert_eq!(curve_ends(curve_of(&cad, a)), ((-10.0, 0.0), (10.0, 0.0)));
}

#[test]
fn a_corner_is_refused_for_one_object_twice_a_locked_layer_or_too_large() {
    let (mut cad, a, b) = corner_lines();
    let before = cad.undo_names().len();
    assert!(
        cad.fillet_entities(a, p(5.0, 0.0), a, p(6.0, 0.0), 1.0, true)
            .is_err()
    );
    assert!(
        cad.fillet_entities(a, p(5.0, 0.0), b, p(0.0, 5.0), 50.0, true)
            .is_err()
    );
    let (c, _) = cad.add_circle(p(30.0, 0.0), 1.0).unwrap();
    assert!(
        cad.chamfer_entities(a, p(5.0, 0.0), c, p(31.0, 0.0), 1.0, 1.0, true)
            .is_err()
    );
    let layer = cad.document().entity(a).unwrap().layer().to_owned();
    let id = cad.document().layer_by_name(&layer).unwrap().id();
    cad.set_layer_locked(id, true).unwrap();
    let before_lock = cad.undo_names().len();
    assert!(
        cad.fillet_entities(a, p(5.0, 0.0), b, p(0.0, 5.0), 1.0, true)
            .is_err()
    );
    assert_eq!(cad.undo_names().len(), before_lock);
    assert!(
        before < before_lock,
        "the circle and the lock were steps of their own"
    );
}

#[test]
fn a_polyline_has_every_corner_filleted_in_one_step() {
    let mut cad = CadDocument::new();
    let square = [
        (p(0.0, 0.0), 0.0),
        (p(10.0, 0.0), 0.0),
        (p(10.0, 10.0), 0.0),
        (p(0.0, 10.0), 0.0),
    ];
    let (pl, _) = cad.add_polyline(&square, true).unwrap();
    let ((done, short), _) = cad
        .cut_polyline_corners(pl, arq_geometry::CornerCut::Fillet(2.0), None)
        .unwrap();
    assert_eq!((done, short), (4, 0));
    assert_eq!(polyline_points(&cad, pl).len(), 8);
    assert_eq!(detail(&cad, pl, "Closed"), "true");
    assert_eq!(cad.undo_name(), Some("FILLET"));
    cad.undo().unwrap();
    assert_eq!(polyline_points(&cad, pl).len(), 4);
}

#[test]
fn two_segments_picked_cut_the_corner_they_share() {
    let mut cad = CadDocument::new();
    let ell = [
        (p(0.0, 0.0), 0.0),
        (p(10.0, 0.0), 0.0),
        (p(10.0, 10.0), 0.0),
        (p(20.0, 10.0), 0.0),
    ];
    let (pl, _) = cad.add_polyline(&ell, false).unwrap();
    let picks = Some((p(5.0, 0.0), p(10.0, 5.0)));
    let ((done, _), _) = cad
        .cut_polyline_corners(pl, arq_geometry::CornerCut::Chamfer(1.0, 2.0), picks)
        .unwrap();
    assert_eq!(done, 1);
    assert_eq!(
        polyline_points(&cad, pl),
        [
            (0.0, 0.0),
            (9.0, 0.0),
            (10.0, 2.0),
            (10.0, 10.0),
            (20.0, 10.0)
        ]
    );
    assert_eq!(cad.undo_name(), Some("CHAMFER"));
    // Segments not next to each other share no corner.
    let far = Some((p(5.0, 0.0), p(15.0, 10.0)));
    assert!(
        cad.cut_polyline_corners(pl, arq_geometry::CornerCut::Fillet(1.0), far)
            .is_err()
    );
}

#[test]
fn stretch_crossing_endpoints_and_whole_objects_is_one_step() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let (b, _) = cad.add_line(p(0.0, 5.0), p(10.0, 5.0)).unwrap();
    let area = arq_geometry::Bounds2::new(p(9.0, -1.0), p(11.0, 1.0));
    let changes = cad
        .stretch_entities(&[a, a, b], v(2.0, 3.0), &[(a, area)])
        .unwrap();
    assert_eq!(changes.modified(), [a, b]);
    assert_eq!(line_ends(&cad, a), (p(0.0, 0.0), p(12.0, 3.0)));
    assert_eq!(line_ends(&cad, b), (p(2.0, 8.0), p(12.0, 8.0)));
    assert_eq!(cad.undo_name(), Some("STRETCH"));
    cad.undo().unwrap();
    assert_eq!(line_ends(&cad, a), (p(0.0, 0.0), p(10.0, 0.0)));
    assert_eq!(line_ends(&cad, b), (p(0.0, 5.0), p(10.0, 5.0)));
    cad.redo().unwrap();
    assert_eq!(line_ends(&cad, a).1, p(12.0, 3.0));
}

#[test]
fn stretch_windows_combine_and_a_crossing_without_endpoints_does_nothing() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let middle = arq_geometry::Bounds2::new(p(4.0, -1.0), p(6.0, 1.0));
    let before = cad.undo_name().map(str::to_owned);
    let changes = cad
        .stretch_entities(&[a], v(2.0, 3.0), &[(a, middle)])
        .unwrap();
    assert!(changes.is_empty());
    assert_eq!(cad.undo_name(), before.as_deref());
    let start = arq_geometry::Bounds2::from_point(p(0.0, 0.0));
    let end = arq_geometry::Bounds2::from_point(p(10.0, 0.0));
    cad.stretch_entities(&[a], v(2.0, 3.0), &[(a, start), (a, end)])
        .unwrap();
    assert_eq!(line_ends(&cad, a), (p(2.0, 3.0), p(12.0, 3.0)));
}

#[test]
fn stretch_failure_restores_earlier_entities_and_history() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 5.0), p(10.0, 5.0)).unwrap();
    let (b, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let area = arq_geometry::Bounds2::from_point(p(10.0, 0.0));
    assert!(
        cad.stretch_entities(&[a, b], v(-10.0, 0.0), &[(b, area)])
            .is_err()
    );
    assert_eq!(line_ends(&cad, a), (p(0.0, 5.0), p(10.0, 5.0)));
    assert_eq!(line_ends(&cad, b), (p(0.0, 0.0), p(10.0, 0.0)));
    assert_eq!(cad.undo_name(), Some("LINE"));
    assert!(
        cad.stretch_entities(&[a, u64::MAX], v(1.0, 0.0), &[])
            .is_err()
    );
    assert_eq!(line_ends(&cad, a).0, p(0.0, 5.0));
}

#[test]
fn break_is_atomic_keeps_properties_and_updates_history_and_queries() {
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    cad.set_entity_color(&[line], Color::Index(3)).unwrap();
    let (left, changes) = cad.break_entity(line, p(3.0, 0.0), p(7.0, 0.0)).unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!(left[0], line);
    assert_eq!(line_ends(&cad, left[0]), (p(0.0, 0.0), p(3.0, 0.0)));
    assert_eq!(line_ends(&cad, left[1]), (p(7.0, 0.0), p(10.0, 0.0)));
    assert_eq!(
        cad.document().entity(left[1]).unwrap().color(),
        Color::Index(3)
    );
    assert_eq!(changes.created(), [left[1]]);
    assert_eq!(cad.undo_name(), Some("BREAK"));
    let gap = arq_geometry::Bounds2::new(p(4.0, -1.0), p(6.0, 1.0));
    assert!(cad.query_entities(gap).is_empty());
    cad.undo().unwrap();
    assert_eq!(line_ends(&cad, line), (p(0.0, 0.0), p(10.0, 0.0)));
    assert!(cad.document().entity(left[1]).is_none());
    cad.redo().unwrap();
    assert_eq!(line_ends(&cad, left[1]), (p(7.0, 0.0), p(10.0, 0.0)));
    let before = cad.undo_names().len();
    assert!(cad.break_entity(line, p(0.0, 0.0), p(0.0, 0.0)).is_err());
    assert_eq!(cad.undo_names().len(), before);
    let layer = cad.document().entity(line).unwrap().layer().to_owned();
    let layer = cad.document().layer_by_name(&layer).unwrap().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(cad.break_entity(line, p(1.0, 0.0), p(2.0, 0.0)).is_err());
    assert_eq!(line_ends(&cad, line), (p(0.0, 0.0), p(3.0, 0.0)));
}

#[test]
fn break_circle_changes_kind_and_undo_restores_the_original_id() {
    let mut cad = CadDocument::new();
    let (circle, _) = cad.add_circle(p(0.0, 0.0), 5.0).unwrap();
    cad.set_entity_color(&[circle], Color::Index(4)).unwrap();
    let (left, changes) = cad.break_entity(circle, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(changes.deleted(), [circle]);
    assert_eq!(changes.created(), left);
    assert_eq!(
        cad.document().entity(left[0]).unwrap().color(),
        Color::Index(4)
    );
    assert_eq!(
        curve_ends(geometry(&cad, left[0])),
        ((0.0, 5.0), (5.0, 0.0))
    );
    cad.undo().unwrap();
    assert!(matches!(geometry(&cad, circle), Geometry2::Circle(_)));
    cad.redo().unwrap();
    assert!(cad.document().entity(circle).is_none());
    assert!(matches!(geometry(&cad, left[0]), Geometry2::Arc(_)));
}

#[test]
fn polygon_stretch_combines_areas_once_and_rolls_back_a_later_collapse() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0., 0.), p(10., 0.)).unwrap();
    let (b, _) = cad.add_line(p(0., 5.), p(12., 5.)).unwrap();
    let polygon = SelectionPolygon::new(
        vec![
            p(9., -1.),
            p(11., -1.),
            p(11., 6.),
            p(10.5, 6.),
            p(10.5, 1.),
            p(9., 1.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(cad.select_crossing_polygon(&polygon), vec![a, b]);
    let areas = vec![
        (a, polygon.clone()),
        (a, polygon.clone()),
        (b, polygon.clone()),
    ];
    let rect = arq_geometry::Bounds2::new(p(9., -1.), p(11., 1.));
    cad.stretch_entities_with_polygons(&[a, b], v(2., 3.), &[(a, rect)], &areas)
        .unwrap();
    assert_eq!(line_ends(&cad, a), (p(0., 0.), p(12., 3.)));
    assert_eq!(
        line_ends(&cad, b),
        (p(0., 5.), p(12., 5.)),
        "the second end is outside the concave polygon"
    );
    assert_eq!(cad.undo_name(), Some("STRETCH"));
    cad.undo().unwrap();
    assert_eq!(line_ends(&cad, a), (p(0., 0.), p(10., 0.)));
    cad.redo().unwrap();
    assert_eq!(line_ends(&cad, a).1, p(12., 3.));
    cad.undo().unwrap();
    let history = cad.undo_names().len();
    let rollback_areas = vec![(a, polygon)];
    assert!(
        cad.stretch_entities_with_polygons(&[b, a], v(-10., 0.), &[], &rollback_areas)
            .is_err()
    );
    assert_eq!(cad.undo_names().len(), history);
    assert_eq!(line_ends(&cad, b), (p(0., 5.), p(12., 5.)));
    assert_eq!(line_ends(&cad, a), (p(0., 0.), p(10., 0.)));
}

#[test]
fn join_from_a_line_keeps_it_erases_the_rest_and_is_one_step() {
    let mut cad = CadDocument::new();
    let (source, _) = cad.add_line(p(2.0, 0.0), p(4.0, 0.0)).unwrap();
    let (ahead, _) = cad.add_line(p(6.0, 0.0), p(9.0, 0.0)).unwrap();
    let (behind, _) = cad.add_line(p(-3.0, 0.0), p(0.0, 0.0)).unwrap();
    let (across, _) = cad.add_line(p(3.0, -1.0), p(3.0, 1.0)).unwrap();
    let (arc, _) = cad.add_arc(p(4.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
    cad.set_entity_color(&[source], Color::Index(3)).unwrap();
    let steps = cad.undo_names().len();

    let (joined, changes) = cad
        .join_entities(Some(source), &[ahead, behind, across, arc, source])
        .unwrap();
    assert_eq!(joined.results, [source], "the source, its id kept");
    assert_eq!((joined.joined, joined.discarded), (2, 2));
    assert_eq!(line_ends(&cad, source), (p(-3.0, 0.0), p(9.0, 0.0)));
    assert_eq!(
        cad.document().entity(source).unwrap().color(),
        Color::Index(3)
    );
    assert!(cad.document().entity(ahead).is_none());
    assert!(cad.document().entity(behind).is_none());
    assert!(cad.document().entity(across).is_some(), "not collinear");
    assert!(
        cad.document().entity(arc).is_some(),
        "a line joins lines only"
    );
    let mut deleted = changes.deleted().to_vec();
    deleted.sort();
    assert_eq!(deleted, {
        let mut d = vec![ahead, behind];
        d.sort();
        d
    });
    assert_eq!(changes.modified(), [source]);
    assert_eq!(cad.undo_names().len(), steps + 1);
    assert_eq!(cad.undo_name(), Some("JOIN"));
    let gap = arq_geometry::Bounds2::new(p(5.2, -0.1), p(5.8, 0.1));
    assert_eq!(
        cad.query_entities(gap),
        [source],
        "the index has the gap filled"
    );

    cad.undo().unwrap();
    assert_eq!(line_ends(&cad, source), (p(2.0, 0.0), p(4.0, 0.0)));
    assert_eq!(line_ends(&cad, ahead), (p(6.0, 0.0), p(9.0, 0.0)));
    cad.redo().unwrap();
    assert!(cad.document().entity(ahead).is_none());
}

#[test]
fn join_from_an_arc_going_all_the_way_round_makes_its_circle() {
    let mut cad = CadDocument::new();
    let (source, _) = cad.add_arc(p(1.0, 1.0), 5.0, 0.0, FRAC_PI_2).unwrap();
    let (rest, _) = cad.add_arc(p(1.0, 1.0), 5.0, FRAC_PI_2, 0.0).unwrap();
    cad.set_entity_color(&[source], Color::Index(5)).unwrap();
    let (joined, changes) = cad.join_entities(Some(source), &[rest]).unwrap();
    let [circle] = joined.results[..] else {
        panic!("{joined:?}")
    };
    assert_eq!(cad.document().entity(circle).unwrap().kind(), "CIRCLE");
    assert_eq!(
        cad.document().entity(circle).unwrap().color(),
        Color::Index(5)
    );
    assert_eq!(changes.created(), [circle]);
    assert!(cad.document().entity(source).is_none());
    assert!(cad.document().entity(rest).is_none());
    cad.undo().unwrap();
    assert!(cad.document().entity(circle).is_none());
    assert_eq!(cad.document().entity(source).unwrap().kind(), "ARC");
}

#[test]
fn close_turns_an_arc_into_its_circle() {
    let mut cad = CadDocument::new();
    let (arc, _) = cad.add_arc(p(0.0, 0.0), 2.0, 0.5, 1.0).unwrap();
    let (circle, _) = cad.close_arc(arc).unwrap();
    let Geometry2::Circle(c) = geometry(&cad, circle) else {
        panic!("not a circle")
    };
    assert_eq!((c.center(), c.radius()), (p(0.0, 0.0), 2.0));
    assert_eq!(cad.undo_name(), Some("JOIN"));
    let (line, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    assert!(cad.close_arc(line).is_err(), "only an arc closes");
}

#[test]
fn join_many_at_once_groups_and_joins_each_from_its_first() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (b, _) = cad.add_arc(p(0.0, 5.0), 1.0, 0.0, 1.0).unwrap();
    let (c, _) = cad.add_line(p(3.0, 0.0), p(4.0, 0.0)).unwrap();
    let (d, _) = cad.add_arc(p(0.0, 5.0), 1.0, 2.0, 3.0).unwrap();
    let (alone, _) = cad.add_line(p(0.0, 9.0), p(1.0, 9.0)).unwrap();
    let (joined, _) = cad.join_entities(None, &[a, b, c, d, alone]).unwrap();
    assert_eq!(joined.results, [a, b]);
    assert_eq!((joined.joined, joined.discarded), (2, 1));
    assert_eq!(line_ends(&cad, a), (p(0.0, 0.0), p(4.0, 0.0)));
    let Geometry2::Arc(arc) = geometry(&cad, b) else {
        panic!("not an arc")
    };
    assert_eq!((arc.start_angle(), arc.end_angle()), (0.0, 3.0));
    assert_eq!(cad.undo_names().iter().filter(|n| **n == "JOIN").count(), 1);
}

#[test]
fn join_leaves_what_cannot_join_and_nothing_joined_changes_nothing() {
    let mut cad = CadDocument::new();
    let (source, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (high, _) = cad.add_line(p(2.0, 0.0), p(3.0, 0.0)).unwrap();
    cad.transact("SETUP", |doc| {
        let raised = PlanGeometry::at_elevation(geometry_of(doc, high), 5.0)?;
        Ok(doc.set_plan_geometry(high, raised)?)
    })
    .unwrap();
    let (locked, _) = cad.add_line(p(4.0, 0.0), p(5.0, 0.0)).unwrap();
    let (layer, _) = cad.create_layer("LOCKED").unwrap();
    cad.set_entity_layer(&[locked], layer).unwrap();
    cad.set_layer_locked(layer, true).unwrap();
    let (circle, _) = cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    let steps = cad.undo_names().len();

    let (joined, changes) = cad
        .join_entities(Some(source), &[high, locked, circle])
        .unwrap();
    assert_eq!(joined.results, []);
    assert_eq!((joined.joined, joined.discarded), (0, 3));
    assert_eq!(changes, ChangeSet::default());
    assert_eq!(cad.undo_names().len(), steps, "no undo step");
    assert_eq!(line_ends(&cad, source), (p(0.0, 0.0), p(1.0, 0.0)));

    assert!(
        cad.join_entities(Some(circle), &[source]).is_err(),
        "a circle is no source"
    );
    assert!(
        cad.join_entities(Some(locked), &[source]).is_err(),
        "locked"
    );
    assert_eq!(cad.undo_names().len(), steps);
}

fn geometry_of(doc: &Document, id: EntityId) -> Geometry2 {
    doc.entity(id).and_then(|e| e.plan()).unwrap().geometry()
}

#[test]
fn stretch_preview_and_commit_share_polygon_union_and_validation_without_history() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let polygon = SelectionPolygon::new(
        vec![p(9.0, -1.0), p(11.0, -1.0), p(11.0, 1.0), p(9.0, 1.0)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let crossing = [(a, arq_geometry::Bounds2::new(p(9.0, -1.0), p(11.0, 1.0)))];
    let polygons = [(a, polygon)];
    let offset = v(2.0, 3.0);
    let shapes = cad
        .stretch_preview(&[a, a], offset, &crossing, &polygons)
        .unwrap();
    assert_eq!(shapes.len(), 1);
    let arq_document::Shape::Line(line) = shapes[0] else {
        panic!()
    };
    assert_eq!((line.start(), line.end()), (p(0.0, 0.0), p(12.0, 3.0)));
    assert_eq!(cad.undo_name(), Some("LINE"));
    assert_eq!(line_ends(&cad, a), (p(0.0, 0.0), p(10.0, 0.0)));
    assert!(
        cad.stretch_preview(&[a, u64::MAX], offset, &crossing, &polygons)
            .is_err()
    );
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(
        cad.stretch_preview(&[a], offset, &crossing, &polygons)
            .is_err()
    );
    cad.set_layer_locked(layer, false).unwrap();
    cad.stretch_entities_with_polygons(&[a], offset, &crossing, &polygons)
        .unwrap();
    assert_eq!(geometry(&cad, a), line.into());
}

#[test]
fn rectangular_array_keeps_sources_styles_and_one_history_step() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let (b, _) = cad.add_circle(p(1.0, 1.0), 0.5).unwrap();
    cad.set_entity_color(&[a], Color::Index(3)).unwrap();
    let before = cad.document().entities().len();
    let (copies, changes) = cad
        .rectangular_array(&[a, b, a], 2, 3, -5.0, 10.0, FRAC_PI_2)
        .unwrap();
    assert_eq!(copies.len(), 10);
    assert_eq!(changes.created(), copies);
    assert!(changes.modified().is_empty() && changes.deleted().is_empty());
    // Column movement is +Y, negative row movement +X. Source direction stays +X.
    for (cell, (x, y)) in [
        (0.0, 10.0),
        (0.0, 20.0),
        (5.0, 0.0),
        (5.0, 10.0),
        (5.0, 20.0),
    ]
    .into_iter()
    .enumerate()
    {
        let id = copies[cell * 2];
        let (start, end) = line_ends(&cad, id);
        assert!(near(start, p(x, y)) && near(end, p(x + 2.0, y)));
        assert_eq!(cad.document().entity(id).unwrap().color(), Color::Index(3));
        let Geometry2::Circle(c) = geometry(&cad, copies[cell * 2 + 1]) else {
            panic!("circle lost")
        };
        assert!(near(c.center(), p(x + 1.0, y + 1.0)));
        assert_eq!(c.radius(), 0.5);
    }
    assert_eq!(line_ends(&cad, a), (p(0.0, 0.0), p(2.0, 0.0)));
    assert_eq!(cad.undo_name(), Some("ARRAYRECT"));
    cad.undo().unwrap();
    assert_eq!(cad.document().entities().len(), before);
    cad.redo().unwrap();
    assert_eq!(cad.document().entities().len(), before + 10);
    assert!(cad.document().entity(copies[9]).is_some());
}

#[test]
fn rectangular_array_rejects_invalid_parameters_without_a_history_step() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    for (rows, cols, rs, cs, angle) in [
        (0, 2, 1.0, 1.0, 0.0),
        (1, 1, 1.0, 1.0, 0.0),
        (usize::MAX, 2, 1.0, 1.0, 0.0),
        (100_001, 1, 1.0, 1.0, 0.0),
        (2, 2, 0.0, 1.0, 0.0),
        (2, 2, 1.0, 0.0, 0.0),
        (2, 2, f64::NAN, 1.0, 0.0),
        (2, 2, 1.0, f64::INFINITY, 0.0),
        (2, 2, 1.0, 1.0, f64::NAN),
        (3, 1, f64::MAX, 1.0, 0.0),
    ] {
        assert!(
            cad.rectangular_array(&[id], rows, cols, rs, cs, angle)
                .is_err()
        );
        assert_eq!(cad.document().entities().len(), 1);
        assert_eq!(cad.undo_name(), Some("LINE"));
    }
    // A non-repeated axis may have zero spacing.
    assert_eq!(
        cad.rectangular_array(&[id], 1, 2, 0.0, -2.0, 0.0)
            .unwrap()
            .0
            .len(),
        1
    );
}

#[test]
fn rectangular_array_errors_and_late_coordinate_overflow_are_atomic() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    assert!(
        cad.rectangular_array(&[a, u64::MAX], 2, 2, 1.0, 1.0, 0.0)
            .is_err()
    );
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(cad.rectangular_array(&[a], 2, 2, 1.0, 1.0, 0.0).is_err());
    cad.set_layer_locked(layer, false).unwrap();
    let (b, _) = cad.add_line(p(1.0e308, 0.0), p(1.1e308, 0.0)).unwrap();
    let before = cad.document().entities().len();
    assert!(
        cad.rectangular_array(&[a, b], 1, 3, 0.0, 5.0e307, 0.0)
            .is_err()
    );
    assert_eq!(cad.document().entities().len(), before);
    assert_eq!(cad.undo_name(), Some("LINE"));
    assert_eq!(line_ends(&cad, a), (p(0.0, 0.0), p(1.0, 0.0)));
}

#[test]
fn break_open_polyline_updates_index_preserves_style_and_undoes_atomically() {
    let mut cad = CadDocument::new();
    let (id, _) = cad
        .add_polyline(
            &[
                (p(0.0, 0.0), 0.0),
                (p(10.0, 0.0), 0.0),
                (p(10.0, 10.0), 0.0),
            ],
            false,
        )
        .unwrap();
    cad.set_entity_color(&[id], Color::Index(3)).unwrap();
    let (ids, changes) = cad.break_entity(id, p(3.0, 0.0), p(10.0, 7.0)).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], id);
    assert_eq!(changes.created(), [ids[1]]);
    assert_eq!(changes.modified(), [id]);
    assert_eq!(
        cad.document().entity(ids[1]).unwrap().color(),
        Color::Index(3)
    );
    assert!(
        cad.query_entities(arq_geometry::Bounds2::from_point(p(5.0, 0.0)))
            .is_empty()
    );
    assert_eq!(cad.undo_name(), Some("BREAK"));
    cad.undo().unwrap();
    assert_eq!(cad.document().entities().len(), 1);
    assert_eq!(
        cad.query_entities(arq_geometry::Bounds2::from_point(p(5.0, 0.0))),
        [id]
    );
    cad.redo().unwrap();
    assert!(cad.document().entity(ids[1]).is_some());
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(cad.break_entity(id, p(1.0, 0.0), p(2.0, 0.0)).is_err());
    assert_eq!(cad.document().entities().len(), 2);
}

fn path_points(cad: &CadDocument, id: EntityId) -> Vec<Point2> {
    let (path, _) = cad.document().join_path(id).unwrap();
    path.vertices.iter().map(|v| v.0).collect()
}

#[test]
fn join_from_a_polyline_chains_what_touches_its_ends() {
    let mut cad = CadDocument::new();
    let (source, _) = cad
        .add_polyline(&[(p(0.0, 0.0), 0.0), (p(2.0, 0.0), 0.0)], false)
        .unwrap();
    cad.set_entity_color(&[source], Color::Index(2)).unwrap();
    let (after, _) = cad.add_line(p(2.0, 0.0), p(2.0, 2.0)).unwrap();
    // From (0,0) round to (-1,1): it starts where the source starts.
    let (before, _) = cad.add_arc(p(-1.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
    let (more, _) = cad
        .add_polyline(&[(p(3.0, 2.0), 0.0), (p(2.0, 2.0), 0.0)], false)
        .unwrap();
    let (away, _) = cad.add_line(p(9.0, 9.0), p(8.0, 8.0)).unwrap();
    let (joined, changes) = cad
        .join_entities(Some(source), &[after, before, more, away])
        .unwrap();
    assert_eq!(joined.results, [source], "the polyline, its id kept");
    assert_eq!((joined.joined, joined.discarded), (3, 1));
    let expected = [
        p(-1.0, 1.0),
        p(0.0, 0.0),
        p(2.0, 0.0),
        p(2.0, 2.0),
        p(3.0, 2.0),
    ];
    let got = path_points(&cad, source);
    assert!(
        got.len() == expected.len() && got.iter().zip(expected).all(|(&a, b)| near(a, b)),
        "{got:?}"
    );
    assert_eq!(
        cad.document().entity(source).unwrap().color(),
        Color::Index(2)
    );
    for gone in [after, before, more] {
        assert!(cad.document().entity(gone).is_none());
    }
    assert_eq!(changes.modified(), [source]);
    assert_eq!(cad.undo_name(), Some("JOIN"));
    cad.undo().unwrap();
    assert_eq!(path_points(&cad, source), [p(0.0, 0.0), p(2.0, 0.0)]);
    assert!(cad.document().entity(more).is_some());
}

#[test]
fn join_many_at_once_makes_polylines_of_touching_lines_and_arcs() {
    let mut cad = CadDocument::new();
    // An L of two lines: a polyline with the first line's properties.
    let (first, _) = cad.add_line(p(0.0, 0.0), p(4.0, 0.0)).unwrap();
    cad.set_entity_color(&[first], Color::Index(6)).unwrap();
    let (up, _) = cad.add_line(p(4.0, 0.0), p(4.0, 3.0)).unwrap();
    // Two collinear lines touching: still a line.
    let (a, _) = cad.add_line(p(0.0, 10.0), p(1.0, 10.0)).unwrap();
    let (b, _) = cad.add_line(p(1.0, 10.0), p(2.0, 10.0)).unwrap();
    // Four sides: a closed polyline.
    let sides = [
        ((10.0, 0.0), (12.0, 0.0)),
        ((12.0, 0.0), (12.0, 2.0)),
        ((10.0, 2.0), (12.0, 2.0)),
        ((10.0, 0.0), (10.0, 2.0)),
    ];
    let sides: Vec<EntityId> = sides
        .iter()
        .map(|&(s, e)| cad.add_line(p(s.0, s.1), p(e.0, e.1)).unwrap().0)
        .collect();
    let mut ids = vec![first, up, a, b];
    ids.extend(&sides);
    let (joined, _) = cad.join_entities(None, &ids).unwrap();
    assert_eq!((joined.joined, joined.discarded), (5, 0));
    let [l, line, square] = joined.results[..] else {
        panic!("{joined:?}")
    };
    assert_eq!(cad.document().entity(l).unwrap().kind(), "LWPOLYLINE");
    assert_eq!(cad.document().entity(l).unwrap().color(), Color::Index(6));
    assert_eq!(
        path_points(&cad, l),
        [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]
    );
    assert!(
        cad.document().entity(first).is_none(),
        "the line became the polyline"
    );
    assert_eq!(line, a);
    assert_eq!(line_ends(&cad, a), (p(0.0, 10.0), p(2.0, 10.0)));
    assert_eq!(cad.document().entity(square).unwrap().kind(), "LWPOLYLINE");
    assert_eq!(detail(&cad, square, "Closed"), "true");
    cad.undo().unwrap();
    assert_eq!(cad.document().entity(first).unwrap().kind(), "LINE");
    assert!(cad.document().entity(l).is_none());
}

#[test]
fn join_leaves_closed_polylines_and_other_heights_out() {
    let mut cad = CadDocument::new();
    let square = [(p(0.0, 0.0), 0.0), (p(1.0, 0.0), 0.0), (p(1.0, 1.0), 0.0)];
    let (closed, _) = cad.add_polyline(&square, true).unwrap();
    let (source, _) = cad
        .add_polyline(&[(p(1.0, 0.0), 0.0), (p(3.0, 0.0), 0.0)], false)
        .unwrap();
    let style = PolylineStyle {
        elevation: 4.0,
        ..PolylineStyle::default()
    };
    let (high, _) = cad
        .add_polyline_with(&[(p(3.0, 0.0), 0.0), (p(5.0, 0.0), 0.0)], false, style)
        .unwrap();
    let steps = cad.undo_names().len();
    let (joined, _) = cad.join_entities(Some(source), &[closed, high]).unwrap();
    assert_eq!((joined.joined, joined.discarded), (0, 2));
    assert_eq!(cad.undo_names().len(), steps);
    assert!(
        cad.join_entities(Some(closed), &[source]).is_err(),
        "closed: no source"
    );
}

#[test]
fn join_many_at_once_closes_touching_arcs_of_one_circle_into_it() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_arc(p(0.0, 0.0), 2.0, 0.0, PI).unwrap();
    let (b, _) = cad.add_arc(p(0.0, 0.0), 2.0, PI, 0.0).unwrap();
    let (joined, _) = cad.join_entities(None, &[a, b]).unwrap();
    let [circle] = joined.results[..] else {
        panic!("{joined:?}")
    };
    assert_eq!(cad.document().entity(circle).unwrap().kind(), "CIRCLE");
}

#[test]
fn matchprop_gives_the_source_properties_in_one_step_and_skips_locked_targets() {
    let mut cad = CadDocument::new();
    let (walls, _) = cad.create_layer("A-WALLS").unwrap();
    let (source, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    cad.set_entity_layer(&[source], walls).unwrap();
    cad.set_entity_color(&[source], Color::Index(1)).unwrap();
    let (a, _) = cad.add_circle(p(5.0, 5.0), 1.0).unwrap();
    let (b, _) = cad.add_line(p(0.0, 3.0), p(1.0, 3.0)).unwrap();
    let (locked_layer, _) = cad.create_layer("LOCKED").unwrap();
    let (c, _) = cad.add_line(p(0.0, 6.0), p(1.0, 6.0)).unwrap();
    cad.set_entity_layer(&[c], locked_layer).unwrap();
    cad.set_layer_locked(locked_layer, true).unwrap();
    // The source may be locked too: it is only read.
    cad.set_layer_locked(walls, true).unwrap();
    let steps = cad.undo_names().len();

    let (matched, changes) = cad
        .match_properties(source, &[a, b, c, source, a], MatchSettings::ALL)
        .unwrap();
    assert_eq!(
        matched,
        Matched {
            changed: 2,
            locked: 1
        }
    );
    let mut modified = changes.modified().to_vec();
    modified.sort();
    assert_eq!(modified, {
        let mut m = vec![a, b];
        m.sort();
        m
    });
    for id in [a, b] {
        let e = cad.document().entity(id).unwrap();
        assert_eq!((e.layer(), e.color()), ("A-WALLS", Color::Index(1)));
    }
    assert_ne!(cad.document().entity(c).unwrap().layer(), "A-WALLS");
    assert_eq!(cad.undo_names().len(), steps + 1);
    assert_eq!(cad.undo_name(), Some("MATCHPROP"));

    // Unlocked again, so that `a` and `b` are matched, not skipped.
    cad.set_layer_locked(walls, false).unwrap();
    let steps = cad.undo_names().len();
    let (again, _) = cad
        .match_properties(source, &[a, b], MatchSettings::ALL)
        .unwrap();
    assert_eq!(
        again,
        Matched {
            changed: 0,
            locked: 0
        }
    );
    assert_eq!(cad.undo_names().len(), steps, "nothing changed: no step");

    cad.undo().unwrap(); // the unlocking
    cad.undo().unwrap();
    assert_ne!(cad.document().entity(a).unwrap().layer(), "A-WALLS");
    assert!(
        cad.match_properties(999_999, &[a], MatchSettings::ALL)
            .is_err()
    );
}

fn drawn_ellipse(cad: &CadDocument, id: EntityId) -> Ellipse {
    let items = cad.document().drawing(id).unwrap();
    let [item] = &items[..] else {
        panic!("{items:?}")
    };
    let arq_document::Shape::Ellipse(e) = item.shape.clone() else {
        panic!("{item:?}")
    };
    e
}

#[test]
fn an_ellipse_is_drawn_and_undone_as_one_step() {
    let mut cad = CadDocument::new();
    let e = Ellipse::from_axis(p(5.0, 1.0), p(10.0, 1.0), 3.0)
        .unwrap()
        .arc_from(v(5.0, 0.0), 0.2, 2.5, false)
        .unwrap();
    let (id, changes) = cad.add_ellipse(e).unwrap();
    assert_eq!(changes.created(), [id]);
    assert_eq!(cad.undo_name(), Some("ELLIPSE"));
    let drawn = drawn_ellipse(&cad, id);
    let (a, b) = (e.ends().unwrap(), drawn.ends().unwrap());
    assert!(near(a.0, b.0) && near(a.1, b.1));
    assert_eq!(cad.document().entity(id).unwrap().kind(), "ELLIPSE");
    cad.undo().unwrap();
    assert!(cad.document().entity(id).is_none());
}

#[test]
fn a_spline_is_drawn_and_undone_as_one_step_and_a_refused_one_draws_nothing() {
    let mut cad = CadDocument::new();
    let points = vec![p(0.0, 0.0), p(2.0, 3.0), p(5.0, 1.0), p(7.0, 4.0)];
    let fit = arq_geometry::SplineFit::new(
        points.clone(),
        arq_geometry::SplineKnots::Chord,
        None,
        None,
        false,
    )
    .unwrap();
    let (id, changes) = cad.add_fit_spline(&fit).unwrap();
    assert_eq!(changes.created(), [id]);
    assert_eq!(cad.undo_name(), Some("SPLINE"));
    assert_eq!(cad.document().entity(id).unwrap().kind(), "SPLINE");
    let cv = arq_geometry::Spline::by_vertices(3, points, true).unwrap();
    let (other, changes) = cad.add_spline(&cv).unwrap();
    assert_eq!(changes.created(), [other]);
    assert_eq!(cad.undo_name(), Some("SPLINE"));
    cad.undo().unwrap();
    assert!(cad.document().entity(other).is_none());
    cad.undo().unwrap();
    assert!(cad.document().entity(id).is_none());
}

#[test]
fn an_ellipse_moves_turns_scales_and_mirrors_as_a_whole() {
    let arc = Ellipse::new(p(2.0, 1.0), v(3.0, 1.0), 0.5, 0.4, 2.6).unwrap();
    let axis = Line::new(p(0.0, 0.0), p(1.0, 3.0)).unwrap();
    type Op = fn(&mut CadDocument, EntityId);
    let ops: [(Op, Transform2); 4] = [
        (
            |cad, id| drop(cad.move_entities(&[id], v(4.0, -1.0)).unwrap()),
            Transform2::translation(v(4.0, -1.0)),
        ),
        (
            |cad, id| drop(cad.rotate_entities(&[id], p(1.0, 1.0), 0.7).unwrap()),
            Transform2::rotation(p(1.0, 1.0), 0.7).unwrap(),
        ),
        (
            |cad, id| drop(cad.scale_entities(&[id], p(0.0, 0.0), 2.5).unwrap()),
            Transform2::scaling(p(0.0, 0.0), 2.5).unwrap(),
        ),
        (
            |cad, id| {
                let axis = Line::new(p(0.0, 0.0), p(1.0, 3.0)).unwrap();
                drop(cad.mirror_entities(&[id], axis).unwrap())
            },
            Transform2::mirror(axis),
        ),
    ];
    for (op, t) in ops {
        let mut cad = CadDocument::new();
        let (id, _) = cad.add_ellipse(arc).unwrap();
        op(&mut cad, id);
        let moved = drawn_ellipse(&cad, id);
        for i in 0..=8 {
            let q = t.apply_point(arc.point_at_param(0.4 + 2.2 * f64::from(i) / 8.0));
            assert!(moved.distance_to(q) < 1e-9, "{t:?}: {q:?} off {moved:?}");
        }
        let (a, b) = arc.ends().unwrap();
        let ends = moved.ends().unwrap();
        let expected = if t.is_reflection() {
            (t.apply_point(b), t.apply_point(a))
        } else {
            (t.apply_point(a), t.apply_point(b))
        };
        assert!(
            near(ends.0, expected.0) && near(ends.1, expected.1),
            "{t:?}"
        );
    }
}

#[test]
fn closed_break_updates_index_and_restores_identity_closure_and_style() {
    let mut cad = CadDocument::new();
    let square = [
        (p(0., 0.), 0.),
        (p(10., 0.), 0.),
        (p(10., 10.), 0.),
        (p(0., 10.), 0.),
    ];
    let (id, _) = cad.add_polyline(&square, true).unwrap();
    cad.set_entity_color(&[id], Color::Index(3)).unwrap();
    let original = cad.grips(&[id]);
    assert!(cad.document().join_path(id).is_err());
    let (ids, changes) = cad.break_entity(id, p(0., 5.), p(5., 0.)).unwrap();
    assert_eq!(ids, [id]);
    assert_eq!(changes.modified(), [id]);
    assert!(changes.created().is_empty() && changes.deleted().is_empty());
    let opened = cad.document().join_path(id).unwrap().0;
    assert_eq!(
        opened.vertices,
        vec![(p(0., 5.), 0.), (p(0., 0.), 0.), (p(5., 0.), 0.)]
    );
    assert_eq!(cad.document().entity(id).unwrap().color(), Color::Index(3));
    let removed = arq_geometry::Bounds2::new(p(9., 9.), p(11., 11.));
    assert!(cad.query_entities(removed).is_empty());
    assert_eq!(cad.undo_name(), Some("BREAK"));
    let undone = cad.undo().unwrap();
    assert_eq!(undone.modified(), [id]);
    assert_eq!(cad.grips(&[id]), original);
    assert!(cad.document().join_path(id).is_err());
    assert_eq!(cad.query_entities(removed), [id]);
    cad.redo().unwrap();
    assert_eq!(cad.document().join_path(id).unwrap().0, opened);
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(cad.break_entity(id, p(1., 0.), p(2., 0.)).is_err());
    assert_eq!(cad.document().join_path(id).unwrap().0, opened);
}

#[test]
fn an_ellipse_is_an_edge_trim_cuts_at() {
    // A line through an ellipse about (10,0), semi-axes 4 and 2: cut at
    // its crossings, (6,0) and (14,0), in both modes.
    for edges in [None, Some(())] {
        let mut cad = CadDocument::new();
        let (line, _) = cad.add_line(p(0.0, 0.0), p(20.0, 0.0)).unwrap();
        let e = Ellipse::from_axis(p(10.0, 0.0), p(14.0, 0.0), 2.0).unwrap();
        let (ellipse, _) = cad.add_ellipse(e).unwrap();
        let chosen = [ellipse];
        let edges = edges.map(|()| &chosen[..]);
        let (left, _) = cad.trim_entity(line, p(10.0, 0.5), edges, false).unwrap();
        assert_eq!(left.len(), 2, "{edges:?}");
        let (a, b) = (
            curve_ends(curve_of(&cad, left[0])),
            curve_ends(curve_of(&cad, left[1])),
        );
        assert!((a.1.0 - 6.0).abs() < 1e-9 && a.0 == (0.0, 0.0), "{a:?}");
        assert!((b.0.0 - 14.0).abs() < 1e-9 && b.1 == (20.0, 0.0), "{b:?}");
    }
}

#[test]
fn polar_array_preserves_sources_styles_order_index_and_atomic_undo() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(10., 0.), p(12., 0.)).unwrap();
    let (b, _) = cad.add_circle(p(10., 2.), 1.).unwrap();
    cad.set_entity_color(&[a], Color::Index(3)).unwrap();
    let (copies, changes) = cad
        .polar_array(&[a, b, a], p(0., 0.), 4, std::f64::consts::TAU, true, None)
        .unwrap();
    assert_eq!(copies.len(), 6);
    assert_eq!(changes.created(), copies);
    assert!(changes.modified().is_empty() && changes.deleted().is_empty());
    let Geometry2::Line(line) = geometry(&cad, copies[0]) else {
        panic!()
    };
    assert!(line.start().approx_eq(p(0., 10.), Tolerance::DEFAULT));
    assert!(line.end().approx_eq(p(0., 12.), Tolerance::DEFAULT));
    let Geometry2::Circle(circle) = geometry(&cad, copies[1]) else {
        panic!()
    };
    assert!(circle.center().approx_eq(p(-2., 10.), Tolerance::DEFAULT));
    assert_eq!(
        cad.document().entity(copies[0]).unwrap().color(),
        Color::Index(3)
    );
    assert_eq!(line_ends(&cad, a), (p(10., 0.), p(12., 0.)));
    let area = arq_geometry::Bounds2::new(p(-0.5, 9.5), p(0.5, 12.5));
    assert!(cad.query_entities(area).contains(&copies[0]));
    assert_eq!(cad.undo_name(), Some("ARRAYPOLAR"));
    cad.undo().unwrap();
    assert_eq!(cad.document().entities().len(), 2);
    assert!(cad.query_entities(area).is_empty());
    cad.redo().unwrap();
    assert!(cad.document().entity(copies[0]).is_some());
}

#[test]
fn polar_array_without_rotation_translates_a_selection_as_one_group() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(10., 0.), p(12., 0.)).unwrap();
    let (b, _) = cad.add_line(p(10., 2.), p(11., 2.)).unwrap();
    let (copies, _) = cad
        .polar_array(&[a, b], p(0., 0.), 3, PI, false, Some(p(10., 0.)))
        .unwrap();
    assert_eq!(copies.len(), 4);
    for (id, (start, end)) in copies.into_iter().zip([
        (p(0., 10.), p(2., 10.)),
        (p(0., 12.), p(1., 12.)),
        (p(-10., 0.), p(-8., 0.)),
        (p(-10., 2.), p(-9., 2.)),
    ]) {
        let (a, b) = line_ends(&cad, id);
        assert!(a.approx_eq(start, Tolerance::DEFAULT) && b.approx_eq(end, Tolerance::DEFAULT));
    }
}

#[test]
fn polar_array_validates_all_sources_and_rolls_back_late_overflow() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(0., 0.), p(1e306, 0.)).unwrap();
    let (b, _) = cad.add_line(p(1e308, 0.), p(1.1e308, 0.)).unwrap();
    let before = cad.undo_names().len();
    assert!(
        cad.polar_array(
            &[a, u64::MAX],
            p(0., 0.),
            4,
            std::f64::consts::TAU,
            true,
            None
        )
        .is_err()
    );
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(
        cad.polar_array(&[a], p(0., 0.), 4, std::f64::consts::TAU, true, None)
            .is_err()
    );
    cad.set_layer_locked(layer, false).unwrap();
    let steps = cad.undo_names().len();
    assert!(
        cad.polar_array(&[a, b], p(0., 0.), 3, PI, false, Some(p(-5e307, 0.)))
            .is_err()
    );
    assert_eq!(cad.document().entities().len(), 2);
    assert_eq!(cad.undo_names().len(), steps);
    assert!(steps >= before);
    assert_eq!(line_ends(&cad, b), (p(1e308, 0.), p(1.1e308, 0.)));
}

#[test]
fn path_array_copies_sources_in_order_keeps_guide_styles_index_and_history() {
    let mut cad = CadDocument::new();
    let (guide, _) = cad.add_line(p(10., 10.), p(30., 10.)).unwrap();
    let (a, _) = cad.add_line(p(0., 0.), p(2., 0.)).unwrap();
    let (b, _) = cad.add_circle(p(0., 3.), 1.).unwrap();
    cad.set_entity_color(&[a], Color::Index(3)).unwrap();
    let (copies, changes) = cad
        .path_array(&[a, b, a], guide, p(0., 0.), 3, true)
        .unwrap();
    assert_eq!(copies.len(), 6);
    assert_eq!(changes.created(), copies);
    assert!(changes.modified().is_empty() && changes.deleted().is_empty());
    for (index, x) in [10., 20., 30.].into_iter().enumerate() {
        assert_eq!(
            line_ends(&cad, copies[index * 2]),
            (p(x, 10.), p(x + 2., 10.))
        );
        assert_eq!(
            cad.document().entity(copies[index * 2]).unwrap().color(),
            Color::Index(3)
        );
        let Geometry2::Circle(circle) = geometry(&cad, copies[index * 2 + 1]) else {
            panic!()
        };
        assert!(near(circle.center(), p(x, 13.)));
    }
    assert_eq!(line_ends(&cad, a), (p(0., 0.), p(2., 0.)));
    assert_eq!(line_ends(&cad, guide), (p(10., 10.), p(30., 10.)));
    let area = arq_geometry::Bounds2::new(p(29., 9.), p(33., 11.));
    assert!(cad.query_entities(area).contains(&copies[4]));
    assert_eq!(cad.undo_name(), Some("ARRAYPATH"));
    cad.undo().unwrap();
    assert_eq!(cad.document().entities().len(), 3);
    assert!(!cad.query_entities(area).contains(&copies[4]));
    cad.redo().unwrap();
    assert_eq!(line_ends(&cad, copies[4]), (p(30., 10.), p(32., 10.)));
}

#[test]
fn path_array_bulged_guide_aligns_relative_to_the_first_item() {
    let mut cad = CadDocument::new();
    let (source, _) = cad.add_line(p(20., 20.), p(22., 20.)).unwrap();
    let (guide, _) = cad
        .add_polyline(&[(p(0., 0.), -1.), (p(10., 0.), 0.)], false)
        .unwrap();
    let (copies, _) = cad
        .path_array(&[source], guide, p(20., 20.), 3, true)
        .unwrap();
    for (id, (start, end)) in copies.into_iter().zip([
        (p(0., 0.), p(2., 0.)),
        (p(5., 5.), p(5., 3.)),
        (p(10., 0.), p(8., 0.)),
    ]) {
        let (a, b) = line_ends(&cad, id);
        assert!(near(a, start) && near(b, end));
    }
    assert_eq!(line_ends(&cad, source), (p(20., 20.), p(22., 20.)));
}

#[test]
fn path_array_invalid_guides_locked_sources_and_late_overflow_are_atomic() {
    let mut cad = CadDocument::new();
    let (source, _) = cad.add_line(p(1e308, 0.), p(1.1e308, 0.)).unwrap();
    let (guide, _) = cad.add_line(p(0., 0.), p(1e308, 0.)).unwrap();
    let steps = cad.undo_names().len();
    assert!(
        cad.path_array(&[source], u64::MAX, p(0., 0.), 3, false)
            .is_err()
    );
    assert!(
        cad.path_array(&[source, u64::MAX], guide, p(0., 0.), 3, false)
            .is_err()
    );
    assert!(
        cad.path_array(&[source], guide, p(0., 0.), 3, false)
            .is_err()
    );
    assert_eq!(cad.document().entities().len(), 2);
    assert_eq!(cad.undo_names().len(), steps);
    let (closed, _) = cad
        .add_polyline(&[(p(0., 0.), 0.), (p(5., 0.), 0.), (p(5., 5.), 0.)], true)
        .unwrap();
    assert!(
        cad.path_array(&[source], closed, p(0., 0.), 3, false)
            .is_err()
    );
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(
        cad.path_array(&[source], guide, p(1e308, 0.), 2, false)
            .is_err()
    );
}

/// An ellipse about (10,0), semi-axes 4 along x and 2, from parameter
/// `start` to `end` (equal: whole), red.
fn red_ellipse(cad: &mut CadDocument, start: f64, end: f64) -> EntityId {
    let e = Ellipse::new(p(10.0, 0.0), v(4.0, 0.0), 0.5, start, end).unwrap();
    let (id, _) = cad.add_ellipse(e).unwrap();
    cad.set_entity_color(&[id], Color::Index(1)).unwrap();
    id
}

fn ends_of(e: Ellipse) -> (Point2, Point2) {
    e.ends().expect("an arc")
}

#[test]
fn a_whole_ellipse_is_trimmed_to_the_arc_left_in_one_step() {
    // x = 12 cuts it at (12, ±√3); Quick, picked on its right.
    let mut cad = CadDocument::new();
    let id = red_ellipse(&mut cad, 0.0, 0.0);
    cad.add_line(p(12.0, -5.0), p(12.0, 5.0)).unwrap();
    let (left, changes) = cad.trim_entity(id, p(14.0, 0.1), None, true).unwrap();
    assert_eq!(left, [id], "the ellipse keeps its id");
    assert_eq!(changes.modified(), [id]);
    let (a, b) = ends_of(drawn_ellipse(&cad, id));
    let h = 3f64.sqrt();
    assert!(near(a, p(12.0, h)) && near(b, p(12.0, -h)), "{a:?} {b:?}");
    assert_eq!(cad.document().entity(id).unwrap().kind(), "ELLIPSE");
    assert_eq!(cad.document().entity(id).unwrap().color(), Color::Index(1));
    assert_eq!(cad.undo_name(), Some("TRIM"));
    cad.undo().unwrap();
    assert!(drawn_ellipse(&cad, id).is_full());
}

#[test]
fn an_elliptical_arc_trimmed_in_the_middle_becomes_two_alike() {
    // The upper half, cut by x = 9 and x = 11, picked at its top.
    let mut cad = CadDocument::new();
    let id = red_ellipse(&mut cad, 0.0, PI);
    let (a, _) = cad.add_line(p(9.0, -5.0), p(9.0, 5.0)).unwrap();
    let (b, _) = cad.add_line(p(11.0, -5.0), p(11.0, 5.0)).unwrap();
    let (left, changes) = cad
        .trim_entity(id, p(10.0, 2.0), Some(&[a, b]), false)
        .unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!((left[0], changes.created()), (id, &[left[1]][..]));
    let y = 2.0 * (1.0 - 1.0 / 16.0f64).sqrt();
    let (s0, e0) = ends_of(drawn_ellipse(&cad, left[0]));
    let (s1, e1) = ends_of(drawn_ellipse(&cad, left[1]));
    assert!(
        near(s0, p(14.0, 0.0)) && near(e0, p(11.0, y)),
        "{s0:?} {e0:?}"
    );
    assert!(
        near(s1, p(9.0, y)) && near(e1, p(6.0, 0.0)),
        "{s1:?} {e1:?}"
    );
    assert_eq!(
        cad.document().entity(left[1]).unwrap().color(),
        Color::Index(1)
    );
    cad.undo().unwrap();
    assert!(cad.document().entity(left[1]).is_none());
    assert_eq!(ends_of(drawn_ellipse(&cad, id)).1.x().round(), 6.0);
}

#[test]
fn an_elliptical_arc_extends_and_breaks_in_one_step_each() {
    // The first quarter, (14,0) to (10,2), grown at its end to x = 7, in
    // both modes: to (7, √(4·(1 - 9/16))).
    for edges in [None, Some(())] {
        let mut cad = CadDocument::new();
        let id = red_ellipse(&mut cad, 0.0, FRAC_PI_2);
        let (wall, _) = cad.add_line(p(7.0, -5.0), p(7.0, 5.0)).unwrap();
        let chosen = [wall];
        cad.extend_entity(id, p(10.1, 2.0), edges.map(|()| &chosen[..]))
            .unwrap();
        let (a, b) = ends_of(drawn_ellipse(&cad, id));
        let y = 2.0 * (1.0 - 9.0 / 16.0f64).sqrt();
        assert!(
            near(a, p(14.0, 0.0)) && near(b, p(7.0, y)),
            "{edges:?}: {b:?}"
        );
        assert_eq!(cad.undo_name(), Some("EXTEND"));
    }
    // A whole ellipse cannot grow; broken, it loses the span between the
    // points, counterclockwise.
    let mut cad = CadDocument::new();
    let id = red_ellipse(&mut cad, 0.0, 0.0);
    cad.add_line(p(0.0, 5.0), p(1.0, 5.0)).unwrap();
    assert!(cad.extend_entity(id, p(14.0, 0.0), None).is_err());
    let (left, _) = cad.break_entity(id, p(14.0, 0.0), p(10.0, 2.0)).unwrap();
    assert_eq!(left, [id]);
    let (a, b) = ends_of(drawn_ellipse(&cad, id));
    assert!(
        near(a, p(10.0, 2.0)) && near(b, p(14.0, 0.0)),
        "{a:?} {b:?}"
    );
    assert_eq!(cad.undo_name(), Some("BREAK"));
}

#[test]
fn a_line_extends_to_an_ellipse_in_both_modes() {
    for edges in [None, Some(())] {
        let mut cad = CadDocument::new();
        let ellipse = red_ellipse(&mut cad, 0.0, 0.0);
        let (line, _) = cad.add_line(p(0.0, 1.0), p(3.0, 1.0)).unwrap();
        let chosen = [ellipse];
        cad.extend_entity(line, p(3.0, 1.0), edges.map(|()| &chosen[..]))
            .unwrap();
        let x = 10.0 - 4.0 * 0.75f64.sqrt();
        let end = curve_of(&cad, line).end_point().unwrap();
        assert!(near(end, p(x, 1.0)), "{edges:?}: {end:?}");
    }
}

#[test]
fn array_preview_plan_is_read_only_deduplicated_and_matches_commit() {
    let mut cad = CadDocument::new();
    let (source, _) = cad.add_line(p(1e6, 2e6), p(1e6 + 2., 2e6)).unwrap();
    let before = cad.undo_names().len();
    let (grid, ids) = cad
        .rectangular_array_preview_plan(&[source, source], 2, 2, -5., 10., FRAC_PI_2)
        .unwrap();
    assert_eq!(ids, [source]);
    assert_eq!(grid.cells(), 4);
    assert_eq!(cad.undo_names().len(), before);
    assert_eq!(cad.document().entities().len(), 1);
    let offsets: Vec<_> = grid.offsets().collect::<Result<_, _>>().unwrap();
    let (copies, _) = cad
        .rectangular_array(&[source], 2, 2, -5., 10., FRAC_PI_2)
        .unwrap();
    for (id, offset) in copies.into_iter().zip(offsets) {
        let (start, end) = line_ends(&cad, id);
        assert!(near(start, p(1e6 + offset.x(), 2e6 + offset.y())));
        assert!(near(end, p(1e6 + offset.x() + 2., 2e6 + offset.y())));
    }
    assert!(
        cad.rectangular_array_preview_plan(&[u64::MAX], 1, 2, 0., 1., 0.)
            .is_err()
    );
    let layer = cad.document().current_layer().id();
    cad.set_layer_locked(layer, true).unwrap();
    assert!(
        cad.rectangular_array_preview_plan(&[source], 1, 2, 0., 1., 0.)
            .is_err()
    );
}

#[test]
fn stretch_moves_an_ellipse_whole_as_a_circle_or_an_arc() {
    let bounds =
        |x0: f64, y0: f64, x1: f64, y1: f64| arq_geometry::Bounds2::new(p(x0, y0), p(x1, y1));
    // Whole, about (10,0): moved when its center is inside, left when not.
    let mut cad = CadDocument::new();
    let whole = red_ellipse(&mut cad, 0.0, 0.0);
    cad.stretch_entities(
        &[whole],
        v(2.0, 3.0),
        &[(whole, bounds(9.0, -1.0, 11.0, 1.0))],
    )
    .unwrap();
    assert!(near(drawn_ellipse(&cad, whole).center(), p(12.0, 3.0)));
    assert_eq!(cad.undo_name(), Some("STRETCH"));
    let before = drawn_ellipse(&cad, whole);
    cad.stretch_entities(
        &[whole],
        v(2.0, 3.0),
        &[(whole, bounds(15.0, 2.0, 16.0, 4.0))],
    )
    .unwrap();
    assert_eq!(drawn_ellipse(&cad, whole), before, "its center outside");
    // An arc, the first quarter (14,0)-(10,2): moved with both ends inside,
    // refused with one, nothing changed.
    let mut cad = CadDocument::new();
    let arc = red_ellipse(&mut cad, 0.0, FRAC_PI_2);
    let before = drawn_ellipse(&cad, arc);
    let one_end = bounds(13.0, -1.0, 15.0, 1.0);
    assert!(
        cad.stretch_entities(&[arc], v(1.0, 0.0), &[(arc, one_end)])
            .is_err()
    );
    assert_eq!(drawn_ellipse(&cad, arc), before);
    cad.stretch_entities(&[arc], v(1.0, 0.0), &[(arc, bounds(9.0, -1.0, 15.0, 3.0))])
        .unwrap();
    let (a, b) = ends_of(drawn_ellipse(&cad, arc));
    assert!(
        near(a, p(15.0, 0.0)) && near(b, p(11.0, 2.0)),
        "{a:?} {b:?}"
    );
}

#[test]
fn stretch_partial_arc_is_atomic_indexed_and_undoable_with_matching_preview() {
    use arq_geometry::{Bounds2, SelectionPolygon};
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, -10.0), p(2.0, -10.0)).unwrap();
    let (arc, _) = cad.add_arc(p(0.0, 0.0), 5.0, 0.0, FRAC_PI_2).unwrap();
    cad.set_entity_color(&[arc], Color::Index(4)).unwrap();
    let before = geometry(&cad, arc);
    let box_ = Bounds2::new(p(-1.0, 4.0), p(1.0, 6.0));
    let poly = SelectionPolygon::new(
        vec![p(-1.0, 4.0), p(1.0, 4.0), p(1.0, 6.0), p(-1.0, 6.0)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let n = cad.undo_names().len();
    let preview = cad
        .stretch_preview(&[arc], v(3.0, 2.0), &[(arc, box_)], &[(arc, poly.clone())])
        .unwrap();
    assert_eq!(cad.undo_names().len(), n);
    assert_eq!(geometry(&cad, arc), before);
    let changes = cad
        .stretch_entities_with_polygons(
            &[line, arc, arc],
            v(3.0, 2.0),
            &[(arc, box_)],
            &[(arc, poly)],
        )
        .unwrap();
    assert_eq!(changes.modified(), [line, arc]);
    assert!(changes.created().is_empty() && changes.deleted().is_empty());
    let Geometry2::Arc(after) = geometry(&cad, arc) else {
        panic!()
    };
    assert!(after.end_point().approx_eq(p(3.0, 7.0), Tolerance::DEFAULT));
    assert_eq!(preview, vec![arq_document::Shape::Arc(after)]);
    assert!(
        cad.query_entities(Bounds2::new(p(2.9, 6.9), p(3.1, 7.1)))
            .contains(&arc)
    );
    assert_eq!(cad.undo_names().len(), n + 1);
    cad.undo().unwrap();
    assert_eq!(geometry(&cad, arc), before);
    assert_eq!(line_ends(&cad, line), (p(0.0, -10.0), p(2.0, -10.0)));
    cad.redo().unwrap();
    assert_eq!(geometry(&cad, arc), Geometry2::Arc(after));
    cad.undo().unwrap();
    let n = cad.undo_names().len();
    assert!(
        cad.stretch_entities(&[line, arc], v(5.0, -5.0), &[(arc, box_)])
            .is_err()
    );
    assert_eq!(geometry(&cad, arc), before);
    assert_eq!(line_ends(&cad, line), (p(0.0, -10.0), p(2.0, -10.0)));
    assert_eq!(cad.undo_names().len(), n);
    assert!(cad.redo_name().is_some());
}

fn path_widths(cad: &CadDocument, id: EntityId) -> Vec<(f64, f64)> {
    cad.document().join_path(id).unwrap().0.widths
}

fn wide_polyline(cad: &mut CadDocument, a: Point2, b: Point2, width: f64) -> EntityId {
    let style = arq_document::PolylineStyle {
        width,
        ..Default::default()
    };
    cad.add_polyline_with(&[(a, 0.0), (b, 0.0)], false, style)
        .unwrap()
        .0
}

#[test]
fn join_gives_lines_and_arcs_a_source_polylines_one_width() {
    // J3: a polyline (0,0)-(4,0) of width 0.5 and the line (4,0)-(4,3):
    // the line's segment is 0.5 wide too, as ProgeCAD gives it.
    let mut cad = CadDocument::new();
    let source = wide_polyline(&mut cad, p(0.0, 0.0), p(4.0, 0.0), 0.5);
    let (line, _) = cad.add_line(p(4.0, 0.0), p(4.0, 3.0)).unwrap();
    cad.join_entities(Some(source), &[line]).unwrap();
    assert_eq!(path_widths(&cad, source), [(0.5, 0.5), (0.5, 0.5)]);
    // An arc before its start takes it too; a polyline of its own width
    // keeps that.
    let mut cad = CadDocument::new();
    let source = wide_polyline(&mut cad, p(0.0, 0.0), p(4.0, 0.0), 0.5);
    let (arc, _) = cad.add_arc(p(-1.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
    let other = wide_polyline(&mut cad, p(4.0, 0.0), p(8.0, 0.0), 0.8);
    cad.join_entities(Some(source), &[arc, other]).unwrap();
    assert_eq!(
        path_widths(&cad, source),
        [(0.5, 0.5), (0.5, 0.5), (0.8, 0.8)]
    );
    // A source of varying width gives none: the line keeps 0.
    let line = cad.add_line(p(8.0, 0.0), p(8.0, 3.0)).unwrap().0;
    cad.join_entities(Some(source), &[line]).unwrap();
    assert_eq!(path_widths(&cad, source).last(), Some(&(0.0, 0.0)));
    // Nor does a source of no width.
    let mut cad = CadDocument::new();
    let source = wide_polyline(&mut cad, p(0.0, 0.0), p(4.0, 0.0), 0.0);
    let (line, _) = cad.add_line(p(4.0, 0.0), p(4.0, 3.0)).unwrap();
    cad.join_entities(Some(source), &[line]).unwrap();
    assert_eq!(path_widths(&cad, source), [(0.0, 0.0), (0.0, 0.0)]);
}

#[test]
fn many_at_once_a_chain_from_a_polyline_of_one_width_gives_it_to_its_lines() {
    // The polyline selected first seeds the chain: the line takes its width.
    let mut cad = CadDocument::new();
    let polyline = wide_polyline(&mut cad, p(0.0, 0.0), p(4.0, 0.0), 0.5);
    let (line, _) = cad.add_line(p(4.0, 0.0), p(4.0, 3.0)).unwrap();
    let (joined, _) = cad.join_entities(None, &[polyline, line]).unwrap();
    assert_eq!(joined.results, [polyline]);
    assert_eq!(path_widths(&cad, polyline), [(0.5, 0.5), (0.5, 0.5)]);
}

#[test]
fn join_from_an_elliptical_arc_takes_its_ellipses_arcs_in_one_step() {
    // About (10,0), semi-axes 4 and 2: 0 to 1, then 1.5 to 2.5 (a gap
    // filled) and an arc of another ellipse, left.
    let mut cad = CadDocument::new();
    let source = red_ellipse(&mut cad, 0.0, 1.0);
    let e = Ellipse::new(p(10.0, 0.0), v(4.0, 0.0), 0.5, 1.5, 2.5).unwrap();
    let (next, _) = cad.add_ellipse(e).unwrap();
    let f = Ellipse::new(p(10.0, 0.0), v(4.0, 0.0), 0.4, 3.0, 4.0).unwrap();
    let (other, _) = cad.add_ellipse(f).unwrap();
    let (line, _) = cad.add_line(p(14.0, 0.0), p(20.0, 0.0)).unwrap();
    let (joined, changes) = cad
        .join_entities(Some(source), &[next, other, line])
        .unwrap();
    assert_eq!(joined.results, [source], "its id kept");
    assert_eq!((joined.joined, joined.discarded), (1, 2));
    let arc = drawn_ellipse(&cad, source);
    assert!(arc.start_param().abs() < 1e-12 && (arc.sweep() - 2.5).abs() < 1e-12);
    assert_eq!(
        cad.document().entity(source).unwrap().color(),
        Color::Index(1)
    );
    assert!(cad.document().entity(next).is_none());
    assert!(cad.document().entity(other).is_some() && cad.document().entity(line).is_some());
    assert_eq!(changes.deleted(), [next]);
    assert_eq!(cad.undo_name(), Some("JOIN"));
    cad.undo().unwrap();
    assert!((drawn_ellipse(&cad, source).sweep() - 1.0).abs() < 1e-12);
    assert!(cad.document().entity(next).is_some());
}

#[test]
fn elliptical_arcs_join_many_at_once_and_close_into_their_ellipse() {
    // Three arcs going round: many at once, the whole ellipse in the
    // first's entity; a polyline beside them chains none of them.
    let mut cad = CadDocument::new();
    let a = red_ellipse(&mut cad, 0.0, 2.0);
    let b = red_ellipse(&mut cad, 2.0, 4.0);
    let c = red_ellipse(&mut cad, 4.0, 0.0);
    let (polyline, _) = cad
        .add_polyline(&[(p(14.0, 0.0), 0.0), (p(20.0, 0.0), 0.0)], false)
        .unwrap();
    let (joined, _) = cad.join_entities(None, &[a, polyline, b, c]).unwrap();
    assert_eq!(joined.results, [a]);
    assert!(drawn_ellipse(&cad, a).is_full());
    assert!(cad.document().entity(b).is_none() && cad.document().entity(c).is_none());
    assert_eq!(path_points(&cad, polyline), [p(14.0, 0.0), p(20.0, 0.0)]);
    // A polyline as the source takes none of an ellipse's arcs.
    let mut cad = CadDocument::new();
    let arc = red_ellipse(&mut cad, 0.0, 1.0);
    let (source, _) = cad
        .add_polyline(&[(p(20.0, 0.0), 0.0), (p(14.0, 0.0), 0.0)], false)
        .unwrap();
    let (joined, _) = cad.join_entities(Some(source), &[arc]).unwrap();
    assert_eq!((joined.joined, joined.discarded), (0, 1));
    // Close: an arc becomes its whole ellipse, its id kept; a whole one,
    // and a whole one as a source, are refused.
    let (closed, _) = cad.close_arc(arc).unwrap();
    assert_eq!(closed, arc);
    assert!(drawn_ellipse(&cad, arc).is_full());
    assert_eq!(cad.undo_name(), Some("JOIN"));
    assert!(cad.close_arc(arc).is_err());
    let other = red_ellipse(&mut cad, 1.0, 2.0);
    let (joined, _) = cad.join_entities(Some(arc), &[other]).unwrap();
    assert_eq!(joined.joined, 0);
}

#[test]
fn a_spline_in_the_plan_moves_turns_scales_and_mirrors_as_a_whole() {
    let codes: Vec<String> = {
        let mut c: Vec<String> = [
            "0", "SPLINE", "8", "0", "210", "0", "220", "0", "230", "1", "70", "8", "71", "3",
            "72", "8", "73", "4", "74", "0",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        for k in [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
            c.extend(["40".to_owned(), format!("{k:?}")]);
        }
        for (x, y) in [(0.0, 0.0), (2.0, 3.0), (4.0, 3.0), (6.0, 0.0)] {
            c.extend([
                "10".to_owned(),
                format!("{x:?}"),
                "20".to_owned(),
                format!("{y:?}"),
                "30".to_owned(),
                "1.5".to_owned(),
            ]);
        }
        c
    };
    let dxf = format!(
        "0\nSECTION\n2\nENTITIES\n{}\n0\nENDSEC\n0\nEOF\n",
        codes.join("\n")
    );
    let drawn = |cad: &CadDocument, id: EntityId| -> Vec<Point2> {
        let items = cad.document().drawing(id).unwrap();
        let [item] = &items[..] else {
            panic!("{items:?}")
        };
        let arq_document::Shape::Spline(s) = &item.shape else {
            panic!("{item:?}")
        };
        s.flatten().to_vec()
    };
    let axis = Line::new(p(0.0, 0.0), p(1.0, 3.0)).unwrap();
    type Op = fn(&mut CadDocument, EntityId);
    let ops: [(Op, Transform2); 4] = [
        (
            |cad, id| drop(cad.move_entities(&[id], v(4.0, -1.0)).unwrap()),
            Transform2::translation(v(4.0, -1.0)),
        ),
        (
            |cad, id| drop(cad.rotate_entities(&[id], p(1.0, 1.0), 0.7).unwrap()),
            Transform2::rotation(p(1.0, 1.0), 0.7).unwrap(),
        ),
        (
            |cad, id| drop(cad.scale_entities(&[id], p(0.0, 0.0), 2.5).unwrap()),
            Transform2::scaling(p(0.0, 0.0), 2.5).unwrap(),
        ),
        (
            |cad, id| {
                let axis = Line::new(p(0.0, 0.0), p(1.0, 3.0)).unwrap();
                drop(cad.mirror_entities(&[id], axis).unwrap())
            },
            Transform2::mirror(axis),
        ),
    ];
    for (op, t) in ops {
        let doc = arq_document::Document::from_dxf_bytes(dxf.clone().into_bytes()).unwrap();
        let mut cad = CadDocument::from_document(doc);
        let id = cad.document().entities()[0].id();
        assert!(cad.document().entity(id).unwrap().is_transformable());
        let before = drawn(&cad, id);
        op(&mut cad, id);
        let after = drawn(&cad, id);
        assert_eq!(before.len(), after.len());
        for (a, b) in before.iter().zip(&after) {
            assert!(near(t.apply_point(*a), *b), "{t:?}: {a:?} -> {b:?}");
        }
        cad.undo().unwrap();
        assert_eq!(drawn(&cad, id), before);
    }
}

/// A drawing of the Bézier spline from (0,0) by (2,3) and (4,3) to (6,0),
/// read from DXF: x is 6u, y is 9u(1 − u).
fn bezier_spline() -> (CadDocument, EntityId) {
    let mut c: Vec<String> = [
        "0", "SPLINE", "8", "0", "210", "0", "220", "0", "230", "1", "70", "8", "71", "3", "72",
        "8", "73", "4", "74", "0",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for k in [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
        c.extend(["40".to_owned(), format!("{k:?}")]);
    }
    for (x, y) in [(0.0, 0.0), (2.0, 3.0), (4.0, 3.0), (6.0, 0.0)] {
        c.extend([
            "10".to_owned(),
            format!("{x:?}"),
            "20".to_owned(),
            format!("{y:?}"),
            "30".to_owned(),
            "0.0".to_owned(),
        ]);
    }
    let dxf = format!(
        "0\nSECTION\n2\nENTITIES\n{}\n0\nENDSEC\n0\nEOF\n",
        c.join("\n")
    );
    let doc = arq_document::Document::from_dxf_bytes(dxf.into_bytes()).unwrap();
    let cad = CadDocument::from_document(doc);
    let id = cad.document().entities()[0].id();
    (cad, id)
}

#[test]
fn trim_cuts_at_a_spline_and_extend_reaches_one() {
    // y = 1 meets it where u(1 − u) = 1/9: x = 6u.
    let root = 5f64.sqrt() / 3.0;
    let (x1, x2) = (3.0 * (1.0 - root), 3.0 * (1.0 + root));
    let (mut cad, spline) = bezier_spline();
    let (line, _) = cad.add_line(p(-1.0, 1.0), p(7.0, 1.0)).unwrap();
    let (left, _) = cad.trim_entity(line, p(3.0, 1.0), None, true).unwrap();
    let mut ends: Vec<f64> = left
        .iter()
        .flat_map(|&id| {
            let (a, b) = line_ends(&cad, id);
            [a.x(), b.x()]
        })
        .collect();
    ends.sort_by(f64::total_cmp);
    assert_eq!(ends.len(), 4, "{ends:?}");
    assert!(
        (ends[1] - x1).abs() < 1e-9 && (ends[2] - x2).abs() < 1e-9,
        "{ends:?}"
    );
    // A line along y = 1 from the left grows to it.
    let (short, _) = cad.add_line(p(-5.0, 1.0), p(-2.0, 1.0)).unwrap();
    cad.extend_entity(short, p(-2.0, 1.0), Some(&[spline]))
        .unwrap();
    let (_, end) = line_ends(&cad, short);
    assert!(near(end, p(x1, 1.0)), "{end:?}");
    // The spline itself is not trimmed yet (ADR 0043 D5).
    let before = cad.document().entity(spline).unwrap().bounds();
    assert!(cad.trim_entity(spline, p(3.0, 2.25), None, true).is_err());
    assert_eq!(cad.document().entity(spline).unwrap().bounds(), before);
}
