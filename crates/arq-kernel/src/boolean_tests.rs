//! Tests of the booleans of planar solids (a submodule of `boolean`).

use std::f64::consts::PI;

use arq_geometry::{Geometry2, Line, Point2, Point3, Transform3, Vector3};

use super::BooleanError;
use crate::brep::Body;
use crate::primitives::ProfilePiece;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn cuboid(a: (f64, f64, f64), b: (f64, f64, f64)) -> Body {
    Body::cuboid(p(a.0, a.1, a.2), p(b.0, b.1, b.2)).unwrap()
}

fn volume(body: &Body) -> f64 {
    body.mass_properties().unwrap().volume
}

/// The volume of a body of flat faces by the divergence theorem, from
/// its loops alone: quick where the general measure integrates (it takes
/// seconds on millimetre bodies).
fn flat_volume(body: &Body) -> f64 {
    let mut total = 0.0;
    for lp in body.loops() {
        let corners: Vec<Vector3> = lp
            .coedges
            .iter()
            .map(|c| {
                let c = &body.coedges()[c.index()];
                let e = body.edge(c.edge);
                body.vertex(if c.reversed { e.end } else { e.start }).point - Point3::ORIGIN
            })
            .collect();
        for k in 1..corners.len() - 1 {
            total += corners[0].dot(corners[k].cross(corners[k + 1])) / 6.0;
        }
    }
    total
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

/// Each edge used once each way and a positive volume: a closed body.
fn assert_closed(body: &Body) {
    let mut uses = std::collections::HashMap::new();
    for c in body.coedges() {
        *uses.entry((c.edge, c.reversed)).or_insert(0) += 1;
    }
    for e in 0..body.edges().len() {
        let id = body
            .coedges()
            .iter()
            .find(|c| c.edge.index() == e)
            .unwrap()
            .edge;
        assert_eq!(
            (uses.get(&(id, false)), uses.get(&(id, true))),
            (Some(&1), Some(&1)),
            "edge {e}"
        );
    }
    assert!(volume(body) > 0.0);
}

/// A box turned about an axis through its corner and moved.
fn turned(body: &Body, axis: (f64, f64, f64), angle: f64, by: (f64, f64, f64)) -> Body {
    let t = Transform3::rotation(Vector3::new(axis.0, axis.1, axis.2).unwrap(), angle)
        .unwrap()
        .then(Transform3::translation(
            Vector3::new(by.0, by.1, by.2).unwrap(),
        ));
    body.transformed(t, 1.0).unwrap()
}

#[test]
fn two_boxes_overlapping_at_a_corner() {
    let (a, b) = (
        cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        cuboid((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)),
    );
    let union = a.union(&b).unwrap();
    assert_closed(&union);
    assert!(close(volume(&union), 15.0));
    assert_eq!((union.lumps().len(), union.faces().len()), (1, 12));
    let both = a.intersection(&b).unwrap().unwrap();
    assert_closed(&both);
    assert!(close(volume(&both), 1.0));
    assert_eq!((both.faces().len(), both.vertices().len()), (6, 8));
    let rest = a.difference(&b).unwrap().unwrap();
    assert_closed(&rest);
    assert!(close(volume(&rest), 7.0) && close(flat_volume(&rest), 7.0));
    let m = rest.mass_properties().unwrap();
    // Seven unit cubes of eight: the centroid moves away from the missing one.
    let c = 1.0 - 0.5 / 7.0;
    assert!(close(m.centroid.x(), c) && close(m.centroid.y(), c) && close(m.centroid.z(), c));
    assert!(close(m.area, 24.0));
}

#[test]
fn boxes_apart() {
    let (a, b) = (
        cuboid((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
        cuboid((3.0, 0.5, 0.5), (4.0, 2.0, 2.0)),
    );
    let union = a.union(&b).unwrap();
    assert_eq!(union.lumps().len(), 2);
    assert!(close(volume(&union), 1.0 + 2.25));
    assert_eq!(a.intersection(&b).unwrap(), None);
    assert!(close(volume(&a.difference(&b).unwrap().unwrap()), 1.0));
}

#[test]
fn a_box_inside_another_leaves_a_void() {
    let (big, small) = (
        cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 4.0)),
        cuboid((1.0, 1.5, 1.25), (2.0, 2.5, 3.0)),
    );
    let hollow = big.difference(&small).unwrap().unwrap();
    assert_eq!(hollow.lumps().len(), 1);
    assert_eq!(hollow.lumps()[0].shells.len(), 2);
    assert!(close(volume(&hollow), 64.0 - 1.75));
    assert!(close(volume(&big.union(&small).unwrap()), 64.0));
    assert!(close(
        volume(&big.intersection(&small).unwrap().unwrap()),
        1.75
    ));
    assert_eq!(small.difference(&big).unwrap(), None);
}

#[test]
fn a_bar_through_a_slab_bores_a_hole() {
    let slab = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let bar = cuboid((1.0, 1.0, -1.0), (2.0, 3.0, 2.0));
    let holed = slab.difference(&bar).unwrap().unwrap();
    assert_closed(&holed);
    assert!(close(volume(&holed), 16.0 - 2.0));
    // The top and the bottom each have a hole: two loops.
    assert_eq!(
        holed.faces().iter().filter(|f| f.loops.len() == 2).count(),
        2
    );
    let crossed = slab.union(&bar).unwrap();
    assert!(close(volume(&crossed), 16.0 + 3.0 * 2.0 - 2.0));
    assert!(close(
        volume(&slab.intersection(&bar).unwrap().unwrap()),
        2.0
    ));
}

#[test]
fn a_pocket_in_the_top() {
    let slab = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let cutter = cuboid((1.0, 1.0, 0.25), (3.0, 2.0, 2.0));
    let pocketed = slab.difference(&cutter).unwrap().unwrap();
    assert_closed(&pocketed);
    assert!(close(volume(&pocketed), 16.0 - 2.0 * 0.75));
    // The cutter's bottom, turned over, is the pocket's floor.
    let floor = pocketed
        .faces()
        .iter()
        .filter(|f| match &f.surface {
            crate::surface::Surface::Plane(pl) => pl.normal().z() > 0.5,
            _ => false,
        })
        .count();
    assert_eq!(floor, 2);
}

#[test]
fn turned_boxes_in_general_position() {
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let b = turned(
        &cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
        (1.0, 2.0, 3.0),
        0.7,
        (0.9, 0.8, 1.1),
    );
    let union = a.union(&b).unwrap();
    let both = a.intersection(&b).unwrap().unwrap();
    let rest = a.difference(&b).unwrap().unwrap();
    let other = b.difference(&a).unwrap().unwrap();
    for body in [&union, &both, &rest, &other] {
        assert_closed(body);
    }
    let (vu, vi, vr, vo) = (volume(&union), volume(&both), volume(&rest), volume(&other));
    assert!(vi > 0.1 && vi < 8.0);
    assert!(close(vu, 16.0 - vi), "{vu} {vi}");
    assert!(close(vr, 8.0 - vi) && close(vo, 8.0 - vi));
}

#[test]
fn an_l_shaped_wall_against_a_turned_post() {
    // An L in plan, extruded: a solid that is not convex.
    let corners = [
        (0.0, 0.0),
        (5.0, 0.0),
        (5.0, 1.0),
        (1.0, 1.0),
        (1.0, 4.0),
        (0.0, 4.0),
    ];
    let pieces: Vec<ProfilePiece> = (0..corners.len())
        .map(|k| {
            let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
            let line = Line::new(
                Point2::new(a.0, a.1).unwrap(),
                Point2::new(b.0, b.1).unwrap(),
            );
            ProfilePiece {
                curve: Geometry2::Line(line.unwrap()),
                backward: false,
            }
        })
        .collect();
    let wall = Body::extrusion(&pieces, 0.0, 3.0).unwrap();
    assert!(close(volume(&wall), 8.0 * 3.0));
    let post = turned(
        &cuboid((0.0, 0.0, -1.0), (1.5, 1.5, 4.0)),
        (0.0, 0.0, 1.0),
        PI / 7.0,
        (0.3, 0.2, 0.0),
    );
    let both = wall.intersection(&post).unwrap().unwrap();
    let union = wall.union(&post).unwrap();
    let rest = wall.difference(&post).unwrap().unwrap();
    for body in [&both, &union, &rest] {
        assert_closed(body);
    }
    let (vi, vp) = (volume(&both), volume(&post));
    assert!(close(volume(&union), 24.0 + vp - vi));
    assert!(close(volume(&rest), 24.0 - vi));
}

#[test]
fn millimetre_walls_crossing_in_general_position() {
    // Walls 200 mm thick, one 3 m tall and one 2.75 m, crossing askew.
    let a = cuboid((0.0, 0.0, 0.0), (6000.0, 200.0, 3000.0));
    let b = turned(
        &cuboid((0.0, -100.0, -50.0), (5000.0, 100.0, 2700.0)),
        (0.0, 0.0, 1.0),
        1.1,
        (2500.0, -1500.0, 0.0),
    );
    let union = a.union(&b).unwrap();
    let both = a.intersection(&b).unwrap().unwrap();
    let rest = a.difference(&b).unwrap().unwrap();
    let (va, vb, vi) = (flat_volume(&a), flat_volume(&b), flat_volume(&both));
    assert!(close(va, 6000.0 * 200.0 * 3000.0) && close(vb, 5000.0 * 200.0 * 2750.0));
    assert!(vi > 0.0);
    assert!(close(flat_volume(&union), va + vb - vi));
    assert!(close(flat_volume(&rest), va - vi));
    // The cross wall is cut through: the long wall loses a piece but
    // stays one, the union is one.
    assert_eq!((union.lumps().len(), rest.lumps().len()), (1, 1));
}

#[test]
fn far_from_the_origin() {
    // Survey coordinates: the tolerance scales and the frames are local.
    let o = (612_345.0, 4_123_456.0, 100.0);
    let a = cuboid(o, (o.0 + 2.0, o.1 + 2.0, o.2 + 2.0));
    let b = cuboid(
        (o.0 + 1.0, o.1 + 1.0, o.2 + 1.0),
        (o.0 + 3.0, o.1 + 3.0, o.2 + 3.0),
    );
    let both = a.intersection(&b).unwrap().unwrap();
    assert!((volume(&both) - 1.0).abs() < 1e-6);
    assert!((volume(&a.union(&b).unwrap()) - 15.0).abs() < 1e-5);
}

#[test]
fn curved_solids_are_refused() {
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let c = Body::cylinder(p(1.0, 1.0, -1.0), 0.5, 4.0).unwrap();
    assert_eq!(a.difference(&c), Err(BooleanError::Curved));
    assert_eq!(c.union(&a), Err(BooleanError::Curved));
    // A whole sphere: a curved face without loops or edges.
    let ball = Body::sphere(p(1.0, 1.0, 1.0), 0.5).unwrap();
    assert_eq!(a.intersection(&ball), Err(BooleanError::Curved));
}

#[test]
fn coplanar_faces_wait_for_their_own_step() {
    // Two boxes on the same floor, overlapping: a shared plane.
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let b = cuboid((1.0, 1.0, 0.0), (3.0, 3.0, 3.0));
    assert_eq!(a.union(&b), Err(BooleanError::Coplanar));
    // The same floor far apart is no meeting: two lumps.
    let c = cuboid((5.0, 5.0, 0.0), (6.0, 6.0, 1.0));
    assert_eq!(a.union(&c).unwrap().lumps().len(), 2);
}

#[test]
fn edges_lying_on_a_face_plane() {
    // A square post turned 45° in plan, its two side edges exactly in the
    // plane of the box's face x = 2 (corners on a plane: the symbolic
    // perturbation), half of it inside the box.
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let r: f64 = 0.6;
    let side = r * 2.0_f64.sqrt();
    let post = turned(
        &cuboid((0.0, 0.0, 0.5), (side, side, 1.5)),
        (0.0, 0.0, 1.0),
        PI / 4.0,
        (2.0, 1.0 - r, 0.0),
    );
    assert!(close(volume(&post), 2.0 * r * r));
    let union = a.union(&post).unwrap();
    assert_closed(&union);
    assert!(close(volume(&union), 8.0 + r * r));
    let both = a.intersection(&post).unwrap().unwrap();
    assert!(close(volume(&both), r * r));
    let rest = a.difference(&post).unwrap().unwrap();
    assert!(close(volume(&rest), 8.0 - r * r));
    // The face x = 2 keeps a rectangular hole where the post passes.
    assert_eq!(
        rest.faces().iter().filter(|f| f.loops.len() == 2).count(),
        1
    );
}

#[test]
fn a_corner_on_a_straight_run_of_an_edge() {
    // A slab whose front edge has a corner halfway (as earlier booleans
    // leave them): the top cut by a turned box keeps the edge whole with
    // the front faces, which still end at that corner.
    let corners = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)];
    let pieces: Vec<ProfilePiece> = (0..corners.len())
        .map(|k| {
            let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
            let line = Line::new(
                Point2::new(a.0, a.1).unwrap(),
                Point2::new(b.0, b.1).unwrap(),
            );
            ProfilePiece {
                curve: Geometry2::Line(line.unwrap()),
                backward: false,
            }
        })
        .collect();
    let slab = Body::extrusion(&pieces, 0.0, 1.0).unwrap();
    let cutter = turned(
        &cuboid((0.0, 0.0, 0.0), (0.5, 0.5, 0.5)),
        (1.0, 1.0, 0.0),
        0.4,
        (0.8, 0.4, 0.8),
    );
    let both = slab.intersection(&cutter).unwrap().unwrap();
    let rest = slab.difference(&cutter).unwrap().unwrap();
    assert_closed(&rest);
    assert!(close(volume(&rest), 2.0 - volume(&both)));
}

