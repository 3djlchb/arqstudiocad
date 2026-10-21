//! Tests of the block editor (a submodule of `block_editor`).

use arq_geometry::{Line, Point2};

use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::{Color, EntityId, PlanGeometry};
use crate::error::DocumentError;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(doc: &mut Document, a: (f64, f64), b: (f64, f64)) -> EntityId {
    let layer = doc.current_layer().id();
    doc.add_entity(
        PlanGeometry::new(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap().into()),
        layer,
    )
    .unwrap()
}

/// The lines drawn for the whole drawing, rounded, sorted.
fn lines(doc: &Document) -> Vec<((i64, i64), (i64, i64))> {
    let r = |q: Point2| ((q.x() * 1e6).round() as i64, (q.y() * 1e6).round() as i64);
    let mut found: Vec<_> = doc
        .drawing_all()
        .into_iter()
        .flat_map(|(_, items)| items)
        .filter_map(|item| match item.shape {
            Shape::Line(l) => Some((r(l.start()), r(l.end()))),
            _ => None,
        })
        .collect();
    found.sort_unstable();
    found
}

fn m(x: i64, y: i64) -> (i64, i64) {
    (x * 1_000_000, y * 1_000_000)
}

/// A drawing with block MARCO, based at (10,10): lines (10,10)-(14,10) and
/// (10,10)-(10,13), referenced where they were; and a loose line
/// (0,0)-(1,0).
fn frame() -> Document {
    let mut doc = Document::new();
    let a = line(&mut doc, (10.0, 10.0), (14.0, 10.0));
    let b = line(&mut doc, (10.0, 10.0), (10.0, 13.0));
    line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    doc.create_block("MARCO", p(10.0, 10.0), &[a, b]).unwrap();
    doc
}

#[test]
fn the_editor_holds_the_block_alone_in_its_own_coordinates() {
    let doc = frame();
    let editor = doc.block_editor("marco").unwrap();
    assert_eq!(
        lines(&editor),
        [(m(10, 10), m(10, 13)), (m(10, 10), m(14, 10))],
        "the contents only, where the definition has them"
    );
    let names = |d: &Document| {
        d.layers()
            .iter()
            .map(|l| l.name().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&editor), names(&doc));
    assert_eq!(editor.block_names(), ["MARCO"]);
    assert_eq!(doc.entities().len(), 2, "the drawing is untouched");
}

#[test]
fn saving_replaces_the_contents_every_reference_draws() {
    let mut doc = frame();
    let mut editor = doc.block_editor("MARCO").unwrap();
    let first = editor.entities()[0].id();
    editor.delete_entity(first).unwrap();
    let walls = editor.create_layer("NUEVA").unwrap();
    editor.set_layer_color(walls, Color::Index(3)).unwrap();
    editor.set_current_layer(walls).unwrap();
    line(&mut editor, (10.0, 10.0), (12.0, 12.0));
    doc.replace_block_contents("MARCO", &editor).unwrap();
    assert_eq!(
        lines(&doc),
        [
            (m(0, 0), m(1, 0)),
            (m(10, 10), m(10, 13)),
            (m(10, 10), m(12, 12))
        ],
        "the reference draws the new contents"
    );
    let added = doc.layer_by_name("NUEVA").expect("the editor's new layer");
    assert_eq!(added.color(), Color::Index(3));
    // Saving again what the editor holds changes nothing more.
    doc.replace_block_contents("MARCO", &editor).unwrap();
    assert_eq!(lines(&doc).len(), 3);
}

#[test]
fn a_block_made_in_the_editor_comes_along() {
    let mut doc = frame();
    let mut editor = doc.block_editor("MARCO").unwrap();
    let bolt = line(&mut editor, (11.0, 11.0), (12.0, 11.0));
    editor
        .create_block("TORNILLO", p(11.0, 11.0), &[bolt])
        .unwrap();
    doc.replace_block_contents("MARCO", &editor).unwrap();
    assert_eq!(doc.block_names(), ["MARCO", "TORNILLO"]);
    assert!(
        lines(&doc).contains(&(m(11, 11), m(12, 11))),
        "drawn nested"
    );
}

#[test]
fn a_block_cannot_hold_itself() {
    let mut doc = frame();
    let mut editor = doc.block_editor("MARCO").unwrap();
    editor.insert_block("MARCO", p(0.0, 0.0), 1.0, 0.0).unwrap();
    let before = lines(&doc);
    let err = doc.replace_block_contents("MARCO", &editor).unwrap_err();
    assert!(matches!(err, DocumentError::InvalidBlockName(m) if m.contains("itself")));
    assert_eq!(lines(&doc), before);
}

#[test]
fn only_blocks_insert_can_place_are_edited() {
    let doc = frame();
    for name in ["NADA", "*Model_Space", ""] {
        assert!(
            matches!(
                doc.block_editor(name),
                Err(DocumentError::InvalidBlockName(_))
            ),
            "{name:?}"
        );
    }
    assert!(!doc.is_dynamic_block("MARCO"));
}
