//! `Geometry2`: any 2D curve the core knows, behind one type.

use crate::arc::Arc;
use crate::bounds::Bounds2;
use crate::circle::Circle;
use crate::error::GeometryResult;
use crate::line::Line;
use crate::point::Point2;
use crate::tolerance::Tolerance;

/// Which curve a [`Geometry2`] holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryKind {
    /// A straight segment.
    Line,
    /// A full circle.
    Circle,
    /// A circular arc.
    Arc,
}

impl GeometryKind {
    /// Stable English name, as used by DWG/DXF and the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Line => "LINE",
            Self::Circle => "CIRCLE",
            Self::Arc => "ARC",
        }
    }
}

/// Any 2D curve. The rest of the core (document, selection, rendering)
/// works with this type and never needs to know which curve it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry2 {
    /// A straight segment.
    Line(Line),
    /// A full circle.
    Circle(Circle),
    /// A circular arc.
    Arc(Arc),
}

/// Calls the same method on whichever curve `self` holds.
macro_rules! each {
    ($self:expr, $g:ident => $body:expr) => {
        match $self {
            Geometry2::Line($g) => $body,
            Geometry2::Circle($g) => $body,
            Geometry2::Arc($g) => $body,
        }
    };
}

impl Geometry2 {
    /// Which curve this is.
    pub fn kind(self) -> GeometryKind {
        match self {
            Self::Line(_) => GeometryKind::Line,
            Self::Circle(_) => GeometryKind::Circle,
            Self::Arc(_) => GeometryKind::Arc,
        }
    }

    /// Whether the curve ends where it starts (a circle).
    pub fn is_closed(self) -> bool {
        matches!(self, Self::Circle(_))
    }

    /// First point, for open curves; `None` for a circle.
    pub fn start_point(self) -> Option<Point2> {
        match self {
            Self::Line(l) => Some(l.start()),
            Self::Arc(a) => Some(a.start_point()),
            Self::Circle(_) => None,
        }
    }

    /// Last point, for open curves; `None` for a circle.
    pub fn end_point(self) -> Option<Point2> {
        match self {
            Self::Line(l) => Some(l.end()),
            Self::Arc(a) => Some(a.end_point()),
            Self::Circle(_) => None,
        }
    }

    /// Length of the curve.
    pub fn length(self) -> f64 {
        each!(self, g => g.length())
    }

    /// Bounding box.
    pub fn bounds(self) -> Bounds2 {
        each!(self, g => g.bounds())
    }

    /// Point at normalized parameter `t` in `[0, 1]`.
    pub fn point_at(self, t: f64) -> Point2 {
        each!(self, g => g.point_at(t))
    }

    /// Point at `distance` along the curve from its start.
    pub fn point_at_distance(self, distance: f64) -> GeometryResult<Point2> {
        each!(self, g => g.point_at_distance(distance))
    }

    /// Point of the curve closest to `p`.
    pub fn closest_point(self, p: Point2) -> Point2 {
        each!(self, g => g.closest_point(p))
    }

    /// Shortest distance from `p` to the curve.
    pub fn distance_to(self, p: Point2) -> f64 {
        each!(self, g => g.distance_to(p))
    }

    /// Whether `p` lies on the curve, within `tol.linear()`.
    pub fn contains_point(self, p: Point2, tol: Tolerance) -> bool {
        each!(self, g => g.contains_point(p, tol))
    }
}

impl From<Line> for Geometry2 {
    fn from(l: Line) -> Self {
        Self::Line(l)
    }
}

impl From<Circle> for Geometry2 {
    fn from(c: Circle) -> Self {
        Self::Circle(c)
    }
}

impl From<Arc> for Geometry2 {
    fn from(a: Arc) -> Self {
        Self::Arc(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn samples() -> [Geometry2; 3] {
        [
            Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into(),
            Circle::new(p(0.0, 0.0), 5.0).unwrap().into(),
            Arc::new(p(0.0, 0.0), 5.0, 0.0, FRAC_PI_2).unwrap().into(),
        ]
    }

    #[test]
    fn kinds_and_names() {
        let [l, c, a] = samples();
        assert_eq!(
            (l.kind(), c.kind(), a.kind()),
            (GeometryKind::Line, GeometryKind::Circle, GeometryKind::Arc)
        );
        assert_eq!(
            [l, c, a].map(|g| g.kind().name()),
            ["LINE", "CIRCLE", "ARC"]
        );
    }

    #[test]
    fn end_points_only_for_open_curves() {
        let [l, c, a] = samples();
        assert_eq!(l.start_point(), Some(p(0.0, 0.0)));
        assert_eq!(l.end_point(), Some(p(10.0, 0.0)));
        assert!(c.is_closed() && c.start_point().is_none() && c.end_point().is_none());
        assert!(a.start_point().unwrap().approx_eq(p(5.0, 0.0), TOL));
        assert!(a.end_point().unwrap().approx_eq(p(0.0, 5.0), TOL));
        assert!(!l.is_closed() && !a.is_closed());
    }

    #[test]
    fn delegates_to_the_curve() {
        let [l, c, a] = samples();
        assert_eq!(l.length(), 10.0);
        assert_eq!(c.bounds(), Bounds2::new(p(-5.0, -5.0), p(5.0, 5.0)));
        assert!(a.contains_point(p(3.0, 4.0), TOL));
        assert_eq!(l.closest_point(p(4.0, 2.0)), p(4.0, 0.0));
        assert_eq!(c.distance_to(p(8.0, 0.0)), 3.0);
        assert!(l.point_at_distance(11.0).is_err());
        assert_eq!(l.point_at(0.5), p(5.0, 0.0));
    }
}
