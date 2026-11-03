//! Focused evidence for milestone 03.4; no accepted losses in product saves.

use std::path::Path;

use super::default_losses;
use crate::{Document, FileFormat};

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn existing_legacy_multileader_styles_survive_verified_saving() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus/libredwg");
    let scratch = tempfile::tempdir().unwrap();
    for version in ["r14", "2000"] {
        let doc = Document::open(root.join(version).join("Constraints.dwg")).unwrap();
        let target = scratch.path().join(format!("{version}.dwg"));
        std::fs::write(&target, b"original").unwrap();
        doc.save(&target).unwrap();
        let back = Document::open(&target).unwrap();
        assert!(back.recovery().is_none());
        assert!(doc.validate_roundtrip(&back).is_empty());
        assert!(doc.save_losses(&back, FileFormat::Dwg).is_empty());
        let mut count = 0;
        for (handle, object) in &doc.cad().objects {
            if let acadrust::objects::ObjectType::MultiLeaderStyle(style) = object {
                assert_eq!(back.cad().objects.get(handle), Some(object));
                assert_eq!(style.description, "Standard");
                count += 1;
            }
        }
        assert_eq!(count, 2);
        // Retaining the styles is not a broader default-loss allowance.
        let mut missing = Document::open(&target).unwrap();
        missing.cad_mut().objects.retain(|_, object| {
            !matches!(object, acadrust::objects::ObjectType::MultiLeaderStyle(_))
        });
        let losses = doc.save_losses(&missing, FileFormat::Dwg);
        let loss = losses
            .iter()
            .find(|loss| loss.kind() == "object MULTILEADERSTYLE")
            .unwrap();
        assert_eq!((loss.before(), loss.after()), (2, 0));
        let allowance = default_losses(FileFormat::Dwg, &doc.version_code());
        assert_eq!(allowance.get("object MULTILEADERSTYLE"), Some(&1));
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

#[test]
#[ignore = "needs a native ProgeCAD unloaded DXF in ARQ_NATIVE_XREF_DXF"]
fn native_progecad_unloaded_dxf_imports_the_explicit_marker() {
    let path = std::env::var_os("ARQ_NATIVE_XREF_DXF")
        .expect("set ARQ_NATIVE_XREF_DXF to the independently exported DXF");
    let doc = Document::open(Path::new(&path)).unwrap();
    assert!(
        doc.recovery().is_none(),
        "the native file must read cleanly"
    );
    let block = doc.cad().block_records.get("child").unwrap();
    assert!(block.flags.is_xref);
    assert!(
        block.flags.is_xref_unloaded,
        "BLOCK 71=1 must survive the product reader"
    );
    assert_eq!(block.xref_path, "child.dwg");
    let scratch = tempfile::tempdir().unwrap();
    for extension in ["dwg", "dxf"] {
        let output = scratch.path().join(format!("native-unloaded.{extension}"));
        doc.save(&output).unwrap();
        let back = Document::open(&output).unwrap();
        assert!(doc.validate_roundtrip(&back).is_empty());
        assert!(
            back.cad()
                .block_records
                .get("child")
                .unwrap()
                .flags
                .is_xref_unloaded
        );
    }
}
