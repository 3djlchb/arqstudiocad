//! Tests of making and placing blocks (a submodule of `blocks`).

use std::f64::consts::FRAC_PI_2;

use arq_geometry::{Line, Point2};

use crate::blocks::{BlockDefinition, BlockObjects};
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

/// Block ROTULO, base (0,0): a line and two attribute definitions, one of
/// them constant.
fn title_block() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "ROTULO", "70", "2", "10", "0", "20", "0", "30", "0", "3",
        "ROTULO", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "4", "21", "0", "31",
        "0", //
        "0", "ATTDEF", "8", "0", "10", "1", "20", "1", "30", "0", "40", "0.5", "1", "P-00", "3",
        "Number", "2", "NUM", "70", "0", //
        "0", "ATTDEF", "8", "0", "10", "0", "20", "2", "30", "0", "40", "0.5", "1", "FIJO", "3",
        "Fixed", "2", "CONST", "70", "2", //
        "0", "ENDBLK", "8", "0",
    ]
}

#[test]
fn block_info_tells_what_the_palette_shows() {
    let door = door_block();
    let title = title_block();
    let mut doc = dxf(LAYERS, &[&door, &title], &[]);
    let record = doc.cad_mut().block_records.get_mut("PUERTA").unwrap();
    record.units = 5;
    record.description = "Puerta de 90 cm".to_owned();
    let info = doc.block_info("puerta").expect("found ignoring case");
    assert_eq!(info.name, "PUERTA", "spelled as the table spells it");
    assert_eq!(
        (info.description.as_str(), info.units, info.attributes),
        ("Puerta de 90 cm", 5, 0)
    );
    assert!(!info.dynamic);
    let title = doc.block_info("ROTULO").unwrap();
    assert_eq!(title.attributes, 2, "constant ones included");
    assert_eq!((title.units, title.description.as_str()), (0, ""));
    assert_eq!(doc.block_info("NADA"), None);
    assert_eq!(
        doc.block_info("*Model_Space"),
        None,
        "only what INSERT places"
    );
}

#[test]
fn block_references_count_everywhere_in_the_drawing() {
    let door = door_block();
    // A block that holds a door: its door counts too.
    let wall: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "MURO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "MURO", //
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "5", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let title = title_block();
    let doc = dxf(
        LAYERS,
        &[&door, &wall, &title],
        &[
            &[
                "0", "INSERT", "8", "0", "2", "PUERTA", "10", "0", "20", "0", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "puerta", "10", "9", "20", "0", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "MURO", "10", "0", "20", "5", "30", "0",
            ],
        ],
    );
    let counts = doc.block_references();
    assert_eq!(
        counts.get("PUERTA"),
        Some(&3),
        "two placed, one inside MURO"
    );
    assert_eq!(counts.get("MURO"), Some(&1));
    assert_eq!(counts.get("ROTULO"), None, "not inserted");
}

#[test]
fn block_drawing_is_the_block_seen_from_its_base_point() {
    let door = door_block(); // base (1,1): lines (1,1)-(2,1) and (1,1)-(1,2)
    let doc = dxf(LAYERS, &[&door], &[]);
    let mut ends: Vec<_> = doc
        .block_drawing("puerta")
        .expect("found ignoring case")
        .into_iter()
        .filter_map(|item| match item.shape {
            Shape::Line(l) => Some(((l.start().x(), l.start().y()), (l.end().x(), l.end().y()))),
            _ => None,
        })
        .collect();
    ends.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(ends, [((0.0, 0.0), (0.0, 1.0)), ((0.0, 0.0), (1.0, 0.0))]);
    assert!(doc.block_drawing("NADA").is_none());
    assert!(doc.block_drawing("*Model_Space").is_none());
}

#[test]
fn a_static_block_has_no_parameters() {
    let door = door_block();
    let doc = dxf(LAYERS, &[&door], &[]);
    let info = doc.block_info("PUERTA").unwrap();
    assert!(!info.dynamic && info.parameters.is_empty());
}

// The Block Definition dialog (ADR 0023 D2a)

fn frame(doc: &mut Document) -> Vec<EntityId> {
    vec![
        line(doc, (10.0, 10.0), (14.0, 10.0)),
        line(doc, (10.0, 10.0), (10.0, 13.0)),
    ]
}

fn described() -> BlockDefinition {
    BlockDefinition {
        description: "A window frame".to_owned(),
        units: 4,
        scale_uniformly: true,
        explodable: false,
    }
}

fn references(doc: &Document, name: &str) -> usize {
    doc.block_references().get(name).copied().unwrap_or(0)
}

#[test]
fn retain_keeps_the_objects_and_places_no_reference() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    let before = [drawn(&doc, ids[0]), drawn(&doc, ids[1])].concat();
    let placed = doc
        .define_block(
            "MARCO",
            p(10.0, 10.0),
            &ids,
            BlockObjects::Retain,
            &described(),
            false,
        )
        .unwrap();
    assert_eq!(placed, None);
    assert_eq!(doc.entities().len(), 2, "the objects stay");
    assert_eq!([drawn(&doc, ids[0]), drawn(&doc, ids[1])].concat(), before);
    assert_eq!(doc.block_names(), ["MARCO"]);
    assert_eq!(references(&doc, "MARCO"), 0);
    let placed = doc.insert_block("MARCO", p(10.0, 10.0), 1.0, 0.0).unwrap();
    let mut both = before.clone();
    both.sort_unstable();
    assert_eq!(
        drawn(&doc, placed),
        both,
        "the block holds what was selected"
    );
}

