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
                Surface::Nurbs(_) => "nurbs",
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

#[test]
fn homogeneous_control_points_divide_by_their_weight() {
    // AcadRust gives a rational net as [x·w, y·w, z·w, w].
    let (points, weights) =
        super::split_homogeneous(&[[2.0, 2.0, 0.0, 2.0], [1.0, 0.0, 0.0, 1.0]]).unwrap();
    assert_eq!(points[0], arq_geometry::Point3::new(1.0, 1.0, 0.0).unwrap());
    assert_eq!(weights, [2.0, 1.0]);
    assert!(
        super::split_homogeneous(&[[1.0, 0.0, 0.0, 0.0]]).is_err(),
        "no weight"
    );
}

/// A drawing holding `sat` as one 3DSOLID, and its id.
fn drawing_with(sat: &SatDocument) -> (Document, u64) {
    let mut cad = CadDocument::new();
    let h = cad
        .add_entity(EntityType::Solid3D(Solid3D::from_sat(&sat.to_sat_string())))
        .unwrap();
    (Document::from_engine(cad), h.value())
}

/// The rounded plan box of what is drawn for `id`, and whether any of it
/// is a placeholder box.
fn drawn(doc: &Document, id: u64) -> ([f64; 4], bool) {
    let items = doc.drawing(id).unwrap();
    let placeholder = items
        .iter()
        .any(|i| matches!(i.shape, crate::drawing::Shape::Placeholder(_)));
    let b = items
        .iter()
        .map(|i| i.shape.bounds())
        .reduce(|a, b| a.union(b))
        .unwrap();
    let r = |v: f64| (v * 1e4).round() / 1e4 + 0.0;
    (
        [
            r(b.min().x()),
            r(b.min().y()),
            r(b.max().x()),
            r(b.max().y()),
        ],
        placeholder,
    )
}

#[test]
fn a_solid_is_drawn_by_its_wireframe() {
    // The box (0..2, -1..1, -1..1) in plan: its edges, no placeholder.
    let (doc, id) = drawing_with(&primitives::build_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0));
    assert_eq!(drawn(&doc, id), ([0.0, -1.0, 2.0, 1.0], false));
    // A cylinder of radius 1 around the z axis: its circles, so its box
    // (for picking and the index) is the whole circle, not its vertices.
    let (doc, id) = drawing_with(&primitives::build_cylinder([0.0, 0.0, 0.0], 1.0, 3.0));
    let (extent, placeholder) = drawn(&doc, id);
    assert!(!placeholder);
    assert!(
        extent
            .iter()
            .zip([-1.0, -1.0, 1.0, 1.0])
            .all(|(a, b)| (a - b).abs() < 1e-3),
        "{extent:?}"
    );
    let b = doc.entity(id).unwrap().bounds().unwrap();
    assert!(
        (b.min().x() + 1.0).abs() < 1e-3 && (b.max().y() - 1.0).abs() < 1e-3,
        "{b:?}"
    );
}

#[test]
fn a_solid_in_a_block_is_drawn_where_the_block_places_it() {
    // The box (0..2, -1..1) as block CAJA based at (0,0), placed at (10,0)
    // turned 90°: x becomes y, so (9..11, 0..2).
    let (mut doc, id) = drawing_with(&primitives::build_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0));
    let base = arq_geometry::Point2::new(0.0, 0.0).unwrap();
    let first = doc.create_block("CAJA", base, &[id]).unwrap();
    let at = arq_geometry::Point2::new(10.0, 0.0).unwrap();
    let placed = doc
        .insert_block("CAJA", at, 1.0, std::f64::consts::FRAC_PI_2)
        .unwrap();
    assert_eq!(drawn(&doc, first), ([0.0, -1.0, 2.0, 1.0], false));
    assert_eq!(drawn(&doc, placed), ([9.0, 0.0, 11.0, 2.0], false));
}

#[test]
fn a_changed_solid_is_drawn_from_its_new_data() {
    // The wireframe cache is keyed by the ACIS data: new data, new drawing.
    let (mut doc, id) = drawing_with(&primitives::build_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0));
    assert_eq!(drawn(&doc, id).0, [0.0, -1.0, 2.0, 1.0]);
    let moved = primitives::build_box([5.0, 0.0, 0.0], 2.0, 2.0, 2.0).to_sat_string();
    if let Some(EntityType::Solid3D(s)) = doc.cad_mut().get_entity_mut(acadrust::Handle::new(id)) {
        s.set_sat_data(&moved);
    }
    assert_eq!(drawn(&doc, id).0, [4.0, -1.0, 6.0, 1.0]);
}
