//! BREAK preserves the original plane, style and interpolated segment widths.
use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn break_polyline_interpolates_widths_and_keeps_plane_and_metadata() {
    for normal_z in [1.0, -1.0] {
        let mut doc = Document::new();
        let mut poly = LwPolyline::new();
        poly.normal = Vector3::new(0.0, 0.0, normal_z);
        poly.elevation = 7.0;
        poly.thickness = 3.0;
        poly.constant_width = 0.5;
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)] {
            poly.add_point_with_bulge(Vector2::new(x * normal_z, y), 0.0);
        }
        poly.vertices[0].start_width = 2.0;
        poly.vertices[0].end_width = 6.0;
        poly.vertices[1].start_width = 10.0;
        poly.vertices[1].end_width = 20.0;
        let id = doc
            .cad_mut()
            .add_entity(EntityType::LwPolyline(poly))
            .unwrap()
            .value();
        let ids = doc.break_polyline(id, p(3.0, -2.0), p(12.0, 7.0)).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], id);
        for (target, widths) in ids.into_iter().zip([(2.0, 3.2), (17.0, 20.0)]) {
            let EntityType::LwPolyline(back) = doc.cad().get_entity(handle(target)).unwrap() else {
                panic!()
            };
            assert_eq!(back.normal.z, normal_z);
            assert_eq!(
                (back.elevation, back.thickness, back.constant_width),
                (7.0, 3.0, 0.5)
            );
            assert!(!back.is_closed);
            assert!((back.vertices[0].start_width - widths.0).abs() < 1e-12);
            assert!((back.vertices[0].end_width - widths.1).abs() < 1e-12);
        }
        assert!(doc.check_model().is_empty());
    }
}

#[test]
fn closed_break_preserves_plane_metadata_and_normalizes_effective_widths() {
    for normal_z in [-1., 1.] {
        for constant in [0., 0.5] {
            let mut doc = Document::new();
            let mut poly = LwPolyline::new();
            poly.is_closed = true;
            poly.normal = Vector3::new(0., 0., normal_z);
            poly.elevation = 7.;
            poly.thickness = 3.;
            poly.constant_width = constant;
            for (x, y) in [(0., 0.), (10., 10.), (0., 10.), (10., 0.)] {
                poly.add_point_with_bulge(Vector2::new(x * normal_z, y), 0.);
            }
            if constant == 0.0 {
                for (i, v) in poly.vertices.iter_mut().enumerate() {
                    v.start_width = 2. + 10. * i as f64;
                    v.end_width = 6. + 10. * i as f64;
                }
            }

            let id = doc
                .cad_mut()
                .add_entity(EntityType::LwPolyline(poly))
                .unwrap()
                .value();
            let before = doc.cad().get_entity(handle(id)).unwrap().clone();
            let ids = doc.break_polyline(id, p(5., 5.), p(5., 5.)).unwrap();
            assert_eq!(ids, [id]);
            let EntityType::LwPolyline(back) = doc.cad().get_entity(handle(id)).unwrap() else {
                panic!()
            };
            assert!(!back.is_closed);
            assert_eq!(
                (
                    back.normal.z,
                    back.elevation,
                    back.thickness,
                    back.constant_width
                ),
                (normal_z, 7., 3., 0.)
            );

            assert_eq!(back.vertices.len(), 6);
            assert_eq!(
                (
                    back.vertices.last().unwrap().start_width,
                    back.vertices.last().unwrap().end_width
                ),
                (0., 0.)
            );
            let expected = if constant > 0. {
                vec![(0.5, 0.5); 5]
            } else {
                vec![(4., 6.), (12., 16.), (22., 26.), (32., 36.), (2., 4.)]
            };
            for (v, (start, end)) in back.vertices.iter().zip(expected) {
                assert!((v.start_width - start).abs() < Tolerance::DEFAULT.linear());
                assert!((v.end_width - end).abs() < Tolerance::DEFAULT.linear());
            }
            assert_ne!(&before, doc.cad().get_entity(handle(id)).unwrap());
            assert!(doc.check_model().is_empty());
        }
    }
}
