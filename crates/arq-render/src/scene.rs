//! `Scene`: what to draw, derived from the document and kept up to date
//! from change sets (contract §8). Never the source of truth.

use std::collections::HashMap;

use arq_core::{CadChangeSet, Document, DrawItem, EntityId, Shape};
use arq_geometry::Bounds2;

/// One entity's shapes and their box.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) id: EntityId,
    pub(crate) items: Vec<DrawItem>,
    pub(crate) bounds: Option<Bounds2>,
}

/// The drawable content of a document, in drawing order.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    entries: Vec<Entry>,
    index: HashMap<EntityId, usize>,
}

impl Scene {
    /// The scene of every model-space entity of `doc`.
    pub fn build(doc: &Document) -> Self {
        let entries = doc
            .drawing_all()
            .into_iter()
            .map(|(id, items)| entry(id, items))
            .collect();
        let mut scene = Self {
            entries,
            index: HashMap::new(),
        };
        scene.reindex();
        scene
    }

    /// Brings the scene up to date after `changes`. Entities are redrawn
    /// one by one; a layer or document change (colors, visibility) redraws
    /// everything. Returns whether anything changed.
    pub fn update(&mut self, doc: &Document, changes: &CadChangeSet) -> bool {
        if changes.is_empty() {
            return false;
        }
        let tables = !changes.layers_created().is_empty()
            || !changes.layers_modified().is_empty()
            || !changes.layers_deleted().is_empty()
            || changes.document_changed();
        if tables {
            *self = Self::build(doc);
            return true;
        }
        if !changes.deleted().is_empty() {
            let gone: std::collections::HashSet<_> = changes.deleted().iter().collect();
            self.entries.retain(|e| !gone.contains(&e.id));
            self.reindex();
        }
        for &id in changes.modified() {
            let items = doc.drawing(id).unwrap_or_default();
            match self.index.get(&id) {
                Some(&i) => self.entries[i] = entry(id, items),
                None => self.push(entry(id, items)),
            }
        }
        for &id in changes.created() {
            if let Some(items) = doc.drawing(id) {
                match self.index.get(&id) {
                    Some(&i) => self.entries[i] = entry(id, items),
                    None => self.push(entry(id, items)),
                }
            }
        }
        true
    }

    /// The box around everything drawn; `None` when nothing is.
    pub fn bounds(&self) -> Option<Bounds2> {
        self.entries
            .iter()
            .filter_map(|e| e.bounds)
            .reduce(Bounds2::union)
    }

    /// How many shapes the scene holds.
    pub fn shape_count(&self) -> usize {
        self.entries.iter().map(|e| e.items.len()).sum()
    }

    /// The shapes of one entity, if it is in the scene.
    pub fn items(&self, id: EntityId) -> Option<&[DrawItem]> {
        self.index
            .get(&id)
            .map(|&i| self.entries[i].items.as_slice())
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn push(&mut self, e: Entry) {
        self.index.insert(e.id, self.entries.len());
        self.entries.push(e);
    }

    fn reindex(&mut self) {
        self.index = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id, i))
            .collect();
    }
}

fn entry(id: EntityId, items: Vec<DrawItem>) -> Entry {
    let bounds = items
        .iter()
        .map(|i| shape_bounds(i.shape))
        .reduce(Bounds2::union);
    Entry { id, items, bounds }
}

pub(crate) fn shape_bounds(shape: Shape) -> Bounds2 {
    match shape {
        Shape::Line(l) => l.bounds(),
        Shape::Circle(c) => c.bounds(),
        Shape::Arc(a) => a.bounds(),
        Shape::Point(p) => Bounds2::from_point(p),
        Shape::Text(b) | Shape::Placeholder(b) => b,
    }
}
