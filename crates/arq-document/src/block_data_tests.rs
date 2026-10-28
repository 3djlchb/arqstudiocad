//! Tests of block data and the currency register (a submodule of
//! `block_data`).

use std::collections::BTreeMap;

use acadrust::objects::{ObjectType, XRecordValue};
use acadrust::tables::TableEntry;
use arq_geometry::{Line, Point2};

use super::{
    BlockData, CURRENCIES_KEY, Currencies, Currency, DATA_KEY, Money, NAME_CODE, VALUE_CODE,
    VERSION_CODE,
};
use crate::block_copy::ClashChoice;
use crate::document::Document;
use crate::entity::{EntityId, PlanGeometry};
use crate::error::DocumentError;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(doc: &mut Document, length: f64) -> EntityId {
    let layer = doc.current_layer().id();
    doc.add_entity(
        PlanGeometry::new(Line::new(p(0.0, 0.0), p(length, 0.0)).unwrap().into()),
        layer,
    )
    .unwrap()
}

/// A drawing with blocks MESA (placed once) and SILLA (placed nowhere).
fn drawing() -> Document {
    let mut doc = Document::new();
    let a = line(&mut doc, 1.0);
    let chair = doc.create_block("SILLA", p(0.0, 0.0), &[a]).unwrap();
    doc.delete_entity(chair).unwrap();
    let b = line(&mut doc, 2.0);
    doc.create_block("MESA", p(0.0, 0.0), &[b]).unwrap();
    doc
}

fn usd(amount: &str) -> Money {
    Money::parse(amount, 2, "USD").unwrap()
}

fn table_data() -> BlockData {
    BlockData {
        category: "Mobiliario".to_owned(),
        kind: "Mesa".to_owned(),
        material: "Roble".to_owned(),
        supplier: "Carpintería Sur".to_owned(),
        cost: Some(usd("120.5")),
    }
}

fn pen() -> Currency {
    Currency {
        code: "PEN".to_owned(),
        name: "Peruvian sol".to_owned(),
        symbol: "S/".to_owned(),
        decimals: 2,
        active: true,
        format: None,
    }
}

fn with_pen() -> Currencies {
    let mut register = Currencies::default();
    register.list.push(pen());
    register
}

fn invalid(result: Result<impl std::fmt::Debug, DocumentError>, part: &str) {
    match result {
        Err(DocumentError::InvalidBlockData(m)) => {
            assert!(m.contains(part), "{m:?} lacks {part:?}")
        }
        other => panic!("expected InvalidBlockData with {part:?}, got {other:?}"),
    }
}

#[test]
fn an_amount_is_exact_with_its_currencys_decimals() {
    let cost = usd("120.5");
    assert_eq!((cost.units, cost.decimals), (12050, 2));
    assert_eq!(cost.amount(), "120.50");
    assert_eq!(usd("0").amount(), "0.00");
    assert_eq!(usd(" 7.05 ").amount(), "7.05");
    let yen = Money::parse("1500", 0, "JPY").unwrap();
    assert_eq!((yen.units, yen.amount()), (1500, "1500".to_owned()));
    assert_eq!(Money::parse("0.0001", 4, "XAU").unwrap().amount(), "0.0001");
    for text in [
        "",
        "-1",
        "+1",
        "1,5",
        "abc",
        ".5",
        "1.2.3",
        "1e3",
        "99999999999999999999",
    ] {
        invalid(Money::parse(text, 2, "USD"), "is not an amount");
    }
    invalid(Money::parse("1.234", 2, "USD"), "more than 2 decimals");
    invalid(Money::parse("1", 5, "USD"), "at most 4 decimals");
}

#[test]
fn a_drawing_without_a_register_has_us_dollars_and_writes_nothing() {
    let doc = drawing();
    let register = doc.currencies().unwrap();
    assert_eq!(register.principal, "USD");
    assert_eq!(register.list, [Currency::us_dollar()]);
    assert!(register.list[0].active && register.list[0].decimals == 2);
    assert_eq!(doc.block_data("MESA").unwrap(), None);
    let objects = doc.cad().objects.len();
    let bytes = doc.to_dxf_bytes().unwrap();
    assert_eq!(doc.cad().objects.len(), objects, "reading adds nothing");
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("ARQSTUDIOCAD_"), "nothing written");
}

