//! Tests of EXTRUDE in the document (a submodule of `extruding`): closed
//! polylines and circles made solids, their profiles erased, the rest
//! refused with why.

use std::f64::consts::PI;

use acadrust::entities::{Circle, Line as CodecLine, LwPolyline};
use acadrust::types::{Vector2, Vector3};

use super::*;

fn drawing(entities: Vec<EntityType>) -> (Document, Vec<EntityId>) {
    let mut cad = acadrust::CadDocument::new();
    let ids = entities
        .into_iter()
        .map(|e| cad.add_entity(e).unwrap().value())
        .collect();
    (Document::from_engine(cad), ids)
}

fn polyline(points: &[(f64, f64, f64)], closed: bool) -> LwPolyline {
    let mut p = LwPolyline::new();
    for &(x, y, bulge) in points {
        p.add_point_with_bulge(Vector2::new(x, y), bulge);
    }
    p.is_closed = closed;
    p
}

fn volume(doc: &Document, id: EntityId) -> (f64, f64, f64) {
    let m = doc.solid(id).unwrap().unwrap().mass_properties().unwrap();
    (m.volume, m.bounds.min().z(), m.bounds.max().z())
}

#[track_caller]
fn about(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} != {b}");
}

#[test]
fn a_closed_polyline_becomes_a_solid_on_its_layer() {
    let mut square = polyline(
        &[
            (0.0, 0.0, 0.0),
            (4.0, 0.0, 0.0),
            (4.0, 2.0, 0.0),
            (0.0, 2.0, 0.0),
        ],
        true,
    );
    square.elevation = 2.0;
    square.common.layer = "MUROS".into();
    let (mut doc, ids) = drawing(vec![EntityType::LwPolyline(square)]);
    doc.create_layer("MUROS").ok();
    let out = doc.extrude_entities(&ids, 3.0).unwrap();
    assert_eq!(out.refused, []);
    let (v, low, high) = volume(&doc, out.solids[0]);
    about(v, 24.0);
    about(low, 2.0);
    about(high, 5.0);
    assert_eq!(doc.entity(out.solids[0]).unwrap().layer(), "MUROS");
    assert!(doc.entity(ids[0]).is_none(), "the profile is erased");
}

#[test]
fn bulges_make_round_sides() {
    // A slot: two straight sides and two half circles (bulge 1), 4 by 2.
    let slot = polyline(
        &[
            (0.0, -1.0, 0.0),
            (4.0, -1.0, 1.0),
            (4.0, 1.0, 0.0),
            (0.0, 1.0, 1.0),
        ],
        true,
    );
    let (mut doc, ids) = drawing(vec![EntityType::LwPolyline(slot)]);
    let out = doc.extrude_entities(&ids, -2.0).unwrap();
    let (v, low, high) = volume(&doc, out.solids[0]);
    about(v, (8.0 + PI) * 2.0);
    about(low, -2.0);
    about(high, 0.0);
    // Run clockwise (negative bulges), the same slot.
    let back = polyline(
        &[
            (0.0, 1.0, 0.0),
            (4.0, 1.0, -1.0),
            (4.0, -1.0, 0.0),
            (0.0, -1.0, -1.0),
        ],
        true,
    );
    let (mut doc, ids) = drawing(vec![EntityType::LwPolyline(back)]);
    let out = doc.extrude_entities(&ids, 2.0).unwrap();
    about(volume(&doc, out.solids[0]).0, (8.0 + PI) * 2.0);
}

