//! Tests of copying blocks between drawings (a submodule of `block_copy`).

use std::collections::BTreeMap;

use acadrust::entities::{AttributeDefinition, MLine};
use acadrust::tables::{LineType, TextStyle};
use acadrust::types::Vector3;
use acadrust::{EntityType, Handle};
use arq_geometry::{Line, Point2};

use crate::block_copy::ClashChoice;
use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::{Color, EntityId, PlanGeometry};
use crate::error::DocumentError;

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

/// What block `name` draws: its lines, rounded, sorted.
fn lines(doc: &Document, name: &str) -> Vec<((i64, i64), (i64, i64))> {
    let r = |q: Point2| ((q.x() * 1e6).round() as i64, (q.y() * 1e6).round() as i64);
    let mut found: Vec<_> = doc
        .block_drawing(name)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| match item.shape {
            Shape::Line(l) => Some((r(l.start()), r(l.end()))),
            _ => None,
        })
        .collect();
    found.sort_unstable();
    found
}

fn m(x: i64, y: i64) -> (i64, i64) {
    (x * 1_000_000, y * 1_000_000)
}

fn none() -> BTreeMap<String, ClashChoice> {
    BTreeMap::new()
}

fn choose(name: &str, choice: ClashChoice) -> BTreeMap<String, ClashChoice> {
    BTreeMap::from([(name.to_owned(), choice)])
}

/// A template: block SILLA, two lines on layer A-MUEBLE (green, DASHED),
/// based at (0,0); block MESA, based at (0,0), a line on layer 0, a
/// reference to SILLA at (5,0) and a text in style ROTULO.
fn template() -> Document {
    let mut doc = Document::new();
    doc.cad_mut().line_types.add(dashed()).unwrap();
    let mut style = TextStyle::new("ROTULO");
    style.handle = doc.cad_mut().allocate_handle();
    doc.cad_mut().text_styles.add(style).unwrap();
    let furniture = doc.create_layer("A-MUEBLE").unwrap();
    doc.set_layer_color(furniture, Color::Index(3)).unwrap();
    if let Some(layer) = doc.cad_mut().layers.get_mut("A-MUEBLE") {
        layer.line_type = "DASHED".to_owned();
    }
    doc.sync_layers();
    doc.set_current_layer(furniture).unwrap();
    let a = line(&mut doc, (0.0, 0.0), (1.0, 0.0));
    let b = line(&mut doc, (0.0, 0.0), (0.0, 1.0));
    let chair = doc.create_block("SILLA", p(0.0, 0.0), &[a, b]).unwrap();
    doc.delete_entity(chair).unwrap();
    let zero = doc.layer_by_name("0").unwrap().id();
    doc.set_current_layer(zero).unwrap();
    let top = line(&mut doc, (0.0, 0.0), (4.0, 0.0));
    let seat = doc.insert_block("SILLA", p(5.0, 0.0), 1.0, 0.0).unwrap();
    let label = doc.add_text("MESA", p(0.0, 2.0), 0.25, 0.0).unwrap();
    if let Some(EntityType::Text(t)) = doc.cad_mut().get_entity_mut(Handle::new(label)) {
        t.style = "ROTULO".to_owned();
    }
    let table = doc
        .create_block("MESA", p(0.0, 0.0), &[top, seat, label])
        .unwrap();
    doc.delete_entity(table).unwrap();
    doc
}

fn dashed() -> LineType {
    let mut dashed = LineType::dashed();
    dashed.name = "DASHED".to_owned();
    dashed
}

