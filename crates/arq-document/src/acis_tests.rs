//! Tests of ACIS bodies seen through their vertices (a submodule of `acis`).

use acadrust::entities::acis::primitives::build_box;
use acadrust::entities::solid3d::{Solid3D, Wire};
use acadrust::types::Vector3;
use acadrust::{EntityType, SabWriter, SatWriter};
use arq_geometry::{Bounds2, Point2};

use crate::document::Document;
use crate::translate::codec_bounds;

/// A 2 × 3 × 4 box centred on (10, 20, -5): plan extent 9..11 × 18.5..21.5.
fn box_bounds() -> Bounds2 {
    Bounds2::new(
        Point2::new(9.0, 18.5).unwrap(),
        Point2::new(11.0, 21.5).unwrap(),
    )
}

fn sat_box(center: [f64; 3]) -> EntityType {
    EntityType::Solid3D(Solid3D::from_sat(&SatWriter::write(&build_box(
        center, 2.0, 3.0, 4.0,
    ))))
}

fn sab_box(center: [f64; 3]) -> EntityType {
    EntityType::Solid3D(Solid3D::from_sab(SabWriter::write(&build_box(
        center, 2.0, 3.0, 4.0,
    ))))
}

#[test]
fn a_body_without_its_wireframe_is_boxed_from_its_vertices() {
    for body in [sat_box([10.0, 20.0, -5.0]), sab_box([10.0, 20.0, -5.0])] {
        assert!(super::is_body(&body));
        assert_eq!(
            super::vertices(&body).map(|v| v.len()),
            Some(8),
            "a box has 8 vertices"
        );
        assert_eq!(codec_bounds(&body), Some(box_bounds()));
    }
    let line = EntityType::Line(acadrust::Line::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 0.0));
    assert!(!super::is_body(&line) && super::vertices(&line).is_none());
}

#[test]
fn a_wireframe_still_boxes_the_body_when_present() {
    let EntityType::Solid3D(mut solid) = sab_box([10.0, 20.0, -5.0]) else {
        unreachable!()
    };
    let mut wire = Wire::new();
    wire.add_point(Vector3::new(0.0, 0.0, 0.0));
    wire.add_point(Vector3::new(1.0, 1.0, 0.0));
    solid.wires.push(wire);
    let bounds = codec_bounds(&EntityType::Solid3D(solid)).unwrap();
    assert_eq!(
        bounds.max(),
        Point2::new(1.0, 1.0).unwrap(),
        "the cache, as before"
    );
}

/// A body read from DWG is boxed from its wireframe cache; the same body
/// read from DXF has no cache: the same vertices make it the same body.
#[test]
fn a_body_is_compared_by_its_vertices_when_boxes_differ() {
    let mut doc = Document::new();
    let EntityType::Solid3D(mut cached) = sab_box([10.0, 20.0, -5.0]) else {
        unreachable!()
    };
    let mut wire = Wire::new();
    wire.add_point(Vector3::new(9.0, 18.5, -7.0));
    wire.add_point(Vector3::new(11.5, 21.5, -3.0));
    cached.wires.push(wire);
    let id = doc
        .cad_mut()
        .add_entity(EntityType::Solid3D(cached))
        .unwrap();

    let mut from_dxf = doc.snapshot();
    *from_dxf.cad_mut().get_entity_mut(id).unwrap() = sat_box([10.0, 20.0, -5.0]);
    from_dxf
        .cad_mut()
        .get_entity_mut(id)
        .unwrap()
        .common_mut()
        .handle = id;
    assert_eq!(doc.validate_roundtrip(&from_dxf), Vec::<String>::new());

    let mut moved = doc.snapshot();
    *moved.cad_mut().get_entity_mut(id).unwrap() = sat_box([12.0, 20.0, -5.0]);
    moved
        .cad_mut()
        .get_entity_mut(id)
        .unwrap()
        .common_mut()
        .handle = id;
    assert!(!doc.validate_roundtrip(&moved).is_empty(), "another body");
}