#[test]
fn block_data_is_set_read_and_removed() {
    let mut doc = drawing();
    assert_eq!(doc.set_block_data("mesa", &table_data()).unwrap(), "MESA");
    assert_eq!(doc.block_data("Mesa").unwrap(), Some(table_data()));
    let partial = BlockData {
        material: "Pino".to_owned(),
        ..BlockData::default()
    };
    doc.set_block_data("SILLA", &partial).unwrap();
    assert_eq!(
        doc.blocks_data(),
        BTreeMap::from([
            ("MESA".to_owned(), table_data()),
            ("SILLA".to_owned(), partial)
        ])
    );
    let record = doc.cad().block_records.get("MESA").unwrap().handle();
    let entry = doc.cad().xrecord(record, DATA_KEY).unwrap().handle;
    doc.set_block_data("MESA", &BlockData::default()).unwrap();
    assert_eq!(doc.block_data("MESA").unwrap(), None);
    assert!(!doc.cad().objects.contains_key(&entry), "its XRECORD went");
    let dictionary = doc.cad().extension_dictionary_handle(record).unwrap();
    let Some(ObjectType::Dictionary(d)) = doc.cad().objects.get(&dictionary) else {
        panic!("the extension dictionary stays");
    };
    assert_eq!(d.get(DATA_KEY), None, "and its entry");
}

/// Replaces the value of the pair named `name` in block MESA's data.
fn set_raw(doc: &mut Document, name: &str, value: Option<&str>) {
    let record = doc.cad().block_records.get("MESA").unwrap().handle();
    let xrecord = doc.cad_mut().xrecord_mut(record, DATA_KEY).unwrap();
    let at = xrecord
        .entries
        .iter()
        .position(|e| e.value.as_string() == Some(name))
        .unwrap();
    match value {
        Some(v) => xrecord.entries[at + 1].value = XRecordValue::String(v.to_owned()),
        None => {
            xrecord.entries.drain(at..at + 2);
        }
    }
}

#[test]
fn a_cost_without_its_currency_or_with_too_many_decimals_does_not_read() {
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    set_raw(&mut doc, "cost", Some("1.12345"));
    invalid(
        doc.block_data("MESA"),
        "block MESA's cost could not be read",
    );
    set_raw(&mut doc, "cost", Some("1.1234"));
    assert_eq!(
        doc.block_data("MESA")
            .unwrap()
            .unwrap()
            .cost
            .unwrap()
            .decimals,
        4
    );
    set_raw(&mut doc, "currency", None);
    invalid(
        doc.block_data("MESA"),
        "block MESA's cost could not be read",
    );
}

#[test]
fn data_with_only_fields_this_version_does_not_know_is_none() {
    let mut doc = drawing();
    let data = BlockData {
        category: "Mobiliario".to_owned(),
        ..BlockData::default()
    };
    doc.set_block_data("MESA", &data).unwrap();
    set_raw(&mut doc, "category", None);
    let record = doc.cad().block_records.get("MESA").unwrap().handle();
    let xrecord = doc.cad_mut().xrecord_mut(record, DATA_KEY).unwrap();
    xrecord.add_string(NAME_CODE, "color");
    xrecord.add_string(VALUE_CODE, "red");
    assert_eq!(doc.block_data("MESA").unwrap(), None);
    assert!(doc.blocks_data().is_empty());
}

#[test]
fn a_cost_in_a_currency_the_register_never_held_does_not_hold_it_back() {
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    set_raw(&mut doc, "currency", Some("BOB"));
    assert_eq!(doc.currency_uses(), BTreeMap::from([("BOB".to_owned(), 1)]));
    doc.set_currencies(&with_pen()).unwrap();
    assert_eq!(doc.currencies().unwrap(), with_pen());
}

