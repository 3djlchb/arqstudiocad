//! Spatial queries of a [`CadDocument`] (contract §4, ADR 0011 D3.8).
//!
//! Queries are purely geometric: they cover every model-space entity whatever
//! its layer state, and callers filter (selection skips off and frozen
//! layers; zoom extents skips frozen ones). The private spatial index returns
//! bounding-box candidates in drawing order.

use arq_geometry::snap::{SnapKind, SnapPoint};
use arq_geometry::{Bounds2, Point2, SelectionPolygon};

use crate::error::{CadError, CadResult};
use crate::history::CadDocument;
use crate::{EntityId, Grip};

impl CadDocument {
    /// The box around every model-space entity that has plan bounds; `None`
    /// for an empty drawing (zoom extents).
    pub fn bounds(&self) -> Option<Bounds2> {
        self.document()
            .entity_bounds()
            .map(|(_, b)| b)
            .reduce(Bounds2::union)
    }

    /// Ids of the model-space entities whose bounding box touches `area`, in
    /// drawing order. These are candidates: the curve itself may miss `area`
    /// (a diagonal line, an arc), and callers that need an exact answer
    /// (selection) check the geometry.
    pub fn query_entities(&self, area: Bounds2) -> Vec<EntityId> {
        self.spatial_index.query(area)
    }

    /// Conservative candidates for a parallel-view XYZ snap aperture.
    /// The infinite sight line's XY projection is clipped to the index root,
    /// then queried in bounded pieces. This uses exact entity bounds, not a
    /// tessellated projected curve that can miss an extremum at high zoom.
    pub fn query_point_ray(
        &self,
        origin: arq_geometry::Point3,
        direction: arq_geometry::Vector3,
        aperture: f64,
    ) -> CadResult<Vec<EntityId>> {
        check_aperture(aperture)?;
        let direction = direction.normalized()?;
        let Some(bounds) = self.spatial_index.bounds() else {
            return Ok(Vec::new());
        };
        let all = || self.query_entities(bounds);
        let Some(limit) = bounds.expanded(aperture).ok() else {
            return Ok(all());
        };
        let o = [origin.x(), origin.y()];
        let d = [direction.x(), direction.y()];
        let lower = [limit.min().x(), limit.min().y()];
        let upper = [limit.max().x(), limit.max().y()];
        let mut lo = f64::NEG_INFINITY;
        let mut hi = f64::INFINITY;
        for i in 0..2 {
            if d[i] == 0.0 {
                if o[i] < lower[i] || o[i] > upper[i] {
                    return Ok(Vec::new());
                }
            } else {
                let a = (lower[i] - o[i]) / d[i];
                let b = (upper[i] - o[i]) / d[i];
                lo = lo.max(a.min(b));
                hi = hi.min(a.max(b));
            }
        }
        if lo > hi {
            return Ok(Vec::new());
        }
        if d == [0.0, 0.0] {
            let Some(area) = Point2::new(o[0], o[1])
                .ok()
                .and_then(|p| aperture_area(p, aperture))
            else {
                return Ok(all());
            };
            return Ok(self.query_entities(area));
        }
        if !lo.is_finite() || !hi.is_finite() {
            return Ok(all());
        }
        // Bound query count without imposing a model-unit length or CPU ISA.
        const PIECES: usize = 64;
        let mut ids = std::collections::BTreeSet::new();
        let point = |t: f64| Point2::new(o[0] + d[0] * t, o[1] + d[1] * t);
        for i in 0..PIECES {
            let t0 = lo + (hi - lo) * (i as f64 / PIECES as f64);
            let t1 = lo + (hi - lo) * ((i + 1) as f64 / PIECES as f64);
            let (Ok(a), Ok(b)) = (point(t0), point(t1)) else {
                return Ok(all());
            };
            let Ok(area) = Bounds2::new(a, b).expanded(aperture) else {
                return Ok(all());
            };
            ids.extend(self.query_entities(area));
        }
        Ok(ids.into_iter().collect())
    }

    /// The entity whose drawn geometry passes closest to `at` within
    /// `aperture` (world units), the one drawn last on a tie; never one on
    /// an off or frozen layer (ADR 0016 D2). A block counts as a whole.
    pub fn pick(&self, at: Point2, aperture: f64) -> CadResult<Option<EntityId>> {
        check_aperture(aperture)?;
        let Some(area) = aperture_area(at, aperture) else {
            return Ok(self.document().pick(at, aperture));
        };
        let candidates = self.query_entities(area);
        Ok(self.document().pick_candidates(at, aperture, &candidates))
    }

    /// The entities wholly inside `area` (a window) or, with `crossing`,
    /// those whose drawn geometry touches it, exactly; none on an off or
    /// frozen layer (ADR 0016 D2). In drawing order.
    pub fn select_window(&self, area: Bounds2, crossing: bool) -> Vec<EntityId> {
        let candidates = self.query_entities(area);
        self.document()
            .select_window_candidates(area, crossing, &candidates)
    }

