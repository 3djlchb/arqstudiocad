use std::fs;
use std::io::Cursor;

use crate::{Document, DocumentError};
use acadrust::objects::{CellStyleMap, DataObject, DataObjectData, ObjectType};
use acadrust::types::Handle;

const KEY: &str = "ACAD_ROUNDTRIP_2008_TABLESTYLE_CELLSTYLEMAP";

fn source() -> (Document, Handle, Handle, Handle) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.dwg");
    Document::new().save(&path).unwrap();
    let mut doc = Document::open(&path).unwrap();
    let owner = doc
        .cad()
        .objects
        .iter()
        .find_map(|(h, o)| match o {
            ObjectType::TableStyle(s) if s.name == "Standard" => Some(*h),
            _ => None,
        })
        .unwrap();
    let cells = if let Some(ObjectType::TableStyle(style)) = doc.cad_mut().objects.get_mut(&owner) {
        style.horizontal_margin = 1.5;
        style.vertical_margin = 1.5;
        let base = &mut style.modern_style.as_mut().unwrap().cell_style;
        base.horizontal_margin = 1.5;
        base.vertical_margin = 1.5;
        // Exercise a legacy/DXF style: its base is synthesized from the
        // explicit global margins when converted to modern DWG.
        style.modern_style = None;
        style
            .modern_overrides
            .iter()
            .map(|(_, c)| c.clone())
            .collect()
    } else {
        panic!()
    };
    let dictionary = doc.cad_mut().ensure_extension_dictionary(owner);
    let handle = doc.cad_mut().allocate_handle();
    let mut object = DataObject::new(DataObjectData::CellStyleMap(CellStyleMap { cells }));
    object.handle = handle;
    object.owner = dictionary;
    object.reactors = vec![dictionary];
    doc.cad_mut()
        .objects
        .insert(handle, ObjectType::DataObject(object));
    if let Some(ObjectType::Dictionary(entries)) = doc.cad_mut().objects.get_mut(&dictionary) {
        entries.add_entry(KEY, handle);
    }
    (doc, owner, dictionary, handle)
}

fn canonical(doc: &mut Document) -> Document {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("converted.dwg");
    // Deliberately bypass only the product gate here, to inspect the writer's
    // candidate and test that the gate rejects subsequent corruption.
    fs::write(&path, doc.to_dwg_bytes().unwrap()).unwrap();
    Document::open(path).unwrap()
}

#[test]
fn a_complete_map_conversion_passes_strict_saving_in_both_formats() {
    let (mut doc, owner, _, map) = source();
    let back = canonical(&mut doc);
    assert!(!back.cad().objects.contains_key(&map));
    assert!(doc.losses_in(&back).is_empty());
    assert!(doc.validate_roundtrip(&back).is_empty());
    let Some(ObjectType::TableStyle(style)) = back.cad().objects.get(&owner) else {
        panic!()
    };
    assert_eq!((style.horizontal_margin, style.vertical_margin), (1.5, 1.5));
    let dir = tempfile::tempdir().unwrap();
    for name in ["safe.dwg", "safe.dxf"] {
        doc.save(dir.path().join(name)).unwrap();
    }
}

#[test]
fn changed_cells_and_styles_are_not_a_conversion_allowance() {
    let (mut doc, owner, _, _) = source();
    let back = canonical(&mut doc);
    for field in 0..11 {
        let mut changed = back.snapshot();
        let Some(ObjectType::TableStyle(style)) = changed.cad_mut().objects.get_mut(&owner) else {
            panic!()
        };
        let cell = &mut style.modern_overrides[0].1.cell_style;
        match field {
            0 => cell.vertical_margin += 1.0,
            1 => cell.horizontal_margin += 1.0,
            2 => cell.bottom_margin += 1.0,
            3 => cell.right_margin += 1.0,
            4 => cell.horizontal_spacing += 1.0,
            5 => cell.vertical_spacing += 1.0,
            6 => cell.content_format.text_style = Handle::new(0xdead),
            7 => cell.borders[0].border.is_invisible = !cell.borders[0].border.is_invisible,
            8 => style.horizontal_margin = 0.06,
            9 => style.modern_overrides[0].1.name = "Changed".to_owned(),
            _ => {
                style
                    .modern_style
                    .as_mut()
                    .unwrap()
                    .cell_style
                    .content_format
                    .text_height = 999.0
            }
        }
        assert!(
            doc.losses_in(&changed)
                .iter()
                .any(|l| l.kind() == "object DATAOBJECT"),
            "field {field}"
        );
        assert!(
            !doc.validate_roundtrip(&changed).is_empty(),
            "field {field}"
        );
    }
}

