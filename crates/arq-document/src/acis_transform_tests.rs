//! Tests of moving, turning, scaling and mirroring ACIS bodies as a whole
//! (a submodule of `acis`; ADR 0024 step 5a): their placement takes the
//! change, SAT or SAB as they were, and they read back where it puts them.

use std::f64::consts::{FRAC_PI_2, PI};

use acadrust::EntityType;
use acadrust::entities::acis::SabWriter;
use acadrust::entities::acis::primitives::build_box;
use acadrust::entities::solid3d::Solid3D;
use acadrust::types::DxfVersion;
use arq_geometry::{Line, Point2, Point3, Transform2, Vector2};

use crate::document::Document;
use crate::entity::EntityId;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn q(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[track_caller]
fn about(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} != {b}");
}

fn measured(doc: &Document, id: EntityId) -> arq_kernel::MassProperties {
    doc.solid(id).unwrap().unwrap().mass_properties().unwrap()
}

/// The transform record's numbers, if any: rows, translation and scale.
fn placement(doc: &Document, id: EntityId) -> Option<([[f64; 3]; 3], [f64; 3], f64)> {
    let e = doc.cad().get_entity(crate::translate::handle(id)).unwrap();
    let sat = super::parsed(e).unwrap();
    sat.records
        .iter()
        .find(|r| r.entity_type == "transform")
        .and_then(super::transform_numbers)
}

fn is_binary(doc: &Document, id: EntityId) -> bool {
    let e = doc.cad().get_entity(crate::translate::handle(id)).unwrap();
    super::acis_data(e).unwrap().is_binary
}

#[test]
fn a_box_moves_turns_scales_and_mirrors_as_a_whole() {
    // A 2 × 3 × 4 box from (1, 2, 0): volume 24, centroid (2, 3.5, 2).
    let mut doc = Document::new();
    let id = doc.add_box(p(1.0, 2.0, 0.0), p(3.0, 5.0, 4.0)).unwrap();
    assert!(doc.entity(id).unwrap().is_transformable());
    // Moved by (10, -1).
    doc.transform_entity(
        id,
        Transform2::translation(Vector2::new(10.0, -1.0).unwrap()),
    )
    .unwrap();
    let m = measured(&doc, id);
    about(m.volume, 24.0);
    about(m.centroid.x(), 12.0);
    about(m.centroid.y(), 2.5);
    about(m.centroid.z(), 2.0);
    let b = doc.entity(id).unwrap().bounds().unwrap();
    about(b.min().x(), 11.0);
    about(b.max().y(), 4.0);
    // A quarter turn about the origin: (x, y) to (-y, x).
    doc.transform_entity(id, Transform2::rotation(q(0.0, 0.0), FRAC_PI_2).unwrap())
        .unwrap();
    let m = measured(&doc, id);
    about(m.centroid.x(), -2.5);
    about(m.centroid.y(), 12.0);
    // Doubled about the origin: 8 times the volume, the height too.
    doc.transform_entity(id, Transform2::scaling(q(0.0, 0.0), 2.0).unwrap())
        .unwrap();
    let m = measured(&doc, id);
    about(m.volume, 192.0);
    about(m.centroid.z(), 4.0);
    about(m.bounds.max().z(), 8.0);
    // Stated as ACIS does: unit rows, the size in the scale.
    let (rows, _, scale) = placement(&doc, id).unwrap();
    about(scale, 2.0);
    for row in rows {
        about(row.iter().map(|x| x * x).sum::<f64>(), 1.0);
    }
    // Mirrored in the Y axis: the volume stays positive, x changes sign.
    let axis = Line::new(q(0.0, 0.0), q(0.0, 1.0)).unwrap();
    doc.transform_entity(id, Transform2::mirror(axis)).unwrap();
    let m = measured(&doc, id);
    about(m.volume, 192.0);
    about(m.centroid.x(), 5.0);
    about(m.centroid.y(), 24.0);
    assert!(!is_binary(&doc, id), "SAT stays SAT");
}

#[test]
fn a_scaled_round_solid_scales_its_radii() {
    // A cylinder of radius 1, 2 high, tripled: radius 3, 6 high.
    let mut doc = Document::new();
    let id = doc.add_cone(p(0.0, 0.0, 0.0), 1.0, 1.0, 2.0).unwrap();
    doc.transform_entity(id, Transform2::scaling(q(5.0, 0.0), 3.0).unwrap())
        .unwrap();
    let m = measured(&doc, id);
    about(m.volume, PI * 9.0 * 6.0);
    about(m.centroid.x(), -10.0);
    about(m.bounds.max().z(), 6.0);
}

