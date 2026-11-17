use super::*;
use arq_geometry::{Point3, Vector3};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn ray_candidates_cover_both_directions_and_long_crossing_entities() {
    let mut cad = CadDocument::new();
    let (a, _) = cad.add_line(p(100., 100.), p(101., 100.)).unwrap();
    let (b, _) = cad.add_line(p(-100., -100.), p(-99., -100.)).unwrap();
    let (cross, _) = cad.add_line(p(0., 50.), p(200., 50.)).unwrap();
    let (miss, _) = cad.add_line(p(0., 80.), p(1., 80.)).unwrap();
    let ids = cad
        .query_point_ray(Point3::ORIGIN, Vector3::new(1., 1., 1.).unwrap(), 0.1)
        .unwrap();
    assert!(ids.contains(&a) && ids.contains(&b) && ids.contains(&cross));
    assert!(!ids.contains(&miss));
    assert_eq!(cad.document().entities().len(), 4);
}

#[test]
fn plan_aperture_keeps_offset_points_and_distant_coordinates() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(1e6 + 0.2, 2e6), p(1e6 + 0.3, 2e6)).unwrap();
    let origin = Point3::new(1e6, 2e6, 700.).unwrap();
    assert_eq!(
        cad.query_point_ray(origin, Vector3::Z, 0.25).unwrap(),
        vec![id]
    );
    assert!(
        cad.query_point_ray(origin, Vector3::Z, 0.1)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_rays_fail_and_an_empty_drawing_stays_empty() {
    let cad = CadDocument::new();
    assert!(
        cad.query_point_ray(Point3::ORIGIN, Vector3::ZERO, 1.)
            .is_err()
    );
    for radius in [-1., f64::NAN, f64::INFINITY] {
        assert!(
            cad.query_point_ray(Point3::ORIGIN, Vector3::Z, radius)
                .is_err()
        );
    }
    assert!(
        cad.query_point_ray(Point3::ORIGIN, Vector3::Z, 1.)
            .unwrap()
            .is_empty()
    );
}