#[test]
fn block_data_that_cannot_be_written_is_refused_and_nothing_changes() {
    let mut doc = drawing();
    let refused = doc.set_block_data("NADA", &table_data());
    assert!(
        matches!(&refused, Err(DocumentError::InvalidBlockName(m)) if m == "no block named \"NADA\""),
        "{refused:?}"
    );
    let with = |f: fn(&mut BlockData)| {
        let mut data = table_data();
        f(&mut data);
        data
    };
    invalid(
        doc.set_block_data("MESA", &with(|d| d.category = "a\nb".to_owned())),
        "the category is one line",
    );
    invalid(
        doc.set_block_data("MESA", &with(|d| d.kind = "x".repeat(256))),
        "the type is one line of at most 255",
    );
    assert!(
        doc.set_block_data("MESA", &with(|d| d.supplier = "ñ".repeat(255)))
            .is_ok(),
        "255 characters, not bytes"
    );
    doc.set_block_data("MESA", &BlockData::default()).unwrap();
    invalid(
        doc.set_block_data("MESA", &with(|d| d.material = "\t".to_owned())),
        "the material",
    );
    invalid(
        doc.set_block_data(
            "MESA",
            &with(|d| d.cost = Some(Money::parse("1", 2, "PEN").unwrap())),
        ),
        "PEN is not an active currency",
    );
    let mut register = with_pen();
    register.list[1].active = false;
    doc.set_currencies(&register).unwrap();
    invalid(
        doc.set_block_data(
            "MESA",
            &with(|d| d.cost = Some(Money::parse("1", 2, "PEN").unwrap())),
        ),
        "PEN is not an active currency",
    );
    invalid(
        doc.set_block_data(
            "MESA",
            &with(|d| d.cost = Some(Money::parse("1", 3, "USD").unwrap())),
        ),
        "a cost in USD has 2 decimals",
    );
    invalid(
        doc.set_block_data(
            "MESA",
            &with(|d| {
                d.cost = Some(Money {
                    units: -1,
                    decimals: 2,
                    currency: "USD".to_owned(),
                })
            }),
        ),
        "zero or more",
    );
    assert_eq!(doc.block_data("MESA").unwrap(), None, "unchanged");
}

#[test]
fn setting_block_data_is_one_change_undone_and_redone() {
    let mut doc = drawing();
    doc.begin_change().unwrap();
    doc.set_block_data("MESA", &table_data()).unwrap();
    let mut change = doc.commit_change().unwrap();
    let summary = change.summary().clone();
    assert_eq!(summary.blocks_modified(), ["MESA"]);
    assert!(summary.created().is_empty() && summary.modified().is_empty());
    assert!(!summary.document_changed());
    doc.undo_change(&mut change).unwrap();
    assert_eq!(doc.block_data("MESA").unwrap(), None);
    doc.redo_change(&mut change).unwrap();
    assert_eq!(doc.block_data("MESA").unwrap(), Some(table_data()));
    // The same data again changes nothing.
    doc.begin_change().unwrap();
    doc.set_block_data("MESA", &table_data()).unwrap();
    assert!(doc.commit_change().unwrap().summary().is_empty());
}

#[test]
fn the_register_is_set_checked_and_undone() {
    let mut doc = drawing();
    doc.begin_change().unwrap();
    doc.set_currencies(&with_pen()).unwrap();
    let mut change = doc.commit_change().unwrap();
    assert!(change.summary().document_changed());
    assert!(change.summary().blocks_modified().is_empty());
    assert_eq!(doc.currencies().unwrap(), with_pen());
    doc.undo_change(&mut change).unwrap();
    assert_eq!(doc.currencies().unwrap(), Currencies::default());
    doc.redo_change(&mut change).unwrap();
    assert_eq!(doc.currencies().unwrap(), with_pen());
    doc.begin_change().unwrap();
    doc.set_currencies(&with_pen()).unwrap();
    assert!(
        doc.commit_change().unwrap().summary().is_empty(),
        "the same again"
    );

    let refuse = |doc: &mut Document, f: fn(&mut Currencies), part: &str| {
        let mut register = with_pen();
        f(&mut register);
        invalid(doc.set_currencies(&register), part);
        assert_eq!(doc.currencies().unwrap(), with_pen(), "unchanged");
    };
    refuse(
        &mut doc,
        |r| r.list[1].code = "pen".to_owned(),
        "\"pen\" is not a currency code",
    );
    refuse(
        &mut doc,
        |r| r.list[1].code = "PE".to_owned(),
        "is not a currency code",
    );
    refuse(
        &mut doc,
        |r| r.list[1].code = "ABCDEFGHIJK".to_owned(),
        "is not a currency code",
    );
    refuse(
        &mut doc,
        |r| r.list[1].code = "USD".to_owned(),
        "USD is in the register twice",
    );
    refuse(
        &mut doc,
        |r| r.list[1].name = " ".to_owned(),
        "PEN has no name",
    );
    refuse(
        &mut doc,
        |r| r.list[1].symbol = "S\n".to_owned(),
        "the symbol",
    );
    refuse(
        &mut doc,
        |r| r.list[1].decimals = 5,
        "PEN has more than 4 decimals",
    );
    refuse(
        &mut doc,
        |r| r.principal = "EUR".to_owned(),
        "the principal currency",
    );
    refuse(
        &mut doc,
        |r| r.list[0].active = false,
        "the principal currency",
    );
    let mut own = with_pen();
    own.list.push(Currency {
        code: "UF2".to_owned(),
        name: "Unidad de fomento".to_owned(),
        symbol: String::new(),
        decimals: 4,
        active: false,
        format: None,
    });
    own.principal = "PEN".to_owned();
    doc.set_currencies(&own).unwrap();
    assert_eq!(doc.currencies().unwrap(), own, "an own code, no symbol");
}

