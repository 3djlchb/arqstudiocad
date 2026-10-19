//! Tests of TEXT and MTEXT (ADR 0022), a submodule of `entities`.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_6};

use acadrust::EntityType;
use acadrust::entities::{AttachmentPoint, TextHorizontalAlignment as H};
use acadrust::types::Vector3;
use arq_geometry::{Line, Point2, Transform2, Vector2};

use crate::document::Document;
use crate::entity::{EntityId, TextData};
use crate::error::DocumentError;
use crate::grips::GripKind;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn add(doc: &mut Document, e: EntityType) -> EntityId {
    doc.cad_mut().add_entity(e).unwrap().value()
}

fn text(value: &str, x: f64, y: f64) -> acadrust::Text {
    acadrust::Text::with_value(value, Vector3::new(x, y, 0.0))
}

fn mtext(value: &str, x: f64, y: f64) -> acadrust::MText {
    let mut t = acadrust::MText::new();
    t.value = value.to_owned();
    t.insertion_point = Vector3::new(x, y, 0.0);
    t.height = 2.0;
    t
}

fn data(doc: &Document, id: EntityId) -> TextData {
    doc.entity(id).unwrap().text().unwrap().clone()
}

fn near(a: Point2, x: f64, y: f64) -> bool {
    (a.x() - x).abs() < 1e-9 && (a.y() - y).abs() < 1e-9
}

/// Same direction, whatever turn it is written with.
fn same_angle(a: f64, b: f64) -> bool {
    (a.cos() - b.cos()).abs() < 1e-9 && (a.sin() - b.sin()).abs() < 1e-9
}

fn vertical_mirror() -> Transform2 {
    Transform2::mirror(Line::new(p(0.0, 0.0), p(0.0, 1.0)).unwrap())
}

#[test]
fn texts_drawn_in_plan_move_and_rotate_as_a_whole() {
    let mut doc = Document::new();
    let single = add(&mut doc, EntityType::Text(text("A", 1.0, 2.0)));
    let multi = add(&mut doc, EntityType::MText(Box::new(mtext("B", 3.0, 4.0))));
    let mut tilted = text("C", 0.0, 0.0);
    tilted.normal = Vector3::new(1.0, 0.0, 0.0);
    let tilted = add(&mut doc, EntityType::Text(tilted));
    assert!(doc.entity(single).unwrap().is_transformable());
    assert!(doc.entity(multi).unwrap().is_transformable());
    assert!(
        !doc.entity(tilted).unwrap().is_transformable(),
        "a tilted text stays read-only"
    );
    assert!(doc.entity(tilted).unwrap().text().is_none());

    let shift = Transform2::translation(Vector2::new(10.0, 0.0).unwrap());
    for id in [single, multi] {
        doc.transform_entity(id, shift).unwrap();
    }
    assert!(near(data(&doc, single).anchor(), 11.0, 2.0));
    assert!(near(data(&doc, multi).anchor(), 13.0, 4.0));

    let quarter = Transform2::rotation(p(0.0, 0.0), FRAC_PI_2).unwrap();
    doc.transform_entity(single, quarter).unwrap();
    let turned = data(&doc, single);
    assert!(near(turned.anchor(), -2.0, 11.0));
    assert!(same_angle(turned.rotation(), FRAC_PI_2));
}

#[test]
fn a_text_in_a_mirrored_ocs_turns_the_right_way() {
    let mut doc = Document::new();
    let mut down = text("A", 10.0, 0.0);
    down.normal = Vector3::new(0.0, 0.0, -1.0);
    let id = add(&mut doc, EntityType::Text(down));
    let before = data(&doc, id);
    assert!(
        near(before.anchor(), -10.0, 0.0),
        "drawn where the OCS puts it"
    );
    assert!(same_angle(before.rotation(), std::f64::consts::PI));
    doc.transform_entity(id, Transform2::rotation(p(0.0, 0.0), FRAC_PI_2).unwrap())
        .unwrap();
    let after = data(&doc, id);
    assert!(near(after.anchor(), 0.0, -10.0));
    assert!(
        same_angle(after.rotation(), 3.0 * FRAC_PI_2),
        "{}",
        after.rotation()
    );
}

#[test]
fn mirrored_texts_stay_readable() {
    let mut doc = Document::new();
    let left = add(&mut doc, EntityType::Text(text("A", 2.0, 3.0)));
    doc.transform_entity(left, vertical_mirror()).unwrap();
    let mirrored = data(&doc, left);
    assert_eq!(
        mirrored.justification(),
        "Right",
        "its other end now hangs there"
    );
    assert!(near(mirrored.anchor(), -2.0, 3.0));
    assert!(same_angle(mirrored.rotation(), 0.0), "still left to right");

    let below = add(&mut doc, EntityType::Text(text("B", 2.0, 3.0)));
    let horizontal = Transform2::mirror(Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap());
    doc.transform_entity(below, horizontal).unwrap();
    let flipped = data(&doc, below);
    assert_eq!(flipped.justification(), "Left");
    assert!(near(flipped.anchor(), 2.0, -3.0));
    assert!(
        same_angle(flipped.rotation(), 0.0),
        "upright, not upside down"
    );

    let mut right = text("C", 0.0, 0.0);
    right.horizontal_alignment = H::Right;
    right.alignment_point = Some(Vector3::new(5.0, 1.0, 0.0));
    let right = add(&mut doc, EntityType::Text(right));
    assert!(
        near(data(&doc, right).anchor(), 5.0, 1.0),
        "a right text hangs from its end"
    );
    doc.transform_entity(right, vertical_mirror()).unwrap();
    let back = data(&doc, right);
    assert_eq!(back.justification(), "Left");
    assert!(near(back.anchor(), -5.0, 1.0));

    let mut slanted = text("D", 4.0, 0.0);
    slanted.rotation = FRAC_PI_6;
    let slanted = add(&mut doc, EntityType::Text(slanted));
    doc.transform_entity(slanted, vertical_mirror()).unwrap();
    assert!(same_angle(data(&doc, slanted).rotation(), -FRAC_PI_6));

    let multi = add(&mut doc, EntityType::MText(Box::new(mtext("E", 2.0, 3.0))));
    doc.transform_entity(multi, vertical_mirror()).unwrap();
    let column = data(&doc, multi);
    assert_eq!(column.justification(), "TopRight");
    assert!(near(column.anchor(), -2.0, 3.0));
    assert!(same_angle(column.rotation(), 0.0));
}

