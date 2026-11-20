//! Booleans of cylinders and cones (ADR 0033 delivery 2, D4): pairs of
//! upright columns, whose results have exact volumes in closed form, and
//! columns with turned boxes, checked against the volume identities.

use std::f64::consts::PI;

use arq_geometry::{Point3 as P3, Transform3, Vector3 as V3};
use arq_kernel::Body;
use truck_meshalgo::prelude::*;
use truck_modeling::{Point3, Rad, Solid, Vector3, builder};

use crate::{Lcg, Outcome, ours, truck, volume_of};

/// An upright column: base center, radius, height.
struct Column {
    at: (f64, f64, f64),
    radius: f64,
    height: f64,
}

impl Column {
    fn ours(&self) -> Body {
        let (x, y, z) = self.at;
        Body::cylinder(P3::new(x, y, z).unwrap(), self.radius, self.height).unwrap()
    }

    fn truck(&self) -> Solid {
        let (x, y, z) = self.at;
        let v = builder::vertex(Point3::new(x + self.radius, y, z));
        let circle = builder::rsweep(
            &v,
            Point3::new(x, y, z),
            Vector3::unit_z(),
            Rad(2.0 * PI),
            4,
        );
        let disc = builder::try_attach_plane(&[circle]).unwrap();
        builder::tsweep(&disc, Vector3::new(0.0, 0.0, self.height))
    }

    fn volume(&self) -> f64 {
        PI * self.radius * self.radius * self.height
    }
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

/// A box from the origin to `size`, turned about `axis` by `angle` and
/// moved by `at`.
fn turned_box(rng: &mut Lcg) -> (Body, Solid, f64) {
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
    let angle = rng.range(0.0, PI);
    let at = (
        rng.range(-1.5, 0.5),
        rng.range(-1.5, 0.5),
        rng.range(-1.5, 0.5),
    );
    let (x, y, z) = size;
    let a = V3::new(axis.0, axis.1, axis.2).unwrap();
    let t = Transform3::rotation(a, angle)
        .unwrap()
        .then(Transform3::translation(V3::new(at.0, at.1, at.2).unwrap()));
    let ours = Body::cuboid(P3::ORIGIN, P3::new(x, y, z).unwrap())
        .unwrap()
        .transformed(t, 1.0)
        .unwrap();
    let v = builder::vertex(Point3::new(0.0, 0.0, 0.0));
    let e = builder::tsweep(&v, Vector3::new(x, 0.0, 0.0));
    let f = builder::tsweep(&e, Vector3::new(0.0, y, 0.0));
    let s = builder::tsweep(&f, Vector3::new(0.0, 0.0, z));
    let axis = Vector3::new(axis.0, axis.1, axis.2).normalize();
    let s = builder::rotated(&s, Point3::new(0.0, 0.0, 0.0), axis, Rad(angle));
    let truck = builder::translated(&s, Vector3::new(at.0, at.1, at.2));
    (ours, truck, x * y * z)
}

fn column(rng: &mut Lcg, x: f64, y: f64) -> Column {
    Column {
        at: (x, y, rng.range(-0.5, 0.5)),
        radius: rng.range(0.5, 1.5),
        height: rng.range(1.0, 3.0),
    }
}

/// The tally of one run.
#[derive(Default)]
struct Tally {
    ours_failed: usize,
    ours_worst: f64,
    truck_answered: usize,
    truck_failed: usize,
    truck_worst: f64,
    truck_within: usize,
}

impl Tally {
    /// One operation's outcomes against its exact volume.
    fn add(
        &mut self,
        label: &str,
        mine: &Outcome,
        theirs: &Outcome,
        exact: f64,
        scale: f64,
        tol: f64,
    ) {
        match volume_of(mine) {
            None => {
                self.ours_failed += 1;
                println!("{label}: ours failed");
            }
            Some(v) => {
                let off = (v - exact).abs() / scale;
                self.ours_worst = self.ours_worst.max(off);
                if off > 1e-9 {
                    println!("{label}: ours {v:.12}, exact {exact:.12}");
                }
            }
        }
        match volume_of(theirs) {
            None => self.truck_failed += 1,
            Some(w) => {
                self.truck_answered += 1;
                let off = (w - exact).abs() / scale;
                self.truck_worst = self.truck_worst.max(off);
                // A mesh within `tol` of a column of radius ≥ 0.5 misses
                // its volume by about 2·tol/0.5 of it.
                if off <= 10.0 * tol {
                    self.truck_within += 1;
                }
            }
        }
    }

