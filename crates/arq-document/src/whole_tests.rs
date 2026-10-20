//! Tests of moving, rotating, scaling, mirroring and copying blocks and
//! polylines as a whole (ADR 0016, D1), a submodule of `entities`.
//!
//! Checked through what is drawn: after a transformation, the drawing of an
//! entity must be the transformation of its drawing before, whatever the
//! codec stores (a mirrored block may keep a negative X or Y scale).

use std::f64::consts::FRAC_PI_2;

use acadrust::EntityType;
use arq_geometry::{Geometry2, Line, Point2, Tolerance, Transform2, Vector2};

use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::EntityId;
use crate::error::DocumentError;
use crate::test_dxf::{LAYERS, door_block, dxf};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

const TOL: Tolerance = Tolerance::DEFAULT;

/// The PUERTA block inserted at (10,5), scale 1, no rotation: lines
/// (10,5)-(11,5) and (10,5)-(10,6), and an attribute NUM at (10.5,5.5).
fn door_insert() -> Vec<&'static str> {
    vec![
        "0",
        "INSERT",
        "8",
        "A-PUERTAS",
        "66",
        "1",
        "2",
        "PUERTA",
        "10",
        "10",
        "20",
        "5",
        "30",
        "0", //
        "0",
        "ATTRIB",
        "8",
        "A-PUERTAS",
        "10",
        "10.5",
        "20",
        "5.5",
        "30",
        "0",
        "40",
        "0.2",
        "1",
        "P1",
        "2",
        "NUM",
        "70",
        "0", //
        "0",
        "SEQEND",
        "8",
        "A-PUERTAS",
    ]
}

/// The same block with normal (0,0,-1): drawn at world (-10,0)-(-11,0) and
/// (-10,0)-(-10,1).
fn mirrored_normal_insert() -> Vec<&'static str> {
    vec![
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "0", "30", "0", "210", "0",
        "220", "0", "230", "-1",
    ]
}

/// A polyline at elevation 3: (0,0)-(4,0), then a half circle (bulge 1) to
/// (4,2), then back to (0,2).
fn bulged_polyline() -> Vec<&'static str> {
    vec![
        "0",
        "LWPOLYLINE",
        "8",
        "MUROS",
        "90",
        "3",
        "70",
        "0",
        "38",
        "3", //
        "10",
        "0",
        "20",
        "0",
        "42",
        "0", //
        "10",
        "4",
        "20",
        "0",
        "42",
        "1", //
        "10",
        "4",
        "20",
        "2",
        "42",
        "0",
    ]
}

/// Block ROSETA, base (0,0): a CIRCLE of radius 1 at (2,0) and a LINE
/// (0,0)-(2,0).
fn rosette_block() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "ROSETA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "ROSETA", //
        "0", "CIRCLE", "8", "0", "10", "2", "20", "0", "30", "0", "40", "1", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "2", "21", "0", "31",
        "0", //
        "0", "ENDBLK", "8", "0",
    ]
}

/// ROSETA at (3,3), scale 2, rotated 30°.
fn rosette_insert() -> Vec<&'static str> {
    vec![
        "0", "INSERT", "8", "0", "2", "ROSETA", "10", "3", "20", "3", "30", "0", "41", "2", "42",
        "2", "43", "2", "50", "30",
    ]
}

/// A block array: two columns.
fn minsert() -> Vec<&'static str> {
    vec![
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "0", "20", "0", "30", "0", "70", "2", "44",
        "3",
    ]
}

fn drawing_with(entities: &[&[&str]]) -> (Document, Vec<EntityId>) {
    let (door, rosette) = (door_block(), rosette_block());
    let doc = dxf(LAYERS, &[&door, &rosette], entities);
    let ids = doc.entities().iter().map(|e| e.id()).collect();
    (doc, ids)
}