#[test]
fn broken_ownership_is_not_a_conversion_allowance() {
    let (mut doc, owner, dictionary, _) = source();
    let mut back = canonical(&mut doc);
    if let Some(ObjectType::Dictionary(entries)) = back.cad_mut().objects.get_mut(&dictionary) {
        entries.owner = Handle::new(0xbeef);
    }
    assert!(
        doc.losses_in(&back)
            .iter()
            .any(|l| l.kind() == "object DATAOBJECT")
    );
    assert!(!doc.validate_roundtrip(&back).is_empty());
    assert!(back.cad().objects.contains_key(&owner));
}

#[test]
fn unrelated_data_objects_still_count_as_losses() {
    let (mut doc, _, _, _) = source();
    let mut dummy = DataObject::new(DataObjectData::Dummy);
    let handle = doc.cad_mut().allocate_handle();
    dummy.handle = handle;
    doc.cad_mut()
        .objects
        .insert(handle, ObjectType::DataObject(dummy));
    let mut back = canonical(&mut doc);
    back.cad_mut().objects.remove(&handle);
    let losses = doc.losses_in(&back);
    assert!(
        losses
            .iter()
            .any(|l| l.kind() == "object DATAOBJECT" && l.before() - l.after() == 1)
    );
}

#[test]
fn accepting_count_losses_cannot_save_corrupted_cell_styles() {
    let (doc, _, _, _) = source();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keep.dwg");
    fs::write(&path, b"previous drawing").unwrap();
    crate::verify::fault::set(|bytes| {
        let mut cad = acadrust::DwgReader::from_stream(Cursor::new(bytes.clone()))
            .read()
            .unwrap();
        let style = cad
            .objects
            .values_mut()
            .find_map(|object| match object {
                ObjectType::TableStyle(style) if style.name == "Standard" => Some(style),
                _ => None,
            })
            .unwrap();
        style.modern_overrides[0].1.cell_style.margin_override_flags = 1;
        style.modern_overrides[0].1.cell_style.horizontal_margin = 999.0;
        *bytes = acadrust::DwgWriter::write_to_vec(&cad).unwrap();
    });
    assert!(matches!(
        doc.save_accepting(&path, |_| true),
        Err(DocumentError::NotVerified { .. })
    ));
    assert_eq!(fs::read(path).unwrap(), b"previous drawing");
}

#[test]
fn legacy_style_loss_is_refused_even_when_count_losses_are_accepted() {
    let (mut doc, owner, _, map) = source();
    doc.cad_mut().version = acadrust::types::DxfVersion::AC1015;
    let back = canonical(&mut doc);
    assert!(!back.cad().objects.contains_key(&owner));
    assert_eq!(back.cad().objects.get(&map), doc.cad().objects.get(&map));
    assert!(
        doc.losses_in(&back)
            .iter()
            .any(|loss| loss.kind() == "object TABLESTYLE")
    );
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("original.dwg");
    fs::write(&destination, b"previous drawing").unwrap();
    assert!(matches!(
        doc.save_accepting(&destination, |_| true),
        Err(DocumentError::NotVerified { .. })
    ));
    assert_eq!(fs::read(destination).unwrap(), b"previous drawing");
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn existing_r2000_style_graph_survives_verified_saving() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus/libredwg");
    let scratch = tempfile::tempdir().unwrap();
    for relative in ["example_2000.dwg", "2000/Constraints.dwg"] {
        let source = Document::open(root.join(relative)).unwrap();
        let destination = scratch.path().join(relative.replace('/', "-"));
        std::fs::write(&destination, b"previous drawing").unwrap();
        // The corpus already permits other declared inventory losses. It must
        // still prove the complete style graph independently before saving.
        source.save_accepting(&destination, |_| true).unwrap();
        let written = Document::open(&destination).unwrap();
        assert!(written.recovery().is_none());
        assert!(source.validate_roundtrip(&written).is_empty());
        assert!(
            !source
                .losses_in(&written)
                .iter()
                .any(|loss| loss.kind() == "object TABLESTYLE")
        );
        assert!(
            written
                .cad()
                .objects
                .values()
                .any(|object| matches!(object, ObjectType::TableStyle(_)))
        );
    }
}
