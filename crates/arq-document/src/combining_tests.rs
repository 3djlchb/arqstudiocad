//! Tests of UNION, SUBTRACT and INTERSECT in the document (a submodule of
//! `combining`): the result in the first solid, which keeps its handle and
//! properties, the others erased, and nothing changed when a solid is
//! curved, too few are chosen or nothing would be left.

use acadrust::types::Color as CodecColor;
use arq_geometry::Point3;

use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

/// A box's two opposite corners.
type Corners = ((f64, f64, f64), (f64, f64, f64));

/// A drawing with boxes between pairs of corners, the first on layer
/// "MUROS" in red, and their ids.
fn boxes(corners: &[Corners]) -> (Document, Vec<EntityId>) {
    let mut doc = Document::new();
    let ids: Vec<EntityId> = corners
        .iter()
        .map(|&(a, b)| doc.add_box(p(a.0, a.1, a.2), p(b.0, b.1, b.2)).unwrap())
        .collect();
    let layer = doc.create_layer("MUROS").unwrap();
    doc.set_entity_layer(ids[0], layer).unwrap();
    let e = doc.cad_mut().get_entity_mut(handle(ids[0])).unwrap();
    e.common_mut().color = CodecColor::from_index(1);
    (doc, ids)
}

fn volume(doc: &Document, id: EntityId) -> f64 {
    doc.solid(id)
        .unwrap()
        .unwrap()
        .mass_properties()
        .unwrap()
        .volume
}

fn solids(doc: &Document) -> Vec<EntityId> {
    doc.entities()
        .into_iter()
        .filter(|e| e.kind() == "3DSOLID")
        .map(|e| e.id())
        .collect()
}

#[track_caller]
fn about(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} != {b}");
}

#[test]
fn union_keeps_the_first_solid_and_erases_the_others() {
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        ((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)),
        ((2.5, 2.5, 2.5), (4.0, 4.0, 4.0)),
    ]);
    let out = doc.union_solids(&ids).unwrap();
    assert_eq!(out.outcome, Combination::Made(ids[0]));
    assert_eq!(out.not_solids, []);
    assert_eq!(solids(&doc), [ids[0]]);
    about(
        volume(&doc, ids[0]),
        8.0 + 8.0 - 1.0 + 1.5f64.powi(3) - 0.5f64.powi(3),
    );
    let kept = doc.entity(ids[0]).unwrap();
    assert_eq!(kept.layer(), "MUROS");
    let e = doc.cad().get_entity(handle(ids[0])).unwrap();
    assert_eq!(e.as_entity().color(), CodecColor::from_index(1));
}

#[test]
fn solids_apart_unite_as_pieces_of_one() {
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
        ((5.0, 0.0, 0.0), (6.0, 2.0, 1.0)),
    ]);
    doc.union_solids(&ids).unwrap();
    let body = doc.solid(ids[0]).unwrap().unwrap();
    assert_eq!(body.lumps().len(), 2);
    about(body.mass_properties().unwrap().volume, 3.0);
}

#[test]
fn subtract_takes_every_second_solid_from_the_first_ones() {
    // A wall of two boxes end to end, less two openings.
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (3.0, 0.2, 2.5)),
        ((3.0, 0.0, 0.0), (6.0, 0.2, 2.5)),
        ((1.0, 0.0, 0.0), (2.0, 0.2, 2.0)),
        ((4.0, 0.0, 1.0), (5.0, 0.2, 2.0)),
    ]);
    let out = doc.subtract_solids(&ids[..2], &ids[2..]).unwrap();
    assert_eq!(out.outcome, Combination::Made(ids[0]));
    assert_eq!(solids(&doc), [ids[0]]);
    about(
        volume(&doc, ids[0]),
        6.0 * 0.2 * 2.5 - 2.0 * 0.2 - 1.0 * 0.2,
    );
}

#[test]
fn a_solid_in_both_sets_counts_once_to_subtract_from() {
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        ((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)),
    ]);
    let out = doc.subtract_solids(&ids[..1], &ids).unwrap();
    assert_eq!(out.outcome, Combination::Made(ids[0]));
    about(volume(&doc, ids[0]), 7.0);
}

#[test]
fn intersect_keeps_what_all_share() {
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        ((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)),
        ((1.5, 0.0, 0.0), (4.0, 4.0, 4.0)),
    ]);
    let out = doc.intersect_solids(&ids).unwrap();
    assert_eq!(out.outcome, Combination::Made(ids[0]));
    assert_eq!(solids(&doc), [ids[0]]);
    about(volume(&doc, ids[0]), 0.5);
}

