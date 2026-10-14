//! Tests of the inventory and of saves that would lose data
//! (ADR 0017 D5.3, D5.4).

use std::fs;

use acadrust::entities::{LwPolyline, Polyline2D};
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{EntityType, Handle};
use arq_geometry::{Line, Point2};

use crate::document::Document;
use crate::entity::PlanGeometry;
use crate::error::DocumentError;
use crate::verify::fault;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// A line carrying XDATA, which the platform does not show: only the
/// inventory sees it.
fn with_xdata() -> Document {
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    let line = Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let id = doc
        .add_entity(PlanGeometry::at_elevation(line.into(), 0.0).unwrap(), layer)
        .unwrap();
    let mut record = ExtendedDataRecord::new("ARQ_TEST");
    record.add_value(XDataValue::String("kept?".to_owned()));
    doc.cad_mut()
        .app_ids
        .add(acadrust::tables::AppId::new("ARQ_TEST"))
        .ok();
    let e = doc.cad_mut().get_entity_mut(Handle::new(id)).unwrap();
    e.common_mut().extended_data.add_record(record);
    doc
}

/// A fault that drops every XDATA group (codes 1001 to 1071) from a DXF.
fn drop_xdata(bytes: &mut Vec<u8>) {
    let text = String::from_utf8(bytes.clone()).unwrap();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut kept = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let code: i32 = lines[i].trim().parse().unwrap_or(0);
        if !(1000..=1071).contains(&code) {
            kept.push(lines[i]);
            kept.push(lines[i + 1]);
        }
        i += 2;
    }
    kept.extend(&lines[i..]);
    *bytes = kept.join("\n").into_bytes();
}

#[test]
fn the_same_drawing_loses_nothing() {
    let doc = with_xdata();
    assert_eq!(doc.losses_in(&doc.snapshot()), []);
}

#[test]
fn fewer_of_a_kind_is_a_loss_more_is_not() {
    let doc = Document::new();
    let mut fewer = doc.snapshot();
    let dictionary = fewer
        .cad()
        .objects
        .iter()
        .find(|(_, o)| matches!(o, acadrust::objects::ObjectType::Dictionary(_)))
        .map(|(h, _)| *h)
        .unwrap();
    fewer.cad_mut().objects.remove(&dictionary);
    let losses = doc.losses_in(&fewer);
    assert_eq!(losses.len(), 1, "{losses:?}");
    assert_eq!(losses[0].kind(), "object DICTIONARY");
    assert_eq!(losses[0].before(), losses[0].after() + 1);
    assert_eq!(fewer.losses_in(&doc), [], "more is not a loss");
}

/// AutoCAD writes a 2D POLYLINE as an LWPOLYLINE (PLINETYPE 2), and so does
/// the DXF writer in blocks: a conversion, not a loss.
#[test]
fn a_polyline_written_as_lightweight_is_not_lost() {
    let mut doc = Document::new();
    doc.cad_mut()
        .add_entity(EntityType::Polyline2D(Polyline2D::new()))
        .unwrap();
    let mut converted = Document::new();
    converted
        .cad_mut()
        .add_entity(EntityType::LwPolyline(LwPolyline::new()))
        .unwrap();
    assert_eq!(doc.losses_in(&converted), []);
    let lost = Document::new();
    let losses = doc.losses_in(&lost);
    assert_eq!(losses.len(), 1);
    assert_eq!(losses[0].to_string(), "entity POLYLINE: 1 → 0");
}

#[test]
fn a_save_that_would_lose_data_replaces_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plano.dxf");
    fs::write(&path, b"the previous version").unwrap();
    fault::set(drop_xdata);
    let error = with_xdata().save(&path).unwrap_err();
    let DocumentError::Lossy {
        path: target,
        losses,
    } = &error
    else {
        panic!("{error}")
    };
    assert_eq!(target, &path);
    assert!(
        losses.iter().any(|l| l.kind() == "XDATA on entities"),
        "{losses:?}"
    );
    assert_eq!(fs::read(&path).unwrap(), b"the previous version");
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        1,
        "no temporary file left"
    );
    assert!(
        error.to_string().contains("XDATA on entities: 1 → 0"),
        "{error}"
    );

    // Accepted, the same loss no longer stops it.
    fault::set(drop_xdata);
    with_xdata()
        .save_accepting(&path, |loss| loss.kind() == "XDATA on entities")
        .unwrap();
    assert_ne!(fs::read(&path).unwrap(), b"the previous version");
}

#[test]
fn intact_xdata_is_no_loss() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["plano.dwg", "plano.dxf"] {
        with_xdata().save(dir.path().join(name)).unwrap();
    }
}
