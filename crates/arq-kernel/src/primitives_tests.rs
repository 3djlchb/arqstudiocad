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

#[test]
fn a_wedge_stands_at_its_first_corner() {
    // 4 long in X, 2 wide, 3 high: half the box, upright at x 0.
    let body = Body::wedge(p(0.0, 0.0, 0.0), p(4.0, 2.0, 3.0)).unwrap();
    assert_eq!(
        (
            body.vertices().len(),
            body.edges().len(),
            body.faces().len()
        ),
        (6, 9, 5)
    );
    let m = body.mass_properties().unwrap();
    assert!((m.volume - 12.0).abs() < 1e-12, "{}", m.volume);
    // A right triangle's centroid sits a third of the way in from its legs.
    let c = m.centroid;
    assert!(
        (c.x() - 4.0 / 3.0).abs() < 1e-12
            && (c.y() - 1.0).abs() < 1e-12
            && (c.z() - 1.0).abs() < 1e-12
    );
    // The base 4 x 2, the upright face 2 x 3, the slope 5 x 2 (its legs 4
    // and 3), and two right triangles 4 x 3 / 2.
    assert!((m.area - 36.0).abs() < 1e-12, "{}", m.area);
}

#[test]
fn a_wedge_runs_any_way_round() {
    // Upright at x 4 instead, sloping toward x 0; then down, then across.
    for (first, opposite, centroid) in [
        (p(4.0, 0.0, 0.0), p(0.0, 2.0, 3.0), (8.0 / 3.0, 1.0, 1.0)),
        (p(0.0, 0.0, 0.0), p(4.0, 2.0, -3.0), (4.0 / 3.0, 1.0, -1.0)),
        (p(0.0, 2.0, 0.0), p(4.0, 0.0, 3.0), (4.0 / 3.0, 1.0, 1.0)),
        (p(4.0, 2.0, 3.0), p(0.0, 0.0, 0.0), (8.0 / 3.0, 1.0, 2.0)),
    ] {
        let m = Body::wedge(first, opposite)
            .unwrap()
            .mass_properties()
            .unwrap();
        assert!((m.volume - 12.0).abs() < 1e-12, "{first:?}: {}", m.volume);
        let c = m.centroid;
        let (x, y, z) = centroid;
        assert!(
            (c.x() - x).abs() < 1e-12 && (c.y() - y).abs() < 1e-12 && (c.z() - z).abs() < 1e-12,
            "{first:?}: {c:?}"
        );
    }
}

#[test]
fn a_flat_wedge_is_refused() {
    for opposite in [p(0.0, 2.0, 3.0), p(4.0, 0.0, 3.0), p(4.0, 2.0, 0.0)] {
        assert_eq!(
            Body::wedge(p(0.0, 0.0, 0.0), opposite),
            Err(BrepError::Geometry(GeometryError::ZeroLength))
        );
    }
}
