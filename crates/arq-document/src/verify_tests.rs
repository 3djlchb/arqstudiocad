//! Tests of read-back validation and safe save (ADR 0017 D5.2, D5.4).

use std::fs;
use std::path::Path;

use arq_geometry::{Circle, Geometry2, Line, Point2};

use super::fault;
use crate::document::Document;
use crate::entity::{Color, PlanGeometry};
use crate::error::DocumentError;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn plan(g: impl Into<Geometry2>) -> PlanGeometry {
    PlanGeometry::at_elevation(g.into(), 0.0).unwrap()
}

fn sample() -> Document {
    let mut doc = Document::new();
    let walls = doc.create_layer("MUROS").unwrap();
    doc.add_entity(
        plan(Line::new(p(0.0, 0.0), p(123.456, 7.0)).unwrap()),
        walls,
    )
    .unwrap();
    doc.add_entity(plan(Circle::new(p(5.0, 5.0), 2.0).unwrap()), walls)
        .unwrap();
    doc
}

/// Every file in `dir`, by name.
fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_drawing_read_back_unchanged_has_no_differences() {
    let doc = sample();
    assert_eq!(
        doc.validate_roundtrip(&doc.snapshot()),
        Vec::<String>::new()
    );
    let dir = tempfile::tempdir().unwrap();
    for name in ["plano.dwg", "plano.dxf"] {
        doc.save(dir.path().join(name)).unwrap();
        let back = Document::open(dir.path().join(name)).unwrap();
        assert!(doc.validate_roundtrip(&back).is_empty(), "{name}");
    }
    assert_eq!(
        files(dir.path()),
        ["plano.dwg", "plano.dxf"],
        "nothing left over"
    );
}

#[test]
fn differences_are_named() {
    let doc = sample();
    let mut other = doc.snapshot();
    let ids: Vec<u64> = other.entities().iter().map(|e| e.id()).collect();
    other.delete_entity(ids[0]).unwrap();
    other.set_entity_color(ids[1], Color::Index(3)).unwrap();
    let extra = other.create_layer("NUEVA").unwrap();
    other.set_current_layer(extra).unwrap();
    let differences = doc.validate_roundtrip(&other);
    let has = |prefix: &str| differences.iter().any(|d| d.starts_with(prefix));
    assert!(has("current layer"), "{differences:?}");
    assert!(
        has("2 entities became 1") && has("lost: ") && has("changed: "),
        "{differences:?}"
    );
    assert!(has("added: "), "the new layer: {differences:?}");
}

#[test]
fn old_versions_may_lose_what_they_cannot_hold() {
    let mut doc = sample();
    let id = doc.entities()[0].id();
    doc.set_entity_color(
        id,
        Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        },
    )
    .unwrap();
    let walls = doc.layer_by_name("MUROS").unwrap().id();
    doc.set_layer_plottable(walls, false).unwrap();
    let mut back = doc.snapshot();
    back.set_entity_color(id, Color::Index(5)).unwrap();
    back.set_layer_plottable(walls, true).unwrap();
    assert_eq!(doc.validate_roundtrip(&back).len(), 2, "2018 keeps both");
    back.cad_mut().version = acadrust::types::DxfVersion::AC1014;
    assert!(
        doc.validate_roundtrip(&back).is_empty(),
        "R14 holds neither"
    );
}

/// An R12 drawing names its linetype `CONTINUOUS`; saved as DWG it is
/// `Continuous`: the same linetype, not a loss.
#[test]
fn linetype_names_ignore_case() {
    let doc = sample();
    let mut back = doc.snapshot();
    for layer in back.cad_mut().layers.iter_mut() {
        layer.line_type = layer.line_type.to_uppercase();
    }
    back.sync_layers();
    assert!(doc.validate_roundtrip(&back).is_empty());
}

#[test]
fn a_file_that_does_not_read_back_is_not_saved_and_is_kept_aside() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plano.dwg");
    fs::write(&path, b"the previous version").unwrap();
    fault::set(|bytes| bytes.truncate(bytes.len() / 2));
    let error = sample().save(&path).unwrap_err();
    let DocumentError::NotVerified {
        path: target,
        kept,
        differences,
    } = &error
    else {
        panic!("{error}")
    };
    assert_eq!(target, &path);
    assert!(
        differences[0].starts_with("the written file cannot be read"),
        "{differences:?}"
    );
    assert_eq!(
        kept.as_deref(),
        Some(dir.path().join("plano.unverified.dwg").as_path())
    );
    assert_eq!(
        fs::read(&path).unwrap(),
        b"the previous version",
        "untouched"
    );
    assert_eq!(files(dir.path()), ["plano.dwg", "plano.unverified.dwg"]);
    assert!(
        error.to_string().contains("the original is unchanged"),
        "{error}"
    );
}

