//! Drawing primitives: what to draw for each entity, seen in plan
//! (ADR 0013, D5.2).
//!
//! Every model-space entity becomes neutral shapes in world coordinates with
//! a resolved color, so codec types never leave this crate: blocks are
//! expanded (nested, guarded against cycles), compound entities are broken
//! down with the codec's `explode`, and layer visibility and ByLayer/ByBlock
//! colors are resolved against the current layer table. Z is dropped: this is
//! the plan view. What cannot be drawn yet becomes its bounding box, marked
//! as a placeholder.

use std::collections::HashMap;

use arq_geometry::{Arc, Bounds2, Circle, Line, Point2};
use opencadcodec::EntityType;
use opencadcodec::entities::Insert;
use opencadcodec::types::{Matrix4, Transform, Vector3};

use crate::document::Document;
use crate::entity::{Color, EntityId};
use crate::ocs::plan_normal;
use crate::translate::{codec_bounds, color_from_codec, handle, plan_of};

/// How deep blocks may nest before expansion stops (AutoCAD drawings rarely
/// pass 5; this only guards against pathological files).
const MAX_DEPTH: usize = 32;

/// The color of top-level ByBlock entities and of anything without a color:
/// ACI 7, drawn white on dark backgrounds and black on light ones.
const DEFAULT_COLOR: Color = Color::Index(7);

/// One shape to draw, in world coordinates seen in plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A straight segment.
    Line(Line),
    /// A full circle.
    Circle(Circle),
    /// A circular arc.
    Arc(Arc),
    /// A point.
    Point(Point2),
    /// A text, as its outline box until fonts are drawn.
    Text(Bounds2),
    /// An entity not drawn yet, as its bounding box.
    Placeholder(Bounds2),
}

/// A shape and the color to draw it in: an ACI index or RGB, never
/// ByLayer or ByBlock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawItem {
    /// What to draw.
    pub shape: Shape,
    /// Its resolved color.
    pub color: Color,
}

/// What the layer table says about drawing one layer.
#[derive(Clone, Copy)]
struct LayerStyle {
    shown: bool,
    frozen: bool,
    color: Color,
}

/// Everything resolved once per drawing call.
struct Painter<'a> {
    doc: &'a Document,
    layers: HashMap<String, LayerStyle>,
}

/// Where an entity sits: inside which inserts.
#[derive(Clone)]
struct Context {
    /// The effective layer of the innermost insert: entities on layer "0"
    /// inside a block take it.
    insert_layer: Option<String>,
    /// The resolved color of the innermost insert, for ByBlock.
    insert_color: Option<Color>,
    /// Block names being expanded, outermost first, to stop cycles.
    blocks: Vec<String>,
}

impl Document {
    /// What to draw for one entity, visible parts only; `None` if no entity
    /// has this id. Empty when it is hidden (off or frozen layer, invisible
    /// flag) or has nothing to draw (a viewport, an infinite line).
    pub fn drawing(&self, id: EntityId) -> Option<Vec<DrawItem>> {
        let entity = self.cad().get_entity(handle(id))?;
        let painter = Painter::new(self);
        let mut out = Vec::new();
        painter.emit(entity, &Context::top(), false, &mut out);
        Some(out)
    }

    /// What to draw for every entity in model space, in drawing order.
    pub fn drawing_all(&self) -> Vec<(EntityId, Vec<DrawItem>)> {
        let painter = Painter::new(self);
        self.cad()
            .model_space_entities()
            .map(|e| {
                let mut out = Vec::new();
                painter.emit(e, &Context::top(), false, &mut out);
                (e.as_entity().handle().value(), out)
            })
            .collect()
    }
}

impl Context {
    fn top() -> Self {
        Self {
            insert_layer: None,
            insert_color: None,
            blocks: Vec::new(),
        }
    }
}

impl<'a> Painter<'a> {
    fn new(doc: &'a Document) -> Self {
        let layers = doc
            .layers()
            .into_iter()
            .map(|l| {
                let style = LayerStyle {
                    shown: l.is_visible(),
                    frozen: l.is_frozen(),
                    color: l.color(),
                };
                (l.name().to_uppercase(), style)
            })
            .collect();
        Self { doc, layers }
    }

    fn style(&self, layer: &str) -> LayerStyle {
        self.layers
            .get(&layer.to_uppercase())
            .copied()
            .unwrap_or(LayerStyle {
                shown: true,
                frozen: false,
                color: DEFAULT_COLOR,
            })
    }

    /// The layer an entity is drawn on: inside a block, layer "0" means the
    /// insert's layer.
    fn effective_layer(&self, e: &EntityType, ctx: &Context) -> String {
        let own = e.as_entity().layer();
        match &ctx.insert_layer {
            Some(outer) if own == "0" => outer.clone(),
            _ => own.to_owned(),
        }
    }