#[test]
fn a_block_comes_with_the_blocks_layers_linetypes_and_styles_it_uses() {
    let source = template();
    let mut doc = Document::new();
    assert_eq!(
        doc.block_clashes(&source, "mesa").unwrap(),
        Vec::<String>::new()
    );
    let name = doc.copy_block(&source, "mesa", &none()).unwrap();
    assert_eq!(name, "MESA", "spelled as the template spells it");
    assert_eq!(doc.block_names(), ["MESA", "SILLA"]);
    assert_eq!(lines(&doc, "MESA"), lines(&source, "MESA"));
    assert_eq!(
        lines(&doc, "SILLA"),
        [(m(0, 0), m(0, 1)), (m(0, 0), m(1, 0))]
    );
    let layer = doc.layer_by_name("A-MUEBLE").expect("the layer came along");
    assert_eq!(layer.color(), Color::Index(3));
    assert_eq!(layer.linetype(), "DASHED");
    assert!(doc.cad().line_types.get("DASHED").is_some());
    assert!(doc.cad().text_styles.get("ROTULO").is_some());
    assert_eq!(
        doc.entities().len(),
        0,
        "nothing is placed: that is INSERT's"
    );
    // A reference to the copy draws it.
    let placed = doc.insert_block("MESA", p(0.0, 0.0), 1.0, 0.0).unwrap();
    assert!(!doc.drawing(placed).unwrap().is_empty());
}

#[test]
fn a_layer_the_drawing_has_is_used_as_it_is() {
    let source = template();
    let mut doc = Document::new();
    let mine = doc.create_layer("a-mueble").unwrap();
    doc.set_layer_color(mine, Color::Index(1)).unwrap();
    doc.copy_block(&source, "SILLA", &none()).unwrap();
    let layer = doc.layer_by_name("A-MUEBLE").unwrap();
    assert_eq!((layer.id(), layer.color()), (mine, Color::Index(1)));
    assert_eq!(doc.layers().len(), 2, "0 and the drawing's own");
}

#[test]
fn a_block_with_the_same_contents_is_used_without_asking() {
    let source = template();
    let mut doc = Document::new();
    doc.copy_block(&source, "MESA", &none()).unwrap();
    assert_eq!(
        doc.block_clashes(&source, "MESA").unwrap(),
        Vec::<String>::new()
    );
    let blocks = doc.block_names();
    assert_eq!(doc.copy_block(&source, "MESA", &none()).unwrap(), "MESA");
    assert_eq!(doc.block_names(), blocks, "nothing new");
}

/// A drawing whose SILLA is a single line (0,0)-(2,0).
fn other_chair() -> Document {
    let mut doc = Document::new();
    let a = line(&mut doc, (0.0, 0.0), (2.0, 0.0));
    let r = doc.create_block("silla", p(0.0, 0.0), &[a]).unwrap();
    doc.delete_entity(r).unwrap();
    doc
}

#[test]
fn a_clash_is_never_resolved_silently() {
    let source = template();
    let mut doc = other_chair();
    assert_eq!(doc.block_clashes(&source, "MESA").unwrap(), ["SILLA"]);
    assert_eq!(doc.block_clashes(&source, "SILLA").unwrap(), ["SILLA"]);
    let before = (doc.block_names(), doc.layers());
    let refused = doc.copy_block(&source, "MESA", &none());
    assert!(
        matches!(&refused, Err(DocumentError::InvalidBlockName(m)) if m.contains("SILLA")),
        "{refused:?}"
    );
    assert_eq!((doc.block_names(), doc.layers()), before, "unchanged");
}

#[test]
fn use_the_drawings_keeps_its_definition() {
    let source = template();
    let mut doc = other_chair();
    let name = doc
        .copy_block(&source, "MESA", &choose("silla", ClashChoice::UseDrawing))
        .unwrap();
    assert_eq!(name, "MESA");
    assert_eq!(
        lines(&doc, "silla"),
        [(m(0, 0), m(2, 0))],
        "the drawing's own"
    );
    assert!(
        doc.layer_by_name("A-MUEBLE").is_none(),
        "the template's SILLA did not come, nor its layer"
    );
}

#[test]
fn redefine_changes_the_definition_and_every_reference() {
    let source = template();
    let mut doc = Document::new();
    let a = line(&mut doc, (2.0, 0.0), (4.0, 0.0));
    let r = doc.create_block("silla", p(2.0, 0.0), &[a]).unwrap();
    doc.delete_entity(r).unwrap();
    let placed = doc.insert_block("silla", p(10.0, 10.0), 1.0, 0.0).unwrap();
    doc.begin_change().unwrap();
    doc.copy_block(&source, "SILLA", &choose("SILLA", ClashChoice::Redefine))
        .unwrap();
    let change = doc.commit_change().unwrap();
    assert_eq!(
        lines(&doc, "silla"),
        [(m(0, 0), m(0, 1)), (m(0, 0), m(1, 0))],
        "the template's contents, from the template's base point"
    );
    assert_eq!(doc.block_names(), ["silla"], "redefined, keeping its name");
    assert!(change.summary().modified().contains(&placed), "redrawn");
    assert_eq!(change.summary().blocks_modified(), ["silla"]);
}

