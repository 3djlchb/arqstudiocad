//! Picking, window selection and object snaps (ADR 0016 D2, D5), on what
//! is drawn: the shapes of `drawing.rs`, so blocks count as a whole and
//! entities on off or frozen layers, which draw nothing, are never found.
//!
//! Exact geometry checks operate on drawn primitives. The core may supply
//! broad-phase candidates from its private spatial index. Distances and
//! apertures are in world units, converted from pixels by the interface.

use arq_geometry::snap::{
    SnapKind, SnapPoint, intersection_points, key_points, nearest_point, perpendicular_points,
    tangent_points,
};
use arq_geometry::{Bounds2, Geometry2, Line, Point2, Tolerance};

use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::EntityId;

const TOL: Tolerance = Tolerance::DEFAULT;

impl Document {
    /// The entity whose drawn geometry passes closest to `at`, within
    /// `aperture`; on a tie, the one drawn last (on top). A text or an
    /// entity not drawn yet is picked anywhere inside its box, as AutoCAD
    /// picks a text. `None` if nothing is that close, or the aperture is
    /// not a finite distance.
    pub fn pick(&self, at: Point2, aperture: f64) -> Option<EntityId> {
        if !(aperture.is_finite() && aperture >= 0.0) {
            return None;
        }
        self.pick_drawings(self.drawing_all(), at, aperture)
    }

    /// Picks exactly among broad-phase candidates supplied in drawing order.
    pub fn pick_candidates(
        &self,
        at: Point2,
        aperture: f64,
        candidates: &[EntityId],
    ) -> Option<EntityId> {
        if !(aperture.is_finite() && aperture >= 0.0) {
            return None;
        }
        self.pick_drawings(self.drawing_entities(candidates), at, aperture)
    }

    fn pick_drawings(
        &self,
        drawings: Vec<(EntityId, Vec<crate::drawing::DrawItem>)>,
        at: Point2,
        aperture: f64,
    ) -> Option<EntityId> {
        let mut best: Option<(EntityId, f64)> = None;
        for (id, items) in drawings {
            let Some(d) = items
                .iter()
                .map(|item| distance(item.shape, at))
                .min_by(f64::total_cmp)
            else {
                continue;
            };
            let tie = TOL.linear() * (1.0 + aperture);
            if d <= aperture && best.is_none_or(|(_, b)| d <= b + tie) {
                best = Some((id, d));
            }
        }
        best.map(|(id, _)| id)
    }

    /// The entities wholly inside `area` (a window), or, with `crossing`,
    /// those whose drawn geometry touches it; exact, not by bounding box.
    /// In drawing order.
    pub fn select_window(&self, area: Bounds2, crossing: bool) -> Vec<EntityId> {
        self.select_drawings(self.drawing_all(), area, crossing)
    }

    /// Selects exactly among broad-phase candidates supplied in drawing order.
    pub fn select_window_candidates(
        &self,
        area: Bounds2,
        crossing: bool,
        candidates: &[EntityId],
    ) -> Vec<EntityId> {
        self.select_drawings(self.drawing_entities(candidates), area, crossing)
    }