/// Points along everything drawn for `id`, as a set to compare (the
/// direction of a curve does not matter): ends and middles of lines and
/// arcs; a circle's center and its radius as a point that far east of the
/// origin, since a rotated circle is the same circle, sampled from a new
/// east point.
fn drawn_points(doc: &Document, id: EntityId) -> Vec<Point2> {
    let mut points = Vec::new();
    let drawn = doc.drawing(id).unwrap();
    // An attribute's letters stay readable when its block is mirrored (ADR
    // 0022 D2): they are not the mirror image of themselves, so they are
    // left out; the block's geometry is what is compared.
    let text_boxes: Vec<_> = drawn
        .iter()
        .filter_map(|item| match item.shape {
            Shape::Text(b) => Some(b),
            _ => None,
        })
        .collect();
    let in_text = |l: Line| {
        text_boxes
            .iter()
            .any(|b| b.contains_point(l.start(), TOL) && b.contains_point(l.end(), TOL))
    };
    for item in drawn {
        let g: Geometry2 = match item.shape {
            Shape::Line(l) if in_text(l) => continue,
            Shape::Line(l) => l.into(),
            Shape::Arc(a) => a.into(),
            Shape::Circle(c) => {
                points.push(c.center());
                continue;
            }
            _ => continue,
        };
        points.extend([0.0, 0.25, 0.5, 0.75, 1.0].map(|t| g.point_at(t)));
    }
    points
}

/// The radii of the circles drawn for `id`, sorted.
fn drawn_radii(doc: &Document, id: EntityId) -> Vec<f64> {
    let mut radii: Vec<f64> = doc
        .drawing(id)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item.shape {
            Shape::Circle(c) => Some(c.radius()),
            _ => None,
        })
        .collect();
    radii.sort_by(f64::total_cmp);
    radii
}

fn same_points(found: &[Point2], expected: &[Point2]) -> bool {
    let tol = Tolerance::new(1e-9, 1e-9).unwrap();
    found.len() == expected.len()
        && expected
            .iter()
            .all(|e| found.iter().any(|f| f.approx_eq(*e, tol)))
        && found
            .iter()
            .all(|f| expected.iter().any(|e| e.approx_eq(*f, tol)))
}

fn transforms() -> Vec<(&'static str, Transform2)> {
    let axis = Line::new(p(12.0, -3.0), p(14.0, 7.0)).unwrap();
    vec![
        ("move", Transform2::translation(v(5.0, -1.0))),
        ("rotate", Transform2::rotation(p(3.0, 4.0), 0.6).unwrap()),
        ("scale", Transform2::scaling(p(-1.0, 2.0), 2.5).unwrap()),
        ("mirror", Transform2::mirror(axis)),
        (
            "mirror then rotate",
            Transform2::mirror(axis).then(Transform2::rotation(p(0.0, 0.0), 1.2).unwrap()),
        ),
    ]
}

#[test]
fn blocks_and_polylines_are_drawn_where_the_transformation_puts_them() {
    for (name, t) in transforms() {
        let (mut doc, ids) = drawing_with(&[
            &door_insert(),
            &mirrored_normal_insert(),
            &bulged_polyline(),
            &rosette_insert(),
        ]);
        for id in ids {
            let kind = doc.entity(id).unwrap().kind();
            let before: Vec<_> = drawn_points(&doc, id)
                .into_iter()
                .map(|q| t.apply_point(q))
                .collect();
            let radii: Vec<_> = drawn_radii(&doc, id)
                .into_iter()
                .map(|r| r * t.scale_factor())
                .collect();
            assert!(!before.is_empty(), "{kind} is drawn");
            doc.transform_entity(id, t).unwrap();
            let after = drawn_points(&doc, id);
            assert!(
                same_points(&after, &before),
                "{name} {kind}: {after:?} != {before:?}"
            );
            let found = drawn_radii(&doc, id);
            assert!(
                found.len() == radii.len()
                    && found.iter().zip(&radii).all(|(f, r)| (f - r).abs() < 1e-9),
                "{name} {kind}: radii {found:?} != {radii:?}"
            );
        }
    }
}