    /// The concrete color of an entity on `layer`.
    fn resolve_color(&self, raw: Color, layer: &str, ctx: &Context) -> Color {
        match raw {
            Color::ByLayer => self.style(layer).color,
            Color::ByBlock => ctx.insert_color.unwrap_or(DEFAULT_COLOR),
            Color::None => DEFAULT_COLOR,
            concrete => concrete,
        }
    }

    /// Adds what to draw for `e`. `ocs_lines`: `e` came from exploding an
    /// OCS entity (polyline, hatch, solid), whose exploded LINEs carry OCS
    /// coordinates although LINE coordinates are normally world ones.
    fn emit(&self, e: &EntityType, ctx: &Context, ocs_lines: bool, out: &mut Vec<DrawItem>) {
        if e.as_entity().is_invisible() || ctx.blocks.len() > MAX_DEPTH {
            return;
        }
        let layer = self.effective_layer(e, ctx);
        let style = self.style(&layer);
        // A frozen layer hides everything on it, a block's contents included.
        // An off layer hides only what is drawn on it: inside a block on an
        // off layer, entities on layer "0" take that layer and hide, the rest
        // stay visible (as AutoCAD does).
        let block_reference = matches!(e, EntityType::Insert(_) | EntityType::Dimension(_));
        if style.frozen || (!style.shown && !block_reference) {
            return;
        }
        let color = self.resolve_color(color_from_codec(e.as_entity().color()), &layer, ctx);
        let mut push = |shape| out.push(DrawItem { shape, color });

        match e {
            EntityType::Line(l) => {
                let (a, b) = if ocs_lines {
                    let Some(n) = plan_normal(l.normal.x, l.normal.y, l.normal.z) else {
                        return self.placeholder(e, color, out);
                    };
                    let (ax, ay, _) = n.map_point(l.start.x, l.start.y, l.start.z);
                    let (bx, by, _) = n.map_point(l.end.x, l.end.y, l.end.z);
                    ((ax, ay), (bx, by))
                } else {
                    ((l.start.x, l.start.y), (l.end.x, l.end.y))
                };
                if let Some(shape) = segment(a, b) {
                    push(shape);
                }
            }
            EntityType::Circle(_) | EntityType::Arc(_) => match plan_of(e) {
                Some(plan) => push(match plan.geometry() {
                    arq_geometry::Geometry2::Circle(c) => Shape::Circle(c),
                    arq_geometry::Geometry2::Arc(a) => Shape::Arc(a),
                    arq_geometry::Geometry2::Line(l) => Shape::Line(l),
                }),
                None => self.placeholder(e, color, out), // on a tilted plane
            },
            EntityType::Point(p) => {
                if let Ok(pt) = Point2::new(p.location.x, p.location.y) {
                    push(Shape::Point(pt));
                }
            }
            EntityType::Text(_)
            | EntityType::MText(_)
            | EntityType::AttributeEntity(_)
            | EntityType::AttributeDefinition(_) => {
                if let Some(b) = codec_bounds(e) {
                    push(Shape::Text(b));
                }
            }
            EntityType::Insert(ins) => self.expand_insert(ins, &layer, color, ctx, out),
            EntityType::Dimension(dim) => {
                // AutoCAD stores what it drew in an anonymous block, in
                // world coordinates; draw that, or decompose without one.
                let name = &dim.base().block_name;
                if !name.is_empty() && self.doc.cad().block_records.get(name).is_some() {
                    self.expand_block(name, &Transform::identity(), &layer, color, ctx, out);
                } else {
                    self.emit_parts(e, ctx, out, color);
                }
            }
            // Nothing to draw in plan.
            EntityType::Viewport(_)
            | EntityType::XLine(_)
            | EntityType::Ray(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_) => {}
            _ => self.emit_parts(e, ctx, out, color),
        }
    }

    /// Draws the pieces the codec breaks `e` into, or its box if none.
    fn emit_parts(&self, e: &EntityType, ctx: &Context, out: &mut Vec<DrawItem>, color: Color) {
        let parts = e.explode();
        if parts.is_empty() {
            return self.placeholder(e, color, out);
        }
        let ocs = matches!(
            e,
            EntityType::LwPolyline(_)
                | EntityType::Polyline2D(_)
                | EntityType::Hatch(_)
                | EntityType::Solid(_)
        );
        let deeper = Context {
            blocks: {
                // Exploding counts toward the depth guard as well.
                let mut b = ctx.blocks.clone();
                b.push(String::new());
                b
            },
            ..ctx.clone()
        };
        for part in &parts {
            self.emit(part, &deeper, ocs, out);
        }
    }