#[test]
fn a_currency_a_cost_uses_stays_active_with_its_decimals() {
    let mut doc = drawing();
    doc.set_currencies(&with_pen()).unwrap();
    let in_soles = |amount: &str| BlockData {
        cost: Some(Money::parse(amount, 2, "PEN").unwrap()),
        ..BlockData::default()
    };
    doc.set_block_data("MESA", &in_soles("450")).unwrap();
    doc.set_block_data("SILLA", &in_soles("80")).unwrap();
    assert_eq!(doc.currency_uses(), BTreeMap::from([("PEN".to_owned(), 2)]));
    let refuse = |doc: &mut Document, f: fn(&mut Currencies)| {
        let mut register = with_pen();
        f(&mut register);
        invalid(
            doc.set_currencies(&register),
            "PEN is used by 2 blocks: it must stay active, with its decimals",
        );
    };
    refuse(&mut doc, |r| {
        r.list.pop();
    });
    refuse(&mut doc, |r| r.list[1].active = false);
    refuse(&mut doc, |r| r.list[1].decimals = 3);
    doc.set_block_data("SILLA", &BlockData::default()).unwrap();
    let mut register = with_pen();
    register.list[1].active = false;
    invalid(doc.set_currencies(&register), "PEN is used by 1 block:");
    // Renaming the currency or its symbol is allowed.
    let mut register = with_pen();
    register.list[1].symbol = "S/.".to_owned();
    doc.set_currencies(&register).unwrap();
    // USD, which nothing uses, can go once another is principal.
    register.list.remove(0);
    register.principal = "PEN".to_owned();
    doc.set_currencies(&register).unwrap();
    assert_eq!(doc.currencies().unwrap().list, [register.list[0].clone()]);
}

#[test]
fn block_data_and_the_register_are_saved_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = drawing();
    doc.set_currencies(&with_pen()).unwrap();
    doc.set_block_data("MESA", &table_data()).unwrap();
    let in_soles = BlockData {
        cost: Some(Money::parse("80", 2, "PEN").unwrap()),
        ..BlockData::default()
    };
    doc.set_block_data("SILLA", &in_soles).unwrap();
    for name in ["data.dwg", "data.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        assert_eq!(
            back.block_data("MESA").unwrap(),
            Some(table_data()),
            "{name}"
        );
        assert_eq!(
            back.block_data("SILLA").unwrap(),
            Some(in_soles.clone()),
            "{name}"
        );
        assert_eq!(back.currencies().unwrap(), with_pen(), "{name}");
        assert!(doc.validate_roundtrip(&back).is_empty(), "{name}");
    }
}

#[test]
fn a_save_that_loses_block_data_or_the_register_is_caught() {
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    let mut back = doc.snapshot();
    assert!(doc.validate_roundtrip(&back).is_empty());
    back.write_block_data("MESA", &BlockData::default());
    back.write_currencies(&with_pen());
    assert_eq!(
        doc.validate_roundtrip(&back),
        ["the currency register changed", "block MESA's data changed"]
    );
}

