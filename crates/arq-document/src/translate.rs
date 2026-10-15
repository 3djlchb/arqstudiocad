//! Translation between the engine's (AcadRust) entities and our types. The only place
//! that reads or writes codec geometry fields.

use acadrust::types::{Matrix4, Transform, Vector3};
use acadrust::{EntityType, Handle};
use arq_geometry::{Arc, Bounds2, Circle, Geometry2, Line, Point2, Transform2};

use crate::entity::{Color, Entity, EntityId, PlanGeometry};
use crate::error::{DocumentError, DocumentResult};
use crate::ocs::{PlanNormal, plan_normal};

/// Our view of a codec entity.
pub(crate) fn read_entity(e: &EntityType) -> Entity {
    let plan = plan_of(e);
    let bounds = bounds_with(e, plan);
    Entity {
        id: e.as_entity().handle().value(),
        kind: e.as_entity().entity_type(),
        layer: e.as_entity().layer().to_owned(),
        color: color_from_codec(e.as_entity().color()),
        bounds,
        plan,
        transformable: plan.is_some() || is_whole_transformable(e),
    }
}

/// Blocks and lightweight polylines drawn in plan, which move, rotate,
/// scale, mirror and copy as a whole (ADR 0016, D1). A block array
/// (MINSERT) is not: its spacing would not follow a scaling.
pub(crate) fn is_whole_transformable(e: &EntityType) -> bool {
    match e {
        EntityType::Insert(i) => {
            plan_normal(i.normal.x, i.normal.y, i.normal.z).is_some()
                && i.column_count <= 1
                && i.row_count <= 1
        }
        EntityType::LwPolyline(p) => plan_normal(p.normal.x, p.normal.y, p.normal.z).is_some(),
        _ => false,
    }
}

/// The grip of a block or a lightweight polyline drawn in plan (ADR 0016
/// D7): `(true, insertion point)` for a block, `(false, first vertex)` for
/// a polyline, in world coordinates.
pub(crate) fn whole_grip(e: &EntityType) -> Option<(bool, Point2)> {
    let world = |normal: &Vector3, x: f64, y: f64| {
        let (x, y, _) = plan_normal(normal.x, normal.y, normal.z)?.map_point(x, y, 0.0);
        point(x, y)
    };
    match e {
        EntityType::Insert(i) => {
            Some((true, world(&i.normal, i.insert_point.x, i.insert_point.y)?))
        }
        EntityType::LwPolyline(p) => {
            let first = p.vertices.first()?.location;
            Some((false, world(&p.normal, first.x, first.y)?))
        }
        _ => None,
    }
}