    fn select_drawings(
        &self,
        drawings: Vec<(EntityId, Vec<crate::drawing::DrawItem>)>,
        area: Bounds2,
        crossing: bool,
    ) -> Vec<EntityId> {
        drawings
            .into_iter()
            .filter(|(_, items)| !items.is_empty())
            .filter(|(_, items)| {
                if crossing {
                    items.iter().any(|item| touches(item.shape, area))
                } else {
                    items
                        .iter()
                        .all(|item| area.contains(shape_bounds(item.shape), TOL))
                }
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// The object snap for the cursor at `cursor`, among the active
    /// `modes`, within `aperture`; `from` is the last point, which
    /// Perpendicular and Tangent need.
    ///
    /// As AutoCAD: a point snap (Endpoint, Midpoint, Quadrant,
    /// Intersection) whose point lies within the aperture wins, the closest
    /// first; else Center, Perpendicular or Tangent of a curve the cursor
    /// rests on, wherever their point is; Nearest last. Snaps work on what
    /// is drawn, blocks and polylines included, and Intersection between
    /// any two curves.
    pub fn snap(
        &self,
        cursor: Point2,
        aperture: f64,
        modes: &[SnapKind],
        from: Option<Point2>,
    ) -> Option<SnapPoint> {
        if !(aperture.is_finite() && aperture >= 0.0) || modes.is_empty() {
            return None;
        }
        self.snap_drawings(self.drawing_all(), cursor, aperture, modes, from)
    }

    /// Snaps exactly among broad-phase candidates supplied in drawing order.
    pub fn snap_candidates(
        &self,
        cursor: Point2,
        aperture: f64,
        modes: &[SnapKind],
        from: Option<Point2>,
        candidates: &[EntityId],
    ) -> Option<SnapPoint> {
        if !(aperture.is_finite() && aperture >= 0.0) || modes.is_empty() {
            return None;
        }
        self.snap_drawings(
            self.drawing_entities(candidates),
            cursor,
            aperture,
            modes,
            from,
        )
    }

    fn snap_drawings(
        &self,
        drawings: Vec<(EntityId, Vec<crate::drawing::DrawItem>)>,
        cursor: Point2,
        aperture: f64,
        modes: &[SnapKind],
        from: Option<Point2>,
    ) -> Option<SnapPoint> {
        let on = |kind: SnapKind| modes.contains(&kind);
        // The curves the cursor rests on, closest first.
        let mut near: Vec<(Geometry2, f64)> = drawings
            .into_iter()
            .flat_map(|(_, items)| items)
            .filter_map(|item| curve(item.shape))
            .map(|g| (g, g.distance_to(cursor)))
            .filter(|(_, d)| *d <= aperture)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));

        let within = |p: &SnapPoint| p.point.distance_to(cursor) <= aperture;
        let closest = |points: Vec<SnapPoint>| {
            points.into_iter().min_by(|a, b| {
                a.point
                    .distance_to(cursor)
                    .total_cmp(&b.point.distance_to(cursor))
            })
        };

        let mut points: Vec<SnapPoint> = near
            .iter()
            .flat_map(|(g, _)| key_points(*g))
            .filter(|p| p.kind != SnapKind::Center && on(p.kind) && within(p))
            .collect();
        if on(SnapKind::Intersection) {
            for (i, (a, _)) in near.iter().enumerate() {
                for (b, _) in &near[i + 1..] {
                    points.extend(intersection_points(*a, *b, TOL).into_iter().filter(within));
                }
            }
        }
        if let Some(p) = closest(points) {
            return Some(p);
        }

        // The curve under the cursor decides; its point may be far away.
        for (g, _) in &near {
            let mut found: Vec<SnapPoint> = Vec::new();
            if on(SnapKind::Center) {
                found.extend(
                    key_points(*g)
                        .into_iter()
                        .filter(|p| p.kind == SnapKind::Center),
                );
            }
            if let Some(from) = from {
                if on(SnapKind::Perpendicular) {
                    found.extend(perpendicular_points(*g, from, TOL));
                }
                if on(SnapKind::Tangent) {
                    found.extend(tangent_points(*g, from, TOL));
                }
            }
            if let Some(p) = closest(found) {
                return Some(p);
            }
        }

        if on(SnapKind::Nearest) {
            return near.first().map(|(g, _)| nearest_point(*g, cursor));
        }
        None
    }
}

/// The geometry of a shape that is a curve.
fn curve(shape: Shape) -> Option<Geometry2> {
    match shape {
        Shape::Line(l) => Some(l.into()),
        Shape::Circle(c) => Some(c.into()),
        Shape::Arc(a) => Some(a.into()),
        Shape::Point(_) | Shape::Text(_) | Shape::Placeholder(_) => None,
    }
}

/// The box of a shape.
fn shape_bounds(shape: Shape) -> Bounds2 {
    match shape {
        Shape::Point(p) => Bounds2::from_point(p),
        Shape::Text(b) | Shape::Placeholder(b) => b,
        _ => curve(shape).map_or_else(|| unreachable!(), Geometry2::bounds),
    }
}

/// How far a shape is from `at`: to the curve, the point, or a box (zero
/// inside it).
fn distance(shape: Shape, at: Point2) -> f64 {
    match shape {
        Shape::Point(p) => p.distance_to(at),
        Shape::Text(b) | Shape::Placeholder(b) => {
            let dx = (b.min().x() - at.x()).max(at.x() - b.max().x()).max(0.0);
            let dy = (b.min().y() - at.y()).max(at.y() - b.max().y()).max(0.0);
            dx.hypot(dy)
        }
        _ => curve(shape).map_or(f64::INFINITY, |g| g.distance_to(at)),
    }
}

/// Whether a shape touches `area`: a point inside it, a box overlapping it,
/// a curve with a point inside it or crossing one of its edges.
fn touches(shape: Shape, area: Bounds2) -> bool {
    let bounds = shape_bounds(shape);
    if !area.intersects(bounds, TOL) {
        return false;
    }
    let Some(g) = curve(shape) else {
        return true; // a point or box whose box meets the area
    };
    if area.contains_point(g.point_at(0.0), TOL) {
        return true;
    }
    edges(area)
        .into_iter()
        .any(|edge| !arq_geometry::intersect(g, edge, TOL).is_empty())
}

/// The edges of a box; none of zero length (a flat box is one segment, a
/// point box has no edges).
fn edges(area: Bounds2) -> Vec<Geometry2> {
    let [a, b, c, d] = area.corners();
    [(a, b), (b, c), (c, d), (d, a)]
        .into_iter()
        .filter_map(|(p, q)| Line::new(p, q).ok().map(Geometry2::from))
        .collect()
}

#[cfg(test)]
#[path = "pick_tests.rs"]
mod tests;
