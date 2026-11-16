//! Tests of the booleans of cylinders with parallel axes (ADR 0033 delivery
//! 2.3c; a submodule of `boolean_curved`): round columns side by side,
//! one through another, stacked and overlapping, each volume exact.

use std::f64::consts::PI;

use arq_geometry::{Point3, Transform3, Vector3};

use crate::boolean::BooleanError;
use crate::brep::Body;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
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

/// The area two circles of radii `a` and `b`, centers `d` apart, share.
fn lens(a: f64, b: f64, d: f64) -> f64 {
    if d >= a + b {
        return 0.0;
    }
    if d <= (a - b).abs() {
        return PI * a.min(b).powi(2);
    }
    let ta = ((d * d + a * a - b * b) / (2.0 * d * a)).acos();
    let tb = ((d * d + b * b - a * a) / (2.0 * d * b)).acos();
    let k = ((-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b)).sqrt();
    a * a * ta + b * b * tb - k / 2.0
}

/// The volumes of `a ∪ b`, `a ∩ b` and `a − b`, 0 for what is empty.
fn volumes(a: &Body, b: &Body) -> (f64, f64, f64) {
    let union = a.union(b).unwrap();
    let inter = a.intersection(b).unwrap().map_or(0.0, |x| volume(&x));
    let diff = a.difference(b).unwrap().map_or(0.0, |x| volume(&x));
    (volume(&union), inter, diff)
}

#[test]
fn two_columns_side_by_side_overlap_in_a_lens() {
    // Two round columns 1.5 apart, radius 1, height 2: their overlap is a
    // lens across, 2 high.
    let a = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let b = column((1.5, 0.0, 0.0), 1.0, 2.0);
    let shared = lens(1.0, 1.0, 1.5) * 2.0;
    let disc = PI * 2.0;
    let (union, inter, diff) = volumes(&a, &b);
    assert!(close(union, 2.0 * disc - shared), "union {union}");
    assert!(close(inter, shared), "intersection {inter}");
    assert!(close(diff, disc - shared), "difference {diff}");
    let united = a.union(&b).unwrap();
    assert_eq!(united.lumps().len(), 1, "one solid");
    assert_eq!(united.shells().len(), 1);
}

#[test]
fn columns_of_other_radii_and_heights_overlap_where_both_reach() {
    // A thinner column from 1 to 4 beside one from 0 to 3: they share a
    // lens from 1 to 3.
    let a = column((0.0, 0.0, 0.0), 1.0, 3.0);
    let b = column((1.2, 0.4, 1.0), 0.6, 3.0);
    let d = 1.2_f64.hypot(0.4);
    let shared = lens(1.0, 0.6, d) * 2.0;
    let (union, inter, diff) = volumes(&a, &b);
    assert!(close(inter, shared), "intersection {inter} vs {shared}");
    assert!(
        close(union, PI * 3.0 + PI * 0.36 * 3.0 - shared),
        "union {union}"
    );
    assert!(close(diff, PI * 3.0 - shared), "difference {diff}");
}

#[test]
fn a_thin_column_through_a_thick_one_on_one_axis_leaves_a_tube() {
    // Coaxial, the sides never meet: the thick one's ends cut the thin one.
    let thick = column((0.0, 0.0, 0.0), 2.0, 3.0);
    let thin = column((0.0, 0.0, -1.0), 0.8, 5.0);
    let (union, inter, diff) = volumes(&thick, &thin);
    assert!(close(diff, PI * (4.0 - 0.64) * 3.0), "tube {diff}");
    assert!(close(inter, PI * 0.64 * 3.0), "core {inter}");
    assert!(
        close(union, PI * 4.0 * 3.0 + PI * 0.64 * 2.0),
        "union {union}"
    );
    let tube = thick.difference(&thin).unwrap().unwrap();
    assert_eq!(tube.lumps().len(), 1);
}

#[test]
fn a_thin_column_off_the_axis_inside_a_thick_one() {
    // Never touching the thick one's side: a round hole off the axis.
    let thick = column((0.0, 0.0, 0.0), 2.0, 3.0);
    let thin = column((0.7, 0.3, -1.0), 0.5, 5.0);
    let (union, inter, diff) = volumes(&thick, &thin);
    assert!(close(diff, PI * (4.0 - 0.25) * 3.0), "{diff}");
    assert!(close(inter, PI * 0.25 * 3.0), "{inter}");
    assert!(close(union, PI * 4.0 * 3.0 + PI * 0.25 * 2.0), "{union}");
}

