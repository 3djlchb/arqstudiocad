//! Tests of making and placing blocks (a submodule of `blocks`).

use std::f64::consts::FRAC_PI_2;

use arq_geometry::{Line, Point2};

use acadrust::entities::Insert;
use acadrust::tables::AppId;
use acadrust::types::Vector3;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{EntityType, tables::TableEntry};

use crate::blocks::{BlockDefinition, BlockObjects, BlockReferences};
use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::{EntityId, PlanGeometry};
use crate::error::DocumentError;
use crate::test_dxf::{LAYERS, door_block, dxf};
use crate::translate::handle;

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

// RENAME (ADR 0025 step 8c)

/// PUERTA placed twice (one spelled in lower case) and once inside MURO,
/// which is placed too.
fn doors() -> Document {
    let door = door_block();
    let wall: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "MURO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "MURO", //
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "5", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    dxf(
        LAYERS,
        &[&door, &wall],
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
    )
}

/// What every reference in model space draws, in order.
fn everything_drawn(doc: &Document) -> Vec<Vec<(i64, i64)>> {
    doc.entities().iter().map(|e| drawn(doc, e.id())).collect()
}

#[test]
fn renaming_a_block_renames_every_reference_to_it() {
    let mut doc = doors();
    let before = everything_drawn(&doc);
    assert_eq!(doc.rename_block("puerta", "Door").unwrap(), "PUERTA");
    assert_eq!(doc.block_names(), ["Door", "MURO"]);
    assert!(doc.block_info("PUERTA").is_none());
    assert_eq!(references(&doc, "Door"), 3, "two placed, one inside MURO");
    assert_eq!(everything_drawn(&doc), before, "drawn as before");
    // Only the case changed: allowed, and spelled as typed.
    doc.rename_block("DOOR", "door").unwrap();
    assert_eq!(doc.block_names(), ["door", "MURO"]);
    assert_eq!(references(&doc, "door"), 3);
    // The same name: nothing to do.
    assert_eq!(doc.rename_block("door", "door").unwrap(), "door");
}

#[test]
fn a_renamed_block_is_saved_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = doors();
    let before = everything_drawn(&doc);
    doc.rename_block("PUERTA", "Puerta de paso").unwrap();
    for name in ["doors.dwg", "doors.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        assert_eq!(back.block_names(), ["MURO", "Puerta de paso"], "{name}");
        assert_eq!(references(&back, "Puerta de paso"), 3, "{name}");
        assert_eq!(everything_drawn(&back), before, "{name}");
    }
}

#[test]
fn what_cannot_be_renamed_is_refused_and_nothing_changes() {
    let mut doc = doors();
    let before = everything_drawn(&doc);
    for (name, new, why) in [
        ("PUERTA", "muro", "a block named \"muro\" exists"),
        ("PUERTA", "A/B", "not a valid block name"),
        ("PUERTA", "*U9", "not a valid block name"),
        ("PUERTA", " PUERTA", "not a valid block name"),
        ("PUERTA", "", "not a valid block name"),
        ("NADA", "OTRA", "no block named"),
        ("*Model_Space", "OTRA", "no block named"),
    ] {
        assert!(
            matches!(
                doc.rename_block(name, new),
                Err(DocumentError::InvalidBlockName(m)) if m.contains(why)
            ),
            "{name:?} to {new:?}"
        );
    }
    assert_eq!(doc.block_names(), ["MURO", "PUERTA"]);
    assert_eq!(everything_drawn(&doc), before);
}

#[test]
fn undo_gives_a_renamed_block_its_name_back() {
    let mut doc = doors();
    let before = everything_drawn(&doc);
    doc.begin_change().unwrap();
    doc.rename_block("PUERTA", "Door").unwrap();
    let mut change = doc.commit_change().unwrap();
    let summary = change.summary().clone();
    assert_eq!(summary.blocks_created(), ["Door"]);
    assert_eq!(summary.blocks_deleted(), ["PUERTA"]);
    assert_eq!(
        summary.modified().len(),
        2,
        "the two placed doors; MURO's reference draws the same"
    );
    let undone = doc.undo_change(&mut change).unwrap();
    assert_eq!(undone.blocks_created(), ["PUERTA"]);
    assert_eq!(doc.block_names(), ["MURO", "PUERTA"]);
    assert_eq!(references(&doc, "PUERTA"), 3);
    assert_eq!(everything_drawn(&doc), before);
    doc.redo_change(&mut change).unwrap();
    assert_eq!(references(&doc, "Door"), 3);
    // Its own name again: nothing changes.
    doc.begin_change().unwrap();
    doc.rename_block("door", "Door").unwrap();
    assert!(doc.commit_change().unwrap().summary().is_empty());
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn a_dynamic_block_is_not_renamed() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/corpus/acadrust/fixtures/dynamic-blocks/lookup.dwg");
    let mut doc = Document::open(path).unwrap();
    let names = doc.block_names();
    let name = names
        .iter()
        .find(|n| doc.is_dynamic_block(n))
        .expect("the sample's dynamic block")
        .clone();
    assert!(matches!(
        doc.rename_block(&name, "OTRO"),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("dynamic")
    ));
    assert_eq!(doc.block_names(), names);
}

