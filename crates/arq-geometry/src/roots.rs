//! Zeros of smooth periodic functions of one parameter, as an ellipse's
//! intersections, perpendiculars and tangencies need (ADR 0041): every
//! crossing of zero is bracketed and refined, and every place where the
//! function comes close to zero without crossing it is offered as a
//! candidate touch for the caller to judge by distance.

use std::f64::consts::TAU;

/// Samples taken over a whole turn. A trigonometric polynomial of degree
/// two, which every case here is, has at most four zeros and four turns,
/// so sampling this finely separates them but for a pair nearer than a
/// 256th of a turn, that is a near tangency.
const SAMPLES: usize = 256;

/// A zero of a periodic function, by its parameter in `[0, 2π)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Zero {
    /// Where it crosses zero.
    Crossing(f64),
    /// Where it turns: a touch, if near enough zero.
    Touch(f64),
    /// Two crossings nearer than the samples, and where it turns between
    /// them: a tangency the rounding has split, if near enough.
    Pair {
        /// The first crossing.
        first: f64,
        /// The second crossing.
        second: f64,
        /// Where it turns between them.
        turn: f64,
    },
}

/// A trigonometric polynomial of degree two in `t`:
/// `cos2·cos 2t + sin2·sin 2t + cos·cos t + sin·sin t + constant`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TrigPoly {
    pub cos2: f64,
    pub sin2: f64,
    pub cos: f64,
    pub sin: f64,
    pub constant: f64,
}

impl TrigPoly {
    /// Its value where the sine and cosine of `t` are `(s, c)`.
    fn value_sc(self, s: f64, c: f64) -> f64 {
        self.cos2 * (c * c - s * s)
            + self.sin2 * (2.0 * s * c)
            + self.cos * c
            + self.sin * s
            + self.constant
    }

    /// Its derivative where the sine and cosine of `t` are `(s, c)`.
    fn slope_sc(self, s: f64, c: f64) -> f64 {
        -2.0 * self.cos2 * (2.0 * s * c) + 2.0 * self.sin2 * (c * c - s * s) - self.cos * s
            + self.sin * c
    }

    /// Its second derivative where the sine and cosine of `t` are `(s, c)`.
    fn curvature_sc(self, s: f64, c: f64) -> f64 {
        -4.0 * self.cos2 * (c * c - s * s)
            - 4.0 * self.sin2 * (2.0 * s * c)
            - self.cos * c
            - self.sin * s
    }

    /// Its value and its derivative at `t`.
    fn value_and_slope(self, t: f64) -> (f64, f64) {
        let (s, c) = t.sin_cos();
        (self.value_sc(s, c), self.slope_sc(s, c))
    }

    /// Its derivative and its second derivative at `t`.
    fn slope_and_curvature(self, t: f64) -> (f64, f64) {
        let (s, c) = t.sin_cos();
        (self.slope_sc(s, c), self.curvature_sc(s, c))
    }

    /// Its value at `t`.
    pub fn value(self, t: f64) -> f64 {
        let (s, c) = t.sin_cos();
        self.value_sc(s, c)
    }
}

/// The sines and cosines of the samples, computed once.
fn samples() -> &'static [(f64, f64)] {
    static SAMPLED: std::sync::OnceLock<Vec<(f64, f64)>> = std::sync::OnceLock::new();
    SAMPLED.get_or_init(|| {
        (0..SAMPLES)
            .map(|i| (TAU * i as f64 / SAMPLES as f64).sin_cos())
            .collect()
    })
}

