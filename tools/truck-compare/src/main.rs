//! Compares arq-kernel's booleans of planar solids with Truck's, on random
//! boxes in general position, where Truck works (ADR 0032 D4): the same
//! volume and the same bounds. Run by hand:
//!
//! ```sh
//! cd tools/truck-compare && cargo run --release -- [pairs]
//! ```
//!
//! Prints one line per disagreement and a summary; exits non-zero if the
//! two kernels disagree on a pair where both gave a result.

use std::f64::consts::PI;
use std::panic::{AssertUnwindSafe, catch_unwind};

use arq_geometry::{Point3 as P3, Transform3, Vector3 as V3};
use arq_kernel::Body;
use truck_meshalgo::prelude::*;
use truck_modeling::{Point3, Rad, Solid, Vector3, builder};

/// The same small generator as the kernel's tests: repeatable pairs.
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

/// A box from the origin to `size`, turned about `axis` by `angle` and
/// moved by `at`: in both kernels.
struct Placed {
    size: (f64, f64, f64),
    axis: (f64, f64, f64),
    angle: f64,
    at: (f64, f64, f64),
}

impl Placed {
    fn random(rng: &mut Lcg) -> Self {
        Self {
            size: (
                rng.range(0.5, 3.0),
                rng.range(0.5, 3.0),
                rng.range(0.5, 3.0),
            ),
            axis: (
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(0.1, 1.0),
            ),
            angle: rng.range(0.0, PI),
            at: (
                rng.range(-0.8, 0.8),
                rng.range(-0.8, 0.8),
                rng.range(-0.8, 0.8),
            ),
        }
    }

    fn ours(&self) -> Body {
        let (x, y, z) = self.size;
        let body = Body::cuboid(P3::ORIGIN, P3::new(x, y, z).unwrap()).unwrap();
        let axis = V3::new(self.axis.0, self.axis.1, self.axis.2).unwrap();
        let t = Transform3::rotation(axis, self.angle)
            .unwrap()
            .then(Transform3::translation(
                V3::new(self.at.0, self.at.1, self.at.2).unwrap(),
            ));
        body.transformed(t, 1.0).unwrap()
    }

    fn truck(&self) -> Solid {
        let (x, y, z) = self.size;
        let v = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let e = builder::tsweep(&v, Vector3::new(x, 0.0, 0.0));
        let f = builder::tsweep(&e, Vector3::new(0.0, y, 0.0));
        let s = builder::tsweep(&f, Vector3::new(0.0, 0.0, z));
        let axis = Vector3::new(self.axis.0, self.axis.1, self.axis.2).normalize();
        let s = builder::rotated(&s, Point3::new(0.0, 0.0, 0.0), axis, Rad(self.angle));
        builder::translated(&s, Vector3::new(self.at.0, self.at.1, self.at.2))
    }
}

/// A result's volume and bounds, empty, or none (an error, a panic).
#[derive(Debug)]
enum Outcome {
    Solid {
        volume: f64,
        low: [f64; 3],
        high: [f64; 3],
    },
    Empty,
    Failed,
}

fn ours(a: &Body, b: &Body, op: usize) -> Outcome {
    let result = match op {
        0 => a.union(b).map(Some),
        1 => a.intersection(b),
        _ => a.difference(b),
    };
    match result {
        Ok(Some(body)) => match body.mass_properties() {
            Ok(m) => {
                let (lo, hi) = (m.bounds.min(), m.bounds.max());
                Outcome::Solid {
                    volume: m.volume,
                    low: [lo.x(), lo.y(), lo.z()],
                    high: [hi.x(), hi.y(), hi.z()],
                }
            }
            Err(_) => Outcome::Failed,
        },
        Ok(None) => Outcome::Empty,
        Err(_) => Outcome::Failed,
    }
}

fn truck(a: &Solid, b: &Solid, op: usize, tol: f64) -> Outcome {
    let run = catch_unwind(AssertUnwindSafe(|| match op {
        0 => truck_shapeops::or(a, b, tol),
        1 => truck_shapeops::and(a, b, tol),
        _ => {
            let mut not_b = b.clone();
            not_b.not();
            truck_shapeops::and(a, &not_b, tol)
        }
    }));
    match run {
        Err(_) | Ok(None) => Outcome::Failed,
        Ok(Some(solid)) if solid.boundaries().iter().all(|s| s.is_empty()) => Outcome::Empty,
        Ok(Some(solid)) => {
            let mut mesh = solid.triangulation(tol).to_polygon();
            // Its mesh keeps positions no triangle uses: dropped, or they
            // widen its bounds.
            mesh.remove_unused_attrs();
            let bounds = mesh.bounding_box();
            let (lo, hi) = (bounds.min(), bounds.max());
            Outcome::Solid {
                volume: mesh.volume(),
                low: [lo.x, lo.y, lo.z],
                high: [hi.x, hi.y, hi.z],
            }
        }
    }
}