#[test]
fn a_binary_body_stays_binary_and_is_saved_where_it_went() {
    // A box written as SAB, as a DWG 2013 or later keeps it.
    let mut doc = Document::new();
    let sab = SabWriter::write(&build_box([0.0, 0.0, 2.0], 2.0, 2.0, 4.0));
    let h = doc
        .cad_mut()
        .add_entity(EntityType::Solid3D(Solid3D::from_sab(sab)))
        .unwrap();
    let id = h.value();
    doc.sync_entity(id);
    doc.transform_entity(id, Transform2::rotation(q(10.0, 0.0), PI).unwrap())
        .unwrap();
    doc.transform_entity(id, Transform2::scaling(q(20.0, 0.0), 0.5).unwrap())
        .unwrap();
    assert!(is_binary(&doc, id), "SAB stays SAB");
    let m = measured(&doc, id);
    about(m.volume, 2.0);
    about(m.centroid.x(), 20.0);
    about(m.centroid.z(), 1.0);
    let dir = tempfile::tempdir().unwrap();
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        doc.cad_mut().version = version;
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("movido-{version:?}.{ext}"));
            doc.save(&path)
                .unwrap_or_else(|e| panic!("{version:?} {ext}: {e}"));
            let back = Document::open(&path).unwrap();
            let solid = back
                .entities()
                .into_iter()
                .find(|e| e.kind() == "3DSOLID")
                .unwrap();
            let n = measured(&back, solid.id());
            about(n.volume, 2.0);
            about(n.centroid.x(), 20.0);
            about(n.centroid.z(), 1.0);
        }
    }
}

#[test]
fn a_copied_solid_leaves_its_history_behind() {
    let mut doc = Document::new();
    let id = doc.add_box(p(0.0, 0.0, 0.0), p(1.0, 1.0, 1.0)).unwrap();
    if let Some(EntityType::Solid3D(s)) = doc.cad_mut().get_entity_mut(crate::translate::handle(id))
    {
        s.history_handle = Some(acadrust::Handle::new(0x4242));
    }
    let copy = doc.copy_entity(id).unwrap();
    doc.transform_entity(
        copy,
        Transform2::translation(Vector2::new(5.0, 0.0).unwrap()),
    )
    .unwrap();
    let history = |id| match doc.cad().get_entity(crate::translate::handle(id)) {
        Some(EntityType::Solid3D(s)) => s.history_handle,
        _ => panic!("a solid"),
    };
    assert_eq!(history(id), Some(acadrust::Handle::new(0x4242)));
    assert_eq!(history(copy), None);
    about(measured(&doc, copy).centroid.x(), 5.5);
    about(measured(&doc, id).centroid.x(), 0.5);
}

#[test]
fn a_body_whose_data_does_not_read_is_left_alone() {
    let mut doc = Document::new();
    let h = doc
        .cad_mut()
        .add_entity(EntityType::Solid3D(Solid3D::from_sat("not ACIS at all")))
        .unwrap();
    doc.sync_entity(h.value());
    assert!(!doc.entity(h.value()).unwrap().is_transformable());
    assert!(
        doc.transform_entity(
            h.value(),
            Transform2::translation(Vector2::new(1.0, 0.0).unwrap())
        )
        .is_err()
    );
}

/// Whether the history object `h` still records and shows.
fn history_on(doc: &Document, h: acadrust::Handle) -> (bool, bool) {
    use acadrust::objects::{DynamicBlockData, ObjectType};
    match doc.cad().objects.get(&h) {
        Some(ObjectType::DynamicBlock(o)) => match &o.data {
            DynamicBlockData::SolidHistory(s) => (s.record_history, s.show_history),
            _ => panic!("not a history"),
        },
        _ => panic!("no history object"),
    }
}