#[test]
fn pairs_a_version_does_not_know_are_kept() {
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    doc.set_currencies(&with_pen()).unwrap();
    let record = doc.cad().block_records.get("MESA").unwrap().handle();
    // A later version added a field to each.
    doc.cad_mut().xrecord_mut(record, DATA_KEY).unwrap().entries[0].value = XRecordValue::Int16(2);
    let extra = |x: &mut acadrust::objects::XRecord| {
        x.add_string(NAME_CODE, "color");
        x.add_string(VALUE_CODE, "red");
    };
    extra(doc.cad_mut().xrecord_mut(record, DATA_KEY).unwrap());
    let register = doc.register().unwrap().handle;
    if let Some(ObjectType::XRecord(x)) = doc.cad_mut().objects.get_mut(&register) {
        extra(x);
    }
    assert_eq!(
        doc.block_data("MESA").unwrap(),
        Some(table_data()),
        "read as before"
    );
    assert_eq!(doc.currencies().unwrap(), with_pen());
    let mut cheaper = table_data();
    cheaper.cost = Some(usd("99"));
    doc.set_block_data("MESA", &cheaper).unwrap();
    let mut register = with_pen();
    register.list[1].name = "Sol".to_owned();
    doc.set_currencies(&register).unwrap();
    let strings = |x: &acadrust::objects::XRecord| -> Vec<String> {
        x.entries
            .iter()
            .filter_map(|e| e.value.as_string().map(str::to_owned))
            .collect()
    };
    let data = strings(doc.cad().xrecord(record, DATA_KEY).unwrap());
    assert!(
        data.ends_with(&["color".to_owned(), "red".to_owned()]),
        "{data:?}"
    );
    assert!(data.contains(&"99.00".to_owned()));
    assert_eq!(
        doc.cad().xrecord(record, DATA_KEY).unwrap().entries[0].value,
        XRecordValue::Int16(1),
        "written as this version writes"
    );
    let kept = strings(doc.register().unwrap());
    assert!(
        kept.ends_with(&["color".to_owned(), "red".to_owned()]),
        "{kept:?}"
    );
    assert!(kept.contains(&"Sol".to_owned()));
}

#[test]
fn data_that_does_not_read_is_reported_and_left_alone() {
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    doc.set_block_data("SILLA", &table_data()).unwrap();
    let record = doc.cad().block_records.get("MESA").unwrap().handle();
    let broken = |x: &mut acadrust::objects::XRecord| x.add_string(NAME_CODE, "dangling");
    broken(doc.cad_mut().xrecord_mut(record, DATA_KEY).unwrap());
    invalid(
        doc.block_data("MESA"),
        "block MESA's data could not be read",
    );
    assert_eq!(doc.blocks_data().keys().collect::<Vec<_>>(), ["SILLA"]);
    let chair = doc.cad().block_records.get("SILLA").unwrap().handle();
    let xrecord = doc.cad_mut().xrecord_mut(chair, DATA_KEY).unwrap();
    xrecord.entries[0].code = VERSION_CODE + 1;
    invalid(doc.block_data("SILLA"), "could not be read");
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    let record = doc.cad().block_records.get("MESA").unwrap().handle();
    let xrecord = doc.cad_mut().xrecord_mut(record, DATA_KEY).unwrap();
    let at = xrecord
        .entries
        .iter()
        .position(|e| e.value.as_string() == Some("120.50"))
        .unwrap();
    xrecord.entries[at].value = XRecordValue::String("12,50".to_owned());
    invalid(
        doc.block_data("MESA"),
        "block MESA's cost could not be read",
    );
    doc.set_currencies(&with_pen()).unwrap();
    let register = doc.register().unwrap().handle;
    if let Some(ObjectType::XRecord(x)) = doc.cad_mut().objects.get_mut(&register) {
        x.entries[0].value = XRecordValue::Int16(0);
    }
    invalid(
        doc.currencies(),
        "the drawing's currencies could not be read",
    );
    let key = CURRENCIES_KEY;
    assert!(doc.register().is_some(), "{key} left in the file");
}