/// The volume of an outcome, 0 if empty, none if it failed.
fn volume_of(o: &Outcome) -> Option<f64> {
    match o {
        Outcome::Solid { volume, .. } => Some(*volume),
        Outcome::Empty => Some(0.0),
        Outcome::Failed => None,
    }
}

/// How far a kernel's union, intersection and difference miss
/// vol(A ∪ B) = vol(A) + vol(B) − vol(A ∩ B) and vol(A − B) = vol(A) −
/// vol(A ∩ B), the boxes' volumes exact; none if one of them failed.
fn miss(results: &[Outcome; 3], va: f64, vb: f64) -> Option<f64> {
    let (u, i, d) = (
        volume_of(&results[0])?,
        volume_of(&results[1])?,
        volume_of(&results[2])?,
    );
    Some((u - (va + vb - i)).abs().max((d - (va - i)).abs()))
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let pairs: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1000);
    // Truck's own tolerance: its booleans and its triangulation.
    let tol: f64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1e-6);
    let names = ["union", "intersection", "difference"];
    let mut rng = Lcg(20_261_107);
    let (mut agreed, mut empty, mut truck_none, mut differed) = (0, 0, 0, 0);
    let (mut ours_failed, mut ours_off, mut truck_off) = (0, 0, 0);
    let mut worst_ours: f64 = 0.0;
    for pair in 0..pairs {
        let (pa, pb) = (Placed::random(&mut rng), Placed::random(&mut rng));
        let (oa, ob, ta, tb) = (pa.ours(), pb.ours(), pa.truck(), pb.truck());
        let box_volume = |p: &Placed| p.size.0 * p.size.1 * p.size.2;
        let (va, vb) = (box_volume(&pa), box_volume(&pb));
        let scale = va + vb;
        let mine: [Outcome; 3] = std::array::from_fn(|op| ours(&oa, &ob, op));
        let theirs: [Outcome; 3] = std::array::from_fn(|op| truck(&ta, &tb, op, tol));
        // Ours always answers, and its answers always add up.
        match miss(&mine, va, vb) {
            None => ours_failed += 1,
            Some(m) => {
                worst_ours = worst_ours.max(m / scale);
                if m > 1e-12 * scale {
                    ours_off += 1;
                    println!("pair {pair}: ours misses the identities by {m:.1e}");
                }
            }
        }
        let truck_miss = miss(&theirs, va, vb);
        for (op, name) in names.iter().enumerate() {
            match (&mine[op], &theirs[op]) {
                (
                    Outcome::Solid {
                        volume: v,
                        low: l,
                        high: h,
                    },
                    Outcome::Solid {
                        volume: w,
                        low: m,
                        high: k,
                    },
                ) => {
                    let size = (0..3).map(|i| h[i] - l[i]).fold(0.0, f64::max);
                    let off = (0..3)
                        .map(|i| (l[i] - m[i]).abs().max((h[i] - k[i]).abs()))
                        .fold(0.0, f64::max);
                    if (v - w).abs() <= 1e-9 * scale && off <= 1e-9 * size {
                        agreed += 1;
                    } else {
                        differed += 1;
                        let verdict = match truck_miss {
                            Some(t) if t > 1e-9 * scale => {
                                truck_off += 1;
                                format!("Truck misses the identities by {t:.1e}")
                            }
                            Some(_) => "both add up".to_string(),
                            None => "Truck failed another operation".to_string(),
                        };
                        println!(
                            "pair {pair} {name}: volume ours {v:.12} Truck {w:.12}, bounds off {off:.1e}; {verdict}"
                        );
                    }
                }
                (Outcome::Empty, Outcome::Empty | Outcome::Failed) => empty += 1,
                (_, Outcome::Failed | Outcome::Empty) => truck_none += 1,
                (o, t) => {
                    differed += 1;
                    println!("pair {pair} {name}: ours {o:?}, Truck {t:?}");
                }
            }
        }
    }
    println!(
        "
{} operations on {pairs} pairs, Truck at tolerance {tol:e}:",
        3 * pairs
    );
    println!(
        "- both give a solid: {agreed} agree, {differed} differ ({truck_off} of them where Truck's own results miss the volume identities)"
    );
    println!("- empty in ours (the boxes apart): {empty}");
    println!("- Truck gives nothing or fails, ours answers: {truck_none}");
    println!(
        "- ours fails: {ours_failed}; ours misses the identities: {ours_off} (worst {worst_ours:.1e} of the boxes' volume)"
    );
    if ours_failed > 0 || ours_off > 0 || differed > truck_off {
        std::process::exit(1);
    }
}
