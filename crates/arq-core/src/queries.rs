//! Spatial queries of a [`CadDocument`] (contract §4, ADR 0011 D3.8).
//!
//! Queries are purely geometric: they cover every model-space entity whatever
//! its layer state, and callers filter (selection skips off and frozen
//! layers; zoom extents skips frozen ones). In this phase they scan all
//! entities; the spatial index of phase 7 replaces the scan behind the same
//! interface, measured by the `query` benchmark.

use arq_geometry::{Bounds2, Tolerance};

use crate::EntityId;
use crate::history::CadDocument;

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
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;
