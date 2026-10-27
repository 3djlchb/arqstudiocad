//! Tests of purging unused blocks (a submodule of `purge`).

use acadrust::Handle;
use acadrust::objects::{Dictionary, ObjectType, XRecord};
use acadrust::tables::TableEntry;

use crate::document::Document;
use crate::error::DocumentError;
use crate::test_dxf::{LAYERS, door_block, dxf};

/// MURO, placed, holds PUERTA; MESA, unused, holds SILLA; SOLO, unused,
/// holds nothing.
fn furniture() -> Document {
    let door = door_block();
    let wall: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "MURO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "MURO", //
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "5", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let chair: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "SILLA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "SILLA", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "1", "21", "0", "31",
        "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let table: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "MESA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "MESA", //
        "0", "INSERT", "8", "0", "2", "SILLA", "10", "1", "20", "0", "30", "0", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "2", "21", "0", "31",
        "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let alone: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "SOLO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "SOLO", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "3", "21", "0", "31",
        "0", //
        "0", "ENDBLK", "8", "0",
    ];
    dxf(
        LAYERS,
        &[&door, &wall, &chair, &table, &alone],
        &[&[
            "0", "INSERT", "8", "0", "2", "MURO", "10", "0", "20", "0", "30", "0",
        ]],
    )
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn unused_blocks_are_those_nothing_references() {
    let doc = furniture();
    assert_eq!(doc.unused_blocks(), ["MESA", "SOLO"], "SILLA is MESA's");
}

#[test]
fn purging_removes_the_block_and_nested_ones_on_request() {
    let mut doc = furniture();
    let entities = doc.entities();
    assert_eq!(
        doc.purge_blocks(&names(&["mesa"]), false).unwrap(),
        ["MESA"]
    );
    assert_eq!(doc.block_names(), ["MURO", "PUERTA", "SILLA", "SOLO"]);
    assert_eq!(doc.unused_blocks(), ["SILLA", "SOLO"], "SILLA unused now");
    assert_eq!(doc.entities(), entities, "model space untouched");

    let mut doc = furniture();
    // What the three blocks held, and their BLOCK and ENDBLK.
    let gone: Vec<Handle> = ["MESA", "SOLO", "SILLA"]
        .iter()
        .flat_map(|n| {
            let r = doc.cad().block_records.get(n).unwrap();
            let mut handles = r.entity_handles.clone();
            handles.extend([r.block_entity_handle, r.block_end_handle]);
            handles
        })
        .collect();
    assert_eq!(
        doc.purge_blocks(&names(&["MESA", "SOLO"]), true).unwrap(),
        ["MESA", "SOLO", "SILLA"]
    );
    assert_eq!(doc.block_names(), ["MURO", "PUERTA"]);
    for h in gone {
        assert!(doc.cad().get_entity(h).is_none(), "{h:?} went too");
    }
}

#[test]
fn nested_purging_keeps_what_something_else_uses() {
    let mut doc = furniture();
    // SILLA also placed in the drawing: purging MESA keeps it.
    doc.insert_block(
        "SILLA",
        arq_geometry::Point2::new(9.0, 9.0).unwrap(),
        1.0,
        0.0,
    )
    .unwrap();
    assert_eq!(doc.purge_blocks(&names(&["MESA"]), true).unwrap(), ["MESA"]);
    assert!(doc.block_names().contains(&"SILLA".to_owned()));
}

#[test]
fn what_cannot_be_purged_is_refused_and_nothing_changes() {
    let mut doc = furniture();
    for (name, why) in [
        ("MURO", "MURO is in use"),
        ("PUERTA", "PUERTA is in use"),
        ("NADA", "no block named"),
        ("*Model_Space", "no block named"),
    ] {
        assert!(
            matches!(
                doc.purge_blocks(&names(&["SOLO", name]), true),
                Err(DocumentError::InvalidBlockName(m)) if m.contains(why)
            ),
            "{name}"
        );
    }
    assert_eq!(
        doc.block_names(),
        ["MESA", "MURO", "PUERTA", "SILLA", "SOLO"]
    );
}