#[test]
fn copy_with_another_name_renames_the_references_to_it() {
    let source = template();
    let mut doc = other_chair();
    let name = doc
        .copy_block(
            &source,
            "MESA",
            &choose("SILLA", ClashChoice::Rename("SILLA_2".to_owned())),
        )
        .unwrap();
    assert_eq!(name, "MESA");
    assert_eq!(doc.block_names(), ["MESA", "silla", "SILLA_2"]);
    assert_eq!(lines(&doc, "SILLA_2"), lines(&source, "SILLA"));
    assert_eq!(lines(&doc, "silla"), [(m(0, 0), m(2, 0))], "untouched");
    assert_eq!(lines(&doc, "MESA"), lines(&source, "MESA"), "draws SILLA_2");
    // The top block renamed: the name to insert is the new one.
    let mut again = other_chair();
    let renamed = again
        .copy_block(
            &source,
            "SILLA",
            &choose("SILLA", ClashChoice::Rename("Silla nueva".to_owned())),
        )
        .unwrap();
    assert_eq!(renamed, "Silla nueva");
}

#[test]
fn two_blocks_cannot_take_the_same_new_name() {
    let source = template();
    let mut doc = other_chair();
    let a = line(&mut doc, (0.0, 0.0), (3.0, 0.0));
    let r = doc.create_block("MESA", p(0.0, 0.0), &[a]).unwrap();
    doc.delete_entity(r).unwrap();
    let both = BTreeMap::from([
        ("MESA".to_owned(), ClashChoice::Rename("NUEVO".to_owned())),
        ("SILLA".to_owned(), ClashChoice::Rename("nuevo".to_owned())),
    ]);
    let refused = doc.copy_block(&source, "MESA", &both);
    assert!(
        matches!(&refused, Err(DocumentError::InvalidBlockName(m)) if m.contains("nuevo")),
        "{refused:?}"
    );
    assert_eq!(doc.block_names(), ["MESA", "silla"]);
}

#[test]
fn a_new_name_must_be_free_and_valid() {
    let source = template();
    for new in ["silla", "bad*name", ""] {
        let mut doc = other_chair();
        let before = doc.block_names();
        let refused = doc.copy_block(
            &source,
            "SILLA",
            &choose("SILLA", ClashChoice::Rename(new.to_owned())),
        );
        assert!(
            matches!(refused, Err(DocumentError::InvalidBlockName(_))),
            "{new:?}"
        );
        assert_eq!(doc.block_names(), before);
    }
}

#[test]
fn contents_a_copy_cannot_carry_are_refused_whole() {
    let mut source = template();
    let owner = source.cad().block_records.get("SILLA").unwrap().handle;
    let mut mline = EntityType::MLine(MLine::new());
    mline.common_mut().owner_handle = owner;
    source.cad_mut().add_entity(mline).unwrap();
    let mut doc = Document::new();
    let refused = doc.copy_block(&source, "MESA", &none());
    assert!(
        matches!(&refused, Err(DocumentError::UnsupportedOperation(m)) if m.contains("SILLA") && m.contains("MLINE")),
        "{refused:?}"
    );
    assert!(doc.block_names().is_empty() && doc.layer_by_name("A-MUEBLE").is_none());
}

#[test]
fn a_missing_block_is_an_error() {
    let source = template();
    let mut doc = Document::new();
    assert!(matches!(
        doc.copy_block(&source, "SOFA", &none()),
        Err(DocumentError::InvalidBlockName(_))
    ));
    assert!(doc.block_clashes(&source, "*Model_Space").is_err());
}

