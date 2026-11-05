//! Tests of the primitives (a submodule of `primitives`).

use arq_geometry::{GeometryError, Point3};

use crate::brep::{Body, BrepError};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

#[test]
fn a_box_between_two_corners() {
    // 2 × 3 × 4 from (1, -2, 3), its corners given the other way round.
    let body = Body::cuboid(p(3.0, 1.0, 7.0), p(1.0, -2.0, 3.0)).unwrap();
    assert_eq!(
        (
            body.vertices().len(),
            body.edges().len(),
            body.faces().len()
        ),
        (8, 12, 6)
    );
    let m = body.mass_properties().unwrap();
    assert!((m.volume - 24.0).abs() < 1e-12 && (m.area - 52.0).abs() < 1e-12);
    let c = m.centroid;
    assert!(
        (c.x() - 2.0).abs() < 1e-12 && (c.y() + 0.5).abs() < 1e-12 && (c.z() - 5.0).abs() < 1e-12
    );
    // Each edge is used once each way: a closed body.
    for e in 0..body.edges().len() {
        let uses: Vec<bool> = body
            .coedges()
            .iter()
            .filter(|c| c.edge.index() == e)
            .map(|c| c.reversed)
            .collect();
        assert_eq!(uses.len(), 2, "edge {e}");
        assert_ne!(uses[0], uses[1], "edge {e}");
    }
}

#[test]
fn a_box_far_from_the_origin() {
    let body = Body::cuboid(p(5e5, 9.97e6, 0.0), p(5e5 + 0.5, 9.97e6 + 0.25, 3.0)).unwrap();
    let m = body.mass_properties().unwrap();
    assert!((m.volume - 0.375).abs() < 1e-9, "{}", m.volume);
}

#[test]
fn a_flat_box_is_refused() {
    for (a, b) in [
        (p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0)),
        (p(0.0, 0.0, 0.0), p(0.0, 1.0, 1.0)),
        (p(0.0, 2.0, 0.0), p(1.0, 2.0, 1.0)),
    ] {
        assert_eq!(
            Body::cuboid(a, b),
            Err(BrepError::Geometry(GeometryError::ZeroLength))
        );
    }
}
