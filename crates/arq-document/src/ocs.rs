//! Plan view of entities stored in their Object Coordinate System (OCS).
//!
//! CIRCLE and ARC store coordinates in the OCS defined by their normal
//! (extrusion). Two cases are plan views (ADR 0011, D3.3):
//!
//! - normal `(0, 0, 1)`: the OCS is the world; coordinates are used as is;
//! - normal `(0, 0, -1)`: DWG's arbitrary-axis rule gives OCS X = world −X,
//!   OCS Y = world Y, OCS Z = world −Z. So X and Z are negated and every
//!   angle `a` becomes `π − a`, which also reverses an arc's direction.
//!   Other CADs write mirrored arcs this way; reading them as `(0, 0, 1)`
//!   puts them in the wrong place.
//!
//! Any other normal is a tilted plane: not editable in 2D.
//!
//! The approach follows IngeCAD's `core/ocs.py` (GPL-3.0-or-later).

use std::f64::consts::PI;

/// Which way a plan entity's normal points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanNormal {
    /// `(0, 0, 1)`: OCS equals the world.
    Up,
    /// `(0, 0, -1)`: mirrored OCS.
    Down,
}

/// The plan orientation of a normal, or `None` for a tilted plane.
pub(crate) fn plan_normal(x: f64, y: f64, z: f64) -> Option<PlanNormal> {
    let len = (x * x + y * y + z * z).sqrt();
    if !(len.is_finite() && len > 0.0) {
        return None;
    }
    let tilt = 1e-9 * len;
    if x.abs() > tilt || y.abs() > tilt {
        return None;
    }
    Some(if z > 0.0 {
        PlanNormal::Up
    } else {
        PlanNormal::Down
    })
}

impl PlanNormal {
    /// OCS `(x, y, z)` to world, and back: the mapping is its own inverse.
    pub(crate) fn map_point(self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        match self {
            Self::Up => (x, y, z),
            Self::Down => (-x, y, -z),
        }
    }

    /// An OCS angle to world, and back (its own inverse too).
    pub(crate) fn map_angle(self, a: f64) -> f64 {
        match self {
            Self::Up => a,
            Self::Down => PI - a,
        }
    }

    /// An arc's `(start, end)` angles between OCS and world. A mirrored OCS
    /// reverses the direction, so start and end swap to stay counterclockwise.
    pub(crate) fn map_arc(self, start: f64, end: f64) -> (f64, f64) {
        match self {
            Self::Up => (start, end),
            Self::Down => (self.map_angle(end), self.map_angle(start)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn classifies_normals() {
        assert_eq!(plan_normal(0.0, 0.0, 1.0), Some(PlanNormal::Up));
        assert_eq!(plan_normal(0.0, 0.0, -1.0), Some(PlanNormal::Down));
        assert_eq!(
            plan_normal(0.0, 0.0, 2.5),
            Some(PlanNormal::Up),
            "not normalized"
        );
        assert_eq!(
            plan_normal(1e-12, 0.0, -1.0),
            Some(PlanNormal::Down),
            "within tolerance"
        );
        assert_eq!(plan_normal(0.1, 0.0, 1.0), None, "tilted");
        assert_eq!(plan_normal(1.0, 0.0, 0.0), None, "vertical plane");
        assert_eq!(plan_normal(0.0, 0.0, 0.0), None, "degenerate");
    }

    #[test]
    fn down_negates_x_and_z() {
        assert_eq!(PlanNormal::Down.map_point(3.0, 4.0, 5.0), (-3.0, 4.0, -5.0));
        assert_eq!(PlanNormal::Up.map_point(3.0, 4.0, 5.0), (3.0, 4.0, 5.0));
    }

    #[test]
    fn down_reverses_arcs() {
        // OCS quarter 0 → 90° about −Z appears in the world as 90° → 180°.
        let (s, e) = PlanNormal::Down.map_arc(0.0, FRAC_PI_2);
        assert!((s - FRAC_PI_2).abs() < 1e-15 && (e - PI).abs() < 1e-15);
        // Mapping twice returns the original.
        let (s2, e2) = PlanNormal::Down.map_arc(s, e);
        assert!(s2.abs() < 1e-15 && (e2 - FRAC_PI_2).abs() < 1e-15);
    }
}