#[test]
fn solids_touching_along_an_edge_wait_for_their_own_step() {
    // A bar turned 45° about X, its lowest edge on the box's top back edge.
    let a = cuboid((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
    let bar = turned(
        &cuboid((-0.5, 0.0, 0.0), (1.5, 0.5, 0.5)),
        (1.0, 0.0, 0.0),
        PI / 4.0,
        (0.0, 1.0, 1.0),
    );
    assert_eq!(a.union(&bar), Err(BooleanError::NonManifold));
}

#[test]
fn a_void_goes_to_the_smallest_piece_around_it() {
    // A hollow box, and in its void a smaller hollow box: two pieces, each
    // with its own void.
    let outer = cuboid((0.0, 0.0, 0.0), (10.0, 10.0, 10.0))
        .difference(&cuboid((1.0, 1.0, 1.0), (9.0, 9.0, 9.0)))
        .unwrap()
        .unwrap();
    let inner = cuboid((3.0, 3.0, 3.0), (7.0, 7.0, 7.0))
        .difference(&cuboid((4.0, 4.0, 4.0), (6.0, 6.0, 6.0)))
        .unwrap()
        .unwrap();
    let both = outer.union(&inner).unwrap();
    assert_eq!(both.lumps().len(), 2);
    assert!(both.lumps().iter().all(|l| l.shells.len() == 2));
    assert!(close(volume(&both), 1000.0 - 512.0 + 64.0 - 8.0));
}

/// The body with every face's outside turned in; with `loops` too, its
/// loops reversed as well, so it is consistently inside out.
fn inside_out(body: &Body, loops: bool) -> Body {
    let mut b = crate::brep::BodyBuilder::new();
    for v in body.vertices() {
        b.vertex(v.point);
    }
    for e in body.edges() {
        b.edge(e.clone());
    }
    for c in body.coedges() {
        let mut c = *c;
        c.reversed ^= loops;
        b.coedge(c);
    }
    for l in body.loops() {
        let mut l = l.clone();
        if loops {
            l.coedges.reverse();
        }
        b.add_loop(l);
    }
    for f in body.faces() {
        let mut f = f.clone();
        f.reversed = !f.reversed;
        b.face(f);
    }
    for s in body.shells() {
        b.shell(s.clone());
    }
    for l in body.lumps() {
        b.lump(l.clone());
    }
    b.build(arq_geometry::Tolerance::DEFAULT).unwrap()
}

#[test]
fn bodies_enclosing_no_volume_are_refused() {
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let b = cuboid((1.0, 1.0, 1.0), (3.0, 3.0, 3.0));
    // Faces turned against their loops.
    assert_eq!(a.union(&inside_out(&b, false)), Err(BooleanError::NotSolid));
    // Consistently inside out: a negative volume.
    assert_eq!(inside_out(&a, true).union(&b), Err(BooleanError::NotSolid));
    // A flat face without loops.
    let mut builder = crate::brep::BodyBuilder::new();
    let plane = crate::surface::Plane::new(p(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0).unwrap());
    let face = builder.face(crate::brep::Face {
        surface: crate::surface::Surface::Plane(plane.unwrap()),
        loops: Vec::new(),
        reversed: false,
    });
    let shell = builder.shell(crate::brep::Shell { faces: vec![face] });
    builder.lump(crate::brep::Lump {
        shells: vec![shell],
    });
    let sheet = builder.build(arq_geometry::Tolerance::DEFAULT).unwrap();
    assert_eq!(a.union(&sheet), Err(BooleanError::NotSolid));
}

/// A small deterministic generator, for repeatable random solids.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.next()
    }
}

