//! Outlines of entities in screen pixels, for the overlay the interface
//! paints over the rendered image: selection, hover highlight (ADR 0016
//! D4). The drawing itself is not rendered again.

use arq_core::{EntityId, Shape};

use crate::camera::Camera;
use crate::scene::Scene;
use crate::view::View;

/// Largest distance, in pixels, between an arc and the polyline drawn for
/// it: as fine as the rendered image.
const ARC_TOLERANCE_PX: f64 = 0.25;

/// Half the size of the cross drawn for a point, in pixels.
const POINT_PX: f64 = 3.0;

/// One polyline, in screen pixels (origin top-left, y down).
pub type Polyline = Vec<(f64, f64)>;

/// The outlines of the given entities as the camera sees them: each line,
/// arc and circle as a polyline fine enough for its size on screen, a
/// point as a small cross, a text or an entity not drawn yet as its box.
/// Entities not in the scene (hidden, unknown) give nothing.
pub fn outline(scene: &Scene, camera: &Camera, ids: &[EntityId]) -> Vec<Polyline> {
    let screen = |p: arq_geometry::Point2| camera.to_screen(p.x(), p.y());
    let mut out = Vec::new();
    for item in ids.iter().filter_map(|&id| scene.items(id)).flatten() {
        match item.shape {
            Shape::Line(l) => out.push(vec![screen(l.start()), screen(l.end())]),
            Shape::Circle(c) => out.push(arc(
                camera,
                c.center(),
                c.radius(),
                0.0,
                std::f64::consts::TAU,
            )),
            Shape::Arc(a) => out.push(arc(
                camera,
                a.center(),
                a.radius(),
                a.start_angle(),
                a.sweep(),
            )),
            Shape::Point(p) => {
                let (x, y) = screen(p);
                out.push(vec![(x - POINT_PX, y), (x + POINT_PX, y)]);
                out.push(vec![(x, y - POINT_PX), (x, y + POINT_PX)]);
            }
            Shape::Text(b) | Shape::Placeholder(b) => {
                let [p0, p1, p2, p3] = b.corners().map(screen);
                out.push(vec![p0, p1, p2, p3, p0]);
            }
        }
    }
    out
}

/// A path of the XY plane as a polyline vertex list holds it, each vertex
/// with the bulge of the segment leaving it, as the camera sees it in
/// `view`: one polyline, its arcs fine enough for their size on screen,
/// back to its first point when `closed`. For a shape being drawn, before
/// it is an entity (RECTANG's preview).
pub fn path_outline(
    camera: &Camera,
    view: View,
    vertices: &[(arq_geometry::Point2, f64)],
    closed: bool,
) -> Polyline {
    let (right, up) = view.axes();
    let screen =
        |x: f64, y: f64| camera.to_screen(x * right[0] + y * right[1], x * up[0] + y * up[1]);
    let n = vertices.len();
    let segments = if closed { n } else { n.saturating_sub(1) };
    let mut out: Polyline = Vec::new();
    if let Some((first, _)) = vertices.first() {
        out.push(screen(first.x(), first.y()));
    }
    for i in 0..segments {
        let ((a, bulge), b) = (vertices[i], vertices[(i + 1) % n].0);
        if let Ok((arc, _)) = arq_geometry::Arc::from_bulge(a, b, bulge) {
            // From a to b: counterclockwise for a positive bulge.
            let (c, r) = (arc.center(), arc.radius());
            let sweep = 4.0 * bulge.atan();
            let from = (a.y() - c.y()).atan2(a.x() - c.x());
            let r_px = r * camera.scale();
            let step = if r_px > ARC_TOLERANCE_PX {
                2.0 * (1.0 - ARC_TOLERANCE_PX / r_px).acos()
            } else {
                sweep.abs()
            };
            let k = ((sweep.abs() / step).ceil() as usize).clamp(1, 4096);
            for j in 1..k {
                let t = from + sweep * (j as f64) / (k as f64);
                out.push(screen(c.x() + r * t.cos(), c.y() + r * t.sin()));
            }
        }
        out.push(screen(b.x(), b.y()));
    }
    out
}