#[test]
fn what_a_text_tells_and_what_can_be_edited() {
    let mut doc = Document::new();
    let mut aligned = text("A", 0.0, 0.0);
    aligned.horizontal_alignment = H::Aligned;
    aligned.alignment_point = Some(Vector3::new(4.0, 0.0, 0.0));
    let aligned = add(&mut doc, EntityType::Text(aligned));
    let formatted = add(
        &mut doc,
        EntityType::MText(Box::new(mtext("{\\fArial;A}\\PB", 0.0, 0.0))),
    );
    let plain = add(
        &mut doc,
        EntityType::MText(Box::new(mtext("Plain", 0.0, 0.0))),
    );
    let mut middle = mtext("M", 0.0, 0.0);
    middle.attachment_point = AttachmentPoint::MiddleCenter;
    let middle = add(&mut doc, EntityType::MText(Box::new(middle)));

    let a = data(&doc, aligned);
    assert_eq!(
        (a.justification(), a.sizes_editable(), a.anchor()),
        ("Aligned", false, p(0.0, 0.0))
    );
    assert!(
        !data(&doc, formatted).content_editable(),
        "formatting is kept"
    );
    let m = data(&doc, plain);
    assert!(m.is_multiline() && m.content_editable() && m.content() == "Plain");
    assert_eq!(data(&doc, middle).justification(), "MiddleCenter");

    for (id, content, height) in [(formatted, Some("X"), None), (aligned, None, Some(3.0))] {
        assert!(matches!(
            doc.set_text(id, content, height, None),
            Err(DocumentError::UnsupportedOperation(_))
        ));
    }
}

#[test]
fn texts_are_written_and_edited_with_checked_values() {
    let mut doc = Document::new();
    assert!((doc.text_size() - 2.5).abs() < 1e-12);
    doc.cad_mut().header.text_height = 0.0;
    assert!((doc.text_size() - 2.5).abs() < 1e-12, "no usable TEXTSIZE");
    doc.cad_mut().header.text_height = 0.2;
    assert!((doc.text_size() - 0.2).abs() < 1e-12);

    let id = doc.add_text("Hola", p(1.0, 1.0), 2.0, 0.3).unwrap();
    let e = doc.entity(id).unwrap();
    assert_eq!((e.kind(), e.layer()), ("TEXT", "0"));
    let t = data(&doc, id);
    assert_eq!(
        (t.content(), t.style(), t.justification()),
        ("Hola", "Standard", "Left")
    );
    assert!(near(t.anchor(), 1.0, 1.0) && (t.height() - 2.0).abs() < 1e-12);
    assert!(same_angle(t.rotation(), 0.3));

    doc.set_text(id, Some("Adiós"), Some(3.0), Some(FRAC_PI_2))
        .unwrap();
    let t = data(&doc, id);
    assert_eq!(t.content(), "Adiós");
    assert!((t.height() - 3.0).abs() < 1e-12 && same_angle(t.rotation(), FRAC_PI_2));

    for (content, height, rotation) in [
        (Some(""), None, None),
        (Some("a\nb"), None, None),
        (None, Some(0.0), None),
        (None, Some(f64::NAN), None),
        (None, None, Some(f64::INFINITY)),
    ] {
        assert!(matches!(
            doc.set_text(id, content, height, rotation),
            Err(DocumentError::InvalidText(_))
        ));
    }
    assert!(matches!(
        doc.add_text("", p(0.0, 0.0), 1.0, 0.0),
        Err(DocumentError::InvalidText(_))
    ));
    let line = doc
        .add_entity(
            crate::entity::PlanGeometry::new(Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap().into()),
            doc.current_layer().id(),
        )
        .unwrap();
    assert!(matches!(
        doc.set_text(line, Some("x"), None, None),
        Err(DocumentError::NotEditable { .. })
    ));
}

#[test]
fn a_text_shows_one_grip_at_its_anchor_and_copies() {
    let mut doc = Document::new();
    let mut centered = text("A", 0.0, 0.0);
    centered.horizontal_alignment = H::Center;
    centered.alignment_point = Some(Vector3::new(7.0, 8.0, 0.0));
    let id = add(&mut doc, EntityType::Text(centered));
    let grips = doc.grips(&[id]);
    assert_eq!(grips.len(), 1);
    assert_eq!(grips[0].kind, GripKind::Insertion);
    assert!(near(grips[0].point, 7.0, 8.0));
    let copy = doc.copy_entity(id).unwrap();
    assert_eq!(data(&doc, copy).content(), "A");
}
