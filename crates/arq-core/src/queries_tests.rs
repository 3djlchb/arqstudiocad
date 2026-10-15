//! Tests of the spatial queries (a submodule of `queries`).
//!
//! Expected boxes are worked out by hand from the drawn shapes.

use std::f64::consts::{FRAC_PI_2, PI};

use arq_document::Document;
use arq_geometry::{Point2, Tolerance};

use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn area(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds2 {
    Bounds2::new(p(x0, y0), p(x1, y1))
}

#[test]
fn an_empty_drawing_has_no_bounds_and_finds_nothing() {
    let cad = CadDocument::new();
    assert_eq!(cad.bounds(), None);
    assert!(cad.query_entities(area(-1e9, -1e9, 1e9, 1e9)).is_empty());
}

#[test]
fn bounds_cover_every_entity() {
    let mut cad = CadDocument::new();
    cad.add_line(p(-2.0, 1.0), p(3.0, 4.0)).unwrap();
    cad.add_circle(p(10.0, 0.0), 1.0).unwrap();
    // Upper half circle around (0, -5), radius 2: x in [-2, 2], y in [-5, -3].
    cad.add_arc(p(0.0, -5.0), 2.0, 0.0, PI).unwrap();
    let b = cad.bounds().unwrap();
    assert!(
        b.approx_eq(area(-2.0, -5.0, 11.0, 4.0), Tolerance::default()),
        "{b:?}"
    );
}

#[test]
fn query_finds_the_entities_whose_box_touches_the_area() {
    let mut cad = CadDocument::new();
    let (left, _) = cad.add_line(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let (right, _) = cad.add_circle(p(10.0, 0.0), 1.0).unwrap();
    // Quarter arc from (21, 0) to (20, 1): its box is [20, 21] × [0, 1].
    let (arc, _) = cad.add_arc(p(20.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();

    assert_eq!(cad.query_entities(area(-1.0, -1.0, 0.5, 1.0)), [left]);
    assert_eq!(cad.query_entities(area(8.0, -5.0, 9.5, 5.0)), [right]);
    assert_eq!(
        cad.query_entities(area(-5.0, -5.0, 30.0, 5.0)),
        [left, right, arc]
    );
    // Touching an edge counts; just beyond it does not.
    assert_eq!(cad.query_entities(area(11.0, 0.0, 12.0, 1.0)), [right]);
    assert!(cad.query_entities(area(11.001, 0.0, 12.0, 1.0)).is_empty());
    // Below the quarter arc: a whole circle there would reach y = -1.
    assert_eq!(cad.query_entities(area(19.0, -2.0, 25.0, -0.5)), []);
}

#[test]
fn query_follows_edits_and_undo() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_circle(p(0.0, 0.0), 1.0).unwrap();
    let there = area(99.0, 99.0, 101.0, 101.0);
    cad.move_entities(&[id], arq_geometry::Vector2::new(100.0, 100.0).unwrap())
        .unwrap();
    assert_eq!(cad.query_entities(there), [id]);
    cad.undo().unwrap();
    assert!(cad.query_entities(there).is_empty());
    cad.delete_entities(&[id]).unwrap();
    assert!(cad.query_entities(area(-2.0, -2.0, 2.0, 2.0)).is_empty());
}

#[test]
fn queries_ignore_layer_state() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    let (hidden, _) = cad.create_layer("OCULTA").unwrap();
    cad.set_entity_layer(&[id], hidden).unwrap();
    cad.set_layer_visible(hidden, false).unwrap();
    cad.set_layer_frozen(hidden, true).unwrap();
    assert_eq!(cad.query_entities(area(0.0, 0.0, 1.0, 1.0)), [id]);
    assert!(cad.bounds().is_some());
}

#[test]
fn read_only_entities_are_found_by_their_codec_box() {
    let mut doc = Document::new();
    let text = doc.insert_text_for_tests("NOTA", 50.0, 50.0);
    let cad = CadDocument::from_document(doc);
    assert_eq!(cad.query_entities(area(49.0, 49.0, 60.0, 60.0)), [text]);
    assert!(cad.query_entities(area(0.0, 0.0, 10.0, 10.0)).is_empty());
}

#[test]
fn pick_window_and_snap_answer_on_drawn_geometry() {
    use arq_geometry::snap::SnapKind;
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let (circle, _) = cad.add_circle(p(20.0, 0.0), 2.0).unwrap();
    assert_eq!(cad.pick(p(5.0, 0.2), 0.5).unwrap(), Some(line));
    assert_eq!(cad.pick(p(5.0, 1.0), 0.5).unwrap(), None);
    assert_eq!(
        cad.select_window(area(-1.0, -1.0, 11.0, 1.0), false),
        [line]
    );
    assert_eq!(
        cad.select_window(area(21.5, -1.0, 23.0, 1.0), true),
        [circle]
    );
    let snap = cad
        .snap(p(9.8, 0.1), 0.5, &[SnapKind::Endpoint], None)
        .unwrap();
    assert_eq!(
        snap.map(|s| (s.kind, s.point)),
        Some((SnapKind::Endpoint, p(10.0, 0.0)))
    );
}

#[test]
fn an_aperture_must_be_a_finite_distance() {
    let cad = CadDocument::new();
    for bad in [f64::NAN, f64::INFINITY, -0.5] {
        assert!(
            matches!(
                cad.pick(p(0.0, 0.0), bad),
                Err(CadError::InvalidGeometry(_))
            ),
            "{bad}"
        );
        assert!(matches!(
            cad.snap(
                p(0.0, 0.0),
                bad,
                &[arq_geometry::snap::SnapKind::Nearest],
                None
            ),
            Err(CadError::InvalidGeometry(_))
        ));
    }
    assert_eq!(cad.pick(p(0.0, 0.0), 0.0).unwrap(), None, "zero is allowed");
}

/// Locked layers are picked (ADR 0016 D2); every operation still refuses
/// their entities.
#[test]
fn entities_on_locked_layers_are_picked_but_not_edited() {
    let mut cad = CadDocument::new();
    let (line, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let layer0 = cad.document().layer_by_name("0").unwrap().id();
    let (other, _) = cad.create_layer("OTRA").unwrap();
    cad.set_current_layer(other).unwrap();
    cad.set_layer_locked(layer0, true).unwrap();
    assert_eq!(cad.pick(p(5.0, 0.1), 0.5).unwrap(), Some(line));
    assert!(
        cad.move_entities(&[line], arq_geometry::Vector2::new(1.0, 0.0).unwrap())
            .is_err()
    );
}
