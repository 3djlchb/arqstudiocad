//! Tests of picking, window selection and snaps (a submodule of `pick`).

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
