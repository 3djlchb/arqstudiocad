//! Drawing primitives: what to draw for each entity, seen in plan
//! (ADR 0013, D5.2).
//!
//! Every model-space entity becomes neutral shapes in world coordinates with
//! a resolved color, so codec types never leave this crate: blocks are
//! expanded (nested, guarded against cycles), compound entities are broken
//! down with the codec's `explode` or, for what AutoCAD keeps whole, its
//! `flatten` (hatches are outlined and ellipses drawn here), and layer
//! visibility and ByLayer/ByBlock
//! colors are resolved against the current layer table. Z is dropped: this is
//! the plan view. What cannot be drawn yet becomes its bounding box, marked
//! as a placeholder.

use std::collections::HashMap;
use std::f64::consts::TAU;

use acadrust::EntityType;
use acadrust::entities::{BoundaryEdge, Ellipse, Hatch, Insert, LwPolyline, Spline};
use acadrust::types::{Matrix4, Transform, Vector2, Vector3};
use arq_geometry::{Arc, Bounds2, Circle, Line, Point2, Point3};
use arq_kernel::Wireframe;

use crate::document::Document;
use crate::entity::{Color, EntityId};
use crate::ocs::plan_normal;
use crate::translate::{codec_bounds, color_from_codec, handle, plan_of, text_layout};

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
    /// A text's box, which picks and selects it: its letters come as
    /// [`Shape::Line`]s drawn in the built-in font (`text_layout.rs`), so
    /// the box itself is not drawn.
    Text(Bounds2),
    /// An entity not drawn yet, as its bounding box.
    Placeholder(Bounds2),
}

impl Shape {
    /// The box around the shape.
    pub fn bounds(self) -> Bounds2 {
        match self {
            Shape::Line(l) => l.bounds(),
            Shape::Circle(c) => c.bounds(),
            Shape::Arc(a) => a.bounds(),
            Shape::Point(p) => Bounds2::from_point(p),
            Shape::Text(b) | Shape::Placeholder(b) => b,
        }
    }
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
    /// Draw what off and frozen layers hide too (an extent, not a picture).
    every_layer: bool,
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
    /// Where the innermost block places its contents, when not where they
    /// are: an ACIS body inside it is drawn from its own data, its points
    /// moved, rather than its ACIS rewritten.
    placement: Option<Transform>,
}

impl Document {
    /// What to draw for one entity, visible parts only; `None` if no entity
    /// has this id. Empty when it is hidden (off or frozen layer, invisible
    /// flag) or has nothing to draw (a viewport, an infinite line).
    pub fn drawing(&self, id: EntityId) -> Option<Vec<DrawItem>> {
        let entity = self.cad().get_entity(handle(id))?;
        let painter = Painter::new(self);
        let mut out = Vec::new();
        painter.emit(entity, &Context::top(), &mut out);
        Some(out)
    }

    /// The box around everything a block reference, a dimension or a
    /// polyline draws, as if every layer were on and thawed: the codec
    /// knows only the insertion point or the vertices. `None` when it draws
    /// nothing.
    pub(crate) fn drawn_extent(&self, entity: &EntityType) -> Option<Bounds2> {
        let painter = Painter {
            every_layer: true,
            ..Painter::new(self)
        };
        let mut out = Vec::new();
        painter.emit(entity, &Context::top(), &mut out);
        out.into_iter()
            .map(|item| item.shape.bounds())
            .reduce(|a, b| a.union(b))
    }

    /// What to draw for every entity in model space, in drawing order.
    pub fn drawing_all(&self) -> Vec<(EntityId, Vec<DrawItem>)> {
        let painter = Painter::new(self);
        self.cad()
            .model_space_entities()
            .map(|e| {
                let mut out = Vec::new();
                painter.emit(e, &Context::top(), &mut out);
                (e.as_entity().handle().value(), out)
            })
            .collect()
    }

