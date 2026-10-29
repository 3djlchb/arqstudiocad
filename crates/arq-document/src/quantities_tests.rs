//! Tests of the budget's quantities (a submodule of `quantities`).

use std::collections::HashMap;

use arq_geometry::{Line, Point2};

use acadrust::EntityType;
use acadrust::entities::Insert;
use acadrust::tables::{AppId, TableEntry};
use acadrust::types::Vector3;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};

use crate::document::Document;
use crate::entity::{EntityId, PlanGeometry};
use crate::error::DocumentError;
use crate::test_dxf::{LAYERS, dxf};

fn chair() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "SILLA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "SILLA", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "1", "21", "0", "31",
        "0", //
        "0", "ENDBLK", "8", "0",
    ]
}

/// A dining set, JUEGO: four chairs.
fn dining_set() -> Vec<&'static str> {
    let mut codes = vec![
        "0", "BLOCK", "8", "0", "2", "JUEGO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "JUEGO",
    ];
    for x in ["0", "2", "4", "6"] {
        codes.extend([
            "0", "INSERT", "8", "0", "2", "SILLA", "10", x, "20", "0", "30", "0",
        ]);
    }
    codes.extend(["0", "ENDBLK", "8", "0"]);
    codes
}

/// A house, CASA: two dining sets and a chair.
fn house() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "CASA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "CASA", //
        "0", "INSERT", "8", "0", "2", "JUEGO", "10", "0", "20", "0", "30", "0", //
        "0", "INSERT", "8", "0", "2", "juego", "10", "0", "20", "9", "30", "0", //
        "0", "INSERT", "8", "0", "2", "SILLA", "10", "9", "20", "9", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ]
}

/// A sheet, LAMINA, only placed in paper space: it holds a chair and the
/// title block, placed nowhere else.
fn sheet() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "LAMINA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "LAMINA", //
        "0", "INSERT", "8", "0", "2", "SILLA", "10", "0", "20", "0", "30", "0", //
        "0", "INSERT", "8", "0", "2", "ROTULO", "10", "0", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ]
}

fn unused() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "ROTULO", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "ROTULO", //
        "0", "ENDBLK", "8", "0",
    ]
}

/// The house drawing, its model-space ids in drawing order: three dining
/// sets (one on a frozen layer, one on an off layer), a chair, a house and
/// a line. Paper space holds a chair and a sheet.
fn furnished() -> (Document, Vec<EntityId>) {
    let doc = dxf(
        LAYERS,
        &[&chair(), &dining_set(), &house(), &sheet(), &unused()],
        &[
            &[
                "0", "INSERT", "8", "0", "2", "JUEGO", "10", "0", "20", "0", "30", "0",
            ],
            &[
                "0",
                "INSERT",
                "8",
                "CONGELADA",
                "2",
                "JUEGO",
                "10",
                "0",
                "20",
                "20",
                "30",
                "0",
            ],
            &[
                "0", "INSERT", "8", "APAGADA", "2", "JUEGO", "10", "0", "20", "40", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "SILLA", "10", "30", "20", "0", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "CASA", "10", "50", "20", "0", "30", "0",
            ],
            &[
                "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "1", "21", "1", "31",
                "0",
            ],
            // In paper space.
            &[
                "0", "INSERT", "8", "0", "67", "1", "2", "SILLA", "10", "0", "20", "0", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "67", "1", "2", "LAMINA", "10", "0", "20", "0", "30", "0",
            ],
        ],
    );
    let ids = doc.entities().iter().map(|e| e.id()).collect();
    (doc, ids)
}

fn counts(pairs: &[(&str, u64)]) -> HashMap<String, u64> {
    pairs.iter().map(|(n, c)| ((*n).to_owned(), *c)).collect()
}

#[test]
fn nested_references_count_once_for_each_placement_of_their_holder() {
    let (doc, ids) = furnished();
    assert_eq!(ids.len(), 6, "model space only");
    // Chairs: 3 sets of 4, one alone, and in the house 2 sets of 4 and one.
    assert_eq!(
        doc.block_quantities(),
        counts(&[("SILLA", 22), ("JUEGO", 5), ("CASA", 1)]),
        "hidden layers count; layouts, LAMINA and ROTULO do not"
    );
}

#[test]
fn a_minsert_counts_each_copy_and_multiplies_what_it_holds() {
    // A row, FILA: five chairs as one MINSERT.
    let row: Vec<&str> = vec![
        "0", "BLOCK", "8", "0", "2", "FILA", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "FILA", //
        "0", "INSERT", "8", "0", "2", "SILLA", "10", "0", "20", "0", "30", "0", "70", "5", "44",
        "2", //
        "0", "ENDBLK", "8", "0",
    ];
    let doc = dxf(
        LAYERS,
        &[&chair(), &dining_set(), &row],
        &[
            // Dining sets, 2 columns by 3 rows.
            &[
                "0", "INSERT", "8", "0", "2", "JUEGO", "10", "0", "20", "0", "30", "0", "70", "2",
                "71", "3", "44", "10", "45", "10",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "FILA", "10", "0", "20", "50", "30", "0",
            ],
            &[
                "0", "INSERT", "8", "0", "2", "FILA", "10", "0", "20", "60", "30", "0",
            ],
        ],
    );
    assert_eq!(
        doc.block_quantities(),
        counts(&[("JUEGO", 6), ("SILLA", 6 * 4 + 2 * 5), ("FILA", 2)])
    );
}

