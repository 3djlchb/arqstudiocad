//! Angle helpers. Everything here is in **radians**: 0 points East (+X) and
//! counterclockwise is positive (ADR 0010). Degrees exist only in the user
//! interface, never in the core.

use std::f64::consts::{PI, TAU};

/// The same direction expressed in `[0, 2π)`.
pub fn normalize(angle: f64) -> f64 {
    let a = angle.rem_euclid(TAU);
    // rem_euclid of a tiny negative angle rounds up to exactly 2π.
    if a >= TAU { 0.0 } else { a }
}

/// Signed smallest rotation from `from` to `to`, in `(-π, π]`.
pub fn difference(from: f64, to: f64) -> f64 {
    let d = normalize(to - from);
    if d > PI { d - TAU } else { d }
}

/// Counterclockwise sweep from `start` to `end`, in `[0, 2π)`: how far an arc
/// that starts at `start` travels to reach `end` (DWG arc convention).
pub fn ccw_sweep(start: f64, end: f64) -> f64 {
    normalize(end - start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn normalize_wraps_into_range() {
        assert_eq!(normalize(0.0), 0.0);
        assert_eq!(normalize(TAU), 0.0);
        assert!((normalize(-FRAC_PI_2) - 3.0 * FRAC_PI_2).abs() < 1e-15);
        assert!((normalize(5.0 * PI) - PI).abs() < 1e-12);
        assert_eq!(normalize(-1e-17), 0.0, "tiny negative must not become 2π");
    }

    #[test]
    fn difference_takes_the_short_way() {
        assert!((difference(0.0, FRAC_PI_2) - FRAC_PI_2).abs() < 1e-15);
        assert!((difference(FRAC_PI_2, 0.0) + FRAC_PI_2).abs() < 1e-15);
        // From 350° to 10° is +20°, not -340°.
        let d = difference(350f64.to_radians(), 10f64.to_radians());
        assert!((d - 20f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn ccw_sweep_follows_dwg_arcs() {
        // An arc from 270° to 90° counterclockwise sweeps 180°.
        let s = ccw_sweep(3.0 * FRAC_PI_2, FRAC_PI_2);
        assert!((s - PI).abs() < 1e-12);
        // And from 90° to 270° too.
        assert!((ccw_sweep(FRAC_PI_2, 3.0 * FRAC_PI_2) - PI).abs() < 1e-12);
        // From 10° to 350° is 340° counterclockwise.
        let s = ccw_sweep(10f64.to_radians(), 350f64.to_radians());
        assert!((s - 340f64.to_radians()).abs() < 1e-12);
    }
}
