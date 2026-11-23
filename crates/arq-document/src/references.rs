//! Exact geometric reference points for read-only XYZ identification.
//! No tessellated rendering vertex is a source of coordinates.

use acadrust::EntityType;
use acadrust::types::{Transform, Vector3};
use arq_geometry::snap::{SnapKind, key_points};
use arq_geometry::{Arc, Circle, Geometry2, Line, Point2, Point3};

pub(crate) fn points(e: &EntityType) -> Vec<(SnapKind, Point3)> {
    let mut out = Vec::new();
    let mut add = |kind, p: Vector3| {
        if let Ok(p) = Point3::new(p.x, p.y, p.z) {
            out.push((kind, p));
        }
    };
    match e {
        EntityType::Line(l) => {
            add(SnapKind::Endpoint, l.start);
            add(SnapKind::Endpoint, l.end);
            add(SnapKind::Midpoint, l.start + (l.end - l.start) * 0.5);
        }
        EntityType::Point(p) => add(SnapKind::Endpoint, p.location),
        EntityType::Circle(c) => {
            if let Ok(center) = Point2::new(c.center.x, c.center.y)
                && let Ok(circle) = Circle::new(center, c.radius)
            {
                planar(&mut out, circle.into(), c.center.z, c.normal);
            }
        }
        EntityType::Arc(a) => {
            if let Ok(center) = Point2::new(a.center.x, a.center.y)
                && let Ok(arc) = Arc::new(center, a.radius, a.start_angle, a.end_angle)
            {
                planar(&mut out, arc.into(), a.center.z, a.normal);
            }
        }
        EntityType::LwPolyline(p) => {
            let transform = super::drawing::ocs_to_world(p.normal);
            for vertex in &p.vertices {
                if let Ok(point) = world(
                    &transform,
                    vertex.location.x,
                    vertex.location.y,
                    p.elevation,
                ) {
                    out.push((SnapKind::Endpoint, point));
                }
            }
            let n = p.vertices.len();
            let count = if p.is_closed { n } else { n.saturating_sub(1) };
            for i in 0..count {
                let a = &p.vertices[i];
                let b = &p.vertices[(i + 1) % n];
                let (Ok(start), Ok(end)) = (
                    Point2::new(a.location.x, a.location.y),
                    Point2::new(b.location.x, b.location.y),
                ) else {
                    continue;
                };
                let curve = if a.bulge == 0.0 {
                    Line::new(start, end).ok().map(Geometry2::Line)
                } else {
                    Arc::from_bulge(start, end, a.bulge)
                        .ok()
                        .map(|(arc, _)| Geometry2::Arc(arc))
                };
                if let Some(curve) = curve {
                    planar(&mut out, curve, p.elevation, p.normal);
                }
            }
        }
        _ if super::acis::is_body(e) => {
            if let Some(Ok(body)) = super::solid::body_of(e) {
                out.extend(
                    body.vertices()
                        .iter()
                        .map(|v| (SnapKind::Endpoint, v.point)),
                );
            }
        }
        _ => {}
    }
    out
}

fn planar(out: &mut Vec<(SnapKind, Point3)>, curve: Geometry2, z: f64, normal: Vector3) {
    let transform = super::drawing::ocs_to_world(normal);
    for snap in key_points(&curve) {
        if let Ok(point) = world(&transform, snap.point.x(), snap.point.y(), z) {
            out.push((snap.kind, point));
        }
    }
}

fn world(t: &Transform, x: f64, y: f64, z: f64) -> Result<Point3, arq_geometry::GeometryError> {
    let p = t.apply(Vector3::new(x, y, z));
    Point3::new(p.x, p.y, p.z)
}

#[cfg(test)]
#[path = "reference_tests.rs"]
mod tests;
