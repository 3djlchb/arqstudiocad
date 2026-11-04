//! `Scene`: what to draw, derived from the document and kept up to date
//! from change sets (contract §8). Never the source of truth.
//!
//! A scene is seen from one view (ADR 0030). In plan it holds the plan
//! shapes; from any other direction, each entity's polylines in space
//! projected onto the view's plane as segments, so the camera and the
//! rasterizer draw every view alike.

use std::collections::HashMap;

use arq_core::{CadChangeSet, Document, DrawItem, EntityId, Shape, SpaceItem};
use arq_geometry::{Bounds2, Line, Point2};

use crate::View;

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
    view: View,
}

impl Scene {
    /// The scene of every model-space entity of `doc`, in plan.
    pub fn build(doc: &Document) -> Self {
        Self::build_in(doc, View::Top)
    }

    /// The scene of every model-space entity of `doc`, seen from `view`.
    pub fn build_in(doc: &Document, view: View) -> Self {
        let drawn = match view {
            View::Top => doc.drawing_all(),
            _ => doc
                .space_drawing_all()
                .into_iter()
                .map(|(id, items)| (id, projected(&items, view)))
                .collect(),
        };
        let mut scene = Self {
            entries: drawn
                .into_iter()
                .map(|(id, items)| entry(id, items))
                .collect(),
            index: HashMap::new(),
            view,
        };
        scene.reindex();
        scene
    }

    /// The view it is seen from.
    pub fn view(&self) -> View {
        self.view
    }

    /// A scene of loose shapes, not of a document's entities: a preview's.
    pub(crate) fn from_items(items: Vec<DrawItem>) -> Self {
        let mut scene = Self {
            entries: vec![entry(0, items)],
            index: HashMap::new(),
            view: View::Top,
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
            *self = Self::build_in(doc, self.view);
            return true;
        }
        if !changes.deleted().is_empty() {
            let gone: std::collections::HashSet<_> = changes.deleted().iter().collect();
            self.entries.retain(|e| !gone.contains(&e.id));
            self.reindex();
        }
        for &id in changes.modified() {
            let items = self.drawing(doc, id).unwrap_or_default();
            match self.index.get(&id) {
                Some(&i) => self.entries[i] = entry(id, items),
                None => self.push(entry(id, items)),
            }
        }
        for &id in changes.created() {
            if let Some(items) = self.drawing(doc, id) {
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

    /// What entity `id` draws in this scene's view.
    fn drawing(&self, doc: &Document, id: EntityId) -> Option<Vec<DrawItem>> {
        match self.view {
            View::Top => doc.drawing(id),
            view => doc.space_drawing(id).map(|items| projected(&items, view)),
        }
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

/// Polylines in space as segments on `view`'s plane, in their colors; a
/// polyline of one point, or one seen end on, as a point; a text's box as
/// the box around its projection, which picks it.
fn projected(items: &[SpaceItem], view: View) -> Vec<DrawItem> {
    let mut out = Vec::new();
    for item in items {
        let points: Vec<Point2> = item
            .points
            .iter()
            .filter_map(|&p| {
                let (x, y) = view.project(p);
                Point2::new(x, y).ok()
            })
            .collect();
        let mut push = |shape| {
            out.push(DrawItem {
                shape,
                color: item.color,
            })
        };
        if item.text_box {
            let around = points
                .iter()
                .map(|&p| Bounds2::from_point(p))
                .reduce(Bounds2::union);
            if let Some(b) = around {
                push(Shape::Text(b));
            }
            continue;
        }
        let segments: Vec<Line> = points
            .windows(2)
            .filter_map(|pair| Line::new(pair[0], pair[1]).ok())
            .collect();
        match (segments.is_empty(), points.first()) {
            (false, _) => segments.into_iter().for_each(|l| push(Shape::Line(l))),
            (true, Some(&p)) => push(Shape::Point(p)),
            (true, None) => {}
        }
    }
    out
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

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