#[test]
fn the_data_follows_the_block() {
    let mut doc = drawing();
    doc.set_block_data("MESA", &table_data()).unwrap();
    // RENAME keeps it.
    doc.rename_block("MESA", "ESCRITORIO").unwrap();
    assert_eq!(doc.block_data("ESCRITORIO").unwrap(), Some(table_data()));
    // The block editor keeps it.
    let editor = doc.block_editor("ESCRITORIO").unwrap();
    doc.replace_block_contents("ESCRITORIO", &editor).unwrap();
    assert_eq!(doc.block_data("ESCRITORIO").unwrap(), Some(table_data()));
    // PURGE removes it with the block.
    doc.set_block_data("SILLA", &table_data()).unwrap();
    let record = doc.cad().block_records.get("SILLA").unwrap().handle();
    let entry = doc.cad().xrecord(record, DATA_KEY).unwrap().handle;
    let dictionary = doc.cad().extension_dictionary_handle(record).unwrap();
    doc.purge_blocks(&["SILLA".to_owned()], false).unwrap();
    assert!(!doc.cad().objects.contains_key(&entry));
    assert!(!doc.cad().objects.contains_key(&dictionary));
}

#[test]
fn a_copied_cost_keeps_the_drawings_currency_or_adds_it_active() {
    let mut source = drawing();
    source.set_currencies(&with_pen()).unwrap();
    let in_soles = BlockData {
        cost: Some(Money::parse("80", 2, "PEN").unwrap()),
        ..BlockData::default()
    };
    source.set_block_data("SILLA", &in_soles).unwrap();
    // The drawing has PEN under another name: it keeps its own, once.
    let mut doc = Document::new();
    let mut own = with_pen();
    own.list[1].name = "Sol".to_owned();
    doc.set_currencies(&own).unwrap();
    doc.copy_block(&source, "SILLA", &BTreeMap::new()).unwrap();
    assert_eq!(doc.currencies().unwrap(), own);
    // The source's register lists it inactive (edited elsewhere): it comes active.
    let mut inactive = with_pen();
    inactive.list[1].active = false;
    source.write_currencies(&inactive);
    let mut doc = Document::new();
    doc.copy_block(&source, "SILLA", &BTreeMap::new()).unwrap();
    assert_eq!(doc.currencies().unwrap(), with_pen());
}

#[test]
fn a_copied_block_brings_its_data_and_its_currency() {
    let mut source = drawing();
    let mut register = with_pen();
    register.list[1].symbol = "S/.".to_owned();
    source.set_currencies(&register).unwrap();
    let in_soles = BlockData {
        supplier: "Maderera".to_owned(),
        cost: Some(Money::parse("80", 2, "PEN").unwrap()),
        ..BlockData::default()
    };
    source.set_block_data("SILLA", &in_soles).unwrap();
    let mut doc = Document::new();
    let mut inactive = Currencies::default();
    inactive.list.push(Currency {
        code: "EUR".to_owned(),
        name: "Euro".to_owned(),
        symbol: "€".to_owned(),
        decimals: 2,
        active: false,
        format: None,
    });
    doc.set_currencies(&inactive).unwrap();
    doc.copy_block(&source, "SILLA", &BTreeMap::new()).unwrap();
    assert_eq!(doc.block_data("SILLA").unwrap(), Some(in_soles.clone()));
    let mut expected = inactive.clone();
    expected.list.push(register.list[1].clone());
    assert_eq!(
        doc.currencies().unwrap(),
        expected,
        "PEN added as the source has it"
    );
    // A cost in a currency the source's register lacks brings one by its code.
    let mut odd = drawing();
    odd.set_block_data("MESA", &table_data()).unwrap();
    let record = odd.cad().block_records.get("MESA").unwrap().handle();
    let xrecord = odd.cad_mut().xrecord_mut(record, DATA_KEY).unwrap();
    let at = xrecord
        .entries
        .iter()
        .position(|e| e.value.as_string() == Some("USD"))
        .unwrap();
    xrecord.entries[at].value = XRecordValue::String("BOB".to_owned());
    let mut doc = Document::new();
    doc.copy_block(&odd, "MESA", &BTreeMap::new()).unwrap();
    let bob = doc.currencies().unwrap().list.pop().unwrap();
    assert_eq!(
        (
            bob.code.as_str(),
            bob.name.as_str(),
            bob.decimals,
            bob.active
        ),
        ("BOB", "BOB", 2, true)
    );
    // Redefining takes the source's data, none included.
    let mut doc = drawing();
    doc.set_block_data("SILLA", &table_data()).unwrap();
    let plain = {
        let mut plain = Document::new();
        let a = line(&mut plain, 5.0);
        let r = plain.create_block("SILLA", p(0.0, 0.0), &[a]).unwrap();
        plain.delete_entity(r).unwrap();
        plain
    };
    let redefine = BTreeMap::from([("SILLA".to_owned(), ClashChoice::Redefine)]);
    doc.copy_block(&plain, "SILLA", &redefine).unwrap();
    assert_eq!(doc.block_data("SILLA").unwrap(), None);
    doc.copy_block(&source, "SILLA", &redefine).unwrap();
    assert_eq!(doc.block_data("SILLA").unwrap(), Some(in_soles));
}