    /// Selects visible entities meeting a validated polygon, in drawing order.
    pub fn select_crossing_polygon(&self, polygon: &SelectionPolygon) -> Vec<EntityId> {
        let bounds = polygon.bounds();
        let broad = bounds
            .expanded(arq_geometry::Tolerance::DEFAULT.linear())
            .unwrap_or(bounds);
        let candidates = self.query_entities(broad);
        self.document()
            .select_polygon_candidates(polygon, &candidates)
    }

    /// Ordered TRIM/EXTEND picks along a fence, or a clockwise crossing
    /// rectangle given by two opposite corners. Each entity occurs once.
    /// This read-only query never changes geometry or history.
    pub fn fence_picks(
        &self,
        points: &[Point2],
        crossing: bool,
    ) -> CadResult<Vec<(EntityId, Point2)>> {
        if points.len() < 2 || (crossing && points.len() != 2) {
            return Err(CadError::InvalidGeometry(
                "a fence needs two or more points; a crossing needs two corners".into(),
            ));
        }
        let mut path = points.to_vec();
        let mut area = None;
        if crossing {
            let (a, b) = (points[0], points[1]);
            let horizontal = Point2::new(b.x(), a.y())?;
            let vertical = Point2::new(a.x(), b.y())?;
            path = if (b.x() > a.x()) == (b.y() > a.y()) {
                vec![a, vertical, b, horizontal, a]
            } else {
                vec![a, horizontal, b, vertical, a]
            };
            area = Some(Bounds2::new(a, b));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut picks = Vec::new();
        for segment in path.windows(2) {
            if segment[0] == segment[1] {
                continue;
            }
            let area = Bounds2::new(segment[0], segment[1])
                .expanded(arq_geometry::Tolerance::default().linear())?;
            let candidates: Vec<_> = self
                .query_entities(area)
                .into_iter()
                .filter(|id| !seen.contains(id))
                .collect();
            for (id, point) in self
                .document()
                .fence_candidates(segment[0], segment[1], &candidates)
            {
                if seen.insert(id) {
                    picks.push((id, point));
                }
            }
        }
        if let Some(area) = area {
            for id in self.select_window(area, false) {
                if seen.contains(&id) {
                    continue;
                }
                let entity = self
                    .document()
                    .entity(id)
                    .ok_or(CadError::EntityNotFound(id))?;
                let point = if let Some(plan) = entity.plan() {
                    Some(plan.geometry().point_at(0.5))
                } else if entity.kind() == "LWPOLYLINE" {
                    self.grips(&[id]).first().map(|g| g.point)
                } else {
                    None
                };
                if let Some(point) = point {
                    seen.insert(id);
                    picks.push((id, point));
                }
            }
        }
        Ok(picks)
    }

    /// The grips of `ids` (ADR 0016 D7), entity by entity in the order
    /// given; kinds without grips yet have none.
    pub fn grips(&self, ids: &[EntityId]) -> Vec<Grip> {
        self.document().grips(ids)
    }

    /// The object snap for the cursor at `cursor` among the active `modes`
    /// within `aperture` (world units); `from` is the last point, for
    /// Perpendicular and Tangent (ADR 0016 D5). See
    /// [`Document::snap`](arq_document::Document::snap) for the order.
    pub fn snap(
        &self,
        cursor: Point2,
        aperture: f64,
        modes: &[SnapKind],
        from: Option<Point2>,
    ) -> CadResult<Option<SnapPoint>> {
        check_aperture(aperture)?;
        let Some(area) = aperture_area(cursor, aperture) else {
            return Ok(self.document().snap(cursor, aperture, modes, from));
        };
        let candidates = self.query_entities(area);
        Ok(self
            .document()
            .snap_candidates(cursor, aperture, modes, from, &candidates))
    }
}

/// The square broad-phase window for a circular pick/snap aperture. If its
/// coordinates overflow, callers use the exact full-scan path instead.
fn aperture_area(center: Point2, aperture: f64) -> Option<Bounds2> {
    let min = Point2::new(center.x() - aperture, center.y() - aperture).ok()?;
    let max = Point2::new(center.x() + aperture, center.y() + aperture).ok()?;
    Some(Bounds2::new(min, max))
}

/// An aperture is a finite distance, zero or more.
fn check_aperture(aperture: f64) -> CadResult<()> {
    if aperture.is_finite() && aperture >= 0.0 {
        Ok(())
    } else {
        Err(CadError::InvalidGeometry(format!(
            "aperture must be a finite distance of zero or more, not {aperture}"
        )))
    }
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "point_ray_tests.rs"]
mod point_ray_tests;
