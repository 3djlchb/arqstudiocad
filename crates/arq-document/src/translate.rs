//! Translation between the engine's (AcadRust) entities and our types. The only place
//! that reads or writes codec geometry fields.

use acadrust::types::{Matrix4, Transform, Vector3};
use acadrust::{EntityType, Handle};
use arq_geometry::{Arc, Bounds2, Circle, Geometry2, Line, Point2, Transform2};

use crate::entity::{Color, Entity, EntityId, PlanGeometry, TextData};
use crate::error::{DocumentError, DocumentResult};
use crate::ocs::{PlanNormal, plan_normal};

/// Our view of a codec entity.
pub(crate) fn read_entity(e: &EntityType) -> Entity {
    let plan = plan_of(e);
    let bounds = bounds_with(e, plan);
    let common = e.common();
    Entity {
        id: e.as_entity().handle().value(),
        kind: e.as_entity().entity_type(),
        layer: e.as_entity().layer().to_owned(),
        color: color_from_codec(e.as_entity().color()),
        linetype: if common.linetype.is_empty() {
            "ByLayer".to_owned()
        } else {
            common.linetype.clone()
        },
        lineweight: lineweight_text(common.line_weight),
        details: details(e),
        bounds,
        plan,
        text: text_data(e),
        transformable: plan.is_some() || is_whole_transformable(e),
    }
}

fn lineweight_text(weight: acadrust::types::LineWeight) -> String {
    match weight {
        acadrust::types::LineWeight::ByLayer => "ByLayer".to_owned(),
        acadrust::types::LineWeight::ByBlock => "ByBlock".to_owned(),
        acadrust::types::LineWeight::Default => "Default".to_owned(),
        acadrust::types::LineWeight::Value(value) => {
            format!("{:.2} mm", f64::from(value) / 100.0)
        }
    }
}

fn details(e: &EntityType) -> Vec<(String, String)> {
    match e {
        EntityType::Insert(insert) => vec![
            ("Block name".to_owned(), insert.block_name.clone()),
            (
                "Insertion point".to_owned(),
                format!(
                    "{:.6}, {:.6}, {:.6}",
                    insert.insert_point.x, insert.insert_point.y, insert.insert_point.z
                ),
            ),
            (
                "Scale".to_owned(),
                format!(
                    "{:.6}, {:.6}, {:.6}",
                    insert.x_scale(),
                    insert.y_scale(),
                    insert.z_scale()
                ),
            ),
            (
                "Rotation".to_owned(),
                format!("{:.6}", insert.rotation.to_degrees()),
            ),
            ("Attributes".to_owned(), insert.attributes.len().to_string()),
            ("Columns".to_owned(), insert.column_count.to_string()),
            ("Rows".to_owned(), insert.row_count.to_string()),
        ],
        EntityType::LwPolyline(polyline) => vec![
            ("Vertices".to_owned(), polyline.vertices.len().to_string()),
            (
                "Closed".to_owned(),
                if polyline.is_closed { "true" } else { "false" }.to_owned(),
            ),
            ("Elevation".to_owned(), format!("{:.6}", polyline.elevation)),
            (
                "Constant width".to_owned(),
                format!("{:.6}", polyline.constant_width),
            ),
            ("Thickness".to_owned(), format!("{:.6}", polyline.thickness)),
        ],
        _ => Vec::new(),
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
        EntityType::Text(t) => plan_normal(t.normal.x, t.normal.y, t.normal.z).is_some(),
        EntityType::MText(t) => plan_normal(t.normal.x, t.normal.y, t.normal.z).is_some(),
        _ => false,
    }
}

/// A TEXT or MTEXT drawn in plan, as the editor sees it (ADR 0022).
pub(crate) fn text_data(e: &EntityType) -> Option<TextData> {
    use acadrust::entities::TextHorizontalAlignment as H;
    match e {
        EntityType::Text(t) => {
            let normal = plan_normal(t.normal.x, t.normal.y, t.normal.z)?;
            let (x, y) = text_anchor_ocs(t);
            let (x, y, _) = normal.map_point(x, y, 0.0);
            Some(TextData {
                content: t.value.clone(),
                anchor: point(x, y)?,
                height: t.height,
                rotation: normal.map_angle(t.rotation),
                style: t.style.clone(),
                justification: text_justification(t),
                width_factor: t.width_factor,
                oblique: t.oblique_angle,
                multiline: false,
                sizes_editable: !matches!(t.horizontal_alignment, H::Aligned | H::Fit),
                content_editable: true,
            })
        }
        EntityType::MText(t) => {
            plan_normal(t.normal.x, t.normal.y, t.normal.z)?;
            Some(TextData {
                content: t.value.clone(),
                anchor: point(t.insertion_point.x, t.insertion_point.y)?,
                height: t.height,
                rotation: t.rotation,
                style: t.style.clone(),
                justification: attachment_name(t.attachment_point),
                width_factor: 1.0,
                oblique: 0.0,
                multiline: true,
                sizes_editable: true,
                content_editable: !has_formatting(&t.value),
            })
        }
        _ => None,
    }
}