#[test]
fn rotating_a_door_by_hand() {
    // Rotating 90° around (10,5): (11,5) -> (10,6) and (10,6) -> (9,5).
    let (mut doc, ids) = drawing_with(&[&door_insert()]);
    doc.transform_entity(
        ids[0],
        Transform2::rotation(p(10.0, 5.0), FRAC_PI_2).unwrap(),
    )
    .unwrap();
    let ends = drawn_points(&doc, ids[0]);
    for expected in [p(10.0, 5.0), p(10.0, 6.0), p(9.0, 5.0)] {
        assert!(
            ends.iter().any(|q| q.approx_eq(expected, TOL)),
            "{expected:?} in {ends:?}"
        );
    }
}

#[test]
fn attributes_follow_their_block() {
    let (mut doc, ids) = drawing_with(&[&door_insert()]);
    doc.transform_entity(ids[0], Transform2::translation(v(5.0, -1.0)))
        .unwrap();
    let Some(EntityType::Insert(insert)) = doc.cad().get_entity(acadrust::Handle::new(ids[0]))
    else {
        panic!("not an insert")
    };
    let at = insert.attributes[0].insertion_point;
    assert!(
        (at.x - 15.5).abs() < 1e-9 && (at.y - 4.5).abs() < 1e-9,
        "{at:?}"
    );
}

#[test]
fn polylines_keep_their_elevation() {
    let (mut doc, ids) = drawing_with(&[&bulged_polyline()]);
    doc.transform_entity(ids[0], Transform2::rotation(p(1.0, 1.0), 2.0).unwrap())
        .unwrap();
    let Some(EntityType::LwPolyline(poly)) = doc.cad().get_entity(acadrust::Handle::new(ids[0]))
    else {
        panic!("not a polyline")
    };
    assert!((poly.elevation - 3.0).abs() < 1e-12);
}

#[test]
fn which_entities_transform_as_a_whole() {
    let (doc, ids) = drawing_with(&[&door_insert(), &bulged_polyline(), &minsert()]);
    let flags: Vec<_> = ids
        .iter()
        .map(|id| doc.entity(*id).unwrap().is_transformable())
        .collect();
    assert_eq!(flags, [true, true, false], "a block array is not");
    for id in &ids[..2] {
        assert!(
            !doc.entity(*id).unwrap().is_editable(),
            "their geometry is not editable"
        );
    }
}

#[test]
fn other_entities_are_refused() {
    let (mut doc, ids) = drawing_with(&[&minsert()]);
    // A POINT is still refused; texts transform since ADR 0022.
    let point = doc
        .cad_mut()
        .add_entity(EntityType::Point(acadrust::Point::from_coords(
            1.0, 2.0, 0.0,
        )))
        .unwrap()
        .value();
    let t = Transform2::translation(v(1.0, 0.0));
    for (id, kind) in [(ids[0], "MINSERT"), (point, "POINT")] {
        assert_eq!(
            doc.transform_entity(id, t),
            Err(DocumentError::NotEditable { id, kind })
        );
        assert_eq!(
            doc.copy_entity(id),
            Err(DocumentError::NotEditable { id, kind })
        );
    }
    assert_eq!(
        doc.transform_entity(999_999, t),
        Err(DocumentError::EntityNotFound(999_999))
    );
    assert_eq!(
        doc.copy_entity(999_999),
        Err(DocumentError::EntityNotFound(999_999))
    );
}

#[test]
fn a_copy_is_a_new_entity_drawn_where_the_original_is() {
    let (mut doc, ids) = drawing_with(&[&door_insert(), &bulged_polyline()]);
    for id in ids {
        let original = doc.entity(id).unwrap();
        let copy = doc.copy_entity(id).unwrap();
        assert_ne!(copy, id);
        let made = doc.entity(copy).unwrap();
        assert_eq!(
            (made.kind(), made.layer(), made.color()),
            (original.kind(), original.layer(), original.color())
        );
        assert!(same_points(
            &drawn_points(&doc, copy),
            &drawn_points(&doc, id)
        ));
        doc.transform_entity(copy, Transform2::translation(v(0.0, 10.0)))
            .unwrap();
        assert!(
            same_points(
                &drawn_points(&doc, id),
                &drawn_points(
                    &{
                        let (fresh, _) = drawing_with(&[&door_insert(), &bulged_polyline()]);
                        fresh
                    },
                    id
                )
            ),
            "the original stays"
        );
    }
    assert_eq!(doc.entities().len(), 4);
}

