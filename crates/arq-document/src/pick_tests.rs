//! Tests of picking, window selection and snaps (a submodule of `pick`).

use std::f64::consts::PI;

use arq_geometry::snap::SnapKind::{self, *};
use arq_geometry::{Bounds2, Point2};

use crate::document::Document;
use crate::entity::EntityId;
use crate::test_dxf::{LAYERS, door_block, dxf};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn area(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds2 {
    Bounds2::new(p(x0, y0), p(x1, y1))
}

/// Entities, in drawing order, and their ids by name.
struct Sample {
    doc: Document,
    line: EntityId,
    line_again: EntityId,
    circle: EntityId,
    arc: EntityId,
    door: EntityId,
    cross: EntityId,
    text: EntityId,
}

/// LINE (0,0)-(10,0); CIRCLE (20,0) r 2; ARC (30,0) r 2 from 0° to 180°;
/// the PUERTA block at (10,5), drawn (10,5)-(11,5) and (10,5)-(10,6); a
/// LINE (5,-5)-(5,5) crossing the first; a TEXT at (40,0); the same LINE
/// (0,0)-(10,0) again, drawn last; lines on an off and a frozen layer.
fn sample() -> Sample {
    let line = [
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "10", "21", "0", "31", "0",
    ];
    let circle = [
        "0", "CIRCLE", "8", "0", "10", "20", "20", "0", "30", "0", "40", "2",
    ];
    let arc = [
        "0", "ARC", "8", "0", "10", "30", "20", "0", "30", "0", "40", "2", "50", "0", "51", "180",
    ];
    let door = [
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "5", "30", "0",
    ];
    let cross = [
        "0", "LINE", "8", "0", "10", "5", "20", "-5", "30", "0", "11", "5", "21", "5", "31", "0",
    ];
    let text = [
        "0", "TEXT", "8", "0", "10", "40", "20", "0", "30", "0", "40", "1", "1", "AB",
    ];
    let off = [
        "0", "LINE", "8", "APAGADA", "10", "0", "20", "10", "30", "0", "11", "10", "21", "10",
        "31", "0",
    ];
    let frozen = [
        "0",
        "LINE",
        "8",
        "CONGELADA",
        "10",
        "0",
        "20",
        "20",
        "30",
        "0",
        "11",
        "10",
        "21",
        "20",
        "31",
        "0",
    ];
    let door_def = door_block();
    let doc = dxf(
        LAYERS,
        &[&door_def],
        &[
            &line, &circle, &arc, &door, &cross, &text, &line, &off, &frozen,
        ],
    );
    let ids: Vec<EntityId> = doc.drawing_all().into_iter().map(|(id, _)| id).collect();
    Sample {
        line: ids[0],
        circle: ids[1],
        arc: ids[2],
        door: ids[3],
        cross: ids[4],
        text: ids[5],
        line_again: ids[6],
        doc,
    }
}

#[test]
fn a_pick_finds_the_closest_drawn_entity_within_the_aperture() {
    let s = sample();
    assert_eq!(
        s.doc.pick(p(20.0, 2.3), 0.5),
        Some(s.circle),
        "on the circle"
    );
    assert_eq!(s.doc.pick(p(30.0, 2.2), 0.5), Some(s.arc));
    assert_eq!(s.doc.pick(p(30.0, -2.2), 0.5), None, "where the arc is not");
    assert_eq!(
        s.doc.pick(p(20.0, 0.0), 0.5),
        None,
        "inside, far from the curve"
    );
    assert_eq!(s.doc.pick(p(5.2, 3.0), 0.5), Some(s.cross));
    assert_eq!(s.doc.pick(p(2.0, 0.6), 0.5), None, "beyond the aperture");
}

#[test]
fn a_block_is_picked_as_a_whole_by_what_it_draws() {
    let s = sample();
    assert_eq!(s.doc.pick(p(10.5, 5.1), 0.2), Some(s.door));
    assert_eq!(
        s.doc.pick(p(10.6, 5.6), 0.2),
        None,
        "inside the block's box, off its lines"
    );
}

#[test]
fn on_a_tie_the_entity_drawn_last_is_picked() {
    let s = sample();
    assert_eq!(s.doc.pick(p(2.0, 0.1), 0.5), Some(s.line_again));
    assert_ne!(s.line, s.line_again);
}

#[test]
fn a_text_is_picked_anywhere_inside_its_box() {
    let s = sample();
    assert_eq!(s.doc.pick(p(40.1, 0.1), 0.0), Some(s.text));
}

#[test]
fn hidden_layers_are_never_picked_and_bad_apertures_find_nothing() {
    let s = sample();
    assert_eq!(s.doc.pick(p(5.0, 10.0), 0.5), None, "off layer");
    assert_eq!(s.doc.pick(p(5.0, 20.0), 0.5), None, "frozen layer");
    assert_eq!(s.doc.pick(p(5.0, 0.0), f64::NAN), None);
    assert_eq!(s.doc.pick(p(5.0, 0.0), -1.0), None);
}

#[test]
fn a_window_takes_what_lies_wholly_inside() {
    let s = sample();
    let found = s.doc.select_window(area(-1.0, -1.0, 11.0, 1.0), false);
    assert_eq!(
        found,
        [s.line, s.line_again],
        "not the crossing line, which pokes out"
    );
    let found = s.doc.select_window(area(9.0, 4.0, 12.0, 7.0), false);
    assert_eq!(found, [s.door], "the whole block");
    assert!(
        s.doc
            .select_window(area(9.0, 4.0, 10.5, 7.0), false)
            .is_empty(),
        "half the block"
    );
    assert!(
        s.doc
            .select_window(area(-1.0, 9.0, 11.0, 21.0), false)
            .is_empty(),
        "hidden layers"
    );
}

#[test]
fn a_crossing_takes_whatever_it_touches_exactly() {
    let s = sample();
    let found = s.doc.select_window(area(4.0, -1.0, 6.0, 1.0), true);
    assert_eq!(found, [s.line, s.cross, s.line_again]);
    assert!(
        s.doc
            .select_window(area(4.0, -1.0, 6.0, 1.0), false)
            .is_empty()
    );
    // Inside the circle without touching it: its box overlaps, the curve does not.
    assert!(
        s.doc
            .select_window(area(19.5, -0.5, 20.5, 0.5), true)
            .is_empty()
    );
    assert_eq!(
        s.doc.select_window(area(21.5, -0.5, 23.0, 0.5), true),
        [s.circle]
    );
    // Wholly inside, touching no edge: a crossing takes it too.
    assert_eq!(
        s.doc.select_window(area(17.0, -3.0, 23.0, 3.0), true),
        [s.circle]
    );
    assert_eq!(
        s.doc.select_window(area(9.0, 4.0, 10.5, 7.0), true),
        [s.door]
    );
    assert_eq!(
        s.doc.select_window(area(39.0, -1.0, 40.2, 0.2), true),
        [s.text]
    );
}

fn snap(
    s: &Sample,
    x: f64,
    y: f64,
    modes: &[SnapKind],
    from: Option<Point2>,
) -> Option<(SnapKind, Point2)> {
    s.doc
        .snap(p(x, y), 0.5, modes, from)
        .map(|sp| (sp.kind, sp.point))
}

fn near(a: Point2, b: Point2) -> bool {
    a.distance_to(b) < 1e-9
}

#[test]
fn point_snaps_take_the_closest_point_within_the_aperture() {
    let s = sample();
    assert_eq!(
        snap(&s, 10.2, 0.1, &[Endpoint], None),
        Some((Endpoint, p(10.0, 0.0)))
    );
    assert_eq!(
        snap(&s, 2.6, 0.1, &[Midpoint], None),
        None,
        "the midpoint is beyond the aperture"
    );
    assert_eq!(snap(&s, 7.6, 0.1, &[Midpoint], None), None);
    assert_eq!(
        snap(&s, 20.1, 2.2, &[Quadrant], None),
        Some((Quadrant, p(20.0, 2.0)))
    );
    assert_eq!(
        snap(&s, 5.2, 0.2, &[Intersection], None),
        Some((Intersection, p(5.0, 0.0)))
    );
    assert_eq!(
        snap(&s, 10.9, 5.1, &[Endpoint], None),
        Some((Endpoint, p(11.0, 5.0))),
        "in a block"
    );
}

#[test]
fn center_perpendicular_and_tangent_come_from_the_curve_under_the_cursor() {
    let s = sample();
    assert_eq!(
        snap(&s, 21.9, 0.6, &[Center], None),
        Some((Center, p(20.0, 0.0))),
        "far from the cursor"
    );
    assert_eq!(
        snap(&s, 20.0, 0.0, &[Center], None),
        None,
        "not on the curve"
    );
    assert_eq!(
        snap(&s, 2.0, 0.1, &[Perpendicular], Some(p(7.0, 3.0))),
        Some((Perpendicular, p(7.0, 0.0)))
    );
    assert_eq!(
        snap(&s, 2.0, 0.1, &[Perpendicular], None),
        None,
        "needs a last point"
    );
    // From (20,10) the tangent points of the circle are (20 ± 2·√0.96, 0.4).
    let (kind, at) = snap(&s, 21.9, 0.5, &[Tangent], Some(p(20.0, 10.0))).unwrap();
    assert_eq!(kind, Tangent);
    assert!(near(at, p(20.0 + 2.0 * 0.96f64.sqrt(), 0.4)), "{at:?}");
}

#[test]
fn nearest_is_the_last_resort_and_point_snaps_come_first() {
    let s = sample();
    assert_eq!(
        snap(&s, 3.0, 0.2, &[Nearest], None),
        Some((Nearest, p(3.0, 0.0)))
    );
    let all = [
        Endpoint,
        Midpoint,
        Center,
        Quadrant,
        Intersection,
        Perpendicular,
        Tangent,
        Nearest,
    ];
    assert_eq!(
        snap(&s, 10.1, 0.1, &all, None),
        Some((Endpoint, p(10.0, 0.0)))
    );
    assert_eq!(
        snap(&s, 21.9, 0.6, &all, None),
        Some((Center, p(20.0, 0.0))),
        "no point snap near"
    );
    assert_eq!(snap(&s, 3.0, 0.2, &all, None), Some((Nearest, p(3.0, 0.0))));
}

#[test]
fn snaps_ignore_hidden_layers_inactive_modes_and_bad_apertures() {
    let s = sample();
    assert_eq!(snap(&s, 0.1, 10.1, &[Endpoint], None), None, "off layer");
    assert_eq!(snap(&s, 0.1, 20.1, &[Endpoint], None), None, "frozen layer");
    assert_eq!(
        snap(&s, 10.1, 0.1, &[Midpoint], None),
        None,
        "endpoint not active"
    );
    assert_eq!(snap(&s, 10.1, 0.1, &[], None), None);
    assert!(
        s.doc
            .snap(p(10.1, 0.1), f64::INFINITY, &[Endpoint], None)
            .is_none()
    );
}

/// A diagonal 2 wide from (0,0) to (10,10) (ADR 0038).
fn wide_diagonal() -> (Document, EntityId) {
    let mut doc = Document::new();
    let style = crate::entity::PolylineStyle {
        width: 2.0,
        ..Default::default()
    };
    let id = doc
        .add_polyline_with(&[(p(0.0, 0.0), 0.0), (p(10.0, 10.0), 0.0)], false, style)
        .unwrap();
    (doc, id)
}

#[test]
fn a_wide_polyline_is_picked_anywhere_on_its_band() {
    let (doc, id) = wide_diagonal();
    // 0.7 off the centerline, inside the band (half of 2 is 1).
    assert_eq!(doc.pick(p(5.5, 4.5), 0.1), Some(id));
    assert_eq!(
        doc.pick(p(6.0, 4.0), 0.1),
        None,
        "1.41 off: beyond the band"
    );
    assert_eq!(
        doc.pick(p(6.0, 4.0), 0.5),
        Some(id),
        "within the aperture of its edge"
    );
}

#[test]
fn windows_and_crossings_see_the_band_exactly() {
    let (doc, id) = wide_diagonal();
    // Inside the band's box but away from the band: the box would touch.
    assert!(
        doc.select_window(area(7.0, 0.0, 10.0, 3.0), true)
            .is_empty()
    );
    assert_eq!(
        doc.select_window(area(4.8, 5.0, 5.2, 5.4), true),
        vec![id],
        "inside it"
    );
    assert_eq!(
        doc.select_window(area(5.5, 3.0, 9.0, 4.4), true),
        vec![id],
        "on its edge"
    );
    // A window must hold the band, not the centerline only.
    assert!(
        doc.select_window(area(-0.1, -0.1, 10.1, 10.1), false)
            .is_empty()
    );
    assert_eq!(
        doc.select_window(area(-1.0, -1.0, 11.0, 11.0), false),
        vec![id]
    );
}

#[test]
fn fence_candidates_find_exact_curves_and_skip_blocks_and_texts() {
    let s = sample();
    let ids: Vec<_> = s.doc.entities().iter().map(|e| e.id()).collect();
    let hits = s.doc.fence_candidates(p(2.0, -1.0), p(2.0, 1.0), &ids);
    assert_eq!(
        hits.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![s.line, s.line_again]
    );
    assert!(
        hits.iter()
            .all(|(_, at)| at.approx_eq(p(2.0, 0.0), arq_geometry::Tolerance::default()))
    );
    assert!(
        s.doc
            .fence_candidates(p(21.8, 1.8), p(22.2, 2.2), &[s.circle])
            .is_empty()
    );
    assert!(
        s.doc
            .fence_candidates(p(10.5, 4.0), p(10.5, 7.0), &[s.door])
            .is_empty()
    );
    assert!(
        s.doc
            .fence_candidates(p(40.0, -2.0), p(40.0, 2.0), &[s.text])
            .is_empty()
    );
}

#[test]
fn crossing_polygon_selects_exact_drawings_instead_of_their_enclosing_box() {
    use arq_geometry::{Circle, Line, SelectionPolygon, Tolerance};
    let mut doc = Document::new();
    let zero = doc.current_layer().id();
    let mut add = |curve| {
        doc.add_entity(crate::PlanGeometry::new(curve), zero)
            .unwrap()
    };
    let a = add(Line::new(p(-1., 0.5), p(5., 0.5)).unwrap().into());
    let b = add(Line::new(p(2., 2.), p(3., 3.)).unwrap().into());
    let circle = add(Circle::new(p(0.5, 3.), 0.2).unwrap().into());
    let polygon = SelectionPolygon::new(
        vec![
            p(0., 0.),
            p(4., 0.),
            p(4., 1.),
            p(1., 1.),
            p(1., 4.),
            p(0., 4.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        doc.select_polygon_candidates(&polygon, &[a, b, circle]),
        vec![a, circle]
    );
    let wide = doc
        .add_polyline_with(
            &[(p(0., 10.), 0.), (p(10., 10.), 0.)],
            false,
            crate::PolylineStyle {
                width: 4.,
                ..Default::default()
            },
        )
        .unwrap();
    let inside_band = SelectionPolygon::new(
        vec![p(4., 9.5), p(6., 9.5), p(6., 10.5), p(4., 10.5)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        doc.select_polygon_candidates(&inside_band, &[a, b, circle, wide]),
        vec![wide]
    );
}

/// An elliptical arc about (10,0), semi-axes 4 along x and 2, parameters 0
/// to 3π/2: from (14,0) round through (10,2) and (6,0) to (10,-2).
fn ellipse_arc() -> Document {
    dxf(
        LAYERS,
        &[],
        &[&[
            "0",
            "ELLIPSE",
            "8",
            "0",
            "10",
            "10",
            "20",
            "0",
            "30",
            "0",
            "11",
            "4",
            "21",
            "0",
            "31",
            "0",
            "210",
            "0",
            "220",
            "0",
            "230",
            "1",
            "40",
            "0.5",
            "41",
            "0",
            "42",
            "4.71238898038469",
        ]],
    )
}

#[test]
fn an_ellipse_snaps_to_its_own_points_not_to_its_chords() {
    let doc = ellipse_arc();
    let at = |cursor: Point2, modes: &[SnapKind]| doc.snap(cursor, 0.3, modes, None);
    // Ends, quadrants and the arc's middle (parameter 3π/4).
    let end = at(p(10.1, -2.1), &[Endpoint]).unwrap();
    assert!(near(end.point, p(10.0, -2.0)));
    let top = at(p(10.1, 2.1), &[Quadrant]).unwrap();
    assert!(near(top.point, p(10.0, 2.0)));
    let middle = (10.0 + 4.0 * (0.75 * PI).cos(), 2.0 * (0.75 * PI).sin());
    let mid = at(p(middle.0 + 0.05, middle.1), &[Midpoint]).unwrap();
    assert!(near(mid.point, p(middle.0, middle.1)), "{mid:?}");
    // No endpoint between chords, where segments used to end.
    let along = (10.0 + 4.0 * 0.3f64.cos(), 2.0 * 0.3f64.sin());
    assert!(at(p(along.0, along.1), &[Endpoint, Midpoint]).is_none());
    // Its center from anywhere on it; the nearest point exactly on it.
    let center = at(p(along.0, along.1), &[Center]).unwrap();
    assert!(near(center.point, p(10.0, 0.0)));
    let nearest = at(p(along.0 + 0.1, along.1 + 0.05), &[Nearest]).unwrap();
    let off = ((nearest.point.x() - 10.0) / 4.0).powi(2) + (nearest.point.y() / 2.0).powi(2);
    assert!((off - 1.0).abs() < 1e-9, "on the ellipse: {nearest:?}");
}

#[test]
fn an_ellipse_is_picked_and_selected_by_its_curve() {
    let doc = ellipse_arc();
    let id = doc.entities()[0].id();
    assert_eq!(doc.pick(p(14.05, 0.0), 0.1), Some(id));
    // Its missing quarter, between (10,-2) and (14,0), is not it.
    assert_eq!(doc.pick(p(12.9, -1.4), 0.1), None);
    assert_eq!(doc.select_window(area(5.9, -2.1, 14.1, 2.1), false), [id]);
    assert_eq!(doc.select_window(area(13.5, -0.5, 15.0, 0.5), true), [id]);
    assert!(
        doc.select_window(area(11.0, -1.0, 13.0, 1.0), true)
            .is_empty(),
        "inside, untouched"
    );
}

#[test]
fn an_ellipse_meets_lines_circles_and_ellipses_at_intersection_snaps() {
    use arq_geometry::{Circle, Ellipse, Line, Vector2};
    let mut doc = ellipse_arc();
    let zero = doc.current_layer().id();
    // y = 1: x = 10 ± 4·√(1 - 1/4) on the arc about (10,0), 4 by 2.
    let dx = 4.0 * 0.75f64.sqrt();
    let line = Line::new(p(4.0, 1.0), p(16.0, 1.0)).unwrap();
    doc.add_entity(crate::PlanGeometry::new(line.into()), zero)
        .unwrap();
    let at = |doc: &Document, cursor: Point2| doc.snap(cursor, 0.3, &[Intersection], None);
    let found = at(&doc, p(10.0 + dx + 0.1, 1.05)).unwrap();
    assert!(near(found.point, p(10.0 + dx, 1.0)), "{found:?}");
    let found = at(&doc, p(10.0 - dx - 0.1, 0.95)).unwrap();
    assert!(near(found.point, p(10.0 - dx, 1.0)), "{found:?}");
    // A circle about (14,0), radius 1, meets it once on the arc, at y > 0;
    // its lower crossing lies on the missing quarter.
    let circle = Circle::new(p(14.0, 0.0), 1.0).unwrap();
    doc.add_entity(crate::PlanGeometry::new(circle.into()), zero)
        .unwrap();
    let upper = at(&doc, p(13.5, 0.85)).unwrap();
    let (x, y) = (upper.point.x(), upper.point.y());
    assert!(((x - 10.0) / 4.0).powi(2) + (y / 2.0).powi(2) - 1.0 < 1e-9 && y > 0.0);
    assert!(((x - 14.0).powi(2) + y * y - 1.0).abs() < 1e-9, "{upper:?}");
    assert!(at(&doc, p(upper.point.x(), -upper.point.y())).is_none());
    // A second ellipse through it, its major axis upright.
    let upright =
        Ellipse::new(p(10.0, 0.0), Vector2::new(0.0, 4.0).unwrap(), 0.5, 0.0, 0.0).unwrap();
    doc.add_ellipse(upright, 0.0).unwrap();
    // By symmetry they cross where |x - 10| = |y|: (x-10)²·(1/16 + 1/4) = 1.
    let c = (1.0 / (1.0 / 16.0 + 0.25f64)).sqrt();
    let crossing = at(&doc, p(10.0 - c - 0.05, c + 0.05)).unwrap();
    assert!(near(crossing.point, p(10.0 - c, c)), "{crossing:?}");
}

#[test]
fn perpendicular_and_tangent_snaps_reach_an_ellipse() {
    let doc = ellipse_arc();
    let at =
        |cursor: Point2, kind: SnapKind, from: Point2| doc.snap(cursor, 0.3, &[kind], Some(from));
    // From (10,5), square onto its top.
    let foot = at(p(10.3, 1.98), Perpendicular, p(10.0, 5.0)).unwrap();
    assert!(near(foot.point, p(10.0, 2.0)), "{foot:?}");
    // From (20,0), the tangents touch where (x-10)·10/16 = 1: x = 11.6,
    // the one above on the arc.
    let touch = at(p(11.7, 1.6), Tangent, p(20.0, 0.0)).unwrap();
    let y = 2.0 * (1.0 - 0.16f64).sqrt();
    assert!(near(touch.point, p(11.6, y)), "{touch:?}");
    // The one below lies on the missing quarter: no tangent there.
    assert!(at(p(11.7, -1.6), Tangent, p(20.0, 0.0)).is_none());
}

#[test]
fn an_ellipse_is_crossed_by_its_curve_not_by_its_chords() {
    use arq_geometry::{SelectionPolygon, Tolerance};
    // Its top, (10,2), falls between two of its drawn points, the nearest
    // at about y = 1.99958: a window from y = 1.9998 meets only the curve.
    let doc = ellipse_arc();
    let id = doc.entities()[0].id();
    assert_eq!(doc.select_window(area(9.9, 1.9998, 10.1, 2.1), true), [id]);
    let polygon = SelectionPolygon::new(
        vec![p(9.9, 1.9998), p(10.1, 1.9998), p(10.0, 2.1)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(doc.select_polygon_candidates(&polygon, &[id]), [id]);
    // Just above its top, nothing.
    assert!(
        doc.select_window(area(9.9, 2.0001, 10.1, 2.1), true)
            .is_empty()
    );
}

#[test]
fn an_ellipse_written_as_plan_geometry_is_the_ellipse_drawn() {
    use arq_geometry::{Ellipse, Vector2};
    let mut doc = Document::new();
    let zero = doc.current_layer().id();
    let e = Ellipse::new(p(3.0, 4.0), Vector2::new(2.0, 1.0).unwrap(), 0.4, 0.5, 4.0).unwrap();
    let id = doc
        .add_entity(crate::PlanGeometry::new(e.into()), zero)
        .unwrap();
    assert_eq!(doc.entity(id).unwrap().kind(), "ELLIPSE");
    let [item] = &doc.drawing(id).unwrap()[..] else {
        panic!()
    };
    let crate::Shape::Ellipse(drawn) = item.shape else {
        panic!("{item:?}")
    };
    let (a, b) = (e.ends().unwrap(), drawn.ends().unwrap());
    assert!(near(a.0, b.0) && near(a.1, b.1) && near(drawn.center(), e.center()));
    assert!((drawn.ratio() - 0.4).abs() < 1e-12);
}

#[test]
fn a_crossing_polygon_selects_an_ellipse_it_crosses_and_not_one_it_misses() {
    use arq_geometry::{SelectionPolygon, Tolerance};
    let doc = ellipse_arc();
    let id = doc.entities()[0].id();
    let polygon =
        |corners: Vec<Point2>| SelectionPolygon::new(corners, Tolerance::DEFAULT).unwrap();
    // An L across its end at (14,0).
    let across = polygon(vec![
        p(13.5, -0.5),
        p(15.0, -0.5),
        p(15.0, 0.5),
        p(14.2, 0.5),
        p(14.2, 3.0),
        p(13.5, 3.0),
    ]);
    assert_eq!(doc.select_polygon_candidates(&across, &[id]), [id]);
    // A thin band across it between two of its drawn points, (14,0) and
    // the next at about (14.0,0.12): only a chord crosses it.
    let thin = polygon(vec![
        p(13.0, 0.04),
        p(15.0, 0.04),
        p(15.0, 0.08),
        p(13.0, 0.08),
    ]);
    assert_eq!(doc.select_polygon_candidates(&thin, &[id]), [id]);
    // Wholly around it.
    let around = polygon(vec![p(5.0, -3.0), p(15.0, -3.0), p(15.0, 3.0), p(5.0, 3.0)]);
    assert_eq!(doc.select_polygon_candidates(&around, &[id]), [id]);
    // Inside it, untouched, and across its missing quarter, about (12.83,-1.41).
    let inside = polygon(vec![p(9.0, -0.5), p(11.0, -0.5), p(11.0, 0.5), p(9.0, 0.5)]);
    let missing = polygon(vec![
        p(12.5, -1.7),
        p(13.1, -1.7),
        p(13.1, -1.1),
        p(12.5, -1.1),
    ]);
    assert!(doc.select_polygon_candidates(&inside, &[id]).is_empty());
    assert!(doc.select_polygon_candidates(&missing, &[id]).is_empty());
}

/// A cubic spline from (0,0) through the control points to (6,0), and a
/// closed one far away.
fn splines() -> Document {
    let spline = |flags: &str, dx: f64| -> Vec<String> {
        let mut c: Vec<String> = [
            "0", "SPLINE", "8", "0", "210", "0", "220", "0", "230", "1", "70", flags, "71", "3",
            "72", "8", "73", "4", "74", "0",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        for k in [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
            c.extend(["40".to_owned(), format!("{k:?}")]);
        }
        for (x, y) in [(0.0, 0.0), (2.0, 3.0), (4.0, 3.0), (6.0, 0.0)] {
            c.extend([
                "10".to_owned(),
                format!("{:?}", x + dx),
                "20".to_owned(),
                format!("{y:?}"),
                "30".to_owned(),
                "0.0".to_owned(),
            ]);
        }
        c
    };
    let (open, closed) = (spline("8", 0.0), spline("9", 100.0));
    let open: Vec<&str> = open.iter().map(String::as_str).collect();
    let closed: Vec<&str> = closed.iter().map(String::as_str).collect();
    dxf(LAYERS, &[], &[&open, &closed])
}

#[test]
fn a_spline_snaps_to_its_ends_and_nearest_point_not_its_chords() {
    let doc = splines();
    let at = |cursor: Point2, modes: &[SnapKind]| doc.snap(cursor, 0.3, modes, None);
    let end = at(p(6.1, 0.1), &[Endpoint]).unwrap();
    assert!(near(end.point, p(6.0, 0.0)), "{end:?}");
    // Its middle, (3, 2.25) for this Bézier, is no end between chords.
    assert!(at(p(3.0, 2.25), &[Endpoint, Midpoint]).is_none());
    let nearest = at(p(3.0, 2.4), &[Nearest]).unwrap();
    assert!(near(nearest.point, p(3.0, 2.25)), "{nearest:?}");
    // A closed one has no ends.
    assert!(at(p(100.0, 0.05), &[Endpoint]).is_none());
}

#[test]
fn a_spline_is_picked_and_selected_by_its_curve() {
    let doc = splines();
    let id = doc.entities()[0].id();
    assert_eq!(doc.pick(p(3.0, 2.3), 0.1), Some(id));
    assert_eq!(doc.pick(p(3.0, 1.5), 0.1), None, "under it, off the curve");
    assert_eq!(doc.select_window(area(-0.1, -0.1, 6.1, 2.3), false), [id]);
    assert_eq!(doc.select_window(area(2.9, 2.0, 3.1, 2.5), true), [id]);
    assert!(
        doc.select_window(area(2.0, 0.5, 4.0, 1.5), true).is_empty(),
        "under it"
    );
    let polygon = arq_geometry::SelectionPolygon::new(
        vec![p(2.9, 2.0), p(3.1, 2.0), p(3.0, 2.6)],
        arq_geometry::Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(doc.select_polygon_candidates(&polygon, &[id]), [id]);
    // The entity's box is its control points' (ADR 0042 D4); its drawn
    // shape's is the curve's, under them, which windows go by.
    let b = doc.entity(id).unwrap().bounds().unwrap();
    assert_eq!((b.max().y(), b.max().x()), (3.0, 6.0));
    let drawn = doc.drawing(id).unwrap()[0].shape.bounds();
    assert!((drawn.max().y() - 2.25).abs() < 1e-9, "{drawn:?}");
}

/// The Bézier of `splines()`, from (0,0) by (2,3) and (4,3) to (6,0): x is
/// 6u, y is 9u(1 − u).
fn bezier(u: f64) -> Point2 {
    p(6.0 * u, 9.0 * u * (1.0 - u))
}

#[test]
fn a_spline_is_crossed_by_its_curve_not_its_chords() {
    // Between two of its sixteen chords a span, the curve lies 0.0088
    // above the chord: a window round the curve there, clear of the
    // chord, selects it (ADR 0043 D3).
    let doc = splines();
    let id = doc.entities()[0].id();
    let q = bezier(0.5 + 1.0 / 32.0);
    let chord = bezier(0.5).lerp(bezier(0.5 + 1.0 / 16.0), 0.5);
    assert!(q.y() - chord.y() > 0.008, "{q:?} {chord:?}");
    let window = area(q.x() - 0.001, q.y() - 0.001, q.x() + 0.001, q.y() + 0.001);
    assert_eq!(doc.select_window(window, true), [id]);
    let polygon = arq_geometry::SelectionPolygon::new(
        vec![
            p(q.x() - 0.002, q.y() - 0.001),
            p(q.x() + 0.002, q.y() - 0.001),
            p(q.x(), q.y() + 0.002),
        ],
        arq_geometry::Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(doc.select_polygon_candidates(&polygon, &[id]), [id]);
    // Just under the curve, between it and the chord: nothing.
    let below = area(q.x() - 0.001, q.y() - 0.006, q.x() + 0.001, q.y() - 0.004);
    assert!(doc.select_window(below, true).is_empty());
}

#[test]
fn a_spline_gives_intersection_perpendicular_and_tangent_snaps() {
    let mut doc = splines();
    let layer = doc.current_layer().id();
    let line = arq_geometry::Line::new(p(-1.0, 1.0), p(7.0, 1.0)).unwrap();
    doc.add_entity(crate::PlanGeometry::new(line.into()), layer)
        .unwrap();
    // y = 1 where u(1 − u) = 1/9.
    let u = (1.0 - 5f64.sqrt() / 3.0) / 2.0;
    let at = |cursor: Point2, modes: &[SnapKind], from: Option<Point2>| {
        doc.snap(cursor, 0.3, modes, from)
    };
    let int = at(p(6.0 * u + 0.05, 1.05), &[Intersection], None).unwrap();
    assert!(near(int.point, bezier(u)), "{int:?}");
    // From (3,5) the perpendicular falls on its top, (3, 2.25).
    let foot = at(p(3.0, 2.3), &[Perpendicular], Some(p(3.0, 5.0))).unwrap();
    assert_eq!(foot.kind, Perpendicular);
    assert!(near(foot.point, p(3.0, 2.25)), "{foot:?}");
    // From (−3,0) a line touches where (B − from) × B′ = 0, that is
    // u² + u − 1/2 = 0.
    let u = (3f64.sqrt() - 1.0) / 2.0;
    let touch = at(
        bezier(u) + arq_geometry::Vector2::new(0.05, 0.05).unwrap(),
        &[Tangent],
        Some(p(-3.0, 0.0)),
    )
    .unwrap();
    assert_eq!(touch.kind, Tangent);
    assert!(near(touch.point, bezier(u)), "{touch:?}");
}

#[test]
fn a_fence_meets_a_spline_to_trim() {
    // TRIM's fences pick a spline in the plan too (ADR 0043 D4).
    let doc = splines();
    let id = doc.entities()[0].id();
    let ids: Vec<EntityId> = doc.entities().iter().map(|e| e.id()).collect();
    let hits = doc.fence_candidates(p(3.0, -1.0), p(3.0, 5.0), &ids);
    let [(hit, point)] = hits[..] else {
        panic!("{hits:?}")
    };
    assert_eq!(hit, id);
    assert!(near(point, p(3.0, 2.25)), "{point:?}");
    assert!(
        doc.trim_search_bounds(id).is_ok(),
        "Quick TRIM's broad phase too"
    );
}
