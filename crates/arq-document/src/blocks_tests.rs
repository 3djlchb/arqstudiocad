//! Tests of making and placing blocks (a submodule of `blocks`).

use std::f64::consts::FRAC_PI_2;

use arq_geometry::{Line, Point2};

use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::{EntityId, PlanGeometry};
use crate::error::DocumentError;
use crate::test_dxf::{LAYERS, door_block, dxf};

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

/// The ends of the lines drawn for `id`, rounded, sorted.
fn drawn(doc: &Document, id: EntityId) -> Vec<(i64, i64)> {
    let mut ends: Vec<(i64, i64)> = doc
        .drawing(id)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item.shape {
            Shape::Line(l) => Some([l.start(), l.end()]),
            _ => None,
        })
        .flatten()
        .map(|q| ((q.x() * 1e6).round() as i64, (q.y() * 1e6).round() as i64))
        .collect();
    ends.sort_unstable();
    ends
}

#[test]
fn block_names_leave_out_spaces_layouts_and_anonymous_blocks() {
    let door = door_block();
    let anonymous: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "*U1", "70", "1", "10", "0", "20", "0", "30", "0", "3", "*U1",
        "0", "ENDBLK", "8", "0",
    ];
    let doc = dxf(LAYERS, &[&door, &anonymous], &[]);
    assert_eq!(doc.block_names(), ["PUERTA"]);
}

#[test]
fn a_block_made_from_entities_draws_them_where_they_were() {
    let mut doc = Document::new();
    let a = line(&mut doc, (10.0, 10.0), (14.0, 10.0));
    let b = line(&mut doc, (10.0, 10.0), (10.0, 13.0));
    let keep = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    let before = [drawn(&doc, a), drawn(&doc, b)].concat();
    let reference = doc.create_block("MARCO", p(10.0, 10.0), &[b, a]).unwrap();

    assert!(
        doc.entity(a).is_none() && doc.entity(b).is_none(),
        "moved into the block"
    );
    assert!(doc.entity(keep).is_some());
    assert_eq!(doc.entity(reference).unwrap().kind(), "INSERT");
    let mut expected = before;
    expected.sort_unstable();
    assert_eq!(drawn(&doc, reference), expected, "drawn as they were");
    assert_eq!(doc.block_names(), ["MARCO"]);
    let base = doc.cad().block_records.get("MARCO").unwrap().base_point;
    assert_eq!((base.x, base.y), (10.0, 10.0));
}

#[test]
fn block_names_and_contents_are_checked() {
    let mut doc = Document::new();
    let a = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    for bad in ["", " A", "A/B", "A:B", "*U2", "A?"] {
        assert!(
            matches!(
                doc.create_block(bad, p(0.0, 0.0), &[a]),
                Err(DocumentError::InvalidBlockName(_))
            ),
            "{bad:?}"
        );
    }
    doc.create_block("A", p(0.0, 0.0), &[a]).unwrap();
    let b = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    assert!(
        matches!(
            doc.create_block("a", p(0.0, 0.0), &[b]),
            Err(DocumentError::InvalidBlockName(ref why)) if why.contains("exists")
        ),
        "a name in use, ignoring case"
    );
    assert!(matches!(
        doc.create_block("B", p(0.0, 0.0), &[b, 999_999]),
        Err(DocumentError::EntityNotFound(999_999))
    ));
    assert!(matches!(
        doc.create_block("C", p(0.0, 0.0), &[]),
        Err(DocumentError::InvalidBlockName(_))
    ));
}

#[test]
fn insert_places_a_block_scaled_and_turned() {
    let door = door_block();
    let mut doc = dxf(LAYERS, &[&door], &[]);
    // PUERTA, base (1,1): lines (1,1)-(2,1) and (1,1)-(1,2).
    let id = doc
        .insert_block("PUERTA", p(5.0, 5.0), 2.0, FRAC_PI_2)
        .unwrap();
    let e = doc.entity(id).unwrap();
    assert_eq!((e.kind(), e.layer()), ("INSERT", "0"));
    let unit = 1_000_000;
    assert_eq!(
        drawn(&doc, id),
        [
            (3 * unit, 5 * unit),
            (5 * unit, 5 * unit),
            (5 * unit, 5 * unit),
            (5 * unit, 7 * unit),
        ],
        "twice as big, a quarter turn round its base point"
    );
    for (name, scale, rotation) in [
        ("NADA", 1.0, 0.0),
        ("PUERTA", 0.0, 0.0),
        ("PUERTA", f64::NAN, 0.0),
        ("PUERTA", 1.0, f64::INFINITY),
        ("*Model_Space", 1.0, 0.0),
    ] {
        assert!(matches!(
            doc.insert_block(name, p(0.0, 0.0), scale, rotation),
            Err(DocumentError::InvalidBlockName(_))
        ));
    }
}

#[test]
fn an_inserted_block_gets_its_attributes_with_their_defaults() {
    let block: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "ROTULO", "70", "2", "10", "0", "20", "0", "30", "0", "3",
        "ROTULO", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "4", "21", "0", "31",
        "0", //
        "0", "ATTDEF", "8", "0", "10", "1", "20", "1", "30", "0", "40", "0.5", "1", "P-00", "3",
        "Number", "2", "NUM", "70", "0", //
        "0", "ATTDEF", "8", "0", "10", "0", "20", "2", "30", "0", "40", "0.5", "1", "FIJO", "3",
        "Fixed", "2", "CONST", "70", "2", //
        "0", "ENDBLK", "8", "0",
    ];
    let mut doc = dxf(LAYERS, &[&block], &[]);
    let id = doc.insert_block("ROTULO", p(10.0, 5.0), 2.0, 0.0).unwrap();
    let Some(acadrust::EntityType::Insert(insert)) =
        doc.cad().get_entity(acadrust::Handle::new(id))
    else {
        panic!("not an insert")
    };
    assert_eq!(
        insert.attributes.len(),
        1,
        "the constant one comes from the block"
    );
    let a = &insert.attributes[0];
    assert_eq!((a.tag.as_str(), a.value.as_str()), ("NUM", "P-00"));
    assert_eq!(
        (a.insertion_point.x, a.insertion_point.y),
        (12.0, 7.0),
        "placed and scaled"
    );
    assert!((a.height - 1.0).abs() < 1e-12);
    assert_eq!(a.common.owner_handle.value(), id);
}