#[test]
fn a_copy_shares_no_links_with_the_original() {
    let (mut doc, ids) = drawing_with(&[&door_insert()]);
    let link = acadrust::Handle::new(0x9999);
    let original = doc
        .cad_mut()
        .get_entity_mut(acadrust::Handle::new(ids[0]))
        .unwrap();
    let common = original.common_mut();
    common.reactors = vec![link];
    common.xdictionary_handle = Some(link);
    common.graphic_data = Some(vec![1, 2, 3]);
    let copy = doc.copy_entity(ids[0]).unwrap();
    let made = doc
        .cad()
        .get_entity(acadrust::Handle::new(copy))
        .unwrap()
        .common();
    assert!(made.reactors.is_empty() && made.xdictionary_handle.is_none());
    assert!(made.graphic_data.is_none());
    let kept = doc
        .cad()
        .get_entity(acadrust::Handle::new(ids[0]))
        .unwrap()
        .common();
    assert_eq!(
        (kept.reactors.as_slice(), kept.xdictionary_handle),
        ([link].as_slice(), Some(link))
    );
}

#[test]
fn a_copied_block_has_attributes_of_its_own_and_survives_saving() {
    let (mut doc, ids) = drawing_with(&[&door_insert()]);
    let copy = doc.copy_entity(ids[0]).unwrap();
    doc.transform_entity(copy, Transform2::translation(v(3.0, 0.0)))
        .unwrap();
    let attribute = |doc: &Document, id: EntityId| {
        let Some(EntityType::Insert(insert)) = doc.cad().get_entity(acadrust::Handle::new(id))
        else {
            panic!("not an insert")
        };
        let a = &insert.attributes[0];
        (
            a.common.handle.value(),
            a.common.owner_handle.value(),
            a.value.clone(),
            a.insertion_point.x,
        )
    };
    let (h0, _, value0, x0) = attribute(&doc, ids[0]);
    let (h1, owner1, value1, x1) = attribute(&doc, copy);
    assert_ne!(h0, h1, "an attribute handle of its own");
    assert!(h1 != copy && h1 != 0);
    assert_eq!(owner1, copy, "owned by the copy");
    assert_eq!((value0.as_str(), value1.as_str()), ("P1", "P1"));
    assert!((x1 - x0 - 3.0).abs() < 1e-9, "only the copy moved");

    let dir = tempfile::tempdir().unwrap();
    for name in ["copia.dwg", "copia.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        let inserts: Vec<_> = back
            .entities()
            .into_iter()
            .filter(|e| e.kind() == "INSERT")
            .map(|e| e.id())
            .collect();
        assert_eq!(inserts.len(), 2, "{name}");
        let mut handles = Vec::new();
        for id in &inserts {
            let (h, _, value, _) = attribute(&back, *id);
            assert_eq!(value, "P1", "{name}");
            handles.extend([h, *id]);
        }
        handles.sort_unstable();
        handles.dedup();
        assert_eq!(handles.len(), 4, "{name}: every handle distinct");
    }
}