#[test]
fn undo_takes_the_copy_and_what_came_with_it_away() {
    let source = template();
    let mut doc = Document::new();
    doc.begin_change().unwrap();
    doc.copy_block(&source, "MESA", &none()).unwrap();
    let mut change = doc.commit_change().unwrap();
    let mut created = change.summary().blocks_created().to_vec();
    created.sort();
    assert_eq!(created, ["MESA", "SILLA"]);
    assert_eq!(change.summary().layers_created().len(), 1);
    doc.undo_change(&mut change).unwrap();
    assert!(doc.block_names().is_empty());
    assert!(doc.layer_by_name("A-MUEBLE").is_none());
    assert!(doc.cad().text_styles.get("ROTULO").is_none());
    doc.redo_change(&mut change).unwrap();
    assert_eq!(doc.block_names(), ["MESA", "SILLA"]);
}

#[test]
fn attributes_and_their_definitions_come_along() {
    let mut source = template();
    let owner = source.cad().block_records.get("SILLA").unwrap().handle;
    let mut tag = AttributeDefinition::new("NUM".into(), "Number".into(), "1".into());
    tag.common.owner_handle = owner;
    tag.common.layer = "0".to_owned();
    tag.insertion_point = Vector3::new(0.5, 0.5, 0.0);
    source
        .cad_mut()
        .add_entity(EntityType::AttributeDefinition(tag))
        .unwrap();
    let mut doc = Document::new();
    doc.copy_block(&source, "SILLA", &none()).unwrap();
    assert_eq!(doc.block_info("SILLA").unwrap().attributes, 1);
    let placed = doc.insert_block("SILLA", p(0.0, 0.0), 1.0, 0.0).unwrap();
    let Some(EntityType::Insert(insert)) = doc.cad().get_entity(Handle::new(placed)) else {
        panic!("a reference");
    };
    assert_eq!(insert.attributes.len(), 1);
    assert_eq!(insert.attributes[0].get_value(), "1");
}

#[test]
fn a_copied_block_is_saved_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let source = template();
    let mut doc = other_chair();
    doc.copy_block(
        &source,
        "MESA",
        &choose("SILLA", ClashChoice::Rename("SILLA_2".to_owned())),
    )
    .unwrap();
    doc.insert_block("MESA", p(0.0, 0.0), 1.0, 0.0).unwrap();
    for name in ["copia.dwg", "copia.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        assert_eq!(back.block_names(), doc.block_names(), "{name}");
        assert_eq!(lines(&back, "MESA"), lines(&doc, "MESA"), "{name}");
        assert_eq!(
            back.layer_by_name("A-MUEBLE").unwrap().linetype(),
            "DASHED",
            "{name}"
        );
    }
}

#[test]
fn every_block_of_the_sample_template_copies_and_saves() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/templates/arqstudiocad-sample.dwt");
    let source = Document::open(path).unwrap();
    let names = source.block_names();
    assert_eq!(names.len(), 10);
    let mut doc = Document::new();
    for name in &names {
        doc.copy_block(&source, name, &none()).unwrap();
        doc.insert_block(name, p(0.0, 0.0), 1.0, 0.0).unwrap();
    }
    assert_eq!(doc.block_names(), names);
    let saved = dir.path().join("plano.dwg");
    doc.save(&saved).unwrap();
    let back = Document::open(&saved).unwrap();
    for name in &names {
        assert_eq!(lines(&back, name), lines(&source, name), "{name}");
    }
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn a_dynamic_block_keeps_its_parameters() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/corpus/acadrust/fixtures/dynamic-blocks/lookup.dwg");
    let source = Document::open(path).unwrap();
    let name = source
        .block_names()
        .into_iter()
        .find(|n| source.is_dynamic_block(n))
        .expect("the sample's dynamic block");
    let parameters = source.block_info(&name).unwrap().parameters;
    let mut doc = Document::new();
    doc.copy_block(&source, &name, &none()).unwrap();
    assert!(doc.is_dynamic_block(&name));
    assert_eq!(doc.block_info(&name).unwrap().parameters, parameters);
    let saved = dir.path().join("plano.dwg");
    doc.save(&saved).unwrap();
    let back = Document::open(&saved).unwrap();
    assert!(back.is_dynamic_block(&name));
    assert_eq!(back.block_info(&name).unwrap().parameters, parameters);

    // Clashing with another block of that name: only the drawing's serves.
    let mut other = Document::new();
    let a = line(&mut other, (0.0, 0.0), (2.0, 0.0));
    let r = other.create_block(&name, p(0.0, 0.0), &[a]).unwrap();
    other.delete_entity(r).unwrap();
    assert_eq!(
        other.block_clashes(&source, &name).unwrap(),
        std::slice::from_ref(&name)
    );
    for choice in [
        ClashChoice::Redefine,
        ClashChoice::Rename("OTRO".to_owned()),
    ] {
        assert!(matches!(
            other.copy_block(&source, &name, &choose(&name, choice)),
            Err(DocumentError::UnsupportedOperation(_))
        ));
    }
    other
        .copy_block(&source, &name, &choose(&name, ClashChoice::UseDrawing))
        .unwrap();
    assert!(!other.is_dynamic_block(&name));
}