#[test]
fn a_polyline_seen_from_below_extrudes_where_it_lies() {
    // Normal down: its OCS X runs west and its elevation 5 is world z -5;
    // a quarter-circle bulge still bulges outward.
    let mut p = polyline(
        &[
            (0.0, 0.0, 0.0),
            (2.0, 0.0, 0.0),
            (2.0, 2.0, 0.0),
            (0.0, 2.0, 0.0),
        ],
        true,
    );
    p.normal = Vector3::new(0.0, 0.0, -1.0);
    p.elevation = 5.0;
    let (mut doc, ids) = drawing(vec![EntityType::LwPolyline(p)]);
    let out = doc.extrude_entities(&ids, 1.0).unwrap();
    let id = out.solids[0];
    let (v, low, high) = volume(&doc, id);
    about(v, 4.0);
    about(low, -5.0);
    about(high, -4.0);
    let m = doc.solid(id).unwrap().unwrap().mass_properties().unwrap();
    about(m.bounds.min().x(), -2.0);
    // Its first side bulging a half circle outward (south in its OCS).
    let mut q = polyline(
        &[
            (0.0, 0.0, 1.0),
            (2.0, 0.0, 0.0),
            (2.0, 2.0, 0.0),
            (0.0, 2.0, 0.0),
        ],
        true,
    );
    q.normal = Vector3::new(0.0, 0.0, -1.0);
    let (mut doc, ids) = drawing(vec![EntityType::LwPolyline(q)]);
    let out = doc.extrude_entities(&ids, 1.0).unwrap();
    about(volume(&doc, out.solids[0]).0, 4.0 + PI / 2.0);
}

#[test]
fn a_circle_becomes_a_cylinder() {
    let mut circle = Circle::new();
    circle.center = Vector3::new(1.0, 1.0, 3.0);
    circle.radius = 2.0;
    let (mut doc, ids) = drawing(vec![EntityType::Circle(circle)]);
    let out = doc.extrude_entities(&ids, 4.0).unwrap();
    let (v, low, high) = volume(&doc, out.solids[0]);
    about(v, 16.0 * PI);
    about(low, 3.0);
    about(high, 7.0);
}

#[test]
fn what_is_not_a_closed_profile_is_left_with_why() {
    let open = polyline(&[(0.0, 0.0, 0.0), (4.0, 0.0, 0.0), (4.0, 2.0, 0.0)], false);
    let bow = polyline(
        &[
            (0.0, 0.0, 0.0),
            (2.0, 2.0, 0.0),
            (2.0, 0.0, 0.0),
            (0.0, 2.0, 0.0),
        ],
        true,
    );
    let mut tilted = polyline(&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0)], true);
    tilted.normal = Vector3::new(1.0, 0.0, 0.0);
    let line = CodecLine::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
    let (mut doc, ids) = drawing(vec![
        EntityType::LwPolyline(open),
        EntityType::LwPolyline(bow),
        EntityType::LwPolyline(tilted),
        EntityType::Line(line),
    ]);
    let out = doc.extrude_entities(&ids, 1.0).unwrap();
    assert_eq!(out.solids, []);
    assert_eq!(
        out.refused,
        [
            (ids[0], Unextruded::Open),
            (ids[1], Unextruded::Profile(ProfileError::Crossing)),
            (ids[2], Unextruded::Tilted),
            (ids[3], Unextruded::NotAProfile),
        ]
    );
    assert!(
        ids.iter().all(|&id| doc.entity(id).is_some()),
        "nothing erased"
    );
    assert!(matches!(
        doc.extrude_entities(&ids, 0.0),
        Err(DocumentError::Geometry(_))
    ));
    assert!(matches!(
        doc.extrude_entities(&[999_999], 1.0),
        Err(DocumentError::EntityNotFound(999_999))
    ));
}

#[test]
fn a_bulge_below_a_half_circle_has_its_center_off_the_chord() {
    // A 2 square whose first side bulges out a quarter circle: bulge
    // tan(22.5°), radius √2, adding a segment of π/2 - 1.
    let quarter = (PI / 8.0).tan();
    let p = polyline(
        &[
            (0.0, 0.0, quarter),
            (2.0, 0.0, 0.0),
            (2.0, 2.0, 0.0),
            (0.0, 2.0, 0.0),
        ],
        true,
    );
    let (mut doc, ids) = drawing(vec![EntityType::LwPolyline(p)]);
    let out = doc.extrude_entities(&ids, 1.0).unwrap();
    let id = out.solids[0];
    about(volume(&doc, id).0, 4.0 + PI / 2.0 - 1.0);
    let m = doc.solid(id).unwrap().unwrap().mass_properties().unwrap();
    // It bulges south by √2 - 1.
    about(m.bounds.min().y(), 1.0 - 2.0f64.sqrt());
}
