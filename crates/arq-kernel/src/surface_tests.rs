//! Tests of the faces' surfaces (a submodule of `surface`). Distances are
//! worked out by hand.

use arq_geometry::{Point3, Transform3, Vector3};

use super::*;
use crate::curve::EllipseCurve;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn distance_to_each_surface() {
    let plane = Plane::new(p(0.0, 0.0, 2.0), v(0.0, 0.0, 3.0)).unwrap();
    assert!(close(plane.distance(p(5.0, 5.0, -1.0)), 3.0));
    // A cylinder of radius 2 along z.
    let cylinder = Cone::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), 2.0, 0.0, 1.0).unwrap();
    assert!(cylinder.is_cylinder());
    assert!(close(cylinder.distance(p(3.0, 4.0, 9.0)), 3.0));
    // A cone of radius 1 at z = 0, half angle 45°: radius 1 + z.
    let (s, c) = (
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    );
    let cone = Cone::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), 1.0, s, c).unwrap();
    assert!(
        close(cone.distance(p(3.0, 0.0, 2.0)), 0.0),
        "radius 3 at z = 2"
    );
    assert!(
        close(cone.distance(p(5.0, 0.0, 2.0)), 2.0 * c),
        "across the side"
    );
    let sphere = Sphere::new(p(1.0, 1.0, 1.0), 2.0).unwrap();
    assert!(close(sphere.distance(p(1.0, 1.0, 6.0)), 3.0));
    // A torus of major radius 3 and minor 1 around z.
    let torus = Torus::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), 3.0, 1.0).unwrap();
    assert!(close(torus.distance(p(3.0, 0.0, 1.0)), 0.0), "on the top");
    assert!(close(torus.distance(p(0.0, 6.0, 0.0)), 2.0));
    // An elliptic cylinder along z, section semi-axes 4 (x) and 2 (y).
    let section =
        EllipseCurve::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), v(4.0, 0.0, 0.0), 0.5).unwrap();
    let elliptic = EllipticCylinder::new(section);
    assert!(
        close(elliptic.distance(p(0.0, 5.0, 8.0)), 3.0),
        "height does not count"
    );
}

#[test]
fn surfaces_are_checked() {
    let (o, z) = (p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
    assert!(Plane::new(o, Vector3::ZERO).is_err());
    assert!(Cone::new(o, z, -1.0, 0.0, 1.0).is_err(), "negative radius");
    assert!(
        Cone::new(o, z, 1.0, 0.5, 0.5).is_err(),
        "not a sine and cosine"
    );
    assert!(
        Cone::new(o, z, 1.0, 0.0, -1.0).is_err(),
        "cosine not positive"
    );
    assert!(Sphere::new(o, 0.0).is_err());
    assert!(Torus::new(o, z, 3.0, 0.0).is_err());
}

#[test]
fn surfaces_move_with_a_transform() {
    let t = Transform3::translation(v(0.0, 0.0, 5.0));
    let sphere = Surface::Sphere(Sphere::new(p(0.0, 0.0, 0.0), 1.0).unwrap());
    assert!(close(
        sphere
            .transformed(t, 1.0)
            .unwrap()
            .distance(p(0.0, 0.0, 6.0)),
        0.0
    ));
    let doubled = Transform3::scaling(p(0.0, 0.0, 0.0), v(2.0, 2.0, 2.0)).unwrap();
    let torus = Surface::Torus(Torus::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), 3.0, 1.0).unwrap());
    let big = torus.transformed(doubled, 2.0).unwrap();
    assert!(close(big.distance(p(8.0, 0.0, 0.0)), 0.0), "radii 6 and 2");
}