    fn report(&self, what: &str, operations: usize) {
        println!("{what}, {operations} operations:");
        println!(
            "- ours fails: {}; worst volume error {:.1e} of the pair's volume",
            self.ours_failed, self.ours_worst
        );
        println!(
            "- Truck answers {} ({} within 10 × its tolerance of the exact volume, worst {:.1e}); fails {}",
            self.truck_answered, self.truck_within, self.truck_worst, self.truck_failed
        );
    }
}

/// Runs `pairs` pairs of parallel columns and as many columns with turned
/// boxes; true if ours always answered, exactly.
pub fn run(pairs: usize, tol: f64) -> bool {
    let mut rng = Lcg(20_261_116);
    let names = ["union", "intersection", "difference"];
    let mut parallel = Tally::default();
    for pair in 0..pairs {
        let a = column(&mut rng, 0.0, 0.0);
        // Apart, touching, crossing, one inside the other, on one axis.
        let reach = a.radius + 1.5;
        let d = match pair % 5 {
            0 => 0.0,
            _ => rng.range(0.0, reach),
        };
        let angle = rng.range(0.0, 2.0 * PI);
        let b = column(&mut rng, d * angle.cos(), d * angle.sin());
        let (oa, ob, ta, tb) = (a.ours(), b.ours(), a.truck(), b.truck());
        let low = a.at.2.max(b.at.2);
        let high = (a.at.2 + a.height).min(b.at.2 + b.height);
        let shared = lens(a.radius, b.radius, d) * (high - low).max(0.0);
        let (va, vb) = (a.volume(), b.volume());
        let exact = [va + vb - shared, shared, va - shared];
        for op in 0..3 {
            let label = format!("columns {pair} {}", names[op]);
            parallel.add(
                &label,
                &ours(&oa, &ob, op),
                &truck(&ta, &tb, op, tol),
                exact[op],
                va + vb,
                tol,
            );
        }
    }
    let mut boxed = Tally::default();
    for pair in 0..pairs {
        let a = column(&mut rng, 0.0, 0.0);
        let (ob, tb, vb) = turned_box(&mut rng);
        let (oa, ta) = (a.ours(), a.truck());
        let va = a.volume();
        let mine: [Outcome; 3] = std::array::from_fn(|op| ours(&oa, &ob, op));
        let theirs: [Outcome; 3] = std::array::from_fn(|op| truck(&ta, &tb, op, tol));
        // No closed form: our intersection is the referee's, checked by
        // the identities through the union and the difference.
        let shared = volume_of(&mine[1]).unwrap_or(f64::NAN);
        let exact = [va + vb - shared, shared, va - shared];
        for op in 0..3 {
            let label = format!("column and box {pair} {}", names[op]);
            boxed.add(&label, &mine[op], &theirs[op], exact[op], va + vb, tol);
        }
    }
    println!();
    parallel.report(
        &format!("Parallel columns, Truck at tolerance {tol:e}"),
        3 * pairs,
    );
    boxed.report(
        &format!("Columns with turned boxes (intersection: ours), Truck at tolerance {tol:e}"),
        3 * pairs,
    );
    parallel.ours_failed + boxed.ours_failed == 0
        && parallel.ours_worst <= 1e-9
        && boxed.ours_worst <= 1e-9
}