#[test]
fn a_moved_solid_s_history_is_set_to_none_and_undone_with_it() {
    use acadrust::objects::{DynamicBlockData, DynamicBlockObject, ObjectType, SolidHistory};
    let mut doc = Document::new();
    let id = doc.add_box(p(0.0, 0.0, 0.0), p(1.0, 1.0, 1.0)).unwrap();
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
    if let Some(EntityType::Solid3D(s)) = doc.cad_mut().get_entity_mut(crate::translate::handle(id))
    {
        s.history_handle = Some(h);
    }
    doc.begin_change().unwrap();
    doc.transform_entity(id, Transform2::translation(Vector2::new(2.0, 0.0).unwrap()))
        .unwrap();
    let mut change = doc.commit_change().unwrap();
    assert_eq!(history_on(&doc, h), (false, false), "History: None");
    doc.undo_change(&mut change).unwrap();
    assert_eq!(history_on(&doc, h), (true, true));
    about(measured(&doc, id).centroid.x(), 0.5);
    doc.redo_change(&mut change).unwrap();
    assert_eq!(history_on(&doc, h), (false, false));
    about(measured(&doc, id).centroid.x(), 2.5);
}

#[test]
#[ignore = "needs tools/fetch_codec_samples.py"]
fn autocad_s_solids_with_history_move_and_reopen_without_it() {
    // ATMOS (DWG 2007): 42 of its 58 solids keep AutoCAD's history.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/corpus/libredwg/2007/ATMOS-DC22S.dwg");
    let mut doc = Document::open(&root).unwrap();
    let solids: Vec<_> = doc
        .entities()
        .into_iter()
        .filter(|e| e.kind() == "3DSOLID")
        .map(|e| e.id())
        .collect();
    let histories: Vec<_> = solids
        .iter()
        .filter_map(|&id| {
            super::history_of(doc.cad().get_entity(crate::translate::handle(id)).unwrap())
        })
        .collect();
    assert_eq!((solids.len(), histories.len()), (58, 42));
    for &id in &solids {
        doc.transform_entity(id, Transform2::rotation(q(0.0, 0.0), 1.0).unwrap())
            .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("atmos.dwg");
    doc.save(&path).unwrap();
    let back = Document::open(&path).unwrap();
    for h in histories {
        assert_eq!(history_on(&back, h), (false, false), "{h:?}");
    }
}

#[test]
fn a_turned_cylinder_keeps_its_isolines_where_it_turned_them() {
    // Written and read back, a cylinder turned 30° keeps its reference:
    // its isolines start turned 30° too.
    let mut doc = Document::new();
    let id = doc.add_cone(p(0.0, 0.0, 0.0), 1.0, 1.0, 2.0).unwrap();
    let before = doc.solid(id).unwrap().unwrap();
    let angle = PI / 6.0;
    doc.transform_entity(id, Transform2::rotation(q(0.0, 0.0), angle).unwrap())
        .unwrap();
    let after = doc.solid(id).unwrap().unwrap();
    let side = |b: &arq_kernel::Body| {
        b.faces()
            .iter()
            .find_map(|f| match f.surface {
                arq_kernel::Surface::Cone(c) => Some(c.reference()),
                _ => None,
            })
            .unwrap()
    };
    let (r0, r1) = (side(&before), side(&after));
    about(r0.x() * r1.x() + r0.y() * r1.y(), angle.cos());
    about(r0.x() * r1.y() - r0.y() * r1.x(), angle.sin());
    let back = crate::solid::translate(
        &acadrust::entities::acis::types::SatDocument::parse(
            &crate::acis_write::sat_of(&after).unwrap().to_sat_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!((side(&back) - r1).length() < 1e-12);
}

#[test]
fn a_turned_solid_is_drawn_by_as_many_points() {
    // A D, 4 long and 2 across, its round end an arc, extruded 1: its box
    // in plan changes as it turns, the lines it is drawn by do not.
    use arq_geometry::{Arc, Geometry2, Line as Line2};
    use arq_kernel::{Body, ProfilePiece};
    let piece = |curve| ProfilePiece {
        curve,
        backward: false,
    };
    let profile = [
        piece(Geometry2::Line(
            Line2::new(q(0.0, -1.0), q(4.0, -1.0)).unwrap(),
        )),
        piece(Geometry2::Arc(
            Arc::new(q(4.0, 0.0), 1.0, -FRAC_PI_2, FRAC_PI_2).unwrap(),
        )),
        piece(Geometry2::Line(
            Line2::new(q(4.0, 1.0), q(0.0, 1.0)).unwrap(),
        )),
        piece(Geometry2::Line(
            Line2::new(q(0.0, 1.0), q(0.0, -1.0)).unwrap(),
        )),
    ];
    let body = Body::extrusion(&profile, 0.0, 1.0).unwrap();
    let mut doc = Document::new();
    let id = doc.add_solid(&body).unwrap();
    let points = |d: &Document| {
        let e = d.cad().get_entity(crate::translate::handle(id)).unwrap();
        let w = d.wireframes.get(e, 4).unwrap();
        w.edges.iter().map(Vec::len).sum::<usize>()
    };
    let before = points(&doc);
    doc.transform_entity(id, Transform2::rotation(q(0.0, 0.0), PI / 4.0).unwrap())
        .unwrap();
    assert_eq!(points(&doc), before);
}

#[test]
fn turned_round_solids_keep_their_references_and_isolines() {
    // A cylinder, a ball and a torus turned 30° about z: the reference of
    // each round face turns, its isolines are the old ones turned, and
    // written and read back it stays.
    let angle = PI / 6.0;
    let t = Transform2::rotation(q(0.0, 0.0), angle).unwrap();
    let mut doc = Document::new();
    let ids = [
        doc.add_cone(p(0.0, 0.0, 0.0), 1.0, 1.0, 2.0).unwrap(),
        doc.add_sphere(p(5.0, 0.0, 0.0), 1.0).unwrap(),
        doc.add_torus(p(10.0, 0.0, 0.0), 3.0, 1.0).unwrap(),
    ];
    let reference = |b: &arq_kernel::Body| {
        b.faces()
            .iter()
            .find_map(|f| match f.surface {
                arq_kernel::Surface::Cone(c) => Some(c.reference()),
                arq_kernel::Surface::Sphere(s) => Some(s.reference()),
                arq_kernel::Surface::Torus(t) => Some(t.reference()),
                _ => None,
            })
            .unwrap()
    };
    for id in ids {
        let before = doc.solid(id).unwrap().unwrap();
        doc.transform_entity(id, t).unwrap();
        let after = doc.solid(id).unwrap().unwrap();
        let (r0, r1) = (reference(&before), reference(&after));
        about(r0.x() * r1.x() + r0.y() * r1.y(), angle.cos());
        about(r0.x() * r1.y() - r0.y() * r1.x(), angle.sin());
        let (w0, w1) = (before.wireframe(0.01, 4), after.wireframe(0.01, 4));
        assert_eq!(w0.isolines.len(), w1.isolines.len(), "{id:X}");
        for line in &w0.isolines {
            let moved: Vec<Point2> = line
                .iter()
                .map(|x| t.apply_point(q(x.x(), x.y())))
                .collect();
            let found = w1.isolines.iter().any(|l| {
                l.len() == line.len()
                    && l.iter()
                        .zip(&moved)
                        .all(|(a, b)| (a.x() - b.x()).hypot(a.y() - b.y()) < 1e-9)
            });
            assert!(found, "{id:X}: an isoline not turned");
        }
        let back = crate::solid::translate(
            &acadrust::entities::acis::types::SatDocument::parse(
                &crate::acis_write::sat_of(&after).unwrap().to_sat_string(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!((reference(&back) - r1).length() < 1e-12, "{id:X}");
    }
}

#[test]
fn a_size_folded_into_the_matrix_still_scales_the_radii() {
    // Another writer may fold a scale into the matrix, the scale left 1,
    // as the codec composes it before we tidy it: radii scale all the same.
    use acadrust::types::{Matrix4, Transform};
    let mut doc = Document::new();
    let id = doc.add_cone(p(0.0, 0.0, 0.0), 1.0, 1.0, 2.0).unwrap();
    let doubled = Transform::from_matrix(Matrix4 {
        m: [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    });
    doc.cad_mut()
        .get_entity_mut(crate::translate::handle(id))
        .unwrap()
        .apply_transform(&doubled);
    let (_, _, scale) = placement(&doc, id).unwrap();
    about(scale, 1.0);
    about(measured(&doc, id).volume, PI * 4.0 * 4.0);
}

#[test]
fn a_body_s_vertices_box_follows_its_placement() {
    // What boxes a body not translated yet: its vertices, placed. A
    // 2 x 2 x 4 box at the origin, turned half round (10, 0): x 19..21.
    let mut doc = Document::new();
    let id = doc.add_box(p(-1.0, -1.0, 0.0), p(1.0, 1.0, 4.0)).unwrap();
    doc.transform_entity(id, Transform2::rotation(q(10.0, 0.0), PI).unwrap())
        .unwrap();
    let e = doc.cad().get_entity(crate::translate::handle(id)).unwrap();
    let b = super::bounds(e).unwrap();
    about(b.min().x(), 19.0);
    about(b.max().x(), 21.0);
    about(b.min().y(), -1.0);
}