/// Where `poly`, periodic over a turn, is zero: its crossings of zero,
/// refined to the parameter's precision; its turning points, where its
/// derivative crosses zero, each a candidate touch for the caller to
/// judge; and pairs of crossings nearer than the samples, with the turning
/// point between them. A turning point past zero between two samples of
/// the other sign holds two crossings the samples missed.
pub(crate) fn periodic_zeros(poly: TrigPoly) -> Vec<Zero> {
    let f = |t: f64| poly.value_and_slope(t);
    let df = |t: f64| poly.slope_and_curvature(t);
    let step = TAU / SAMPLES as f64;
    let at = |i: usize| step * i as f64;
    let table = samples();
    let values: Vec<f64> = table.iter().map(|&(s, c)| poly.value_sc(s, c)).collect();
    let slopes: Vec<f64> = table.iter().map(|&(s, c)| poly.slope_sc(s, c)).collect();
    let value = |i: usize| values[i % SAMPLES];
    let slope = |i: usize| slopes[i % SAMPLES];
    let sign_change = |a: f64, b: f64| a == 0.0 || (a.signum() != b.signum() && b != 0.0);
    let mut crossings = Vec::new();
    let mut turns = Vec::new();
    for i in 0..SAMPLES {
        let (a, b) = (value(i), value(i + 1));
        if a == 0.0 {
            crossings.push(at(i));
        } else if sign_change(a, b) {
            crossings.push(refine(&f, at(i), at(i + 1)));
        }
        if sign_change(slope(i), slope(i + 1)) {
            let turn = if slope(i) == 0.0 {
                at(i)
            } else {
                refine(&df, at(i), at(i + 1))
            };
            if a != 0.0 && !sign_change(a, b) && poly.value(turn).signum() != a.signum() {
                // Past zero and back between two samples.
                crossings.push(refine(&f, at(i), turn));
                crossings.push(refine(&f, turn, at(i + 1)));
            }
            turns.push(turn);
        }
    }
    let wrap = |t: f64| crate::angle::normalize(t);
    let mut crossings: Vec<f64> = crossings.into_iter().map(wrap).collect();
    crossings.sort_by(f64::total_cmp);
    crossings.dedup();
    let mut turns: Vec<f64> = turns.into_iter().map(wrap).collect();
    // Neighboring crossings nearer than a sample, a turning point between
    // them, are a pair, going round.
    let n = crossings.len();
    let mut paired = vec![false; n];
    let mut zeros = Vec::new();
    for k in 0..n {
        let next = (k + 1) % n;
        if n < 2 || paired[k] || paired[next] {
            continue;
        }
        let (first, second) = (crossings[k], crossings[next]);
        let gap = crate::angle::ccw_sweep(first, second);
        let between = turns
            .iter()
            .position(|&t| crate::angle::ccw_sweep(first, t) <= gap);
        if let (true, Some(index)) = (gap < step, between) {
            zeros.push(Zero::Pair {
                first,
                second,
                turn: turns.swap_remove(index),
            });
            paired[k] = true;
            paired[next] = true;
        }
    }
    zeros.extend(
        crossings
            .iter()
            .zip(&paired)
            .filter(|(_, paired)| !**paired)
            .map(|(&t, _)| Zero::Crossing(t)),
    );
    zeros.extend(turns.into_iter().map(Zero::Touch));
    zeros
}