// SELECTBLOCK (ADR 0025 step 8d)

#[test]
fn the_references_found_are_those_that_can_be_selected() {
    let door = door_block();
    let wall: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "MURO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "MURO", //
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "5", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let mut doc = dxf(
        LAYERS,
        &[&door, &wall],
        &[
            &[
                "0", "INSERT", "8", "0", "2", "PUERTA", "10", "0", "20", "0", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "MURO", "10", "0", "20", "5", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "puerta", "10", "9", "20", "0", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "PUERTA", "10", "20", "20", "0", "30", "0",
            ],
            // In paper space.
            &[
                "0", "INSERT", "8", "0", "67", "1", "2", "PUERTA", "10", "0", "20", "0", "30", "0",
            ],
        ],
    );
    let ids: Vec<EntityId> = doc.entities().iter().map(|e| e.id()).collect();
    assert_eq!(ids.len(), 4, "model space only");
    let found = doc.find_block_references("puerta").unwrap();
    assert_eq!(found.shown, [ids[0], ids[2], ids[3]], "in the order drawn");
    assert_eq!((found.hidden, found.nested, found.in_layouts), (0, 1, 1));
    // On a frozen layer it draws nothing; on an off layer its line on
    // MUROS still shows, so a click finds it (as AutoCAD).
    let frozen = doc.layer_by_name("CONGELADA").unwrap().id();
    let off = doc.layer_by_name("APAGADA").unwrap().id();
    doc.set_entity_layer(ids[2], frozen).unwrap();
    doc.set_entity_layer(ids[3], off).unwrap();
    let found = doc.find_block_references("PUERTA").unwrap();
    assert_eq!(found.shown, [ids[0], ids[3]]);
    assert_eq!((found.hidden, found.nested), (1, 1));
    let wall = doc.find_block_references("MURO").unwrap();
    assert_eq!(
        wall,
        BlockReferences {
            shown: vec![ids[1]],
            ..BlockReferences::default()
        }
    );
    assert!(matches!(
        doc.find_block_references("NADA"),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("no block named")
    ));
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn a_dynamic_blocks_references_are_found_through_their_anonymous_copies() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/corpus/acadrust/fixtures/dynamic-blocks/lookup.dwg");
    let doc = Document::open(path).unwrap();
    let name = doc
        .block_names()
        .into_iter()
        .find(|n| doc.is_dynamic_block(n))
        .expect("the sample's dynamic block");
    let found = doc.find_block_references(&name).unwrap();
    let total = found.shown.len() + found.hidden + found.nested + found.in_layouts;
    assert_eq!(
        Some(&total),
        doc.block_references().get(&name),
        "as the palette counts"
    );
    assert!(!found.shown.is_empty());
    for id in &found.shown {
        assert_eq!(doc.entity(*id).unwrap().kind(), "INSERT");
    }
}

// Anonymous copies that lost their link (maintainer, 2026-10-26)

/// Registers application `app` and gives block record `block` its
/// extended data, as AutoCAD writes it on a dynamic block's copies.
fn tag(doc: &mut Document, block: &str, app: &str, values: Vec<XDataValue>) {
    let cad = doc.cad_mut();
    if !cad.app_ids.contains(app) {
        let mut id = AppId::new(app);
        id.handle = cad.allocate_handle();
        cad.app_ids.add(id).unwrap();
    }
    let h = cad.block_records.get(block).unwrap().handle();
    let mut record = ExtendedDataRecord::new(app);
    record.values = values;
    cad.object_xdata.entry(h).or_default().add_record(record);
}

fn edit_tag(n: i32) -> Vec<XDataValue> {
    vec![XDataValue::Integer16(1), XDataValue::Integer32(n)]
}

/// A reference to block record `name`, anonymous ones included, at `at`.
fn place(doc: &mut Document, name: &str, at: (f64, f64)) -> EntityId {
    let insert = Insert::new(name, Vector3::new(at.0, at.1, 0.0));
    let h = doc
        .cad_mut()
        .add_entity(EntityType::Insert(insert))
        .unwrap();
    doc.sync_entity(h.value());
    h.value()
}