/// A change the engine reads without complaint is still caught: the
/// comparison, not the reader, finds it.
#[test]
fn a_silently_changed_value_is_caught_by_the_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plano.dxf");
    fault::set(|bytes| {
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("123.456"), "the line's end is written so");
        *bytes = text.replacen("123.456", "124.456", 1).into_bytes();
    });
    let error = sample().save(&path).unwrap_err();
    let DocumentError::NotVerified { differences, .. } = &error else {
        panic!("{error}")
    };
    assert!(
        differences.iter().any(|d| d.starts_with("changed: ")),
        "{differences:?}"
    );
    assert!(!path.exists());
    assert_eq!(files(dir.path()), ["plano.unverified.dxf"]);
}

/// A written file that only recovery can read is not a saved drawing.
#[test]
fn a_file_that_reads_back_as_damaged_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plano.dxf");
    fault::set(|bytes| {
        // An invalid group code right after the CIRCLE's type.
        let text = String::from_utf8(bytes.clone()).unwrap();
        let mut lines: Vec<&str> = text.split('\n').collect();
        let entities = lines.iter().position(|l| l.trim() == "ENTITIES").unwrap();
        let circle = (entities..lines.len())
            .find(|&i| lines[i].trim() == "CIRCLE" && lines[i - 1].trim() == "0")
            .unwrap();
        lines[circle + 1] = "zz";
        *bytes = lines.join("\n").into_bytes();
    });
    let error = sample().save(&path).unwrap_err();
    let DocumentError::NotVerified { differences, .. } = &error else {
        panic!("{error}")
    };
    assert!(
        differences[0].starts_with("the written file reads as damaged"),
        "{differences:?}"
    );
    assert!(!path.exists());
}

#[test]
fn unverified_files_keep_their_extension() {
    assert_eq!(
        crate::document::unverified_path(Path::new("a/plano.v2.dwg")),
        Path::new("a/plano.v2.unverified.dwg")
    );
}

fn xref_sample() -> Document {
    Document::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xrefs/attachment.dxf"),
    )
    .unwrap()
}

#[test]
fn xref_metadata_changes_are_caught_even_when_counts_match() {
    let doc = xref_sample();
    for change in 0..4 {
        let mut back = doc.snapshot();
        let block = back.cad_mut().block_records.get_mut("SITE").unwrap();
        match change {
            0 => block.xref_path = "other.dxf".into(),
            1 => block.flags.is_xref_overlay = true,
            2 => block.flags.is_xref_unloaded = true,
            _ => {
                block.xref_path.clear();
                block.flags.is_xref = false;
            }
        }
        assert!(
            doc.validate_roundtrip(&back)
                .iter()
                .any(|d| d.starts_with("Xref SITE metadata changed"))
        );
    }
}

#[test]
fn xref_out_of_plane_placement_changes_are_caught() {
    let doc = xref_sample();
    let id = doc.cad().entities().next().unwrap().common().handle;
    for change in 0..3 {
        let mut back = doc.snapshot();
        let acadrust::EntityType::Insert(i) = back.cad_mut().get_entity_mut(id).unwrap() else {
            panic!("expected INSERT")
        };
        match change {
            0 => i.insert_point.z += 1.0,
            1 => i.set_z_scale(3.0),
            _ => i.row_spacing += 4.0,
        }
        assert!(
            doc.validate_roundtrip(&back)
                .iter()
                .any(|d| d.starts_with("Xref INSERT"))
        );
    }
}

#[test]
fn unloaded_xref_dxf_is_refused_without_replacing_the_target() {
    let mut doc = xref_sample();
    doc.cad_mut()
        .block_records
        .get_mut("SITE")
        .unwrap()
        .flags
        .is_xref_unloaded = true;
    let dir = tempfile::tempdir().unwrap();
    let dwg = dir.path().join("host.dwg");
    doc.save(&dwg).unwrap();
    let back = Document::open(&dwg).unwrap();
    assert!(
        back.cad()
            .block_records
            .get("SITE")
            .unwrap()
            .flags
            .is_xref_unloaded
    );
    let dxf = dir.path().join("host.dxf");
    fs::write(&dxf, b"previous file").unwrap();
    let error = doc.save_accepting(&dxf, |_| true).unwrap_err();
    let DocumentError::NotVerified {
        differences, kept, ..
    } = error
    else {
        panic!("expected read-back rejection")
    };
    assert!(
        differences
            .iter()
            .any(|d| d.starts_with("Xref SITE metadata changed"))
    );
    assert!(kept.unwrap().exists());
    assert_eq!(fs::read(dxf).unwrap(), b"previous file");
}