/// Applies a plan transformation to a block or a lightweight polyline as a
/// whole, through the codec: a block's attributes follow it, a polyline's
/// bulges flip under a mirror (the codec's mirror path does the same for
/// these two kinds). Elevations are kept.
pub(crate) fn transform_whole(e: &mut EntityType, t: Transform2) {
    let [[a, b, tx], [c, d, ty]] = t.affine();
    let codec = Transform::from_matrix(Matrix4 {
        m: [
            [a, b, 0.0, tx],
            [c, d, 0.0, ty],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    });
    e.apply_transform(&codec);
}

/// Plan bounds of an entity: from our geometry when it has plan geometry,
/// else the codec's box.
pub(crate) fn bounds_of(e: &EntityType) -> Option<Bounds2> {
    bounds_with(e, plan_of(e))
}

fn bounds_with(e: &EntityType, plan: Option<PlanGeometry>) -> Option<Bounds2> {
    match plan {
        Some(p) => Some(p.geometry().bounds()),
        None => codec_bounds(e),
    }
}

/// Editable plan geometry of LINE, CIRCLE and ARC drawn in plan.
pub(crate) fn plan_of(e: &EntityType) -> Option<PlanGeometry> {
    match e {
        EntityType::Line(l) => {
            let z = l.start.z;
            if (l.end.z - z).abs() > 1e-9 * (1.0 + z.abs()) {
                return None; // a sloped line is not in plan
            }
            let line = Line::new(point(l.start.x, l.start.y)?, point(l.end.x, l.end.y)?).ok()?;
            PlanGeometry::at_elevation(line.into(), z).ok()
        }
        EntityType::Circle(c) => {
            let n = plan_normal(c.normal.x, c.normal.y, c.normal.z)?;
            let (x, y, z) = n.map_point(c.center.x, c.center.y, c.center.z);
            let circle = Circle::new(point(x, y)?, c.radius).ok()?;
            PlanGeometry::at_elevation(circle.into(), z).ok()
        }
        EntityType::Arc(a) => {
            let n = plan_normal(a.normal.x, a.normal.y, a.normal.z)?;
            let (x, y, z) = n.map_point(a.center.x, a.center.y, a.center.z);
            let (start, end) = n.map_arc(a.start_angle, a.end_angle);
            let arc = Arc::new(point(x, y)?, a.radius, start, end).ok()?;
            PlanGeometry::at_elevation(arc.into(), z).ok()
        }
        _ => None,
    }
}

fn point(x: f64, y: f64) -> Option<Point2> {
    Point2::new(x, y).ok()
}

/// Plan extent from the codec, for entities we cannot edit yet. An ACIS
/// body without a wireframe cache (as read from DXF) is boxed from its
/// vertices instead of the codec's empty box (`acis.rs`).
pub(crate) fn codec_bounds(e: &EntityType) -> Option<Bounds2> {
    let b = e.as_entity().bounding_box();
    let empty = [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]
        .iter()
        .all(|c| *c == 0.0);
    if empty
        && crate::acis::is_body(e)
        && let Some(bounds) = crate::acis::bounds(e)
    {
        return Some(bounds);
    }
    let min = Point2::new(b.min.x, b.min.y).ok()?;
    let max = Point2::new(b.max.x, b.max.y).ok()?;
    (min.x() <= max.x() && min.y() <= max.y()).then(|| Bounds2::new(min, max))
}

/// Writes plan geometry into an existing codec entity of the same kind,
/// keeping everything else: handle, layer, color, thickness, the direction of
/// its normal, XDATA.
pub(crate) fn write_plan(e: &mut EntityType, plan: PlanGeometry) -> DocumentResult<()> {
    let id = e.as_entity().handle().value();
    let kind = e.as_entity().entity_type();
    let z = plan.elevation();
    match (e, plan.geometry()) {
        (EntityType::Line(l), Geometry2::Line(g)) => {
            l.start = Vector3::new(g.start().x(), g.start().y(), z);
            l.end = Vector3::new(g.end().x(), g.end().y(), z);
        }
        (EntityType::Circle(c), Geometry2::Circle(g)) => {
            let n = editable_normal(id, kind, c.normal)?;
            let (x, y, oz) = n.map_point(g.center().x(), g.center().y(), z);
            c.center = Vector3::new(x, y, oz);
            c.radius = g.radius();
        }
        (EntityType::Arc(a), Geometry2::Arc(g)) => {
            let n = editable_normal(id, kind, a.normal)?;
            let (x, y, oz) = n.map_point(g.center().x(), g.center().y(), z);
            let (start, end) = n.map_arc(g.start_angle(), g.end_angle());
            a.center = Vector3::new(x, y, oz);
            a.radius = g.radius();
            a.start_angle = start;
            a.end_angle = end;
        }
        (EntityType::Line(_) | EntityType::Circle(_) | EntityType::Arc(_), _) => {
            return Err(DocumentError::KindMismatch { id, kind });
        }
        _ => return Err(DocumentError::NotEditable { id, kind }),
    }
    Ok(())
}

fn editable_normal(id: EntityId, kind: &'static str, n: Vector3) -> DocumentResult<PlanNormal> {
    plan_normal(n.x, n.y, n.z).ok_or(DocumentError::NotEditable { id, kind })
}

/// A new codec entity for plan geometry: normal `(0, 0, 1)`, no thickness,
/// on `layer`. The document assigns its handle.
pub(crate) fn new_entity(plan: PlanGeometry, layer: &str) -> EntityType {
    let z = plan.elevation();
    let mut e = match plan.geometry() {
        Geometry2::Line(g) => EntityType::Line(acadrust::Line::from_coords(
            g.start().x(),
            g.start().y(),
            z,
            g.end().x(),
            g.end().y(),
            z,
        )),
        Geometry2::Circle(g) => {
            let mut c = acadrust::Circle::new();
            c.center = Vector3::new(g.center().x(), g.center().y(), z);
            c.radius = g.radius();
            EntityType::Circle(c)
        }
        Geometry2::Arc(g) => {
            let mut a = acadrust::Arc::new();
            a.center = Vector3::new(g.center().x(), g.center().y(), z);
            a.radius = g.radius();
            a.start_angle = g.start_angle();
            a.end_angle = g.end_angle();
            EntityType::Arc(a)
        }
    };
    e.common_mut().layer = layer.to_owned();
    e
}

pub(crate) fn handle(id: EntityId) -> Handle {
    Handle::new(id)
}

pub(crate) fn color_from_codec(c: acadrust::Color) -> Color {
    match c {
        acadrust::Color::ByLayer => Color::ByLayer,
        acadrust::Color::ByBlock => Color::ByBlock,
        acadrust::Color::Index(i) => Color::Index(i),
        acadrust::Color::Rgb { r, g, b } => Color::Rgb { r, g, b },
        acadrust::Color::None => Color::None,
    }
}

pub(crate) fn color_to_codec(c: Color) -> acadrust::Color {
    match c {
        Color::ByLayer => acadrust::Color::ByLayer,
        Color::ByBlock => acadrust::Color::ByBlock,
        Color::Index(i) => acadrust::Color::Index(i),
        Color::Rgb { r, g, b } => acadrust::Color::Rgb { r, g, b },
        Color::None => acadrust::Color::None,
    }
}
