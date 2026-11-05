//! Tests of the primitives (a submodule of `primitives`).

use arq_geometry::{GeometryError, Point3};

use super::{ExtrusionError, ProfileError, ProfilePiece};
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

#[track_caller]
fn about(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-10 * b.abs().max(1.0), "{a} != {b}");
}

#[test]
fn a_cylinder_on_its_base() {
    use std::f64::consts::PI;
    let (r, h) = (1.5, 2.0);
    let body = Body::cylinder(p(3.0, -1.0, 4.0), r, h).unwrap();
    // Two circles, each with its vertex; the ends and the side.
    assert_eq!(
        (
            body.vertices().len(),
            body.edges().len(),
            body.faces().len()
        ),
        (2, 2, 3)
    );
    let m = body.mass_properties().unwrap();
    about(m.volume, PI * r * r * h);
    about(m.area, 2.0 * PI * r * (r + h));
    about(m.centroid.z(), 5.0);
    about(m.bounds.max().x(), 4.5);
}

#[test]
fn a_cone_to_a_point_and_a_frustum() {
    use std::f64::consts::PI;
    let (r, h) = (3.0, 4.0);
    let cone = Body::cone(p(0.0, 0.0, 0.0), r, 0.0, h).unwrap();
    // The base circle and the apex, each an edge with its vertex.
    assert_eq!((cone.edges().len(), cone.faces().len()), (2, 2));
    assert!(cone.edges().iter().any(|e| e.curve.is_none()), "the apex");
    let m = cone.mass_properties().unwrap();
    about(m.volume, PI * r * r * h / 3.0);
    // The base, and the side: π r times the slant height, 5.
    about(m.area, PI * r * r + PI * r * 5.0);
    about(m.centroid.z(), h / 4.0);
    let (big, small) = (3.0, 1.0);
    let frustum = Body::cone(p(0.0, 0.0, 0.0), big, small, h).unwrap();
    let m = frustum.mass_properties().unwrap();
    about(
        m.volume,
        PI * h / 3.0 * (big * big + big * small + small * small),
    );
    about(m.bounds.max().z(), h);
}

#[test]
fn a_cone_down_or_upside_down() {
    use std::f64::consts::PI;
    // Downward: below its base, the same cone turned over.
    let down = Body::cone(p(0.0, 0.0, 0.0), 3.0, 0.0, -4.0).unwrap();
    let m = down.mass_properties().unwrap();
    about(m.volume, PI * 12.0);
    about(m.centroid.z(), -1.0);
    // A point at its base, widening up to its top.
    let inverted = Body::cone(p(0.0, 0.0, 0.0), 0.0, 3.0, 4.0).unwrap();
    let m = inverted.mass_properties().unwrap();
    about(m.volume, PI * 12.0);
    about(m.centroid.z(), 3.0);
    // A cylinder downward, and one wider at the top than the base.
    let m = Body::cylinder(p(0.0, 0.0, 1.0), 1.0, -2.0)
        .unwrap()
        .mass_properties()
        .unwrap();
    about(m.volume, PI * 2.0);
    about(m.centroid.z(), 0.0);
    let m = Body::cone(p(0.0, 0.0, 0.0), 1.0, 2.0, 3.0)
        .unwrap()
        .mass_properties()
        .unwrap();
    about(m.volume, PI * 3.0 / 3.0 * (1.0 + 2.0 + 4.0));
}

#[test]
fn impossible_cones_are_refused() {
    let origin = p(0.0, 0.0, 0.0);
    for (base, top, height, why) in [
        (0.0, 0.0, 1.0, GeometryError::InvalidRadius),
        (-1.0, 0.0, 1.0, GeometryError::InvalidRadius),
        (1.0, f64::NAN, 1.0, GeometryError::InvalidRadius),
        (1.0, 0.0, 0.0, GeometryError::ZeroLength),
        (1.0, 1.0, f64::INFINITY, GeometryError::ZeroLength),
    ] {
        assert_eq!(
            Body::cone(origin, base, top, height),
            Err(BrepError::Geometry(why))
        );
    }
}