    fn placeholder(&self, e: &EntityType, color: Color, out: &mut Vec<DrawItem>) {
        if let Some(b) = codec_bounds(e) {
            out.push(DrawItem {
                shape: Shape::Placeholder(b),
                color,
            });
        }
    }

    /// Draws a block reference: its block at each array position, then its
    /// attributes.
    fn expand_insert(
        &self,
        ins: &Insert,
        layer: &str,
        color: Color,
        ctx: &Context,
        out: &mut Vec<DrawItem>,
    ) {
        let Some(record) = self.doc.cad().block_records.get(&ins.block_name) else {
            return;
        };
        let base = record.base_point;
        let ocs = ocs_to_world(ins.normal);
        let rows = ins.row_count.max(1);
        let columns = ins.column_count.max(1);
        for row in 0..rows {
            for column in 0..columns {
                let offset = Vector3::new(
                    f64::from(column) * ins.column_spacing,
                    f64::from(row) * ins.row_spacing,
                    0.0,
                );
                let t = Transform::from_translation(Vector3::new(-base.x, -base.y, -base.z))
                    .then(&Transform::from_scaling(Vector3::new(
                        ins.x_scale(),
                        ins.y_scale(),
                        ins.z_scale(),
                    )))
                    .then(&Transform::from_translation(offset))
                    .then(&Transform::from_rotation(
                        Vector3::new(0.0, 0.0, 1.0),
                        ins.rotation,
                    ))
                    .then(&Transform::from_translation(ins.insert_point))
                    .then(&ocs);
                self.expand_block(&ins.block_name, &t, layer, color, ctx, out);
            }
        }
        // Attribute values already sit in world coordinates.
        let inner = Context {
            insert_layer: Some(layer.to_owned()),
            insert_color: Some(color),
            ..ctx.clone()
        };
        for attribute in &ins.attributes {
            self.emit(
                &EntityType::AttributeEntity(attribute.clone()),
                &inner,
                false,
                out,
            );
        }
    }

    /// Draws the entities of block `name` moved by `t`, as seen from an
    /// insert on `layer` with `color`.
    fn expand_block(
        &self,
        name: &str,
        t: &Transform,
        layer: &str,
        color: Color,
        ctx: &Context,
        out: &mut Vec<DrawItem>,
    ) {
        if ctx.blocks.iter().any(|b| b.eq_ignore_ascii_case(name)) {
            return; // a block inside itself: stop
        }
        let mut blocks = ctx.blocks.clone();
        blocks.push(name.to_owned());
        let inner = Context {
            insert_layer: Some(layer.to_owned()),
            insert_color: Some(color),
            blocks,
        };
        let identity = is_identity(t);
        for child in self.doc.cad().entities_in_block(name) {
            if matches!(child, EntityType::AttributeDefinition(_)) {
                continue; // definitions are drawn through the insert's values
            }
            if identity {
                self.emit(child, &inner, false, out);
            } else {
                let mut moved = child.clone();
                moved.apply_transform(t);
                self.emit(&moved, &inner, false, out);
            }
        }
    }
}

/// A segment, or a point when both ends coincide.
fn segment(a: (f64, f64), b: (f64, f64)) -> Option<Shape> {
    let a = Point2::new(a.0, a.1).ok()?;
    let b = Point2::new(b.0, b.1).ok()?;
    Some(match Line::new(a, b) {
        Ok(line) => Shape::Line(line),
        Err(_) => Shape::Point(a),
    })
}

/// The OCS of `normal` to world: AutoCAD's arbitrary axis algorithm.
fn ocs_to_world(normal: Vector3) -> Transform {
    let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
    if !(len.is_finite() && len > 0.0) {
        return Transform::identity();
    }
    let n = [normal.x / len, normal.y / len, normal.z / len];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let unit = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let ax = if n[0].abs() < 1.0 / 64.0 && n[1].abs() < 1.0 / 64.0 {
        unit(cross([0.0, 1.0, 0.0], n))
    } else {
        unit(cross([0.0, 0.0, 1.0], n))
    };
    let ay = unit(cross(n, ax));
    let mut m = Matrix4::identity();
    for row in 0..3 {
        m.m[row][0] = ax[row];
        m.m[row][1] = ay[row];
        m.m[row][2] = n[row];
    }
    Transform::from_matrix(m)
}

fn is_identity(t: &Transform) -> bool {
    let id = Matrix4::identity();
    (0..4).all(|r| (0..4).all(|c| t.matrix.m[r][c] == id.m[r][c]))
}

#[cfg(test)]
#[path = "drawing_tests.rs"]
mod tests;
