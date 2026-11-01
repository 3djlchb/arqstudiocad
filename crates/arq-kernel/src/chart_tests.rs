//! Tests of the surfaces' charts (a submodule of `chart`): a point's
//! parameters lead back to it, the derivatives are the map's, and the
//! periods and poles are where the surfaces close and collapse.

use std::f64::consts::{FRAC_PI_2, TAU};

use arq_geometry::{Point3, Vector3};

use crate::curve::EllipseCurve;
use crate::nurbs::NurbsSurface;
use crate::surface::{Cone, EllipticCylinder, Plane, Sphere, Surface, Torus};

use super::Chart;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z).unwrap()
}

fn surfaces() -> Vec<Surface> {
    let tilted = v(1.0, -2.0, 0.5).normalized().unwrap();
    vec![
        Surface::Plane(Plane::new(p(1.0, 2.0, 3.0), tilted).unwrap()),
        Surface::Cone(Cone::new(p(1.0, 2.0, 3.0), tilted, 2.0, 0.6, 0.8).unwrap()),
        Surface::Cone(Cone::new(p(1.0, 2.0, 3.0), tilted, 2.0, 0.0, 1.0).unwrap()),
        Surface::EllipticCylinder(EllipticCylinder::new(
            EllipseCurve::new(p(1.0, 2.0, 3.0), v(0.0, 0.0, 1.0), v(3.0, 0.0, 0.0), 0.4).unwrap(),
        )),
        Surface::Sphere(Sphere::new(p(1.0, 2.0, 3.0), 2.5).unwrap()),
        Surface::Torus(Torus::new(p(1.0, 2.0, 3.0), tilted, 4.0, 1.5).unwrap()),
    ]
}

#[test]
fn a_points_parameters_lead_back_to_it() {
    for surface in surfaces() {
        let chart = Chart::new(&surface);
        for (u, v) in [(0.3, 0.2), (2.0, -0.7), (-2.5, 1.1), (3.0, 0.0)] {
            let at = chart.point(u, v);
            let (u2, v2) = chart.uv(at);
            let back = chart.point(u2, v2);
            assert!(back.distance_to(at) < 1e-9, "{surface:?} at ({u}, {v})");
        }
    }
}

#[test]
fn the_derivatives_are_the_maps() {
    let h = 1e-6;
    for surface in surfaces() {
        let chart = Chart::new(&surface);
        for (u, v) in [(0.3, 0.2), (2.0, -0.7), (-2.5, 1.1)] {
            let (su, sv) = chart.partials(u, v);
            let du = (chart.point(u + h, v) - chart.point(u - h, v)) / (2.0 * h);
            let dv = (chart.point(u, v + h) - chart.point(u, v - h)) / (2.0 * h);
            assert!((su - du).length() < 1e-6, "{surface:?}: ∂u at ({u}, {v})");
            assert!((sv - dv).length() < 1e-6, "{surface:?}: ∂v at ({u}, {v})");
        }
    }
}

#[test]
fn periods_and_poles() {
    let [plane, cone, cylinder, elliptic, sphere, torus] = surfaces().try_into().unwrap();
    assert_eq!(Chart::new(&plane).periods(), (None, None));
    assert_eq!(Chart::new(&cylinder).periods(), (Some(TAU), None));
    assert_eq!(Chart::new(&elliptic).periods(), (Some(TAU), None));
    assert_eq!(Chart::new(&torus).periods(), (Some(TAU), Some(TAU)));
    assert_eq!(Chart::new(&sphere).poles(), vec![-FRAC_PI_2, FRAC_PI_2]);
    assert!(Chart::new(&cylinder).poles().is_empty());
    // The apex: where the radius 2 + v·0.75 comes to nothing.
    let apex = Chart::new(&cone).poles();
    assert_eq!(apex.len(), 1);
    assert!((apex[0] + 2.0 / 0.75).abs() < 1e-12);
    let chart = Chart::new(&cone);
    assert!(
        chart.partials(0.4, apex[0]).0.length() < 1e-12,
        "it collapses there"
    );
}

/// A quarter turn of a cylinder of radius 1 as a NURBS surface, `u` along
/// the axis and `v` around it, written four times to close it.
fn closed_nurbs(swapped: bool) -> NurbsSurface {
    let w = 0.5f64.sqrt();
    // The circle's rational quadratic net, nine points for four quarters.
    let ring = [
        (1.0, 0.0, 1.0),
        (1.0, 1.0, w),
        (0.0, 1.0, 1.0),
        (-1.0, 1.0, w),
        (-1.0, 0.0, 1.0),
        (-1.0, -1.0, w),
        (0.0, -1.0, 1.0),
        (1.0, -1.0, w),
        (1.0, 0.0, 1.0),
    ];
    let around = vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0];
    let along = vec![0.0, 0.0, 1.0, 1.0];
    let mut points = Vec::new();
    let mut weights = Vec::new();
    if swapped {
        // u around, v along: rows by angle.
        for &(x, y, wt) in &ring {
            for z in [0.0, 2.0] {
                points.push(p(x, y, z));
                weights.push(wt);
            }
        }
        NurbsSurface::new(2, 1, around, along, 9, 2, points, weights).unwrap()
    } else {
        for z in [0.0, 2.0] {
            for &(x, y, wt) in &ring {
                points.push(p(x, y, z));
                weights.push(wt);
            }
        }
        NurbsSurface::new(1, 2, along, around, 2, 9, points, weights).unwrap()
    }
}

#[test]
fn a_nurbs_surface_closed_one_way_puts_that_way_first() {
    for swapped in [false, true] {
        let surface = Surface::Nurbs(closed_nurbs(swapped));
        let chart = Chart::new(&surface);
        // Closed around, whichever of its own parameters goes round.
        assert_eq!(chart.periods(), (Some(4.0), None), "swapped {swapped}");
        assert!(chart.poles().is_empty());
        let at = chart.point(1.5, 0.25);
        assert!((at.x().hypot(at.y()) - 1.0).abs() < 1e-12);
        assert!((at.z() - 0.5).abs() < 1e-12, "v runs along the axis");
        // u past the period wraps round.
        assert!(chart.point(5.5, 0.25).distance_to(at) < 1e-12);
    }
}