#[test]
fn selection_only_counts_the_chosen_references_and_what_they_hold() {
    let (doc, ids) = furnished();
    let (set, frozen, chair, house, line) = (ids[0], ids[1], ids[3], ids[4], ids[5]);
    assert_eq!(
        doc.block_quantities_of(&[set, line, set]).unwrap(),
        counts(&[("JUEGO", 1), ("SILLA", 4)]),
        "lines ignored; each reference once"
    );
    assert_eq!(
        doc.block_quantities_of(&[house, chair, frozen]).unwrap(),
        counts(&[("CASA", 1), ("JUEGO", 3), ("SILLA", 14)])
    );
    assert!(doc.block_quantities_of(&[]).unwrap().is_empty());
    assert!(doc.block_quantities_of(&[line]).unwrap().is_empty());
    assert!(matches!(
        doc.block_quantities_of(&[set, 999_999]),
        Err(DocumentError::EntityNotFound(999_999))
    ));
}

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

/// A reference to block record `name` at `at`, in model space.
fn place(doc: &mut Document, name: &str, at: (f64, f64)) -> EntityId {
    let insert = Insert::new(name, Vector3::new(at.0, at.1, 0.0));
    let h = doc
        .cad_mut()
        .add_entity(EntityType::Insert(insert))
        .unwrap();
    doc.sync_entity(h.value());
    h.value()
}

/// Block record `name` holding a reference to each of `blocks`.
fn holding(doc: &mut Document, name: &str, blocks: &[&str]) {
    let owner = doc
        .add_block_record(name, Vector3::new(0.0, 0.0, 0.0))
        .unwrap();
    for (n, block) in blocks.iter().enumerate() {
        let insert = Insert::new(*block, Vector3::new(n as f64, 0.0, 0.0));
        doc.add_copy(EntityType::Insert(insert), owner).unwrap();
    }
}

/// Gives block record `block` AutoCAD's true name of a dynamic block's
/// anonymous copy.
fn true_name(doc: &mut Document, block: &str, name: &str) {
    let app = "AcDbDynamicBlockTrueName";
    let cad = doc.cad_mut();
    if !cad.app_ids.contains(app) {
        let mut id = AppId::new(app);
        id.handle = cad.allocate_handle();
        cad.app_ids.add(id).unwrap();
    }
    let h = cad.block_records.get(block).unwrap().handle();
    let mut record = ExtendedDataRecord::new(app);
    record.values = vec![XDataValue::String(name.to_owned())];
    cad.object_xdata.entry(h).or_default().add_record(record);
}

/// SILLA from a line, placed where the line was, and JUEGO holding two
/// chairs, placed once.
fn chairs_and_set() -> Document {
    let mut doc = Document::new();
    let a = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    doc.create_block("SILLA", p(0.0, 0.0), &[a]).unwrap();
    holding(&mut doc, "JUEGO", &["SILLA", "SILLA"]);
    place(&mut doc, "JUEGO", (0.0, 10.0));
    doc
}

#[test]
fn an_anonymous_copy_counts_for_its_block_and_what_it_holds_counts_too() {
    let mut doc = chairs_and_set();
    // A stretched variant of JUEGO with three chairs, placed twice.
    holding(&mut doc, "*U3", &["SILLA", "SILLA", "SILLA"]);
    true_name(&mut doc, "*U3", "juego");
    place(&mut doc, "*U3", (0.0, 20.0));
    place(&mut doc, "*U3", (0.0, 30.0));
    // A copy whose block is unknown: not listed, its chair counts.
    holding(&mut doc, "*U9", &["SILLA"]);
    place(&mut doc, "*U9", (0.0, 40.0));
    assert_eq!(
        doc.block_quantities(),
        counts(&[("JUEGO", 3), ("SILLA", 1 + 2 + 2 * 3 + 1)])
    );
}

#[test]
fn a_block_holding_itself_through_another_does_not_loop() {
    // Only a broken file can hold it (ADR 0023 refuses it).
    let mut doc = chairs_and_set();
    holding(&mut doc, "MESA", &["JUEGO"]);
    let set = doc.cad().block_records.get("JUEGO").unwrap().handle();
    let insert = Insert::new("MESA", Vector3::new(0.0, 0.0, 0.0));
    doc.add_copy(EntityType::Insert(insert), set).unwrap();
    // JUEGO is counted where it is placed; what the loop holds is not.
    assert_eq!(
        doc.block_quantities(),
        counts(&[("JUEGO", 1), ("SILLA", 1)])
    );
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn a_dynamic_blocks_quantity_counts_its_anonymous_copies() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/corpus/acadrust/fixtures/dynamic-blocks/lookup.dwg");
    let doc = Document::open(path).unwrap();
    let name = doc
        .block_names()
        .into_iter()
        .find(|n| doc.is_dynamic_block(n))
        .expect("the sample's dynamic block");
    let found = doc.find_block_references(&name).unwrap();
    let quantities = doc.block_quantities();
    let placed = (found.shown.len() + found.hidden) as u64;
    assert!(placed > 0);
    assert!(quantities.get(&name).is_some_and(|&q| q >= placed));
    let names = doc.block_names();
    assert!(quantities.keys().all(|n| names.contains(n)));
}
