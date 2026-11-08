//! Tests of the booleans of solids with cylindrical faces (a submodule of
//! `boolean_curved`).

use std::f64::consts::PI;

use arq_geometry::{Point3, Transform3, Vector3};

use crate::boolean::BooleanError;
use crate::brep::Body;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn cuboid(a: (f64, f64, f64), b: (f64, f64, f64)) -> Body {
    Body::cuboid(p(a.0, a.1, a.2), p(b.0, b.1, b.2)).unwrap()
}

fn column(c: (f64, f64, f64), r: f64, h: f64) -> Body {
    Body::cylinder(p(c.0, c.1, c.2), r, h).unwrap()
}

fn volume(body: &Body) -> f64 {
    body.mass_properties().unwrap().volume
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

/// The body turned about `axis` through the origin by `angle`, then moved.
fn turned(body: &Body, axis: (f64, f64, f64), angle: f64, by: (f64, f64, f64)) -> Body {
    let t = Transform3::rotation(Vector3::new(axis.0, axis.1, axis.2).unwrap(), angle)
        .unwrap()
        .then(Transform3::translation(
            Vector3::new(by.0, by.1, by.2).unwrap(),
        ));
    body.transformed(t, 1.0).unwrap()
}

#[test]
fn a_column_through_a_slab_bores_a_round_hole() {
    let slab = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let col = column((2.0, 2.0, -1.0), 0.5, 3.0);
    let disc = PI * 0.25;
    let holed = slab.difference(&col).unwrap().unwrap();
    assert!(close(volume(&holed), 16.0 - disc), "{}", volume(&holed));
    let both = slab.intersection(&col).unwrap().unwrap();
    assert!(close(volume(&both), disc), "{}", volume(&both));
    let union = slab.union(&col).unwrap();
    assert!(
        close(volume(&union), 16.0 + 3.0 * disc - disc),
        "{}",
        volume(&union)
    );
    // The column less the slab: two pieces, above and below.
    let parts = col.difference(&slab).unwrap().unwrap();
    assert_eq!(parts.lumps().len(), 2);
    assert!(close(volume(&parts), 2.0 * disc));
}

#[test]
fn a_column_inside_a_box_leaves_a_round_void() {
    let block = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
    let col = column((2.0, 2.0, 1.0), 1.0, 2.0);
    let hollow = block.difference(&col).unwrap().unwrap();
    assert_eq!(hollow.lumps().len(), 1);
    assert_eq!(hollow.lumps()[0].shells.len(), 2);
    assert!(close(volume(&hollow), 64.0 - 2.0 * PI));
    assert!(close(volume(&block.union(&col).unwrap()), 64.0));
}

#[test]
fn a_turned_box_cuts_a_column_in_ellipses() {
    let col = column((0.0, 0.0, 0.0), 1.0, 4.0);
    let cutter = turned(
        &cuboid((-2.0, -2.0, 0.0), (2.0, 2.0, 1.5)),
        (1.0, 0.3, 0.0),
        0.4,
        (0.1, -0.2, 1.0),
    );
    let (vc, vb) = (volume(&col), volume(&cutter));
    let both = col.intersection(&cutter).unwrap().unwrap();
    let rest = col.difference(&cutter).unwrap().unwrap();
    let union = col.union(&cutter).unwrap();
    let vi = volume(&both);
    assert!(vi > 0.5 && vi < vc);
    assert!(
        close(volume(&rest), vc - vi),
        "{} {}",
        volume(&rest),
        vc - vi
    );
    assert!(
        close(volume(&union), vc + vb - vi),
        "{} {}",
        volume(&union),
        vc + vb - vi
    );
}

#[test]
fn a_millimetre_column_through_a_slab() {
    let slab = cuboid((0.0, 0.0, 0.0), (6000.0, 4000.0, 200.0));
    let col = column((2500.0, 1500.0, -500.0), 200.0, 3500.0);
    let holed = slab.difference(&col).unwrap().unwrap();
    let exact = 6000.0 * 4000.0 * 200.0 - PI * 200.0 * 200.0 * 200.0;
    assert!(close(volume(&holed), exact));
}

#[test]
fn two_columns_meeting_wait_for_delivery_4() {
    let a = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let b = turned(
        &column((0.0, 0.0, -2.0), 0.5, 4.0),
        (1.0, 0.0, 0.0),
        PI / 2.0,
        (0.0, 0.0, 1.0),
    );
    assert_eq!(a.union(&b), Err(BooleanError::Curved));
    // Apart they unite as two pieces.
    let c = column((5.0, 0.0, 0.0), 1.0, 2.0);
    let both = a.union(&c).unwrap();
    assert_eq!(both.lumps().len(), 2);
    assert!(close(volume(&both), 4.0 * PI));
}

/// A small deterministic generator, as the planar tests'.
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
fn the_volumes_add_up_for_boxes_and_turned_columns() {
    let mut rng = Lcg(20_261_108);
    let mut cut = 0;
    for _ in 0..40 {
        let size = (
            rng.range(1.0, 3.0),
            rng.range(1.0, 3.0),
            rng.range(1.0, 3.0),
        );
        let block = cuboid((0.0, 0.0, 0.0), size);
        let col = turned(
            &column((0.0, 0.0, -1.5), rng.range(0.3, 1.0), rng.range(2.0, 4.0)),
            (
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(0.1, 1.0),
            ),
            rng.range(0.0, PI),
            (
                rng.range(0.3, 1.5),
                rng.range(0.3, 1.5),
                rng.range(0.3, 1.5),
            ),
        );
        let (va, vb) = (volume(&block), volume(&col));
        let union = block.union(&col).unwrap();
        let vi = block
            .intersection(&col)
            .unwrap()
            .map_or(0.0, |x| volume(&x));
        let vr = block.difference(&col).unwrap().map_or(0.0, |x| volume(&x));
        assert!(
            close(volume(&union), va + vb - vi),
            "union {} vs {}",
            volume(&union),
            va + vb - vi
        );
        assert!(close(vr, va - vi), "difference {vr} vs {}", va - vi);
        cut += usize::from(vi > 0.0);
    }
    assert!(cut > 20, "{cut}");
}

#[test]
fn planes_along_the_axis_cut_a_column_lengthwise() {
    // Through the axis: half a column. The plane holds the generators at
    // the angle 0 the column's frame starts from, so the window must not.
    let col = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let half = col
        .intersection(&cuboid((-2.0, 0.0, -1.0), (2.0, 2.0, 3.0)))
        .unwrap()
        .unwrap();
    assert!(close(volume(&half), PI), "{}", volume(&half));
    // Half a radius off it: a segment of the circle, two lines on the side.
    let d: f64 = 0.5;
    let segment = (d).acos() - d * (1.0 - d * d).sqrt();
    let cap = col
        .intersection(&cuboid((-2.0, d, -1.0), (2.0, 2.0, 3.0)))
        .unwrap()
        .unwrap();
    assert!(
        close(volume(&cap), 2.0 * segment),
        "{} {}",
        volume(&cap),
        2.0 * segment
    );
}

#[test]
fn a_mirrored_column_bores_the_same_hole() {
    // Mirrored, the column's circles turn the other way about its axis than
    // its side's frame: each closed circle must still pair with itself.
    let slab = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let mirror = Transform3::new(
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        p(2.0, 0.0, 0.0),
        Vector3::new(0.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    let col = column((1.5, 2.0, -1.0), 0.5, 3.0)
        .transformed(mirror, 1.0)
        .unwrap();
    let holed = slab.difference(&col).unwrap().unwrap();
    assert!(
        close(volume(&holed), 16.0 - PI * 0.25),
        "{}",
        volume(&holed)
    );
}

#[test]
fn a_mirrored_column_s_own_ends_meet_its_side() {
    // United, the mirrored column's end circles, written turning the other
    // way about its axis, meet the circles its side lifts.
    let slab = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let mirror = Transform3::new(
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        p(2.0, 0.0, 0.0),
        Vector3::new(0.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    let col = column((1.5, 2.0, -1.0), 0.5, 3.0)
        .transformed(mirror, 1.0)
        .unwrap();
    let joined = slab.union(&col).unwrap();
    assert!(
        close(volume(&joined), 16.0 + PI * 0.25 * 2.0),
        "{}",
        volume(&joined)
    );
}

#[test]
fn a_plane_through_the_axis_where_the_angle_starts() {
    // The column's angle starts along +y: the plane x = 0 holds the
    // generators at 0 and π, which no window may start on.
    let col = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let half = col
        .intersection(&cuboid((0.0, -2.0, -1.0), (2.0, 2.0, 3.0)))
        .unwrap()
        .unwrap();
    assert!(close(volume(&half), PI), "{}", volume(&half));
}