#[test]
fn delete_removes_the_objects_and_places_no_reference() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    let keep = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    let placed = doc
        .define_block(
            "MARCO",
            p(10.0, 10.0),
            &ids,
            BlockObjects::Delete,
            &described(),
            false,
        )
        .unwrap();
    assert_eq!(placed, None);
    let left: Vec<EntityId> = doc.entities().iter().map(|e| e.id()).collect();
    assert_eq!(left, [keep], "only what was not selected");
    assert_eq!(doc.block_names(), ["MARCO"]);
}

#[test]
fn convert_places_a_reference_where_the_objects_were() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    let before = {
        let mut ends = [drawn(&doc, ids[0]), drawn(&doc, ids[1])].concat();
        ends.sort_unstable();
        ends
    };
    let placed = doc
        .define_block(
            "MARCO",
            p(10.0, 10.0),
            &ids,
            BlockObjects::Convert,
            &described(),
            false,
        )
        .unwrap()
        .expect("a reference");
    assert_eq!(drawn(&doc, placed), before);
    assert_eq!(doc.entities().len(), 1);
}

#[test]
fn the_record_takes_the_description_units_and_flags() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    doc.define_block(
        "MARCO",
        p(10.0, 10.0),
        &ids,
        BlockObjects::Retain,
        &described(),
        false,
    )
    .unwrap();
    let info = doc.block_info("MARCO").unwrap();
    assert_eq!(info.description, "A window frame");
    assert_eq!(info.units, 4, "millimeters");
    assert!(info.scale_uniformly && !info.explodable);
    let other = frame(&mut doc);
    doc.create_block("PLAIN", p(0.0, 0.0), &other).unwrap();
    let plain = doc.block_info("PLAIN").unwrap();
    assert_eq!(
        (
            plain.description.as_str(),
            plain.units,
            plain.scale_uniformly,
            plain.explodable
        ),
        ("", 0, false, true),
        "BLOCK's defaults, as before the dialog"
    );
}

#[test]
fn the_record_is_saved_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    doc.define_block(
        "MARCO",
        p(10.0, 10.0),
        &ids,
        BlockObjects::Convert,
        &described(),
        false,
    )
    .unwrap();
    for name in ["marco.dwg", "marco.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        let info = back.block_info("MARCO").unwrap();
        assert_eq!(info.description, "A window frame", "{name}");
        assert_eq!(info.units, 4, "{name}");
        assert!(info.scale_uniformly, "{name}");
        assert!(!info.explodable, "{name}");
    }
}

#[test]
fn redefine_replaces_the_contents_and_base_and_redraws_references() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    let reference = doc.create_block("MARCO", p(10.0, 10.0), &ids).unwrap();
    let wide = line(&mut doc, (0.0, 0.0), (6.0, 0.0));
    let placed = doc
        .define_block(
            "marco",
            p(0.0, 0.0),
            &[wide],
            BlockObjects::Delete,
            &described(),
            true,
        )
        .unwrap();
    assert_eq!(placed, None);
    assert_eq!(
        doc.block_names(),
        ["MARCO"],
        "the drawing's spelling, one block"
    );
    // The reference at (10,10) now draws the new contents from the new base.
    assert_eq!(
        drawn(&doc, reference),
        [(10_000_000, 10_000_000), (16_000_000, 10_000_000)]
    );
    assert_eq!(
        doc.block_info("MARCO").unwrap().description,
        "A window frame"
    );
    assert_eq!(doc.entities().len(), 1, "the reference alone");
}

#[test]
fn what_cannot_be_defined_is_refused_and_nothing_changes() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    let reference = doc.create_block("MARCO", p(10.0, 10.0), &ids).unwrap();
    let count = doc.entities().len();
    let other = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    let new = |doc: &mut Document, name: &str, ids: &[EntityId], redefine: bool| {
        doc.define_block(
            name,
            p(0.0, 0.0),
            ids,
            BlockObjects::Convert,
            &described(),
            redefine,
        )
    };
    assert!(matches!(
        new(&mut doc, "MARCO", &[other], false),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("exists")
    ));
    assert!(matches!(
        new(&mut doc, "NADA", &[other], true),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("no block named")
    ));
    assert!(matches!(
        new(&mut doc, "MARCO", &[reference], true),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("itself")
    ));
    assert!(matches!(
        new(&mut doc, "VACIO", &[], false),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("needs objects")
    ));
    assert_eq!(doc.entities().len(), count + 1);
    assert_eq!(doc.block_names(), ["MARCO"]);
}

#[test]
fn a_block_that_does_not_allow_exploding_is_not_exploded() {
    let mut doc = Document::new();
    let ids = frame(&mut doc);
    let reference = doc
        .define_block(
            "MARCO",
            p(10.0, 10.0),
            &ids,
            BlockObjects::Convert,
            &described(),
            false,
        )
        .unwrap()
        .unwrap();
    assert!(matches!(
        doc.explode_insert(reference),
        Err(DocumentError::UnsupportedOperation(m)) if m == "MARCO cannot be exploded"
    ));
    let other = frame(&mut doc);
    let free = doc.create_block("LIBRE", p(10.0, 10.0), &other).unwrap();
    assert_eq!(doc.explode_insert(free).unwrap().len(), 2);
}