/// A TEXT anchors at its insertion point when left-justified on the
/// baseline, else at its alignment point (ADR 0022 D3); in its OCS.
fn text_anchor_ocs(t: &acadrust::Text) -> (f64, f64) {
    use acadrust::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let first = (t.insertion_point.x, t.insertion_point.y);
    if matches!(t.horizontal_alignment, H::Left | H::Aligned | H::Fit)
        && matches!(t.vertical_alignment, V::Baseline)
    {
        return first;
    }
    t.alignment_point.map_or(first, |a| (a.x, a.y))
}

fn text_justification(t: &acadrust::Text) -> &'static str {
    use acadrust::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    match (t.horizontal_alignment, t.vertical_alignment) {
        (H::Aligned, _) => "Aligned",
        (H::Fit, _) => "Fit",
        (H::Middle, _) => "Middle",
        (H::Left, V::Baseline) => "Left",
        (H::Center, V::Baseline) => "Center",
        (H::Right, V::Baseline) => "Right",
        (H::Left, V::Bottom) => "BottomLeft",
        (H::Center, V::Bottom) => "BottomCenter",
        (H::Right, V::Bottom) => "BottomRight",
        (H::Left, V::Middle) => "MiddleLeft",
        (H::Center, V::Middle) => "MiddleCenter",
        (H::Right, V::Middle) => "MiddleRight",
        (H::Left, V::Top) => "TopLeft",
        (H::Center, V::Top) => "TopCenter",
        (H::Right, V::Top) => "TopRight",
    }
}

fn attachment_name(a: acadrust::entities::AttachmentPoint) -> &'static str {
    use acadrust::entities::AttachmentPoint as A;
    match a {
        A::TopLeft => "TopLeft",
        A::TopCenter => "TopCenter",
        A::TopRight => "TopRight",
        A::MiddleLeft => "MiddleLeft",
        A::MiddleCenter => "MiddleCenter",
        A::MiddleRight => "MiddleRight",
        A::BottomLeft => "BottomLeft",
        A::BottomCenter => "BottomCenter",
        A::BottomRight => "BottomRight",
    }
}

/// Whether MTEXT content holds formatting codes: a backslash code or a
/// brace group (ADR 0022 D4).
fn has_formatting(value: &str) -> bool {
    value.contains('\\') || value.contains('{') || value.contains('}')
}

/// After a transformation that reflects, a text stays readable (ADR 0022
/// D2, AutoCAD's `MIRRTEXT = 0`): of the two upright frames that fit the
/// mirrored text, the one whose up direction is closer to the original's is
/// kept; when it reverses the reading direction, the horizontal
/// justification is mirrored too. `before` is the reading direction
/// (world radians) before `t`.
fn keep_text_readable(e: &mut EntityType, t: Transform2, before: f64) {
    let [[a, b, _], [c, d, _]] = t.affine();
    let (dx, dy) = (
        a * before.cos() + b * before.sin(),
        c * before.cos() + d * before.sin(),
    );
    let (ux, uy) = (-before.sin(), before.cos());
    let mirrored_up = (a * ux + b * uy, c * ux + d * uy);
    let mut world = dy.atan2(dx);
    // A text drawn along the mirrored direction has up = −(mirrored up);
    // turning it half round gives up = +(mirrored up), reading backwards.
    let turn = mirrored_up.0 * ux + mirrored_up.1 * uy > 0.0;
    if turn {
        world += std::f64::consts::PI;
    }
    match e {
        EntityType::Text(text) => {
            let Some(normal) = plan_normal(text.normal.x, text.normal.y, text.normal.z) else {
                return;
            };
            text.rotation = normal.map_angle(world);
            if turn {
                mirror_text_justification(text);
            }
        }
        EntityType::MText(text) => {
            text.rotation = world;
            text.dwg_x_direction = None;
            if turn {
                text.attachment_point = mirrored_attachment(text.attachment_point);
            }
        }
        _ => {}
    }
}

/// Left ↔ right for a TEXT whose reading direction was reversed: its
/// anchor stays where the mirror put it. Aligned and Fit texts swap their
/// two points instead.
fn mirror_text_justification(t: &mut acadrust::Text) {
    use acadrust::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let anchor = t.alignment_point.unwrap_or(t.insertion_point);
    match t.horizontal_alignment {
        H::Aligned | H::Fit => {
            if let Some(second) = t.alignment_point {
                t.alignment_point = Some(t.insertion_point);
                t.insertion_point = second;
            }
        }
        H::Left => {
            let on_baseline = matches!(t.vertical_alignment, V::Baseline);
            let at = if on_baseline {
                t.insertion_point
            } else {
                anchor
            };
            t.horizontal_alignment = H::Right;
            t.alignment_point = Some(at);
            t.insertion_point = at;
        }
        H::Right => {
            t.horizontal_alignment = H::Left;
            if matches!(t.vertical_alignment, V::Baseline) {
                t.insertion_point = anchor;
                t.alignment_point = None;
            }
        }
        H::Center | H::Middle => {}
    }
}