#[test]
fn a_sphere_and_a_torus() {
    use std::f64::consts::PI;
    let ball = Body::sphere(p(1.0, 2.0, 3.0), 2.0).unwrap();
    assert_eq!(
        (ball.faces().len(), ball.edges().len(), ball.loops().len()),
        (1, 0, 0)
    );
    let m = ball.mass_properties().unwrap();
    about(m.volume, 4.0 / 3.0 * PI * 8.0);
    about(m.area, 4.0 * PI * 4.0);
    about(m.centroid.z(), 3.0);
    about(m.bounds.max().x(), 3.0);
    // A ring 3 round with a tube 1 thick: 2π²Rr² and 4π²Rr.
    let ring = Body::torus(p(0.0, 0.0, 5.0), 3.0, 1.0).unwrap();
    let m = ring.mass_properties().unwrap();
    about(m.volume, 2.0 * PI * PI * 3.0);
    about(m.area, 4.0 * PI * PI * 3.0);
    about(m.centroid.z(), 5.0);
    about(m.bounds.max().x(), 4.0);
    about(m.bounds.max().z(), 6.0);
}

#[test]
fn impossible_spheres_and_tori_are_refused() {
    let at = p(0.0, 0.0, 0.0);
    let invalid = Err(BrepError::Geometry(GeometryError::InvalidRadius));
    assert_eq!(Body::sphere(at, 0.0), invalid);
    assert_eq!(Body::sphere(at, f64::NAN), invalid);
    // A tube as thick as the ring is wide, or thicker, crosses itself.
    for (radius, tube) in [
        (1.0, 1.0),
        (1.0, 2.0),
        (3.0, 0.0),
        (3.0, -1.0),
        (f64::INFINITY, 1.0),
    ] {
        assert_eq!(Body::torus(at, radius, tube), invalid, "{radius} {tube}");
    }
}

fn q(x: f64, y: f64) -> arq_geometry::Point2 {
    arq_geometry::Point2::new(x, y).unwrap()
}

fn segment(a: (f64, f64), b: (f64, f64)) -> ProfilePiece {
    let line = arq_geometry::Line::new(q(a.0, a.1), q(b.0, b.1)).unwrap();
    ProfilePiece {
        curve: arq_geometry::Geometry2::Line(line),
        backward: false,
    }
}

fn arc(center: (f64, f64), radius: f64, from: f64, to: f64, backward: bool) -> ProfilePiece {
    let a = arq_geometry::Arc::new(q(center.0, center.1), radius, from, to).unwrap();
    ProfilePiece {
        curve: arq_geometry::Geometry2::Arc(a),
        backward,
    }
}

fn rectangle(w: f64, h: f64) -> Vec<ProfilePiece> {
    vec![
        segment((0.0, 0.0), (w, 0.0)),
        segment((w, 0.0), (w, h)),
        segment((w, h), (0.0, h)),
        segment((0.0, h), (0.0, 0.0)),
    ]
}

#[test]
fn a_rectangle_extruded_is_a_box() {
    let body = Body::extrusion(&rectangle(4.0, 2.0), 1.0, 3.0).unwrap();
    assert_eq!(
        (
            body.faces().len(),
            body.edges().len(),
            body.vertices().len()
        ),
        (6, 12, 8)
    );
    let m = body.mass_properties().unwrap();
    about(m.volume, 24.0);
    about(m.area, 2.0 * 8.0 + 2.0 * 12.0 + 2.0 * 6.0);
    about(m.centroid.z(), 2.5);
    // Drawn clockwise, or extruded downward: the same box, outward.
    let clockwise: Vec<_> = rectangle(4.0, 2.0)
        .into_iter()
        .rev()
        .map(|p| ProfilePiece {
            backward: !p.backward,
            ..p
        })
        .collect();
    about(
        Body::extrusion(&clockwise, 1.0, 3.0)
            .unwrap()
            .mass_properties()
            .unwrap()
            .volume,
        24.0,
    );
    let down = Body::extrusion(&rectangle(4.0, 2.0), 1.0, -3.0)
        .unwrap()
        .mass_properties()
        .unwrap();
    about(down.volume, 24.0);
    about(down.centroid.z(), -0.5);
}