/// A zero of a function between `a` and `b`, where it changes sign, `g`
/// giving its value and its derivative: Newton's method, falling back to
/// halving the bracket whenever a step would leave it.
fn refine(g: &impl Fn(f64) -> (f64, f64), mut a: f64, mut b: f64) -> f64 {
    let (mut fa, _) = g(a);
    if fa == 0.0 {
        return a;
    }
    let mut t = (a + b) / 2.0;
    for _ in 0..100 {
        let (ft, dt) = g(t);
        if ft == 0.0 {
            return t;
        }
        if ft.signum() == fa.signum() {
            a = t;
            fa = ft;
        } else {
            b = t;
        }
        let step = ft / dt;
        let newton = t - step;
        if step.abs() <= 2.0 * f64::EPSILON * t.abs().max(1.0) && newton >= a && newton <= b {
            return newton;
        }
        let next = if dt != 0.0 && newton > a && newton < b {
            newton
        } else {
            (a + b) / 2.0
        };
        if next <= a || next >= b {
            break;
        }
        t = next;
    }
    (a + b) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn crossings(zeros: &[Zero]) -> Vec<f64> {
        zeros
            .iter()
            .filter_map(|z| match z {
                Zero::Crossing(t) => Some(*t),
                _ => None,
            })
            .collect()
    }

    fn touches(zeros: &[Zero]) -> Vec<f64> {
        zeros
            .iter()
            .filter_map(|z| match z {
                Zero::Touch(t) => Some(*t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_polynomial_of_degree_two_and_its_slope() {
        let p = TrigPoly {
            cos2: 0.7,
            sin2: -0.2,
            cos: 1.5,
            sin: 0.3,
            constant: -0.4,
        };
        for t in [0.0f64, 0.4, 2.0, 5.5] {
            let direct =
                0.7 * (2.0 * t).cos() - 0.2 * (2.0 * t).sin() + 1.5 * t.cos() + 0.3 * t.sin() - 0.4;
            assert!((p.value(t) - direct).abs() < 1e-14);
            let h = 1e-6;
            let numeric = (p.value(t + h) - p.value(t - h)) / (2.0 * h);
            assert!((p.value_and_slope(t).1 - numeric).abs() < 1e-8);
            let bend = (p.value_and_slope(t + h).1 - p.value_and_slope(t - h).1) / (2.0 * h);
            assert!((p.slope_and_curvature(t).1 - bend).abs() < 1e-8);
        }
    }

    #[test]
    fn crossings_touches_and_pairs_of_trigonometric_polynomials() {
        let near = |a: f64, b: f64| {
            crate::angle::ccw_sweep(a, b).min(crate::angle::ccw_sweep(b, a)) < 1e-12
        };
        // cos t crosses zero at π/2 and 3π/2, and turns at 0 and π.
        let z = periodic_zeros(TrigPoly {
            cos: 1.0,
            ..TrigPoly::default()
        });
        let c = crossings(&z);
        assert_eq!(c.len(), 2, "{z:?}");
        assert!(near(c[0], TAU / 4.0) && near(c[1], 3.0 * TAU / 4.0));
        let t = touches(&z);
        assert_eq!(t.len(), 2, "{z:?}");
        assert!(t.iter().any(|&x| near(x, 0.0)) && t.iter().any(|&x| near(x, PI)));
        // 1 - cos(t - 0.3) touches zero at 0.3 without crossing.
        let z = periodic_zeros(TrigPoly {
            cos: -0.3f64.cos(),
            sin: -0.3f64.sin(),
            constant: 1.0,
            ..TrigPoly::default()
        });
        assert!(crossings(&z).is_empty(), "{z:?}");
        assert!(
            touches(&z).iter().any(|&x| (x - 0.3).abs() < 1e-14),
            "{z:?}"
        );
        // A zero falling on a sample is a crossing.
        assert_eq!(
            crossings(&periodic_zeros(TrigPoly {
                cos: -1.0,
                constant: 1.0,
                ..TrigPoly::default()
            })),
            [0.0]
        );
        // Two crossings closer than the samples, between two of them:
        // cos(t - 0.01) - cos 0.005, at 0.005 and 0.015, turning at 0.01.
        let z = periodic_zeros(TrigPoly {
            cos: 0.01f64.cos(),
            sin: 0.01f64.sin(),
            constant: -0.005f64.cos(),
            ..TrigPoly::default()
        });
        let pairs: Vec<_> = z
            .iter()
            .filter(|z| matches!(z, Zero::Pair { .. }))
            .collect();
        let [
            Zero::Pair {
                first,
                second,
                turn,
            },
        ] = pairs[..]
        else {
            panic!("{z:?}")
        };
        assert!(
            (first - 0.005).abs() < 1e-12 && (second - 0.015).abs() < 1e-12,
            "{z:?}"
        );
        assert!((turn - 0.01).abs() < 1e-14, "{z:?}");
        assert!(crossings(&z).is_empty(), "{z:?}");
        // The same across a sample, going round from just before 2π to
        // just after 0.
        let z = periodic_zeros(TrigPoly {
            cos: 1.0,
            constant: -0.005f64.cos(),
            ..TrigPoly::default()
        });
        let pairs: Vec<_> = z
            .iter()
            .filter(|z| matches!(z, Zero::Pair { .. }))
            .collect();
        let [
            Zero::Pair {
                first,
                second,
                turn,
            },
        ] = pairs[..]
        else {
            panic!("{z:?}")
        };
        assert!((first - (TAU - 0.005)).abs() < 1e-12 && (second - 0.005).abs() < 1e-12);
        assert!(near(*turn, 0.0), "{z:?}");
    }
}
