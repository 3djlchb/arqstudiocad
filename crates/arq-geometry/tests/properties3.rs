//! Property tests for the 3D geometric foundation.

use arq_geometry::{Point3, Tolerance, Transform3, Vector3};
use proptest::prelude::*;

fn point3() -> impl Strategy<Value = Point3> {
    (
        -1_000_000.0..1_000_000.0,
        -1_000_000.0..1_000_000.0,
        -10_000.0..10_000.0,
    )
        .prop_map(|(x, y, z)| Point3::new(x, y, z).unwrap())
}

fn vector3() -> impl Strategy<Value = Vector3> {
    (-1000.0..1000.0, -1000.0..1000.0, -1000.0..1000.0)
        .prop_map(|(x, y, z)| Vector3::new(x, y, z).unwrap())
}

fn close3(a: Point3, b: Point3, magnitude: f64) -> bool {
    let tolerance = 1e-10 * (1.0 + magnitude);
    (a.x() - b.x()).abs() <= tolerance
        && (a.y() - b.y()).abs() <= tolerance
        && (a.z() - b.z()).abs() <= tolerance
}

proptest! {
    #[test]
    fn point_plus_vector_roundtrips(point in point3(), vector in vector3()) {
        let magnitude = point.to_vector().length() + vector.length();
        prop_assert!(close3((point + vector) - vector, point, magnitude));
        prop_assert!(((point + vector) - point).approx_eq(vector, Tolerance::new(1e-8, 1e-10).unwrap()));
    }

    #[test]
    fn cross_product_is_orthogonal(a in vector3(), b in vector3()) {
        let cross = a.cross(b);
        let scale = 1.0 + a.length() * b.length();
        prop_assert!(cross.dot(a).abs() <= 1e-10 * scale);
        prop_assert!(cross.dot(b).abs() <= 1e-10 * scale);
    }

    #[test]
    fn affine_transform_roundtrips_at_large_coordinates(
        point in point3(),
        pivot in point3(),
        axis in vector3(),
        angle in -6.0..6.0f64,
        sx in 0.2..4.0f64,
        sy in 0.2..4.0f64,
        sz in 0.2..4.0f64,
    ) {
        prop_assume!(axis.length() > 1e-8);
        let rotate = Transform3::rotation_about(pivot, axis, angle).unwrap();
        let scale = Transform3::scaling(pivot, Vector3::new(sx, sy, sz).unwrap()).unwrap();
        let transform = rotate.then(scale);
        let recovered = transform.inverse().unwrap().apply_point(transform.apply_point(point));
        let magnitude = point.to_vector().length() + pivot.to_vector().length();
        prop_assert!(close3(recovered, point, magnitude));
    }
}