#[test]
fn a_slot_has_round_ends() {
    use std::f64::consts::{FRAC_PI_2, PI};
    // Four long and two wide, its ends half circles.
    let slot = vec![
        segment((0.0, -1.0), (4.0, -1.0)),
        arc((4.0, 0.0), 1.0, -FRAC_PI_2, FRAC_PI_2, false),
        segment((4.0, 1.0), (0.0, 1.0)),
        arc((0.0, 0.0), 1.0, FRAC_PI_2, 3.0 * FRAC_PI_2, false),
    ];
    let body = Body::extrusion(&slot, 0.0, 2.0).unwrap();
    assert_eq!(body.faces().len(), 6);
    let m = body.mass_properties().unwrap();
    about(m.volume, (8.0 + PI) * 2.0);
    about(m.area, 2.0 * (8.0 + PI) + 2.0 * (8.0 + 2.0 * PI));
    about(m.bounds.min().x(), -1.0);
    about(m.bounds.max().x(), 5.0);
}

#[test]
fn a_concave_arc_cuts_into_the_profile() {
    use std::f64::consts::PI;
    // A 4 square whose top dips: an arc round (2, 6) from (4, 4) to (0, 4),
    // run clockwise; it takes away a segment of 2π - 4.
    let r = 8.0f64.sqrt();
    let dipped = vec![
        segment((0.0, 0.0), (4.0, 0.0)),
        segment((4.0, 0.0), (4.0, 4.0)),
        arc((2.0, 6.0), r, 1.25 * PI, 1.75 * PI, true),
        segment((0.0, 4.0), (0.0, 0.0)),
    ];
    let m = Body::extrusion(&dipped, 0.0, 1.0)
        .unwrap()
        .mass_properties()
        .unwrap();
    about(m.volume, 20.0 - 2.0 * PI);
    about(m.bounds.max().y(), 4.0);
}

#[test]
fn profiles_that_make_no_solid_are_refused() {
    let profile = |e| Err(ExtrusionError::Profile(e));
    let mut open = rectangle(4.0, 2.0);
    open.pop();
    assert_eq!(
        Body::extrusion(&open, 0.0, 1.0),
        profile(ProfileError::Open)
    );
    assert_eq!(
        Body::extrusion(&open[..1], 0.0, 1.0),
        profile(ProfileError::Open)
    );
    // A bow tie crosses itself in its middle.
    let bow = vec![
        segment((0.0, 0.0), (2.0, 2.0)),
        segment((2.0, 2.0), (2.0, 0.0)),
        segment((2.0, 0.0), (0.0, 2.0)),
        segment((0.0, 2.0), (0.0, 0.0)),
    ];
    assert_eq!(
        Body::extrusion(&bow, 0.0, 1.0),
        profile(ProfileError::Crossing)
    );
    // Out and back along one line: no area, and its pieces overlap.
    let back = vec![
        segment((0.0, 0.0), (2.0, 0.0)),
        segment((2.0, 0.0), (0.0, 0.0)),
    ];
    assert_eq!(
        Body::extrusion(&back, 0.0, 1.0),
        profile(ProfileError::Crossing)
    );
    assert_eq!(
        Body::extrusion(&rectangle(1.0, 1.0), 0.0, 0.0),
        Err(ExtrusionError::Geometry(BrepError::Geometry(
            GeometryError::ZeroLength
        )))
    );
    assert_eq!(
        ProfileError::Crossing.to_string(),
        "the profile crosses itself"
    );
}

#[test]
fn two_half_circles_make_a_round_profile_either_way() {
    use std::f64::consts::PI;
    // A circle drawn as two half circles: its chords enclose nothing, its
    // arcs all of it; clockwise too.
    let ccw = vec![
        arc((0.0, 0.0), 2.0, 0.0, PI, false),
        arc((0.0, 0.0), 2.0, PI, 0.0, false),
    ];
    let cw = vec![
        arc((0.0, 0.0), 2.0, 0.0, PI, true),
        arc((0.0, 0.0), 2.0, PI, 0.0, true),
    ];
    for profile in [ccw, cw] {
        let m = Body::extrusion(&profile, 0.0, 3.0)
            .unwrap()
            .mass_properties()
            .unwrap();
        about(m.volume, 12.0 * PI);
    }
}