#[test]
fn columns_of_one_radius_on_one_axis_stack_and_overlap() {
    // Their sides lie on one cylinder: the ends bound the section.
    let low = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let high = column((0.0, 0.0, 2.0), 1.0, 2.0);
    let (union, inter, diff) = volumes(&low, &high);
    assert!(close(union, 4.0 * PI), "stacked {union}");
    assert!(close(inter, 0.0), "{inter}");
    assert!(close(diff, 2.0 * PI), "{diff}");
    let over = column((0.0, 0.0, 1.0), 1.0, 2.0);
    let (union, inter, diff) = volumes(&low, &over);
    assert!(close(union, 3.0 * PI), "overlapping {union}");
    assert!(close(inter, PI), "{inter}");
    assert!(close(diff, PI), "{diff}");
}

#[test]
fn parallel_columns_lying_down_overlap_as_standing_ones() {
    // Both turned to lie along the y axis, then moved: the same lens.
    let lie = |c: (f64, f64, f64)| {
        turned(
            &column((0.0, 0.0, 0.0), 1.0, 2.0),
            (1.0, 0.0, 0.0),
            PI / 2.0,
            c,
        )
    };
    let a = lie((0.0, 3.0, 5.0));
    let b = lie((1.0, 3.0, 5.6));
    let shared = lens(1.0, 1.0, 1.0_f64.hypot(0.6)) * 2.0;
    let (union, inter, diff) = volumes(&a, &b);
    assert!(close(inter, shared), "{inter} vs {shared}");
    assert!(close(union, 4.0 * PI - shared), "{union}");
    assert!(close(diff, 2.0 * PI - shared), "{diff}");
}

#[test]
fn columns_crossing_at_an_angle_still_wait_for_delivery_4() {
    let a = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let b = turned(
        &column((0.0, 0.0, -2.0), 0.5, 4.0),
        (1.0, 0.0, 0.0),
        PI / 3.0,
        (0.3, 0.0, 1.0),
    );
    assert_eq!(a.union(&b), Err(BooleanError::Curved));
}

/// A small deterministic generator, as the other tests'.
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
fn the_volumes_add_up_and_match_the_lens_for_parallel_columns() {
    let mut rng = Lcg(20_261_116);
    let mut met = 0;
    for _ in 0..40 {
        let (ra, rb) = (rng.range(0.4, 1.5), rng.range(0.4, 1.5));
        let (ha, hb) = (rng.range(1.0, 3.0), rng.range(1.0, 3.0));
        let (x, y, z) = (
            rng.range(-2.0, 2.0),
            rng.range(-2.0, 2.0),
            rng.range(-1.0, 2.0),
        );
        let d = x.hypot(y);
        // Clear of touching, where the faces would meet along a line only.
        if (d - (ra + rb)).abs() < 1e-3 || (d - (ra - rb).abs()).abs() < 1e-3 {
            continue;
        }
        let a = column((0.0, 0.0, 0.0), ra, ha);
        let b = column((x, y, z), rb, hb);
        let overlap = (ha.min(z + hb) - z.max(0.0)).max(0.0);
        let shared = lens(ra, rb, d) * overlap;
        let (va, vb) = (PI * ra * ra * ha, PI * rb * rb * hb);
        let (union, inter, diff) = volumes(&a, &b);
        assert!(close(inter, shared), "intersection {inter} vs {shared}");
        assert!(
            close(union, va + vb - inter),
            "union {union} vs {}",
            va + vb - inter
        );
        assert!(
            close(diff, va - inter),
            "difference {diff} vs {}",
            va - inter
        );
        met += usize::from(shared > 0.0);
    }
    assert!(met > 10, "{met}");
}

#[test]
fn columns_touching_along_a_line_unite_as_two_solids() {
    // Side by side, touching: their ends touch at a point only, so they
    // stay two lumps, as two boxes touching along an edge do.
    let a = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let b = column((2.0, 0.0, 0.0), 1.0, 2.0);
    let united = a.union(&b).unwrap();
    assert_eq!(united.lumps().len(), 2);
    assert!(close(volume(&united), 4.0 * PI));
    assert_eq!(a.intersection(&b).unwrap(), None);
    assert!(close(volume(&a.difference(&b).unwrap().unwrap()), 2.0 * PI));
}

#[test]
fn stacked_columns_unite_with_a_circle_where_they_met() {
    // One solid; its ends merge as flat faces do, but its side keeps the
    // circle where the two met (coincident cylindrical faces are not merged
    // yet).
    let low = column((0.0, 0.0, 0.0), 1.0, 2.0);
    let high = column((0.0, 0.0, 2.0), 1.0, 2.0);
    let united = low.union(&high).unwrap();
    assert_eq!(united.lumps().len(), 1);
    assert_eq!(united.faces().len(), 4, "two ends, the side in two");
}