/// Adds material `name` (described, so not an implicit default) to `doc`'s
/// material dictionary and puts layer A-MUEBLE on it.
fn layer_material(doc: &mut Document, name: &str) {
    use acadrust::objects::{Material, ObjectType};
    let dictionary = doc.cad().header.acad_material_dict_handle;
    let h = doc.cad_mut().allocate_handle();
    let material = Material {
        handle: h,
        owner: dictionary,
        name: name.to_owned(),
        description: "edited".to_owned(),
        ..Default::default()
    };
    doc.cad_mut()
        .objects
        .insert(h, ObjectType::Material(material));
    if let Some(ObjectType::Dictionary(d)) = doc.cad_mut().objects.get_mut(&dictionary) {
        d.add_entry(name, h);
    }
    doc.cad_mut().layers.get_mut("A-MUEBLE").unwrap().material = h;
}

#[test]
fn a_layer_takes_this_drawings_material_of_the_same_name() {
    let mut source = template();
    layer_material(&mut source, "Global");
    let mut doc = Document::new();
    doc.copy_block(&source, "SILLA", &none()).unwrap();
    assert_eq!(
        doc.cad().layers.get("A-MUEBLE").unwrap().material,
        Handle::NULL,
        "this drawing's implicit Global"
    );
    // A material this drawing does not have is not copied: refused.
    let mut source = template();
    layer_material(&mut source, "Madera");
    let mut doc = Document::new();
    let refused = doc.copy_block(&source, "MESA", &none());
    assert!(
        matches!(&refused, Err(DocumentError::UnsupportedOperation(m)) if m.contains("material")),
        "{refused:?}"
    );
    assert!(doc.block_names().is_empty(), "unchanged");
}

#[test]
fn a_nested_references_attributes_follow_their_copied_definitions() {
    let mut source = Document::new();
    let a = line(&mut source, (0.0, 0.0), (1.0, 0.0));
    let r = source.create_block("SILLA", p(0.0, 0.0), &[a]).unwrap();
    source.delete_entity(r).unwrap();
    let owner = source.cad().block_records.get("SILLA").unwrap().handle;
    let mut tag = AttributeDefinition::new("NUM".into(), "Number".into(), "1".into());
    tag.common.owner_handle = owner;
    tag.common.layer = "0".to_owned();
    source
        .cad_mut()
        .add_entity(EntityType::AttributeDefinition(tag))
        .unwrap();
    let seat = source.insert_block("SILLA", p(5.0, 0.0), 1.0, 0.0).unwrap();
    let definition = |d: &Document| {
        d.cad()
            .entities_in_block("SILLA")
            .find(|e| matches!(e, EntityType::AttributeDefinition(_)))
            .map(|e| e.common().handle)
            .unwrap()
    };
    let own = definition(&source);
    if let Some(EntityType::Insert(i)) = source.cad_mut().get_entity_mut(Handle::new(seat)) {
        i.attributes[0].attdef_handle = own;
    }
    let table = source.create_block("MESA", p(0.0, 0.0), &[seat]).unwrap();
    source.delete_entity(table).unwrap();

    let mut doc = Document::new();
    doc.copy_block(&source, "MESA", &none()).unwrap();
    let linked: Vec<Handle> = doc
        .cad()
        .entities_in_block("MESA")
        .filter_map(|e| match e {
            EntityType::Insert(i) => Some(i.attributes[0].attdef_handle),
            _ => None,
        })
        .collect();
    assert_eq!(linked, [definition(&doc)]);
    assert_ne!(definition(&doc), definition(&source));
}
