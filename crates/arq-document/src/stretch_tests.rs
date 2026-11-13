//! STRETCH keeps existing entity metadata and respects world-plan OCS.

use super::*;
use arq_geometry::Vector2 as Offset;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn stretch_preserves_polyline_metadata_bulges_and_downward_normal() {
    for normal_z in [1.0, -1.0] {
        let mut doc = Document::new();
        let mut poly = LwPolyline::new();
        poly.normal = Vector3::new(0.0, 0.0, normal_z);
        poly.elevation = 7.0;
        poly.thickness = 3.0;
        poly.constant_width = 0.5;
        poly.is_closed = true;
        for (x, y, b) in [(0.0, 0.0, 0.5), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)] {
            poly.add_point_with_bulge(Vector2::new(x * normal_z, y), b);
        }
        poly.vertices[1].start_width = 0.2;
        poly.vertices[1].end_width = 0.4;
        let id = doc
            .cad_mut()
            .add_entity(EntityType::LwPolyline(poly))
            .unwrap()
            .value();
        let area = Bounds2::new(p(9.0, -1.0), p(11.0, 11.0));
        doc.stretch_entity(id, &[area], Offset::new(2.0, 3.0).unwrap())
            .unwrap();
        let EntityType::LwPolyline(back) = doc.cad().get_entity(handle(id)).unwrap() else {
            panic!()
        };
        assert_eq!(back.normal.z, normal_z);
        assert_eq!(
            (back.elevation, back.thickness, back.constant_width),
            (7.0, 3.0, 0.5)
        );
        assert_eq!(back.vertices[0].bulge, 0.5);
        assert_eq!(
            (back.vertices[1].start_width, back.vertices[1].end_width),
            (0.2, 0.4)
        );
        assert!(back.is_closed);
        assert_eq!(
            doc.grips(&[id]).iter().map(|g| g.point).collect::<Vec<_>>(),
            [p(0.0, 0.0), p(12.0, 3.0), p(12.0, 13.0)]
        );
    }
}

#[test]
fn stretch_line_keeps_elevation_and_the_owned_model_in_sync() {
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    let line = arq_geometry::Line::new(p(1e6, 2e6), p(1e6 + 10.0, 2e6)).unwrap();
    let id = doc
        .add_entity(PlanGeometry::at_elevation(line.into(), 8.0).unwrap(), layer)
        .unwrap();
    let area = Bounds2::from_point(p(1e6 + 10.0, 2e6));
    doc.stretch_entity(id, &[area], Offset::new(0.25, -0.5).unwrap())
        .unwrap();
    let plan = doc.entity(id).unwrap().plan().unwrap();
    assert_eq!(plan.elevation(), 8.0);
    assert_eq!(plan.geometry().end_point(), Some(p(1e6 + 10.25, 2e6 - 0.5)));
    assert!(doc.check_model().is_empty());
}
