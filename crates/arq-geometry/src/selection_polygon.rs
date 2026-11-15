//! Validated straight crossing polygons, over the existing Region predicates.

use crate::error::GeometryResult;
use crate::{
    Bounds2, Containment, Geometry2, GeometryError, Line, Point2, Region, Tolerance, intersect,
};

/// A simple, non-degenerate polygon, with either winding and no holes.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionPolygon {
    region: Region,
    edges: Vec<Line>,
    bounds: Bounds2,
}

impl SelectionPolygon {
    /// Validates the closed contour; an explicit repeated last corner is allowed.
    /// Self-crossings, self-touching, short edges and zero area are rejected.
    pub fn new(mut points: Vec<Point2>, tol: Tolerance) -> GeometryResult<Self> {
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if points.len() < 3 {
            return Err(GeometryError::ShortContour);
        }
        let edges: Vec<_> = (0..points.len())
            .map(|i| Line::new(points[i], points[(i + 1) % points.len()]))
            .collect::<GeometryResult<_>>()?;
        if edges.iter().any(|edge| edge.length() <= tol.linear()) {
            return Err(GeometryError::ZeroLength);
        }
        for (i, &a) in edges.iter().enumerate() {
            for (j, &b) in edges.iter().enumerate().skip(i + 1) {
                let hit = intersect(a.into(), b.into(), tol);
                let adjacent = j == i + 1 || (i == 0 && j == edges.len() - 1);
                if !hit.overlaps().is_empty() || (!adjacent && !hit.is_empty()) {
                    return Err(GeometryError::OutOfRange {
                        what: "crossing polygon",
                    });
                }
            }
        }
        let bounds = Bounds2::from_points(points.iter().copied()).expect("three corners");
        let region = Region::new(vec![points])?;
        let area = region.area().abs();
        if !area.is_finite() || area <= tol.linear() * tol.linear() {
            return Err(GeometryError::OutOfRange {
                what: "crossing polygon",
            });
        }
        Ok(Self {
            region,
            edges,
            bounds,
        })
    }

    /// Its corners, with implicit closure.
    pub fn vertices(&self) -> &[Point2] {
        &self.region.contours()[0]
    }

    /// Its broad-phase bounding rectangle.
    pub fn bounds(&self) -> Bounds2 {
        self.bounds
    }

    /// Boundary points count as inside, using the existing local-frame predicate.
    pub fn contains(&self, point: Point2, tol: Tolerance) -> bool {
        self.bounds.contains_point(point, tol)
            && self.region.contains(point, tol) != Containment::Outside
    }

    /// Whether an exact line, arc or circle meets the enclosed area or boundary.
    pub fn touches_curve(&self, curve: Geometry2, tol: Tolerance) -> bool {
        self.contains(curve.point_at(0.0), tol)
            || self
                .edges
                .iter()
                .any(|&edge| !intersect(curve, edge.into(), tol).is_empty())
    }
}

/// Selection provenance used by STRETCH's shared endpoint/vertex writer.
#[derive(Clone, Debug, PartialEq)]
pub enum CrossingArea {
    /// The existing crossing rectangle, including degenerate rectangles.
    Rectangle(Bounds2),
    /// A validated simple crossing polygon.
    Polygon(SelectionPolygon),
}

impl CrossingArea {
    /// Whether the endpoint or vertex belongs to this crossing selection.
    pub fn contains(&self, point: Point2, tol: Tolerance) -> bool {
        match self {
            Self::Rectangle(area) => area.contains_point(point, tol),
            Self::Polygon(area) => area.contains(point, tol),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circle;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }
    const TOL: Tolerance = Tolerance::DEFAULT;

    #[test]
    fn crossing_polygon_uses_concave_contour_both_windings_and_local_coordinates() {
        let corners = [
            p(0., 0.),
            p(4., 0.),
            p(4., 1.),
            p(1., 1.),
            p(1., 4.),
            p(0., 4.),
        ];
        for reverse in [false, true] {
            for origin in [0., 1e6] {
                let mut points: Vec<_> = corners
                    .iter()
                    .map(|q| p(q.x() + origin, q.y() + origin))
                    .collect();
                if reverse {
                    points.reverse();
                }
                let polygon = SelectionPolygon::new(points, TOL).unwrap();
                for (x, y) in [(0.5, 3.), (3., 0.5), (1., 2.), (0., 0.)] {
                    assert!(polygon.contains(p(x + origin, y + origin), TOL));
                }
                assert!(
                    !polygon.contains(p(2. + origin, 2. + origin), TOL),
                    "not the enclosing box"
                );
                assert!(!polygon.contains(p(5. + origin, 0. + origin), TOL));
                assert!(
                    polygon.touches_curve(
                        Line::new(p(-1. + origin, 0.5 + origin), p(5. + origin, 0.5 + origin))
                            .unwrap()
                            .into(),
                        TOL
                    )
                );
                assert!(
                    !polygon.touches_curve(
                        Circle::new(p(2. + origin, 2. + origin), 0.2)
                            .unwrap()
                            .into(),
                        TOL
                    )
                );
            }
        }
    }

    #[test]
    fn crossing_polygon_validates_closure_and_refuses_degenerate_or_self_touching_contours() {
        let valid =
            SelectionPolygon::new(vec![p(0., 0.), p(4., 0.), p(0., 4.), p(0., 0.)], TOL).unwrap();
        assert_eq!(valid.vertices().len(), 3);
        assert!(valid.contains(p(1., 1.), TOL));
        for corners in [
            vec![p(0., 0.), p(1., 0.)],
            vec![p(0., 0.), p(1., 0.), p(2., 0.)],
            vec![p(0., 0.), p(2., 2.), p(0., 2.), p(2., 0.)],
            vec![p(0., 0.), p(4., 0.), p(2., 0.), p(0., 4.)],
            vec![p(0., 0.), p(2., 0.), p(2., 2.), p(0., 0.), p(0., 2.)],
            vec![p(0., 0.), p(1e-12, 0.), p(0., 1.)],
        ] {
            assert!(SelectionPolygon::new(corners, TOL).is_err());
        }
    }
}
