//! Spatial queries of a [`CadDocument`] (contract §4, ADR 0011 D3.8).
//!
//! Queries are purely geometric: they cover every model-space entity whatever
//! its layer state, and callers filter (selection skips off and frozen
//! layers; zoom extents skips frozen ones). In this phase they scan all
//! entities; the spatial index of phase 7 replaces the scan behind the same
//! interface, measured by the `query` benchmark.

use arq_geometry::snap::{SnapKind, SnapPoint};
use arq_geometry::{Bounds2, Point2, Tolerance};

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
        let tol = Tolerance::default();
        self.document()
            .entity_bounds()
            .filter(|(_, b)| b.intersects(area, tol))
            .map(|(id, _)| id)
            .collect()
    }

    /// The entity whose drawn geometry passes closest to `at` within
    /// `aperture` (world units), the one drawn last on a tie; never one on
    /// an off or frozen layer (ADR 0016 D2). A block counts as a whole.
    pub fn pick(&self, at: Point2, aperture: f64) -> CadResult<Option<EntityId>> {
        check_aperture(aperture)?;
        Ok(self.document().pick(at, aperture))
    }

    /// The entities wholly inside `area` (a window) or, with `crossing`,
    /// those whose drawn geometry touches it, exactly; none on an off or
    /// frozen layer (ADR 0016 D2). In drawing order.
    pub fn select_window(&self, area: Bounds2, crossing: bool) -> Vec<EntityId> {
        self.document().select_window(area, crossing)
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
        Ok(self.document().snap(cursor, aperture, modes, from))
    }
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