#[test]
fn the_objects_a_block_owns_go_with_it() {
    let mut doc = furniture();
    let record = doc.cad().block_records.get("SOLO").unwrap().handle();
    let other = doc.cad().block_records.get("MURO").unwrap().handle();
    let add = |doc: &mut Document, owner| {
        let cad = doc.cad_mut();
        let mut dictionary = Dictionary::new();
        dictionary.handle = cad.allocate_handle();
        dictionary.owner = owner;
        let mut xrecord = XRecord::new();
        xrecord.handle = cad.allocate_handle();
        xrecord.owner = dictionary.handle;
        let handles = (dictionary.handle, xrecord.handle);
        cad.objects
            .insert(dictionary.handle, ObjectType::Dictionary(dictionary));
        cad.objects
            .insert(xrecord.handle, ObjectType::XRecord(xrecord));
        handles
    };
    let (dictionary, xrecord) = add(&mut doc, record);
    let (kept, kept_record) = add(&mut doc, other);
    doc.purge_blocks(&names(&["SOLO"]), false).unwrap();
    let objects = &doc.cad().objects;
    assert!(!objects.contains_key(&dictionary) && !objects.contains_key(&xrecord));
    assert!(objects.contains_key(&kept) && objects.contains_key(&kept_record));
}

#[test]
fn undo_brings_purged_blocks_back() {
    let mut doc = furniture();
    let issues = doc.cad().audit().issues.len();
    doc.begin_change().unwrap();
    doc.purge_blocks(&names(&["MESA"]), true).unwrap();
    let mut change = doc.commit_change().unwrap();
    let mut deleted = change.summary().blocks_deleted().to_vec();
    deleted.sort();
    assert_eq!(deleted, ["MESA", "SILLA"]);
    doc.undo_change(&mut change).unwrap();
    assert_eq!(
        doc.block_names(),
        ["MESA", "MURO", "PUERTA", "SILLA", "SOLO"]
    );
    assert_eq!(doc.unused_blocks(), ["MESA", "SOLO"]);
    assert_eq!(doc.cad().entities_in_block("MESA").count(), 2);
    assert_eq!(doc.cad().audit().issues.len(), issues, "sound again");
}

#[test]
fn a_purged_drawing_is_saved_sound_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = furniture();
    let before = doc.cad().audit().issues.len();
    doc.purge_blocks(&names(&["MESA", "SOLO"]), true).unwrap();
    assert_eq!(doc.cad().audit().issues.len(), before, "no new audit issue");
    for name in ["purged.dwg", "purged.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        assert_eq!(back.block_names(), ["MURO", "PUERTA"], "{name}");
        assert_eq!(back.entities().len(), 1, "{name}");
    }
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn a_dynamic_block_is_not_purged() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/corpus/acadrust/fixtures/dynamic-blocks/lookup.dwg");
    let mut doc = Document::open(path).unwrap();
    let name = doc
        .block_names()
        .into_iter()
        .find(|n| doc.is_dynamic_block(n))
        .expect("the sample's dynamic block");
    // Unused once its references are gone.
    let placed = doc.find_block_references(&name).unwrap().shown;
    for id in placed {
        doc.delete_entity(id).unwrap();
    }
    assert!(doc.unused_blocks().contains(&name), "nothing else uses it");
    assert!(matches!(
        doc.purge_blocks(std::slice::from_ref(&name), true),
        Err(DocumentError::InvalidBlockName(m)) if m.contains("dynamic")
    ));
    assert!(doc.block_names().contains(&name));
}

#[test]
fn the_objects_a_nested_references_attributes_own_go_too() {
    let title: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "ROTULO", "70", "2", "10", "0", "20", "0", "30", "0", "3",
        "ROTULO", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "4", "21", "0", "31",
        "0", //
        "0", "ATTDEF", "8", "0", "10", "1", "20", "1", "30", "0", "40", "0.5", "1", "P-00", "3",
        "Number", "2", "NUM", "70", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let mut doc = dxf(LAYERS, &[&title], &[]);
    let origin = arq_geometry::Point2::new(0.0, 0.0).unwrap();
    let placed = doc.insert_block("ROTULO", origin, 1.0, 0.0).unwrap();
    let boxed = doc.create_block("CAJA", origin, &[placed]).unwrap();
    doc.delete_entity(boxed).unwrap();
    let attribute = doc
        .cad()
        .entities_in_block("CAJA")
        .find_map(|e| match e {
            acadrust::EntityType::Insert(i) => i.attributes.first().map(|a| a.common.handle),
            _ => None,
        })
        .expect("CAJA's reference has its attribute");
    let cad = doc.cad_mut();
    let mut dictionary = Dictionary::new();
    dictionary.handle = cad.allocate_handle();
    dictionary.owner = attribute;
    let owned = dictionary.handle;
    cad.objects
        .insert(owned, ObjectType::Dictionary(dictionary));
    doc.purge_blocks(&names(&["CAJA"]), false).unwrap();
    assert!(!doc.cad().objects.contains_key(&owned));
    assert_eq!(doc.block_names(), ["ROTULO"]);
}
