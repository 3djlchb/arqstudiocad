//! Tests of reading ACIS bodies into the kernel's B-rep (a submodule of
//! `solid`). The bodies are AcadRust's ACIS primitives; their expected
//! shapes are worked out by hand.

use acadrust::entities::Solid3D;
use acadrust::entities::acis::primitives;
use acadrust::entities::acis::types::{SatBody, SatDocument, SatPointer, SatRecord, SatToken};
use acadrust::{CadDocument, EntityType};
use arq_kernel::{Body, Surface};

use super::Untranslated;
use crate::document::Document;

/// A drawing holding `sat` as one 3DSOLID; the solid read back.
fn read(sat: &SatDocument) -> Result<Body, Untranslated> {
    let mut cad = CadDocument::new();
    let h = cad
        .add_entity(EntityType::Solid3D(Solid3D::from_sat(&sat.to_sat_string())))
        .unwrap();
    Document::from_engine(cad)
        .solid(h.value())
        .expect("an ACIS body")
}

/// Rounded `[min, max]` of the body's vertices.
fn corners(body: &Body) -> [[f64; 3]; 2] {
    let b = body.vertex_bounds().unwrap();
    let r = |v: f64| (v * 1e6).round() / 1e6 + 0.0;
    [
        [r(b.min().x()), r(b.min().y()), r(b.min().z())],
        [r(b.max().x()), r(b.max().y()), r(b.max().z())],
    ]
}

#[test]
fn a_box_reads_as_six_planar_faces() {
    let body = read(&primitives::build_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0)).unwrap();
    assert_eq!(body.faces().len(), 6);
    assert_eq!(body.edges().len(), 12);
    assert_eq!(body.vertices().len(), 8);
    assert!(
        body.faces()
            .iter()
            .all(|f| matches!(f.surface, Surface::Plane(_)))
    );
    assert_eq!(corners(&body), [[0.0, -1.0, -1.0], [2.0, 1.0, 1.0]]);
}

#[test]
fn curved_primitives_read_with_their_surfaces() {
    let kinds = |body: &Body| {
        let mut k: Vec<&str> = body
            .faces()
            .iter()
            .map(|f| match f.surface {
                Surface::Plane(_) => "plane",
                Surface::Cone(c) if c.is_cylinder() => "cylinder",
                Surface::Cone(_) => "cone",
                Surface::EllipticCylinder(_) => "elliptic cylinder",
                Surface::Sphere(_) => "sphere",
                Surface::Torus(_) => "torus",
            })
            .collect();
        k.sort_unstable();
        k
    };
    let cylinder = read(&primitives::build_cylinder([0.0, 0.0, 0.0], 1.0, 3.0)).unwrap();
    assert_eq!(kinds(&cylinder), ["cylinder", "plane", "plane"]);
    let cone = read(&primitives::build_cone([0.0, 0.0, 0.0], 1.0, 3.0)).unwrap();
    assert!(kinds(&cone).contains(&"cone"), "{:?}", kinds(&cone));
    let sphere = read(&primitives::build_sphere([0.0, 0.0, 0.0], 2.0)).unwrap();
    assert_eq!(kinds(&sphere), ["sphere"]);
    let torus = read(&primitives::build_torus([0.0, 0.0, 0.0], 3.0, 1.0)).unwrap();
    assert_eq!(kinds(&torus), ["torus"]);
}

#[test]
fn a_transform_moves_the_body_as_acis_applies_it() {
    // ACIS applies a transform to row vectors: p' = p · R · s + t. With
    // rows (0,1,0), (-1,0,0), (0,0,1), scale 2 and t = (10,0,0),
    // (x, y, z) goes to (10 - 2y, 2x, 2z). The box (0..2,
    // -1..1, -1..1) goes to (8..12, 0..4, -2..2); the other convention
    // would put y in -4..0.
    let mut sat = primitives::build_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let mut transform = SatRecord::new(0, "transform");
    transform.tokens = [
        0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0, 2.0,
    ]
    .into_iter()
    .map(SatToken::Float)
    .chain(["rotate", "no_reflect", "no_shear"].map(|s| SatToken::Enum(s.into())))
    .collect();
    let index = sat.add_record(transform);
    let body = sat
        .records
        .iter()
        .position(|r| SatBody::from_record(r).is_some())
        .unwrap();
    let slot = sat.records[body]
        .tokens
        .iter()
        .rposition(|t| matches!(t, SatToken::Pointer(_)))
        .unwrap();
    sat.records[body].tokens[slot] = SatToken::Pointer(SatPointer(index));
    assert_eq!(
        SatBody::from_record(&sat.records[body])
            .unwrap()
            .transform(),
        SatPointer(index),
        "the body points to the transform"
    );
    let body = read(&sat).unwrap();
    assert_eq!(corners(&body), [[8.0, 0.0, -2.0], [12.0, 4.0, 2.0]]);
}

#[test]
fn what_is_not_translated_says_why() {
    let mut sat = primitives::build_box([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    // A spline surface where a plane was.
    let plane = sat
        .records
        .iter()
        .position(|r| r.entity_type == "plane-surface")
        .unwrap();
    sat.records[plane].entity_type = "spline-surface".into();
    assert_eq!(
        read(&sat).unwrap_err(),
        Untranslated::Unsupported("spline-surface".into())
    );
    // A vertex moved off its edges.
    let mut sat = primitives::build_box([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let point = sat
        .records
        .iter()
        .position(|r| r.entity_type == "point")
        .unwrap();
    let slot = sat.records[point]
        .tokens
        .iter()
        .position(|t| matches!(t, SatToken::Float(_)))
        .unwrap();
    sat.records[point].tokens[slot] = SatToken::Float(0.5);
    assert!(matches!(read(&sat), Err(Untranslated::Invalid(_))));
    // Not a body at all, and data that does not parse.
    let mut cad = CadDocument::new();
    let line = cad
        .add_entity(EntityType::Line(acadrust::entities::Line::from_coords(
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
        )))
        .unwrap();
    let broken = cad
        .add_entity(EntityType::Solid3D(Solid3D::from_sat("not ACIS")))
        .unwrap();
    let doc = Document::from_engine(cad);
    assert!(doc.solid(line.value()).is_none());
    assert_eq!(
        doc.solid(broken.value()),
        Some(Err(Untranslated::Unreadable))
    );
}