#[test]
fn the_register_keeps_how_amounts_are_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = drawing();
    let register = doc.currencies().unwrap();
    assert_eq!(
        (register.thousands, register.decimal),
        (',', '.'),
        "1,250.50"
    );
    for (thousands, decimal) in [('.', ','), (' ', ','), (' ', '.'), (',', '.')] {
        let mut format = with_pen();
        format.thousands = thousands;
        format.decimal = decimal;
        doc.set_currencies(&format).unwrap();
        assert_eq!(doc.currencies().unwrap(), format);
        for name in ["format.dwg", "format.dxf"] {
            let path = dir.path().join(name);
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            assert_eq!(back.currencies().unwrap(), format, "{name}: {thousands:?}");
        }
    }
    for (thousands, decimal) in [('.', '.'), (',', ','), ('.', ' '), ('_', '.'), (',', ';')] {
        let mut format = with_pen();
        format.thousands = thousands;
        format.decimal = decimal;
        invalid(
            doc.set_currencies(&format),
            "thousands are separated by a comma, a point or a space",
        );
    }
    // A name the register does not know does not read.
    let handle = doc.register().unwrap().handle;
    if let Some(ObjectType::XRecord(x)) = doc.cad_mut().objects.get_mut(&handle) {
        let at = x
            .entries
            .iter()
            .position(|e| e.value.as_string() == Some("decimal"))
            .unwrap();
        x.entries[at + 1].value = XRecordValue::String("tab".to_owned());
    }
    invalid(
        doc.currencies(),
        "the drawing's currencies could not be read",
    );
}

#[test]
fn a_currency_may_write_amounts_its_own_way() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = drawing();
    let mut register = with_pen();
    register.list.push(Currency {
        code: "EUR".to_owned(),
        name: "Euro".to_owned(),
        symbol: "€".to_owned(),
        decimals: 2,
        active: true,
        format: Some(('.', ',')),
    });
    register.list[1].format = Some((' ', '.'));
    register.thousands = ' ';
    register.decimal = ',';
    doc.set_currencies(&register).unwrap();
    let back = doc.currencies().unwrap();
    assert_eq!(back, register);
    assert_eq!(back.format_of("EUR"), ('.', ','));
    assert_eq!(back.format_of("PEN"), (' ', '.'));
    assert_eq!(back.format_of("USD"), (' ', ','), "the drawing's");
    assert_eq!(back.format_of("BOB"), (' ', ','), "not in the register");
    for name in ["own.dwg", "own.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        assert_eq!(
            Document::open(&path).unwrap().currencies().unwrap(),
            register,
            "{name}"
        );
    }
    // Back to the drawing's.
    register.list[2].format = None;
    doc.set_currencies(&register).unwrap();
    assert_eq!(doc.currencies().unwrap().format_of("EUR"), (' ', ','));
    // Refused like the drawing's.
    for format in [('.', '.'), ('_', ','), (',', ' ')] {
        let mut bad = register.clone();
        bad.list[1].format = Some(format);
        invalid(doc.set_currencies(&bad), "PEN: thousands are separated by");
    }
    // A currency given one separator alone, either one, does not read.
    for alone in ["decimal", "thousands"] {
        let mut doc = drawing();
        doc.set_currencies(&register).unwrap();
        let handle = doc.register().unwrap().handle;
        if let Some(ObjectType::XRecord(x)) = doc.cad_mut().objects.get_mut(&handle) {
            let at = x
                .entries
                .iter()
                .rposition(|e| e.value.as_string() == Some(alone))
                .unwrap();
            x.entries.drain(at..at + 2);
        }
        invalid(
            doc.currencies(),
            "the drawing's currencies could not be read",
        );
    }
}
