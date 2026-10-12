//! Translation between opencadcodec entities and our types. The only place
//! that reads or writes codec geometry fields.

use arq_geometry::{Arc, Bounds2, Circle, Geometry2, Line, Point2};
use opencadcodec::types::Vector3;
use opencadcodec::{EntityType, Handle};

use crate::entity::{Color, Entity, EntityId, PlanGeometry};
use crate::error::{DocumentError, DocumentResult};
use crate::ocs::{PlanNormal, plan_normal};

/// Our view of a codec entity.
pub(crate) fn read_entity(e: &EntityType) -> Entity {
    let plan = plan_of(e);
    let bounds = match plan {
        Some(p) => Some(p.geometry().bounds()),
        None => codec_bounds(e),
    };
    Entity {
        id: e.as_entity().handle().value(),
        kind: e.as_entity().entity_type(),
        layer: e.as_entity().layer().to_owned(),
        color: color_from_codec(e.as_entity().color()),
        bounds,
        plan,
    }
}

/// Editable plan geometry of LINE, CIRCLE and ARC drawn in plan.
fn plan_of(e: &EntityType) -> Option<PlanGeometry> {
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

/// Plan extent from the codec, for entities we cannot edit yet.
fn codec_bounds(e: &EntityType) -> Option<Bounds2> {
    let b = e.as_entity().bounding_box();
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
        Geometry2::Line(g) => EntityType::Line(opencadcodec::Line::from_coords(
            g.start().x(),
            g.start().y(),
            z,
            g.end().x(),
            g.end().y(),
            z,
        )),
        Geometry2::Circle(g) => {
            let mut c = opencadcodec::Circle::new();
            c.center = Vector3::new(g.center().x(), g.center().y(), z);
            c.radius = g.radius();
            EntityType::Circle(c)
        }
        Geometry2::Arc(g) => {
            let mut a = opencadcodec::Arc::new();
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

pub(crate) fn color_from_codec(c: opencadcodec::Color) -> Color {
    match c {
        opencadcodec::Color::ByLayer => Color::ByLayer,
        opencadcodec::Color::ByBlock => Color::ByBlock,
        opencadcodec::Color::Index(i) => Color::Index(i),
        opencadcodec::Color::Rgb { r, g, b } => Color::Rgb { r, g, b },
        opencadcodec::Color::None => Color::None,
    }
}

pub(crate) fn color_to_codec(c: Color) -> opencadcodec::Color {
    match c {
        Color::ByLayer => opencadcodec::Color::ByLayer,
        Color::ByBlock => opencadcodec::Color::ByBlock,
        Color::Index(i) => opencadcodec::Color::Index(i),
        Color::Rgb { r, g, b } => opencadcodec::Color::Rgb { r, g, b },
        Color::None => opencadcodec::Color::None,
    }
}