fn mirrored_attachment(
    a: acadrust::entities::AttachmentPoint,
) -> acadrust::entities::AttachmentPoint {
    use acadrust::entities::AttachmentPoint as A;
    match a {
        A::TopLeft => A::TopRight,
        A::TopRight => A::TopLeft,
        A::MiddleLeft => A::MiddleRight,
        A::MiddleRight => A::MiddleLeft,
        A::BottomLeft => A::BottomRight,
        A::BottomRight => A::BottomLeft,
        other => other,
    }
}

/// A new single-line TEXT, left-justified on the baseline at `anchor`
/// (world), on `layer` with `style` (ADR 0022 D1).
pub(crate) fn new_text(
    content: &str,
    anchor: Point2,
    height: f64,
    rotation: f64,
    layer: &str,
    style: &str,
) -> EntityType {
    let mut t = acadrust::Text::with_value(content, Vector3::new(anchor.x(), anchor.y(), 0.0));
    t.height = height;
    t.rotation = rotation;
    t.style = style.to_owned();
    t.common.layer = layer.to_owned();
    EntityType::Text(t)
}

/// Sets what is given of a text's content, height and rotation (world
/// radians); the caller has checked the values and that they apply.
pub(crate) fn edit_text(
    e: &mut EntityType,
    content: Option<&str>,
    height: Option<f64>,
    rotation: Option<f64>,
) {
    match e {
        EntityType::Text(t) => {
            if let Some(content) = content {
                t.value = content.to_owned();
            }
            if let Some(height) = height {
                t.height = height;
            }
            if let (Some(rotation), Some(normal)) =
                (rotation, plan_normal(t.normal.x, t.normal.y, t.normal.z))
            {
                t.rotation = normal.map_angle(rotation);
            }
        }
        EntityType::MText(t) => {
            if let Some(content) = content {
                t.value = content.to_owned();
            }
            if let Some(height) = height {
                t.height = height;
            }
            if let Some(rotation) = rotation {
                t.rotation = rotation;
                t.dwg_x_direction = None;
            }
        }
        _ => {}
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
        EntityType::Text(_) | EntityType::MText(_) => Some((true, text_data(e)?.anchor())),
        _ => None,
    }
}

/// Applies a plan transformation to a block or a lightweight polyline as a
/// whole, through the codec: a block's attributes follow it, a polyline's
/// bulges flip under a mirror (the codec's mirror path does the same for
/// these two kinds). Elevations are kept.
pub(crate) fn transform_whole(e: &mut EntityType, t: Transform2) {
    let reading = text_data(e).map(|text| text.rotation());
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
    if let Some(before) = reading {
        // The codec turns a TEXT's stored angle as a world angle, wrong in a
        // mirrored OCS, and reflects texts: we set the reading direction.
        if t.is_reflection() {
            keep_text_readable(e, t, before);
        } else {
            let [[a, b, _], [c, d, _]] = t.affine();
            let world =
                (c * before.cos() + d * before.sin()).atan2(a * before.cos() + b * before.sin());
            edit_text(e, None, None, Some(world));
        }
    }
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

#[cfg(test)]
mod details_tests {
    use acadrust::EntityType;
    use acadrust::entities::{Insert, LwPolyline};
    use acadrust::types::{LineWeight, Vector3};

    use super::{details, lineweight_text};

    #[test]
    fn block_reference_details_are_read_only_values() {
        let insert = Insert::new("DOOR", Vector3::new(1.0, 2.0, 0.0))
            .with_scale(2.0, 3.0, 1.0)
            .with_rotation(std::f64::consts::FRAC_PI_2);
        let details = details(&EntityType::Insert(insert));

        assert!(details.contains(&("Block name".to_owned(), "DOOR".to_owned())));
        assert!(details.contains(&(
            "Insertion point".to_owned(),
            "1.000000, 2.000000, 0.000000".to_owned()
        )));
        assert!(details.contains(&(
            "Scale".to_owned(),
            "2.000000, 3.000000, 1.000000".to_owned()
        )));
        assert!(details.contains(&("Rotation".to_owned(), "90.000000".to_owned())));
    }

    #[test]
    fn lightweight_polyline_details_include_shape_data() {
        let mut polyline = LwPolyline::new();
        polyline.close();
        polyline.elevation = 2.5;
        polyline.constant_width = 0.25;
        let details = details(&EntityType::LwPolyline(polyline));

        assert!(details.contains(&("Vertices".to_owned(), "0".to_owned())));
        assert!(details.contains(&("Closed".to_owned(), "true".to_owned())));
        assert!(details.contains(&("Elevation".to_owned(), "2.500000".to_owned())));
        assert!(details.contains(&("Constant width".to_owned(), "0.250000".to_owned())));
    }

    #[test]
    fn lineweight_values_are_displayed_in_millimeters() {
        assert_eq!(lineweight_text(LineWeight::ByLayer), "ByLayer");
        assert_eq!(lineweight_text(LineWeight::Value(25)), "0.25 mm");
    }
}