#[test]
fn nothing_left_changes_nothing() {
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
        ((5.0, 0.0, 0.0), (6.0, 1.0, 1.0)),
    ]);
    let before = doc.cad().get_entity(handle(ids[0])).cloned();
    assert_eq!(
        doc.intersect_solids(&ids).unwrap().outcome,
        Combination::Empty
    );
    let (mut cover, covered) = boxes(&[
        ((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
        ((-1.0, -1.0, -1.0), (2.0, 2.0, 2.0)),
    ]);
    assert_eq!(
        cover
            .subtract_solids(&covered[..1], &covered[1..])
            .unwrap()
            .outcome,
        Combination::Empty
    );
    assert_eq!(solids(&doc), ids);
    assert_eq!(solids(&cover), covered);
    assert_eq!(doc.cad().get_entity(handle(ids[0])).cloned(), before);
}

#[test]
fn curved_solids_change_nothing() {
    let (mut doc, ids) = boxes(&[((0.0, 0.0, 0.0), (2.0, 2.0, 2.0))]);
    let column = doc.add_cone(p(1.0, 1.0, -1.0), 0.5, 0.5, 4.0).unwrap();
    let out = doc.subtract_solids(&ids, &[column]).unwrap();
    assert_eq!(out.outcome, Combination::Curved(vec![column]));
    assert_eq!(solids(&doc), [ids[0], column]);
    about(volume(&doc, ids[0]), 8.0);
}

#[test]
fn too_few_solids_and_other_entities() {
    let (mut doc, ids) = boxes(&[((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))]);
    let point = doc
        .cad_mut()
        .add_entity(EntityType::Point(acadrust::Point::from_coords(
            5.0, 5.0, 0.0,
        )))
        .unwrap()
        .value();
    let out = doc.union_solids(&[ids[0], point]).unwrap();
    assert_eq!(out.outcome, Combination::TooFew);
    assert_eq!(out.not_solids, [point]);
    assert_eq!(
        doc.subtract_solids(&ids, &[point]).unwrap().outcome,
        Combination::TooFew
    );
    assert_eq!(
        doc.union_solids(&[ids[0], ids[0]]).unwrap().outcome,
        Combination::TooFew,
        "one solid chosen twice is one"
    );
    assert!(doc.entity(point).is_some());
}

#[test]
fn the_result_is_saved_in_every_version_and_reads_back() {
    use acadrust::types::DxfVersion;
    let dir = tempfile::tempdir().unwrap();
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1032,
    ] {
        // A slab with a flush opening and a box on top: holed faces, a
        // touching union, merged faces.
        let (mut doc, ids) = boxes(&[
            ((0.0, 0.0, 0.0), (6.0, 4.0, 0.2)),
            ((1.0, 1.0, 0.0), (2.0, 2.0, 0.2)),
            ((3.0, 1.0, 0.2), (5.0, 3.0, 1.2)),
        ]);
        doc.cad_mut().version = version;
        doc.subtract_solids(&ids[..1], &ids[1..2]).unwrap();
        doc.union_solids(&[ids[0], ids[2]]).unwrap();
        let exact = 6.0 * 4.0 * 0.2 - 0.2 + 2.0 * 2.0;
        about(volume(&doc, ids[0]), exact);
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("losa-{version:?}.{ext}"));
            doc.save(&path)
                .unwrap_or_else(|e| panic!("{version:?} {ext}: {e}"));
            let back = Document::open(&path).unwrap();
            let found = solids(&back);
            assert_eq!(found.len(), 1, "{version:?} {ext}");
            about(volume(&back, found[0]), exact);
        }
    }
}

#[test]
fn a_union_is_one_change() {
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        ((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)),
    ]);
    doc.begin_change().unwrap();
    doc.union_solids(&ids).unwrap();
    let change = doc.commit_change().unwrap();
    assert_eq!(change.summary().deleted(), [ids[1]]);
    assert_eq!(change.summary().modified(), [ids[0]]);
}

#[test]
fn an_unreadable_solid_changes_nothing() {
    let (mut doc, ids) = boxes(&[((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))]);
    let garbled = doc
        .cad_mut()
        .add_entity(EntityType::Solid3D(Solid3D::from_sat("not ACIS at all")))
        .unwrap()
        .value();
    let out = doc.union_solids(&[ids[0], garbled]).unwrap();
    assert_eq!(out.outcome, Combination::Unreadable(vec![garbled]));
    assert_eq!(solids(&doc), [ids[0], garbled]);
}

#[test]
fn the_kept_solid_drops_its_old_wires_and_history() {
    use acadrust::entities::Wire;
    use acadrust::objects::{DynamicBlockData, DynamicBlockObject, ObjectType, SolidHistory};
    let (mut doc, ids) = boxes(&[
        ((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        ((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)),
    ]);
    let h = doc.cad_mut().allocate_handle();
    let mut object = DynamicBlockObject::new("ACSH_HISTORY_CLASS", "AcDbShHistory");
    object.handle = h;
    object.data = DynamicBlockData::SolidHistory(SolidHistory {
        record_history: true,
        show_history: true,
        ..SolidHistory::default()
    });
    doc.cad_mut()
        .objects
        .insert(h, ObjectType::DynamicBlock(object));
    if let Some(EntityType::Solid3D(s)) = doc.cad_mut().get_entity_mut(handle(ids[0])) {
        s.history_handle = Some(h);
        let mut wire = Wire::new();
        wire.add_point(acadrust::types::Vector3::new(0.0, 0.0, 0.0));
        wire.add_point(acadrust::types::Vector3::new(2.0, 2.0, 2.0));
        s.wires.push(wire);
    }
    let history = |doc: &Document| match doc.cad().objects.get(&h) {
        Some(ObjectType::DynamicBlock(o)) => match &o.data {
            DynamicBlockData::SolidHistory(s) => (s.record_history, s.show_history),
            _ => panic!("not a history"),
        },
        _ => panic!("no history object"),
    };
    doc.begin_change().unwrap();
    doc.union_solids(&ids).unwrap();
    let mut change = doc.commit_change().unwrap();
    let Some(EntityType::Solid3D(s)) = doc.cad().get_entity(handle(ids[0])) else {
        panic!("the solid is kept")
    };
    assert!(s.wires.is_empty(), "the old body's wires would draw it");
    assert_eq!(history(&doc), (false, false), "History: None");
    doc.undo_change(&mut change).unwrap();
    assert_eq!(history(&doc), (true, true));
    about(volume(&doc, ids[0]), 8.0);
}