    /// What to draw for model-space entities selected by a caller's broad
    /// phase, preserving the order of `ids`.
    pub(crate) fn drawing_entities(&self, ids: &[EntityId]) -> Vec<(EntityId, Vec<DrawItem>)> {
        let painter = Painter::new(self);
        ids.iter()
            .filter_map(|&id| {
                let entity = self.cad().get_entity(handle(id))?;
                let mut out = Vec::new();
                painter.emit(entity, &Context::top(), &mut out);
                Some((id, out))
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
            placement: None,
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
        Self {
            doc,
            layers,
            every_layer: false,
        }
    }

    fn style(&self, layer: &str) -> LayerStyle {
        let style = self
            .layers
            .get(&layer.to_uppercase())
            .copied()
            .unwrap_or(LayerStyle {
                shown: true,
                frozen: false,
                color: DEFAULT_COLOR,
            });
        if self.every_layer {
            LayerStyle {
                shown: true,
                frozen: false,
                ..style
            }
        } else {
            style
        }
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

    /// Adds what to draw for `e`.
    fn emit(&self, e: &EntityType, ctx: &Context, out: &mut Vec<DrawItem>) {
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
                let (a, b) = ((l.start.x, l.start.y), (l.end.x, l.end.y));
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
            | EntityType::AttributeDefinition(_) => match text_layout(e) {
                Some(laid) => {
                    push(Shape::Text(laid.bounds));
                    for (a, b) in laid.strokes {
                        push(match Line::new(a, b) {
                            Ok(line) => Shape::Line(line),
                            Err(_) => Shape::Point(a),
                        });
                    }
                }
                // On a tilted plane, or with no usable height: its box.
                None => {
                    if let Some(b) = codec_bounds(e) {
                        push(Shape::Text(b));
                    }
                }
            },
            EntityType::Ellipse(el) => outline_ellipse(el, color, out),
            EntityType::Hatch(h) => self.outline_hatch(h, e, ctx, color, out),
            EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_) => match self.doc.wireframes.get(e, self.isolines()) {
                Some(wire) => outline_body(&wire, ctx.placement.as_ref(), color, out),
                // Not translated yet: as the engine breaks it down.
                _ => match &ctx.placement {
                    None => self.emit_parts(e, ctx, out, color),
                    Some(t) => {
                        let mut moved = e.clone();
                        moved.apply_transform(t);
                        self.emit_parts(&moved, ctx, out, color);
                    }
                },
            },
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

    /// The drawing's ISOLINES, the lines drawn on each curved face of a
    /// solid (0 to 64).
    fn isolines(&self) -> usize {
        usize::try_from(self.doc.cad().header.isolines.clamp(0, 64)).unwrap_or(4)
    }

    /// A hatch's boundary, its fill pattern being later work (ADR 0013).
    /// Each edge becomes a codec curve in world coordinates (a polyline
    /// edge keeps the hatch's OCS), drawn like the hatch itself.
    fn outline_hatch(
        &self,
        h: &Hatch,
        e: &EntityType,
        ctx: &Context,
        color: Color,
        out: &mut Vec<DrawItem>,
    ) {
        let Some(n) = plan_normal(h.normal.x, h.normal.y, h.normal.z) else {
            return self.placeholder(e, color, out);
        };
        let world = |p: Vector2| {
            let (x, y, z) = n.map_point(p.x, p.y, h.elevation);
            Vector3::new(x, y, z)
        };
        let mut parts = Vec::new();
        for edge in h.paths.iter().flat_map(|path| &path.edges) {
            match edge {
                BoundaryEdge::Line(l) => {
                    let mut line =
                        acadrust::entities::Line::from_points(world(l.start), world(l.end));
                    line.common = h.common.clone();
                    parts.push(EntityType::Line(line));
                }
                BoundaryEdge::CircularArc(a) => {
                    let (start, end) = ccw_range(a.start_angle, a.end_angle, a.counter_clockwise);
                    let (start, end) = n.map_arc(start, end);
                    let mut arc = acadrust::entities::Arc::new();
                    arc.common = h.common.clone();
                    arc.center = world(a.center);
                    arc.radius = a.radius;
                    arc.start_angle = start;
                    arc.end_angle = end;
                    parts.push(EntityType::Arc(arc));
                }
                BoundaryEdge::EllipticArc(a) => {
                    // Mirroring the center and the axis and keeping the
                    // normal draws the same points (the minor axis is the
                    // normal crossed with the major one).
                    let (start, end) = ccw_range(a.start_angle, a.end_angle, a.counter_clockwise);
                    let axis = world(a.major_axis_endpoint) - world(Vector2::new(0.0, 0.0));
                    let mut ellipse = Ellipse::new();
                    ellipse.common = h.common.clone();
                    ellipse.center = world(a.center);
                    ellipse.major_axis = axis;
                    ellipse.minor_axis_ratio = a.minor_axis_ratio;
                    ellipse.start_parameter = start;
                    ellipse.end_parameter = end;
                    ellipse.normal = h.normal;
                    parts.push(EntityType::Ellipse(ellipse));
                }
                BoundaryEdge::Spline(s) if s.control_points.len() >= 2 => {
                    // A boundary control point holds its weight in z.
                    let points = s
                        .control_points
                        .iter()
                        .map(|p| world(Vector2::new(p.x, p.y)))
                        .collect();
                    let mut spline = Spline::from_control_points(s.degree, points);
                    spline.common = h.common.clone();
                    spline.knots = s.knots.clone();
                    spline.flags.periodic = s.periodic;
                    if s.rational {
                        spline.flags.rational = true;
                        spline.weights = s.control_points.iter().map(|p| p.z).collect();
                    }
                    parts.push(EntityType::Spline(spline));
                }
                BoundaryEdge::Spline(_) => {}
                BoundaryEdge::Polyline(p) => {
                    let mut polyline = LwPolyline::new();
                    polyline.common = h.common.clone();
                    for v in &p.vertices {
                        polyline.add_point_with_bulge(Vector2::new(v.x, v.y), v.z);
                    }
                    polyline.is_closed = p.is_closed;
                    polyline.elevation = h.elevation;
                    polyline.normal = h.normal;
                    parts.push(EntityType::LwPolyline(polyline));
                }
            }
        }
        if parts.is_empty() {
            return self.placeholder(e, color, out);
        }
        for part in &parts {
            self.emit(part, ctx, out);
        }
    }

    /// Draws the pieces the codec breaks `e` into, or its box if none.
    fn emit_parts(&self, e: &EntityType, ctx: &Context, out: &mut Vec<DrawItem>, color: Color) {
        // The codec explodes as AutoCAD does, keeping whole what AutoCAD
        // keeps whole (ELLIPSE, SPLINE, 3DFACE…); its `flatten` still breaks
        // those into primitives.
        let mut parts = e.explode();
        if parts.is_empty() {
            parts = e.flatten();
        }
        if parts.is_empty() {
            return self.placeholder(e, color, out);
        }
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
            self.emit(part, &deeper, out);
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
            self.emit(&EntityType::AttributeEntity(attribute.clone()), &inner, out);
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
        let identity = is_identity(t);
        let inner = Context {
            insert_layer: Some(layer.to_owned()),
            insert_color: Some(color),
            blocks,
            placement: (!identity).then_some(*t),
        };
        for child in self.doc.cad().entities_in_block(name) {
            if matches!(child, EntityType::AttributeDefinition(_)) {
                continue; // definitions are drawn through the insert's values
            }
            if identity || crate::acis::is_body(child) {
                self.emit(child, &inner, out);
            } else {
                let mut moved = child.clone();
                moved.apply_transform(t);
                self.emit(&moved, &inner, out);
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

/// An ellipse as segments between points on it, seen in plan (the codec
/// would approximate it with a spline through points near it).
fn outline_ellipse(el: &Ellipse, color: Color, out: &mut Vec<DrawItem>) {
    let major = el.major_axis;
    let minor = el.normal.cross(&major).normalize() * (major.length() * el.minor_axis_ratio);
    let (start, mut end) = if el.is_full() {
        (0.0, TAU)
    } else {
        (el.start_parameter, el.end_parameter)
    };
    while end <= start {
        end += TAU;
    }
    let sweep = end - start;
    let steps = ((sweep * 16.0).ceil() as usize).clamp(8, 128);
    let at = |i: usize| {
        let t = start + sweep * i as f64 / steps as f64;
        let p = el.center + major * t.cos() + minor * t.sin();
        (p.x, p.y)
    };
    for i in 0..steps {
        if let Some(shape) = segment(at(i), at(i + 1)) {
            out.push(DrawItem { shape, color });
        }
    }
}

/// An ACIS body's wireframe in plan (ADR 0024 D4, step 2): its edges and
/// isolines, moved by `placement` when it sits in a block placed elsewhere.
fn outline_body(
    wire: &Wireframe,
    placement: Option<&Transform>,
    color: Color,
    out: &mut Vec<DrawItem>,
) {
    let plan = |p: Point3| {
        let p = Vector3::new(p.x(), p.y(), p.z());
        let q = placement.map_or(p, |t| t.apply(p));
        (q.x, q.y)
    };
    for line in wire.edges.iter().chain(&wire.isolines) {
        for pair in line.windows(2) {
            if let Some(shape) = segment(plan(pair[0]), plan(pair[1])) {
                out.push(DrawItem { shape, color });
            }
        }
    }
}

/// A hatch edge's angle (or parameter) range, counterclockwise. A
/// clockwise edge stores its angles mirrored: it runs clockwise from
/// `-start` to `-end`, so counterclockwise from `-end` to `-start` (as each
/// clockwise arc of the maintainer's drawings meets its neighbors).
fn ccw_range(start: f64, end: f64, counter_clockwise: bool) -> (f64, f64) {
    if counter_clockwise {
        (start, end)
    } else {
        (-end, -start)
    }
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
