//! Tests of the booleans of solids with conical faces (ADR 0033 delivery
//! 2.3b; a submodule of `boolean_curved`).

use std::f64::consts::PI;

use arq_geometry::{Point3, Transform3, Vector3};

use crate::brep::Body;
use crate::curve::Curve;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn cuboid(a: (f64, f64, f64), b: (f64, f64, f64)) -> Body {
    Body::cuboid(p(a.0, a.1, a.2), p(b.0, b.1, b.2)).unwrap()
}

/// A cone standing at `c`: base radius `base`, top radius `top`, `h` high.
fn cone(c: (f64, f64, f64), base: f64, top: f64, h: f64) -> Body {
    Body::cone(p(c.0, c.1, c.2), base, top, h).unwrap()
}

fn volume(body: &Body) -> f64 {
    body.mass_properties().unwrap().volume
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

/// Close to a reference integrated numerically.
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-7 * (1.0 + a.abs().max(b.abs()))
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

/// The half-space `(p − q)·n ≥ 0` for `n = (0, −sin α, cos α)`, as a big box
/// turned about X by `α` and moved to `q`, its bottom face the plane.
fn half_space(alpha: f64, q: (f64, f64, f64)) -> Body {
    turned(
        &cuboid((-10.0, -10.0, 0.0), (10.0, 10.0, 20.0)),
        (1.0, 0.0, 0.0),
        alpha,
        q,
    )
}

/// The part of a disc of radius `rho` beyond the chord at `d` from its
/// center.
fn beyond(rho: f64, d: f64) -> f64 {
    if d >= rho {
        0.0
    } else if d <= -rho {
        PI * rho * rho
    } else {
        rho * rho * (d / rho).acos() - d * (rho * rho - d * d).sqrt()
    }
}

/// `∫ f` from `a` to `b`, by adaptive Simpson's rule.
fn integral(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    /// The rule on `(a, b)`, `f` there at its ends and middle, its whole
    /// estimate `whole`.
    fn step(
        f: &dyn Fn(f64) -> f64,
        (a, b): (f64, f64),
        (fa, fm, fb): (f64, f64, f64),
        whole: f64,
        depth: u32,
    ) -> f64 {
        let m = (a + b) / 2.0;
        let (flm, frm) = (f((a + m) / 2.0), f((m + b) / 2.0));
        let left = (m - a) / 6.0 * (fa + 4.0 * flm + fm);
        let right = (b - m) / 6.0 * (fm + 4.0 * frm + fb);
        if depth == 0 || (left + right - whole).abs() <= 1e-13 {
            return left + right + (left + right - whole) / 15.0;
        }
        step(f, (a, m), (fa, flm, fm), left, depth - 1)
            + step(f, (m, b), (fm, frm, fb), right, depth - 1)
    }
    let (fa, fm, fb) = (f(a), f((a + b) / 2.0), f(b));
    let whole = (b - a) / 6.0 * (fa + 4.0 * fm + fb);
    step(f, (a, b), (fa, fm, fb), whole, 40)
}

/// How many of the body's edges are of each kind: lines, circles, other
/// ellipses, NURBS, and points (a cone's apex).
fn kinds(body: &Body) -> [usize; 5] {
    let mut k = [0; 5];
    for e in body.edges() {
        match &e.curve {
            Some(Curve::Line(_)) => k[0] += 1,
            Some(Curve::Ellipse(c)) if (c.ratio() - 1.0).abs() < 1e-9 => k[1] += 1,
            Some(Curve::Ellipse(_)) => k[2] += 1,
            Some(Curve::Nurbs(_)) => k[3] += 1,
            None => k[4] += 1,
        }
    }
    k
}

/// The weights of the body's NURBS edges' middle control points.
fn weights(body: &Body) -> Vec<f64> {
    body.edges()
        .iter()
        .filter_map(|e| match &e.curve {
            Some(Curve::Nurbs(n)) => {
                Some(n.weights()[1] / (n.weights()[0] * n.weights()[2]).sqrt())
            }
            _ => None,
        })
        .collect()
}

/// A frustum's volume: height `h`, end radii `a` and `b`.
fn frustum(h: f64, a: f64, b: f64) -> f64 {
    PI * h / 3.0 * (a * a + a * b + b * b)
}

#[test]
fn a_slab_cuts_a_cone_in_circles() {
    // A cone from radius 1 at z = 0 to its apex at z = 2, through a slab
    // from z = 0.5 to 1: the radius 0.75, then 0.5.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let slab = cuboid((-2.0, -2.0, 0.5), (2.0, 2.0, 1.0));
    let ring = frustum(0.5, 0.75, 0.5);
    let both = tip.intersection(&slab).unwrap().unwrap();
    assert!(close(volume(&both), ring), "{}", volume(&both));
    assert_eq!(kinds(&both)[2..], [0, 0, 0], "circles only, no apex");
    let rest = tip.difference(&slab).unwrap().unwrap();
    assert_eq!(rest.lumps().len(), 2, "the foot and the tip");
    assert!(close(volume(&rest), volume(&tip) - ring));
    assert_eq!(kinds(&rest)[4], 1, "the tip keeps its apex");
    let holed = slab.difference(&tip).unwrap().unwrap();
    assert!(close(volume(&holed), 8.0 - ring));
    assert!(close(
        volume(&slab.union(&tip).unwrap()),
        8.0 + volume(&tip) - ring
    ));
}

#[test]
fn a_box_round_the_apex_keeps_it() {
    // The top of a cone: from z = 0.5 (radius 0.75) to the apex.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let top = cuboid((-2.0, -2.0, 0.5), (2.0, 2.0, 3.0));
    let small = PI / 3.0 * 0.75 * 0.75 * 1.5;
    let both = tip.intersection(&top).unwrap().unwrap();
    assert!(close(volume(&both), small), "{}", volume(&both));
    assert_eq!(kinds(&both)[4], 1, "its apex");
    // Upside down: the apex at the bottom.
    let cup = cone((0.0, 0.0, 0.0), 0.0, 1.0, 2.0);
    let low = cuboid((-2.0, -2.0, -1.0), (2.0, 2.0, 1.0));
    let both = cup.intersection(&low).unwrap().unwrap();
    assert!(close(volume(&both), PI / 3.0 * 0.25), "{}", volume(&both));
    assert_eq!(kinds(&both)[4], 1);
}

#[test]
fn a_plane_through_the_axis_halves_a_cone_along_its_side() {
    // The plane y = 0: two lines through the apex.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let half = tip
        .intersection(&half_space(PI / 2.0, (0.0, 0.0, 0.0)))
        .unwrap()
        .unwrap();
    assert!(
        close(volume(&half), volume(&tip) / 2.0),
        "{}",
        volume(&half)
    );
    let k = kinds(&half);
    assert_eq!(
        k[4], 0,
        "the apex is a corner of the cut, not a point of its own"
    );
    assert!(
        k[0] >= 3,
        "the base's diameter and two lines along the side: {k:?}"
    );
}

#[test]
fn a_plane_along_the_axis_cuts_a_cone_in_a_hyperbola() {
    // The plane y = 0.3, the cone's side meets it in a hyperbola.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let cut = half_space(PI / 2.0, (0.0, 0.3, 0.0));
    let rho = |z: f64| 1.0 - z / 2.0;
    let kept = integral(&|z| PI * rho(z) * rho(z) - beyond(rho(z), 0.3), 0.0, 2.0);
    let both = tip.intersection(&cut).unwrap().unwrap();
    assert!(near(volume(&both), kept), "{} {kept}", volume(&both));
    assert_eq!(kinds(&both)[4], 1, "the apex is kept");
    let w = weights(&both);
    assert_eq!(w.len(), 1);
    assert!(w[0] > 1.0, "a hyperbola: {w:?}");
    let rest = tip.difference(&cut).unwrap().unwrap();
    assert!(near(volume(&rest), volume(&tip) - kept));
    assert_eq!(kinds(&rest)[4], 0);
}

#[test]
fn a_plane_along_a_generator_cuts_a_parabola() {
    // The cone narrows by 1/2 per unit up; the plane y = 0.4 − z/2 runs
    // along its side, 0.6 in from it.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let alpha = PI - 2f64.atan();
    let cut = half_space(alpha, (0.0, 0.4, 0.0));
    let rho = |z: f64| 1.0 - z / 2.0;
    let d = |z: f64| 0.4 - z / 2.0;
    let kept = integral(&|z| PI * rho(z) * rho(z) - beyond(rho(z), d(z)), 0.0, 1.4);
    let both = tip.intersection(&cut).unwrap().unwrap();
    assert!(near(volume(&both), kept), "{} {kept}", volume(&both));
    let w = weights(&both);
    assert_eq!(w.len(), 1, "{w:?} {:?}", kinds(&both));
    assert!((w[0] - 1.0).abs() < 1e-9, "a parabola: {w:?}");
    let rest = tip.difference(&cut).unwrap().unwrap();
    assert!(near(volume(&rest), volume(&tip) - kept));
    assert_eq!(kinds(&rest)[4], 1, "the apex is on this side");
}

#[test]
fn a_tilted_plane_cuts_a_cone_all_round_in_an_ellipse() {
    // z ≥ 0.8 + y tan 0.3: an ellipse, the apex above it.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let alpha: f64 = 0.3;
    let cut = half_space(alpha, (0.0, 0.0, 0.8));
    let rho = |z: f64| 1.0 - z / 2.0;
    let d = |z: f64| (z - 0.8) / alpha.tan();
    let kept = integral(&|z| PI * rho(z) * rho(z) - beyond(rho(z), d(z)), 0.0, 2.0);
    let both = tip.intersection(&cut).unwrap().unwrap();
    assert!(near(volume(&both), kept), "{} {kept}", volume(&both));
    let k = kinds(&both);
    assert_eq!(
        (k[2], k[3], k[4]),
        (1, 0, 1),
        "one ellipse and the apex: {k:?}"
    );
    let rest = tip.difference(&cut).unwrap().unwrap();
    assert!(near(volume(&rest), volume(&tip) - kept));
}

#[test]
fn a_cut_cone_cut_again() {
    // Its hyperbola read back as a NURBS edge and cut by another plane.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let once = tip
        .intersection(&half_space(PI / 2.0, (0.0, 0.3, 0.0)))
        .unwrap()
        .unwrap();
    let low = cuboid((-2.0, -2.0, -1.0), (2.0, 2.0, 0.4));
    let rho = |z: f64| 1.0 - z / 2.0;
    let kept = integral(&|z| PI * rho(z) * rho(z) - beyond(rho(z), 0.3), 0.0, 0.4);
    let twice = once.intersection(&low).unwrap().unwrap();
    assert!(near(volume(&twice), kept), "{} {kept}", volume(&twice));
    let top = once.difference(&low).unwrap().unwrap();
    assert!(near(volume(&top), volume(&once) - kept));
    // Its own planar faces with the hyperbola, cut by the plane x = 0.
    let side = half_space(PI / 2.0, (0.0, 0.0, 0.0));
    let turned_side = turned(&side, (0.0, 0.0, 1.0), PI / 2.0, (0.0, 0.0, 0.0));
    let quarter = once.intersection(&turned_side).unwrap().unwrap();
    let rest = once.difference(&turned_side).unwrap().unwrap();
    assert!(close(volume(&quarter) + volume(&rest), volume(&once)));
}

#[test]
fn a_cone_standing_on_a_slab_and_through_it_flush() {
    let slab = cuboid((-2.0, -2.0, -1.0), (2.0, 2.0, 0.0));
    // Standing on it: one solid, the cone's foot gone.
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let joined = slab.union(&tip).unwrap();
    assert_eq!(joined.lumps().len(), 1);
    assert!(close(volume(&joined), 16.0 + volume(&tip)));
    assert_eq!(slab.intersection(&tip).unwrap(), None);
    // A frustum exactly as thick as the slab: a tapered hole, flush.
    let plug = cone((0.0, 0.0, -1.0), 1.0, 0.5, 1.0);
    let holed = slab.difference(&plug).unwrap().unwrap();
    assert!(
        close(volume(&holed), 16.0 - frustum(1.0, 1.0, 0.5)),
        "{}",
        volume(&holed)
    );
    assert!(close(volume(&slab.union(&plug).unwrap()), 16.0));
    assert_eq!(plug.difference(&slab).unwrap(), None);
    // A countersink: the cone's apex below the slab, its widest flush with
    // the top.
    let sink = cone((0.0, 0.0, 0.0), 0.8, 0.0, -1.5);
    let sunk = slab.difference(&sink).unwrap().unwrap();
    let inside = frustum(1.0, 0.8, 0.8 * 0.5 / 1.5);
    assert!(close(volume(&sunk), 16.0 - inside), "{}", volume(&sunk));
}

#[test]
fn a_cone_inside_a_box_leaves_a_conical_void() {
    let block = cuboid((-2.0, -2.0, -1.0), (2.0, 2.0, 3.0));
    let tip = cone((0.0, 0.0, 0.0), 1.0, 0.0, 2.0);
    let hollow = block.difference(&tip).unwrap().unwrap();
    assert_eq!(hollow.lumps().len(), 1);
    assert_eq!(hollow.lumps()[0].shells.len(), 2, "the box and the void");
    assert!(close(volume(&hollow), 64.0 - volume(&tip)));
    // A second box beside it: the void stays in the first.
    let beside = cuboid((5.0, -2.0, -1.0), (9.0, 2.0, 3.0));
    let both = hollow.union(&beside).unwrap();
    assert_eq!(both.lumps().len(), 2);
    let holed = both.lumps().iter().filter(|l| l.shells.len() == 2).count();
    assert_eq!(holed, 1);
    assert!(close(volume(&both), 128.0 - volume(&tip)));
}

#[test]
fn a_box_inside_a_cone_leaves_a_square_void() {
    // The ray from the void meets the cone's side.
    let big = cone((0.0, 0.0, 0.0), 3.0, 0.0, 6.0);
    let cube = cuboid((-0.3, -0.3, 1.0), (0.3, 0.3, 1.6));
    let hollow = big.difference(&cube).unwrap().unwrap();
    assert_eq!(hollow.lumps().len(), 1);
    assert_eq!(hollow.lumps()[0].shells.len(), 2);
    assert!(close(volume(&hollow), volume(&big) - 0.216));
}

#[test]
fn a_slab_on_a_frustum_s_small_end() {
    // The frustum narrows from 1 at z = −1 to 0.5 at z = 0, where the slab
    // lies on it: they meet along the small end's circle, not the
    // reference one.
    let slab = cuboid((-2.0, -2.0, 0.0), (2.0, 2.0, 1.0));
    let stand = cone((0.0, 0.0, -1.0), 1.0, 0.5, 1.0);
    let joined = slab.union(&stand).unwrap();
    assert_eq!(joined.lumps().len(), 1);
    assert!(close(volume(&joined), 16.0 + volume(&stand)));
    assert_eq!(slab.intersection(&stand).unwrap(), None);
    assert!(close(
        volume(&stand.difference(&slab).unwrap().unwrap()),
        volume(&stand)
    ));
    // A slab across its middle, flush with nothing: the ring between.
    let thin = cuboid((-2.0, -2.0, -0.5), (2.0, 2.0, 0.0));
    let ring = stand.intersection(&thin).unwrap().unwrap();
    assert!(
        close(volume(&ring), frustum(0.5, 0.75, 0.5)),
        "{}",
        volume(&ring)
    );
}

/// A cone's domain: radius `r` at `origin`, widening by `k` per unit up the
/// axis `(1, 2, 2) / 3`.
fn a_cone_domain(r: f64, k: f64, sigma: f64) -> super::Domain {
    let axis = Vector3::new(1.0, 2.0, 2.0).unwrap().normalized().unwrap();
    let e1 = Vector3::new(2.0, -2.0, 1.0).unwrap().normalized().unwrap();
    super::Domain::Cone {
        origin: p(0.5, -1.0, 2.0),
        axis,
        r,
        k,
        e1,
        e2: axis.cross(e1),
        sigma,
        x0: 0.0,
    }
}

#[test]
fn rays_meet_a_cone_on_its_side() {
    for (r, k) in [(1.0, 0.0), (1.0, 0.4), (2.0, -0.7)] {
        let domain = a_cone_domain(r, k, 1.0);
        let surface = crate::surface::Cone::new(
            p(0.5, -1.0, 2.0),
            Vector3::new(1.0, 2.0, 2.0).unwrap(),
            r,
            k / k.hypot(1.0),
            1.0 / k.hypot(1.0),
        )
        .unwrap();
        let w = Vector3::new(0.53, 0.31, 0.76)
            .unwrap()
            .normalized()
            .unwrap();
        let mut met = 0;
        for i in 0..20 {
            let from = domain.lift([i as f64 * 0.7, 0.3 * i as f64 - 2.0])
                + Vector3::new(0.1, -0.2, 0.05).unwrap() * (i % 5) as f64;
            for s in super::ray_hits(&domain, from, w) {
                let q = from + w * s;
                assert!(
                    surface.distance(q) < 1e-12,
                    "{r} {k}: {}",
                    surface.distance(q)
                );
                // On the face's nappe: its radius there is positive.
                let h = (q - p(0.5, -1.0, 2.0))
                    .dot(Vector3::new(1.0, 2.0, 2.0).unwrap().normalized().unwrap());
                assert!(r + k * h >= -1e-12);
                met += 1;
            }
        }
        assert!(met > 10, "{r} {k}: {met}");
    }
}

#[test]
fn heights_off_a_cone_along_its_normal() {
    for (r, k, sigma) in [(1.0, 0.0, 1.0), (1.0, 0.4, 1.0), (2.0, -0.7, -1.0)] {
        let domain = a_cone_domain(r, k, sigma);
        for i in 0..10 {
            let on = domain.lift([i as f64 * 0.6, 0.25 * i as f64 - 1.0]);
            assert!(super::height(&domain, on).abs() < 1e-12);
            let n = domain.normal(on);
            for d in [0.01, -0.02] {
                let h = super::height(&domain, on + n * d);
                assert!((h - d).abs() < 1e-9, "{r} {k}: {h} {d}");
            }
        }
    }
}

/// A small generator of numbers, the same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

#[test]
fn the_volumes_add_up_for_cones_and_turned_boxes() {
    let mut rng = Lcg(20_261_110);
    for case in 0..30 {
        let (base, top) = if case % 3 == 0 {
            (rng.range(0.5, 1.5), 0.0)
        } else {
            (rng.range(0.5, 1.5), rng.range(0.1, 1.0))
        };
        let h = rng.range(1.0, 3.0) * if case % 4 == 1 { -1.0 } else { 1.0 };
        let a = cone((0.0, 0.0, 0.0), base, top, h);
        let size = (
            rng.range(0.5, 2.0),
            rng.range(0.5, 2.0),
            rng.range(0.5, 2.0),
        );
        let b = turned(
            &cuboid((-size.0, -size.1, -size.2), size),
            (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), 1.0),
            rng.range(0.0, PI),
            (
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0) + h / 2.0,
            ),
        );
        let (va, vb) = (volume(&a), volume(&b));
        let union = a
            .union(&b)
            .unwrap_or_else(|e| panic!("case {case}: union: {e}"));
        let vi = a
            .intersection(&b)
            .unwrap_or_else(|e| panic!("case {case}: intersection: {e}"))
            .map_or(0.0, |x| volume(&x));
        let vr = a
            .difference(&b)
            .unwrap_or_else(|e| panic!("case {case}: difference: {e}"))
            .map_or(0.0, |x| volume(&x));
        let vs = b
            .difference(&a)
            .unwrap_or_else(|e| panic!("case {case}: other difference: {e}"))
            .map_or(0.0, |x| volume(&x));
        assert!(close(volume(&union), va + vb - vi), "case {case}: union");
        assert!(close(vr, va - vi), "case {case}: difference");
        assert!(close(vs, vb - vi), "case {case}: other difference");
    }
}
