//! JOIN's polyline writer and reader: widths per segment, a constant width
//! kept only while every segment has it, either plane, a polyline made
//! from a line with the line's properties.
use super::*;
use crate::format::FileFormat;
use arq_geometry::{Arc, Line, chain_paths};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn raw(doc: &Document, id: EntityId) -> &LwPolyline {
    let Some(EntityType::LwPolyline(poly)) = doc.cad().get_entity(handle(id)) else {
        panic!("not a polyline")
    };
    poly
}

fn polyline(doc: &mut Document, normal_z: f64, points: &[(f64, f64)], constant: f64) -> EntityId {
    let mut poly = LwPolyline::new();
    poly.normal = Vector3::new(0.0, 0.0, normal_z);
    poly.elevation = 2.0;
    poly.thickness = 3.0;
    poly.constant_width = constant;
    for &(x, y) in points {
        poly.add_point_with_bulge(Vector2::new(x * normal_z, y), 0.0);
    }
    doc.cad_mut()
        .add_entity(EntityType::LwPolyline(poly))
        .unwrap()
        .value()
}

fn line(doc: &mut Document, a: (f64, f64), b: (f64, f64), height: f64) -> EntityId {
    let plan = PlanGeometry::at_elevation(
        Geometry2::Line(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap()),
        height,
    )
    .unwrap();
    let layer = doc.current_layer().id();
    doc.add_entity(plan, layer).unwrap()
}

#[test]
fn a_joined_polyline_keeps_each_segment_width_in_either_plane() {
    for normal_z in [1.0, -1.0] {
        let mut doc = Document::new();
        let source = polyline(&mut doc, normal_z, &[(0.0, 0.0), (4.0, 0.0)], 0.5);
        let (path, height) = doc.join_path(source).unwrap();
        assert_eq!(height, 2.0 * normal_z, "its height in the world");
        assert_eq!(
            path.widths,
            [(0.5, 0.5)],
            "a constant width on every segment"
        );
        assert_eq!(path.start(), p(0.0, 0.0));
        let next = line(&mut doc, (4.0, 0.0), (4.0, 3.0), height);
        let (next_path, _) = doc.join_path(next).unwrap();
        let chain = chain_paths(&path, &[next_path], Tolerance::DEFAULT);
        assert_eq!(
            doc.write_joined_polyline(source, &chain, height).unwrap(),
            source
        );
        let back = raw(&doc, source);
        assert_eq!(back.normal.z, normal_z, "its own plane");
        assert_eq!((back.elevation, back.thickness), (2.0, 3.0));
        assert_eq!(
            back.constant_width, 0.0,
            "no longer the same on every segment"
        );
        let widths: Vec<_> = back
            .vertices
            .iter()
            .map(|v| (v.start_width, v.end_width))
            .collect();
        assert_eq!(widths, [(0.5, 0.5), (0.0, 0.0), (0.0, 0.0)]);
        let (read, _) = doc.join_path(source).unwrap();
        assert_eq!(
            read.vertices.iter().map(|v| v.0).collect::<Vec<_>>(),
            [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]
        );
        assert!(doc.check_model().is_empty());
    }
}

#[test]
fn a_constant_width_stays_while_every_segment_has_it() {
    let mut doc = Document::new();
    let a = polyline(&mut doc, 1.0, &[(0.0, 0.0), (1.0, 0.0)], 0.25);
    let b = polyline(&mut doc, 1.0, &[(2.0, 0.0), (1.0, 0.0)], 0.25);
    let (first, height) = doc.join_path(a).unwrap();
    let (second, _) = doc.join_path(b).unwrap();
    let chain = chain_paths(&first, &[second], Tolerance::DEFAULT);
    doc.write_joined_polyline(a, &chain, height).unwrap();
    let back = raw(&doc, a);
    assert_eq!(back.constant_width, 0.25);
    assert_eq!(back.vertices.len(), 3);
    assert!(
        back.vertices
            .iter()
            .all(|v| v.start_width == 0.0 && v.end_width == 0.0)
    );
}

#[test]
fn a_polyline_made_from_a_line_takes_its_properties_and_closes() {
    let mut doc = Document::new();
    let first = line(&mut doc, (0.0, 0.0), (4.0, 0.0), 1.5);
    doc.set_entity_color(first, Color::Index(5)).unwrap();
    let mut others = Vec::new();
    for (a, b) in [((4.0, 0.0), (4.0, 3.0)), ((4.0, 3.0), (0.0, 3.0))] {
        others.push(line(&mut doc, a, b, 1.5));
    }
    let arc = Arc::new(
        p(0.0, 1.5),
        1.5,
        std::f64::consts::FRAC_PI_2,
        -std::f64::consts::FRAC_PI_2,
    )
    .unwrap();
    let layer = doc.current_layer().id();
    others.push(
        doc.add_entity(
            PlanGeometry::at_elevation(Geometry2::Arc(arc), 1.5).unwrap(),
            layer,
        )
        .unwrap(),
    );
    let (path, height) = doc.join_path(first).unwrap();
    let paths: Vec<_> = others
        .iter()
        .map(|&id| doc.join_path(id).unwrap().0)
        .collect();
    let chain = chain_paths(&path, &paths, Tolerance::DEFAULT);
    assert!(chain.closed);
    let made = doc.write_joined_polyline(first, &chain, height).unwrap();
    assert_ne!(made, first, "a polyline of its own");
    assert_eq!(doc.entity(made).unwrap().color(), Color::Index(5));
    let back = raw(&doc, made);
    assert!(back.is_closed);
    assert_eq!((back.normal.z, back.elevation), (1.0, 1.5));
    assert_eq!(back.vertices.len(), 4);
    let bulge = back.vertices[3].bulge;
    assert!(
        (bulge - 1.0).abs() < 1e-12,
        "the half circle, counterclockwise: {bulge}"
    );
    assert!(
        doc.join_path(made).is_err(),
        "a closed polyline joins nothing"
    );
}

#[test]
fn a_joined_polylines_widths_survive_dwg_and_dxf() {
    let mut doc = Document::new();
    let source = polyline(&mut doc, 1.0, &[(0.0, 0.0), (4.0, 0.0)], 0.5);
    let (path, height) = doc.join_path(source).unwrap();
    let next = line(&mut doc, (4.0, 0.0), (4.0, 3.0), height);
    let (next_path, _) = doc.join_path(next).unwrap();
    let chain = chain_paths(&path, &[next_path], Tolerance::DEFAULT);
    doc.write_joined_polyline(source, &chain, height).unwrap();
    doc.delete_entity(next).unwrap();
    let dxf = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    let dwg = Document::read(
        doc.to_dwg_bytes().unwrap(),
        FileFormat::Dwg,
        std::path::Path::new("join.dwg"),
    )
    .unwrap();
    for back in [dxf, dwg] {
        let polylines: Vec<_> = back
            .entities()
            .iter()
            .filter(|e| e.kind() == "LWPOLYLINE")
            .map(|e| e.id())
            .collect();
        let [id] = polylines[..] else {
            panic!("{polylines:?}")
        };
        let (read, _) = back.join_path(id).unwrap();
        assert_eq!(read.widths, [(0.5, 0.5), (0.0, 0.0)]);
    }
}