/// Anonymous block `name` holding a copy of what block `of` holds, or of
/// `other` when given, placed once at `at`.
fn copy_of(
    doc: &mut Document,
    name: &str,
    of: &str,
    other: Option<EntityId>,
    at: (f64, f64),
) -> EntityId {
    let owner = doc
        .add_block_record(name, Vector3::new(0.0, 0.0, 0.0))
        .unwrap();
    let contents: Vec<EntityType> = match other {
        Some(id) => vec![doc.cad().get_entity(handle(id)).unwrap().clone()],
        None => doc.cad().entities_in_block(of).cloned().collect(),
    };
    for entity in contents {
        doc.add_copy(entity, owner).unwrap();
    }
    place(doc, name, at)
}

/// COLUMNA from a line, placed once where the line was.
fn column() -> (Document, EntityId) {
    let mut doc = Document::new();
    let a = line(&mut doc, (0.0, 0.0), (0.0, 3.0));
    let placed = doc.create_block("COLUMNA", p(0.0, 0.0), &[a]).unwrap();
    (doc, placed)
}

#[test]
fn a_copy_with_the_same_edit_tag_and_contents_counts_for_its_block() {
    let (mut doc, placed) = column();
    let copy = copy_of(&mut doc, "*U52", "COLUMNA", None, (5.0, 0.0));
    let again = place(&mut doc, "*U52", (10.0, 0.0));
    assert_eq!(references(&doc, "COLUMNA"), 1, "no link yet");
    tag(&mut doc, "COLUMNA", "AcDbBlockRepETag", edit_tag(4));
    tag(&mut doc, "*U52", "AcDbBlockRepETag", edit_tag(4));
    assert_eq!(references(&doc, "COLUMNA"), 3);
    let found = doc.find_block_references("columna").unwrap();
    assert_eq!(found.shown, [placed, copy, again]);
    assert!(
        !doc.block_names().contains(&"*U52".to_owned()),
        "still anonymous"
    );
}

#[test]
fn a_copy_that_differs_or_matches_twice_stays_apart() {
    let (mut doc, _) = column();
    let other = line(&mut doc, (0.0, 0.0), (0.0, 5.0));
    copy_of(&mut doc, "*U1", "", Some(other), (5.0, 0.0));
    copy_of(&mut doc, "*U2", "COLUMNA", None, (9.0, 0.0));
    for block in ["COLUMNA", "*U1", "*U2"] {
        tag(&mut doc, block, "AcDbBlockRepETag", edit_tag(4));
    }
    // *U1 draws a longer line; *U2 has another tag.
    tag(&mut doc, "*U2", "AcDbBlockRepETag", edit_tag(5));
    assert_eq!(references(&doc, "COLUMNA"), 1);
    // A second block with the same tag and contents: which one is unknown.
    let b = line(&mut doc, (20.0, 0.0), (20.0, 3.0));
    doc.create_block("PILAR", p(20.0, 0.0), &[b]).unwrap();
    tag(&mut doc, "PILAR", "AcDbBlockRepETag", edit_tag(5));
    assert_eq!(references(&doc, "PILAR"), 2, "*U2 is PILAR's");
    tag(&mut doc, "COLUMNA", "AcDbBlockRepETag", edit_tag(5));
    assert_eq!(
        (references(&doc, "PILAR"), references(&doc, "COLUMNA")),
        (1, 1),
        "two candidates: none chosen"
    );
}

#[test]
fn a_copy_names_its_block_by_its_true_name() {
    let (mut doc, _) = column();
    // A stretched variant: it draws something else, but says whose it is.
    let longer = line(&mut doc, (0.0, 0.0), (0.0, 6.0));
    copy_of(&mut doc, "*U7", "", Some(longer), (5.0, 0.0));
    tag(
        &mut doc,
        "*U7",
        "AcDbDynamicBlockTrueName",
        vec![XDataValue::String("columna".to_owned())],
    );
    assert_eq!(references(&doc, "COLUMNA"), 2);
    // A true name no block has links nothing.
    copy_of(&mut doc, "*U8", "", Some(longer), (9.0, 0.0));
    tag(
        &mut doc,
        "*U8",
        "AcDbDynamicBlockTrueName",
        vec![XDataValue::String("VIGA".to_owned())],
    );
    assert_eq!(references(&doc, "COLUMNA"), 2);
}

#[test]
fn a_named_block_is_never_another_blocks_copy() {
    let (mut doc, _) = column();
    // The same drawing, edit tag and even true name, but a name of its own.
    let b = line(&mut doc, (20.0, 0.0), (20.0, 3.0));
    doc.create_block("PILAR", p(20.0, 0.0), &[b]).unwrap();
    for block in ["COLUMNA", "PILAR"] {
        tag(&mut doc, block, "AcDbBlockRepETag", edit_tag(4));
    }
    tag(
        &mut doc,
        "PILAR",
        "AcDbDynamicBlockTrueName",
        vec![XDataValue::String("COLUMNA".to_owned())],
    );
    assert_eq!(
        (references(&doc, "COLUMNA"), references(&doc, "PILAR")),
        (1, 1)
    );
}
