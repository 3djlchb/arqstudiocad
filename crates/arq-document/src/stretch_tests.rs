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

#[test]
fn stretch_preview_is_read_only_and_matches_committed_line_and_bulged_path() {
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    let line = arq_geometry::Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap();
    let id = doc
        .add_entity(PlanGeometry::new(line.into()), layer)
        .unwrap();
    let area = CrossingArea::Rectangle(Bounds2::new(p(9.0, -1.0), p(11.0, 1.0)));
    let offset = Offset::new(2.0, 3.0).unwrap();
    let preview = doc
        .stretch_shapes(id, std::slice::from_ref(&area), offset)
        .unwrap();
    let Shape::Line(shown) = preview[0] else {
        panic!()
    };
    assert_eq!((shown.start(), shown.end()), (p(0.0, 0.0), p(12.0, 3.0)));
    assert_eq!(
        doc.entity(id).unwrap().plan().unwrap().geometry(),
        line.into()
    );
    doc.stretch_entity_in_areas(id, &[area], offset).unwrap();
    assert_eq!(
        doc.drawing(id)
            .iter()
            .flatten()
            .map(|i| i.shape.clone())
            .collect::<Vec<_>>(),
        preview
    );

    let poly = doc
        .add_polyline(
            &[(p(0.0, 5.0), 1.0), (p(10.0, 5.0), 0.0), (p(10.0, 8.0), 0.0)],
            false,
        )
        .unwrap();
    let area = CrossingArea::Rectangle(Bounds2::new(p(9.0, 4.0), p(11.0, 9.0)));
    let original = doc.drawing(poly);
    let preview = doc
        .stretch_shapes(poly, std::slice::from_ref(&area), offset)
        .unwrap();
    assert!(matches!(preview[0], Shape::Arc(_)));
    assert_eq!(doc.drawing(poly), original);
    doc.stretch_entity_in_areas(poly, &[area], offset).unwrap();
    assert_eq!(
        doc.drawing(poly)
            .iter()
            .flatten()
            .map(|i| i.shape.clone())
            .collect::<Vec<_>>(),
        preview
    );
}

#[test]
fn stretch_preview_preserves_whole_block_expansion_and_partial_arcs() {
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    let line = arq_geometry::Line::new(p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let id = doc
        .add_entity(PlanGeometry::new(line.into()), layer)
        .unwrap();
    let block = doc.create_block("preview", p(0.0, 0.0), &[id]).unwrap();
    let offset = Offset::new(3.0, 4.0).unwrap();
    let preview = doc.stretch_shapes(block, &[], offset).unwrap();
    let Shape::Line(shown) = preview[0] else {
        panic!()
    };
    assert_eq!((shown.start(), shown.end()), (p(3.0, 4.0), p(5.0, 4.0)));
    doc.stretch_entity_in_areas(block, &[], offset).unwrap();
    assert_eq!(
        doc.drawing(block)
            .iter()
            .flatten()
            .map(|i| i.shape.clone())
            .collect::<Vec<_>>(),
        preview
    );
    let arc = arq_geometry::Arc::new(p(0.0, 0.0), 5.0, 0.0, std::f64::consts::FRAC_PI_2).unwrap();
    let id = doc
        .add_entity(PlanGeometry::new(arc.into()), layer)
        .unwrap();
    let before = doc.drawing(id);
    let area = CrossingArea::Rectangle(Bounds2::new(p(4.0, -1.0), p(6.0, 1.0)));
    let preview = doc
        .stretch_shapes(id, std::slice::from_ref(&area), offset)
        .unwrap();
    assert_eq!(doc.drawing(id), before);
    let Shape::Arc(shown) = preview[0] else {
        panic!()
    };
    assert!(
        shown
            .start_point()
            .approx_eq(p(8.0, 4.0), Tolerance::DEFAULT)
    );
    assert!(shown.end_point().approx_eq(p(0.0, 5.0), Tolerance::DEFAULT));
    doc.stretch_entity_in_areas(id, &[area], offset).unwrap();
    assert_eq!(
        doc.entity(id).unwrap().plan().unwrap().geometry(),
        shown.into()
    );
}

#[test]
fn stretch_arc_preserves_ocs_metadata_owned_model_and_preview() {
    use std::f64::consts::{FRAC_PI_2, PI};
    for nz in [1.0, -1.0] {
        let mut doc = Document::new();
        let mut arc = acadrust::Arc::new();
        // Both OCS orientations describe world-plan 0 -> 90 degrees.
        arc.center = Vector3::new(0.0, 0.0, 7.0 * nz);
        arc.radius = 5.0;
        arc.normal = Vector3::new(0.0, 0.0, nz);
        arc.thickness = 2.0;
        arc.start_angle = if nz > 0.0 { 0.0 } else { FRAC_PI_2 };
        arc.end_angle = if nz > 0.0 { FRAC_PI_2 } else { PI };
        let id = doc
            .cad_mut()
            .add_entity(EntityType::Arc(arc))
            .unwrap()
            .value();
        doc.rebuild_model();
        doc.set_entity_color(id, Color::Index(3)).unwrap();
        let original = doc.entity(id).unwrap();
        let area = CrossingArea::Rectangle(Bounds2::new(p(-1.0, 4.0), p(1.0, 6.0)));
        let offset = Offset::new(3.0, 2.0).unwrap();
        let preview = doc
            .stretch_shapes(id, std::slice::from_ref(&area), offset)
            .unwrap();
        assert_eq!(doc.entity(id).unwrap().plan(), original.plan());
        doc.stretch_entity_in_areas(id, &[area], offset).unwrap();
        let EntityType::Arc(back) = doc.cad().get_entity(handle(id)).unwrap() else {
            panic!()
        };
        assert_eq!(back.normal.z, nz);
        assert_eq!((back.center.z, back.thickness), (7.0 * nz, 2.0));
        let info = doc.entity(id).unwrap();
        assert_eq!(info.color(), Color::Index(3));
        assert_eq!(info.layer(), original.layer());
        let plan = info.plan().unwrap();
        assert_eq!(plan.elevation(), 7.0);
        let Geometry2::Arc(result) = plan.geometry() else {
            panic!()
        };
        assert!(result.center().distance_to(p(0.3542858741, 2.4583673926)) < 1e-10);
        assert_eq!(preview.len(), 1);
        let Shape::Arc(shown) = preview[0] else {
            panic!()
        };
        assert!(
            shown
                .center()
                .approx_eq(result.center(), Tolerance::DEFAULT)
        );
        assert!((shown.radius() - result.radius()).abs() <= Tolerance::DEFAULT.linear());
        // Mapping OCS angles to world and back can round the last bit.
        assert!((shown.start_angle() - result.start_angle()).abs() <= Tolerance::DEFAULT.angular());
        assert!((shown.end_angle() - result.end_angle()).abs() <= Tolerance::DEFAULT.angular());
        assert!(doc.check_model().is_empty());
    }
}