#[test]
fn the_volumes_add_up_for_random_turned_boxes_and_wedges() {
    let mut rng = Lcg(20_261_107);
    let mut checked = 0;
    for _ in 0..150 {
        let mut random_box = || {
            let size = (
                rng.range(0.5, 3.0),
                rng.range(0.5, 3.0),
                rng.range(0.5, 3.0),
            );
            let axis = (
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(0.1, 1.0),
            );
            let at = (
                rng.range(-0.8, 0.8),
                rng.range(-0.8, 0.8),
                rng.range(-0.8, 0.8),
            );
            // Boxes and wedges (sloped faces, triangular sides).
            let shape = if rng.next() < 0.5 {
                cuboid((0.0, 0.0, 0.0), size)
            } else {
                Body::wedge(p(0.0, 0.0, 0.0), p(size.0, size.1, size.2)).unwrap()
            };
            turned(&shape, axis, rng.range(0.0, PI), at)
        };
        let (a, b) = (random_box(), random_box());
        let (va, vb) = (volume(&a), volume(&b));
        let union = a.union(&b).unwrap();
        assert_closed(&union);
        let vi = a.intersection(&b).unwrap().map_or(0.0, |x| volume(&x));
        let vr = a.difference(&b).unwrap().map_or(0.0, |x| volume(&x));
        assert!(
            close(volume(&union), va + vb - vi),
            "union {} vs {}",
            volume(&union),
            va + vb - vi
        );
        assert!(close(vr, va - vi), "difference {vr} vs {}", va - vi);
        if vi > 0.0 {
            checked += 1;
        }
    }
    // Most pairs overlap: the identities were tested on real cuts.
    assert!(checked > 75, "{checked}");
}
