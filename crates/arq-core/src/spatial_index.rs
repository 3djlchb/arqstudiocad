//! The document's private, derived index for entity bounding boxes.

use std::collections::HashMap;

use arq_document::{ChangeSet, Document, EntityId};
use arq_geometry::{Bounds2, Tolerance};
use rstar::{AABB, RTree, RTreeObject};

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: EntityId,
    order: usize,
    bounds: Bounds2,
    envelope: AABB<[f64; 2]>,
}

impl Entry {
    fn new(id: EntityId, order: usize, bounds: Bounds2) -> Self {
        Self {
            id,
            order,
            bounds,
            envelope: envelope(bounds),
        }
    }
}

impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// A bounding-box index kept in step with the document's committed changes.
pub(crate) struct SpatialIndex {
    tree: RTree<Entry>,
    entries: HashMap<EntityId, Entry>,
    /// Drawing order survives deletion and undo for the life of this document.
    order: HashMap<EntityId, usize>,
    next_order: usize,
}

impl SpatialIndex {
    pub(crate) fn from_document(document: &Document) -> Self {
        let mut entries = HashMap::new();
        let mut order = HashMap::new();
        let mut indexed = Vec::new();
        for (position, (id, bounds)) in document.entity_bounds().enumerate() {
            let entry = Entry::new(id, position, bounds);
            order.insert(id, position);
            entries.insert(id, entry.clone());
            indexed.push(entry);
        }
        let next_order = indexed.len();
        Self {
            tree: RTree::bulk_load(indexed),
            entries,
            order,
            next_order,
        }
    }

    pub(crate) fn query(&self, area: Bounds2) -> Vec<EntityId> {
        let tol = Tolerance::default();
        let search = area.expanded(tol.linear()).unwrap_or(area);
        let mut candidates: Vec<_> = self
            .tree
            .locate_in_envelope_intersecting(envelope(search))
            .filter(|entry| entry.bounds.intersects(area, tol))
            .map(|entry| (entry.order, entry.id))
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().map(|(_, id)| id).collect()
    }

    pub(crate) fn apply_change(&mut self, document: &Document, change: &ChangeSet) {
        for &id in change.deleted() {
            self.remove(id);
        }
        for &id in change.modified().iter().chain(change.created()) {
            let bounds = document.entity(id).and_then(|entity| entity.bounds());
            if self.entries.get(&id).map(|entry| entry.bounds) == bounds {
                continue;
            }
            self.remove(id);
            if let Some(bounds) = bounds {
                let order = *self.order.entry(id).or_insert_with(|| {
                    let order = self.next_order;
                    self.next_order += 1;
                    order
                });
                let entry = Entry::new(id, order, bounds);
                self.tree.insert(entry.clone());
                self.entries.insert(id, entry);
            }
        }
    }

    fn remove(&mut self, id: EntityId) {
        if let Some(entry) = self.entries.remove(&id) {
            self.tree.remove(&entry);
        }
    }
}

fn envelope(bounds: Bounds2) -> AABB<[f64; 2]> {
    let min = bounds.min();
    let max = bounds.max();
    AABB::from_corners([min.x(), min.y()], [max.x(), max.y()])
}