/// An arc as a polyline fine enough for its size on screen.
fn arc(
    camera: &Camera,
    center: arq_geometry::Point2,
    radius: f64,
    start: f64,
    sweep: f64,
) -> Polyline {
    let r_px = radius * camera.scale();
    let (cx, cy) = (center.x(), center.y());
    if r_px < 0.5 {
        let p = camera.to_screen(cx, cy);
        return vec![p, (p.0 + 0.5, p.1)];
    }
    // Largest angle step whose chord stays within the tolerance.
    let step = 2.0 * (1.0 - (ARC_TOLERANCE_PX / r_px).min(1.0)).acos();
    let n = ((sweep / step).ceil() as usize).clamp(2, 4096);
    (0..=n)
        .map(|i| {
            let a = start + sweep * (i as f64) / (n as f64);
            camera.to_screen(cx + radius * a.cos(), cy + radius * a.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use arq_core::{Document, PlanGeometry};
    use arq_geometry::{Circle, Line, Point2};

    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    /// A line (0,0)-(10,0) and a circle at (20,0), radius 2, seen at 10
    /// pixels per unit in a 400 × 200 view centred on (10,0).
    fn setup() -> (Scene, Camera, EntityId, EntityId) {
        let mut doc = Document::new();
        let layer = doc.current_layer().id();
        let line = doc
            .add_entity(
                PlanGeometry::at_elevation(
                    Line::new(p(0.0, 0.0), p(10.0, 0.0)).unwrap().into(),
                    0.0,
                )
                .unwrap(),
                layer,
            )
            .unwrap();
        let circle = doc
            .add_entity(
                PlanGeometry::at_elevation(Circle::new(p(20.0, 0.0), 2.0).unwrap().into(), 0.0)
                    .unwrap(),
                layer,
            )
            .unwrap();
        let mut camera = Camera::new(400, 200);
        camera.fit(arq_geometry::Bounds2::new(p(-10.0, -10.0), p(30.0, 10.0)));
        (Scene::build(&doc), camera, line, circle)
    }

    #[test]
    fn a_line_is_its_two_ends_on_screen() {
        let (scene, camera, line, _) = setup();
        let out = outline(&scene, &camera, &[line]);
        assert_eq!(
            out,
            [vec![
                camera.to_screen(0.0, 0.0),
                camera.to_screen(10.0, 0.0)
            ]]
        );
    }

    #[test]
    fn a_circle_is_a_closed_polyline_on_its_curve() {
        let (scene, camera, _, circle) = setup();
        let out = outline(&scene, &camera, &[circle]);
        assert_eq!(out.len(), 1);
        let poly = &out[0];
        assert!(poly.len() > 16, "fine enough: {} points", poly.len());
        let (cx, cy) = camera.to_screen(20.0, 0.0);
        let r = 2.0 * camera.scale();
        for (x, y) in poly {
            assert!(((x - cx).hypot(y - cy) - r).abs() < 1e-9);
        }
        let (first, last) = (poly[0], poly[poly.len() - 1]);
        assert!((first.0 - last.0).abs() < 1e-9 && (first.1 - last.1).abs() < 1e-9);
    }

    #[test]
    fn a_path_with_arcs_lies_on_its_curves_and_closes() {
        let (_, camera, _, _) = setup();
        // A 4 × 2 slot: two straight sides and two half circles of radius 1.
        let path = [
            (p(1.0, 0.0), 0.0),
            (p(3.0, 0.0), 1.0),
            (p(3.0, 2.0), 0.0),
            (p(1.0, 2.0), 1.0),
        ];
        let poly = path_outline(&camera, View::Top, &path, true);
        assert!(poly.len() > 12, "the arcs are fine: {} points", poly.len());
        assert_eq!(poly[0], camera.to_screen(1.0, 0.0));
        assert_eq!(poly[poly.len() - 1], poly[0], "closed");
        let r = camera.scale();
        let centers = [camera.to_screen(3.0, 1.0), camera.to_screen(1.0, 1.0)];
        let (left, right) = (camera.to_screen(1.0, 0.0).0, camera.to_screen(3.0, 0.0).0);
        for &(x, y) in &poly {
            let on_arc = centers
                .iter()
                .any(|&(cx, cy)| ((x - cx).hypot(y - cy) - r).abs() < 1e-6);
            let on_side = x >= left - 1e-9 && x <= right + 1e-9;
            assert!(on_arc || on_side, "({x}, {y}) is on the slot");
        }
        // The right arc bulges right, beyond x = 3: counterclockwise.
        assert!(poly.iter().any(|&(x, _)| x > right + 0.9 * r));
        // Open: the last point is the last vertex.
        let open = path_outline(&camera, View::Top, &path[..2], false);
        assert_eq!(
            open,
            vec![camera.to_screen(1.0, 0.0), camera.to_screen(3.0, 0.0)]
        );
    }

    #[test]
    fn a_path_in_a_view_is_where_the_plane_is_seen() {
        let (_, camera, _, _) = setup();
        let path = [(p(1.0, 0.0), 0.0), (p(3.0, 0.0), 0.0)];
        for view in [View::Top, View::Front, View::SwIso] {
            let (right, up) = view.axes();
            let poly = path_outline(&camera, view, &path, false);
            let seen = |x: f64, y: f64| {
                camera.to_screen(x * right[0] + y * right[1], x * up[0] + y * up[1])
            };
            assert_eq!(poly, vec![seen(1.0, 0.0), seen(3.0, 0.0)], "{view:?}");
        }
    }

    #[test]
    fn unknown_entities_give_nothing() {
        let (scene, camera, line, circle) = setup();
        assert!(outline(&scene, &camera, &[999_999]).is_empty());
        assert_eq!(outline(&scene, &camera, &[line, 999_999, circle]).len(), 2);
    }
}