/// Block ROTULO, base (0,0): a LINE (0,0)-(4,0), a constant attribute
/// "CONST" and an ordinary one (tag NUM); a reference at (10,5) whose NUM is
/// "P7", on layer A-PUERTAS, with a centered attribute ALT "C" hanging from
/// (12,8).
fn label_drawing() -> (Document, EntityId) {
    let block: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "ROTULO", "70", "2", "10", "0", "20", "0", "30", "0", "3",
        "ROTULO", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "4", "21", "0", "31",
        "0", //
        "0", "ATTDEF", "8", "0", "10", "0", "20", "1", "30", "0", "40", "0.5", "1", "CONST", "3",
        "Fixed", "2", "FIJO", "70", "2", //
        "0", "ATTDEF", "8", "0", "10", "0", "20", "2", "30", "0", "40", "0.5", "1", "", "3",
        "Number", "2", "NUM", "70", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let insert: Vec<&str> = vec![
        "0",
        "INSERT",
        "8",
        "A-PUERTAS",
        "66",
        "1",
        "2",
        "ROTULO",
        "10",
        "10",
        "20",
        "5",
        "30",
        "0", //
        "0",
        "ATTRIB",
        "8",
        "A-PUERTAS",
        "10",
        "10",
        "20",
        "7",
        "30",
        "0",
        "40",
        "0.5",
        "1",
        "P7",
        "2",
        "NUM",
        "70",
        "0", //
        "0",
        "ATTRIB",
        "8",
        "A-PUERTAS",
        "10",
        "11",
        "20",
        "8",
        "30",
        "0",
        "40",
        "0.5",
        "1",
        "C",
        "2",
        "ALT",
        "70",
        "0",
        "72",
        "1",
        "11",
        "12",
        "21",
        "8",
        "31",
        "0", //
        "0",
        "SEQEND",
        "8",
        "A-PUERTAS",
    ];
    let doc = dxf(LAYERS, &[&block], &[&insert]);
    let id = doc.entities()[0].id();
    (doc, id)
}

fn texts(doc: &Document) -> Vec<(String, String, (f64, f64))> {
    doc.entities()
        .iter()
        .filter_map(|e| {
            let t = e.text()?;
            let a = t.anchor();
            let round = |v: f64| (v * 1e9).round() / 1e9;
            Some((
                t.content().to_owned(),
                e.layer().to_owned(),
                (round(a.x()), round(a.y())),
            ))
        })
        .collect()
}

#[test]
fn explode_keeps_attribute_values_as_texts() {
    let (mut doc, id) = label_drawing();
    let created = doc.explode_insert(id).unwrap();
    let kinds: Vec<_> = doc.entities().iter().map(|e| e.kind()).collect();
    assert!(
        !kinds.contains(&"ATTDEF") && !kinds.contains(&"ATTRIB"),
        "{kinds:?}"
    );
    assert!(!kinds.contains(&"INSERT"));
    let mut found = texts(&doc);
    found.sort_by(|a, b| a.0.cmp(&b.0));
    // The constant attribute sits on layer 0 in the block, as its line:
    // both land where EXPLODE puts the block's layer-0 contents.
    let line_layer = doc
        .entities()
        .iter()
        .find(|e| e.kind() == "LINE")
        .map(|e| e.layer().to_owned())
        .unwrap();
    assert_eq!(
        found,
        [
            ("C".to_owned(), "A-PUERTAS".to_owned(), (12.0, 8.0)),
            ("CONST".to_owned(), line_layer, (10.0, 6.0)),
            ("P7".to_owned(), "A-PUERTAS".to_owned(), (10.0, 7.0)),
        ],
        "the value shown, where it was drawn; the definition's NUM is not copied"
    );
    assert_eq!(created.len(), 4, "the line and three texts");
    let centered = doc
        .entities()
        .into_iter()
        .find(|e| e.text().is_some_and(|t| t.content() == "C"))
        .unwrap();
    assert_eq!(
        centered.text().unwrap().justification(),
        "Center",
        "justified as it was"
    );
}

#[test]
fn explode_drops_invisible_attributes() {
    let (mut doc, id) = label_drawing();
    if let Some(EntityType::Insert(insert)) =
        doc.cad_mut().get_entity_mut(acadrust::Handle::new(id))
    {
        insert.attributes[0].flags.invisible = true;
    }
    doc.explode_insert(id).unwrap();
    let mut found: Vec<_> = texts(&doc).into_iter().map(|t| t.0).collect();
    found.sort();
    assert_eq!(found, ["C", "CONST"], "P7, invisible, is gone");
}
