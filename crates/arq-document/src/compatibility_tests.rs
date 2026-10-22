//! Focused evidence for milestone 03.4; no accepted losses in product saves.

use std::path::Path;

use super::default_losses;
use crate::{Document, DocumentError, FileFormat};

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn legacy_multileaderstyle_refusals_exceed_the_default_allowance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus/libredwg");
    let scratch = tempfile::tempdir().unwrap();
    for version in ["r14", "2000"] {
        let doc = Document::open(root.join(version).join("Constraints.dwg")).unwrap();
        let raw = scratch.path().join("raw.dwg");
        std::fs::write(&raw, doc.to_dwg_bytes().unwrap()).unwrap();
        let back = Document::open(raw).unwrap();
        let loss = doc
            .losses_in(&back)
            .into_iter()
            .find(|l| l.kind() == "object MULTILEADERSTYLE")
            .unwrap();
        let allowance = default_losses(FileFormat::Dwg, &doc.version_code());
        println!("{version}: {loss}; default allowance {allowance:?}");
        let mut changed: Vec<bool> = doc
            .cad()
            .objects
            .values()
            .filter_map(|object| {
                if let acadrust::objects::ObjectType::MultiLeaderStyle(style) = object {
                    assert_eq!(style.description, "Standard");
                    Some(style.property_changed)
                } else {
                    None
                }
            })
            .collect();
        changed.sort();
        assert_eq!(
            changed,
            [false, true],
            "the two decoded styles are not identical"
        );
        assert_eq!((loss.before(), loss.after()), (2, 0));
        assert_eq!(allowance.get("object MULTILEADERSTYLE"), Some(&1));
        let target = scratch.path().join(format!("{version}.dwg"));
        std::fs::write(&target, b"original").unwrap();
        assert!(matches!(
            doc.save(&target),
            Err(DocumentError::Lossy { .. })
        ));
        assert_eq!(std::fs::read(target).unwrap(), b"original");
    }
}

#[test]
fn xref_fixture_metadata_roundtrips() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xrefs");
    let scratch = tempfile::tempdir().unwrap();
    assert!(root.join("child.dxf").is_file());
    assert!(!root.join("absent.dwg").exists());
    for (file, name, path, overlay) in [
        ("attachment.dxf", "SITE", "child.dxf", false),
        ("overlay.dxf", "SITE", "child.dxf", true),
        ("missing.dxf", "SITE", "absent.dwg", false),
        ("nested.dxf", "PARENT", "attachment.dxf", false),
    ] {
        let doc = Document::open(root.join(file)).unwrap();
        assert!(doc.recovery().is_none());
        let block = doc.cad().block_records.get(name).unwrap();
        assert_eq!(block.xref_path, path);
        assert!(block.flags.is_xref);
        assert_eq!(block.flags.is_xref_overlay, overlay);
        assert!(!doc.block_names().contains(&name.to_owned()));
        check_insert(&doc, name);
        for extension in ["dwg", "dxf"] {
            let target = scratch.path().join(format!("{file}.{extension}"));
            doc.save(&target).unwrap();
            let back = Document::open(target).unwrap();
            let kept = back.cad().block_records.get(name).unwrap();
            assert_eq!(kept.xref_path, path, "{file} -> {extension}");
            assert_eq!(kept.flags.is_xref, block.flags.is_xref);
            assert_eq!(kept.flags.is_xref_overlay, overlay);
            check_insert(&back, name);
        }
    }
}

fn check_insert(doc: &Document, name: &str) {
    let entities: Vec<_> = doc.cad().entities().collect();
    assert_eq!(
        entities.len(),
        1,
        "external geometry was not merged into the host"
    );
    let acadrust::EntityType::Insert(insert) = entities[0] else {
        panic!("expected one external INSERT");
    };
    assert_eq!(insert.block_name, name);
    assert_eq!(
        insert.insert_point,
        acadrust::types::Vector3::new(10.0, 20.0, 0.0)
    );
    assert_eq!(
        (insert.x_scale(), insert.y_scale(), insert.z_scale()),
        (2.0, 2.0, 1.0)
    );
    assert!((insert.rotation - 30.0_f64.to_radians()).abs() < 1e-12);
}
