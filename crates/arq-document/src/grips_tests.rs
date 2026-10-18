//! Tests of grips (a submodule of `grips`).

use super::{Grip, GripKind};
use crate::document::Document;
use crate::entity::EntityId;
use crate::test_dxf::{LAYERS, door_block, dxf};

/// A LINE (0,0)-(10,0); a CIRCLE at (20,0) radius 2; an ARC at (30,0)
/// radius 2 from 0 to 90 degrees; the PUERTA block at (10,5); the same
/// block with normal (0,0,-1) at OCS (10,0), drawn at world (-10,0); a
/// polyline from (1,2); a TEXT, which has no grips yet.
fn sample() -> (Document, Vec<EntityId>) {
    let line = [
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "10", "21", "0", "31", "0",
    ];
    let circle = [
        "0", "CIRCLE", "8", "0", "10", "20", "20", "0", "30", "0", "40", "2",
    ];
    let arc = [
        "0", "ARC", "8", "0", "10", "30", "20", "0", "30", "0", "40", "2", "50", "0", "51", "90",
    ];
    let door = [
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "5", "30", "0",
    ];
    let mirrored = [
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "0", "30", "0", "210", "0",
        "220", "0", "230", "-1",
    ];
    let polyline = [
        "0",
        "LWPOLYLINE",
        "8",
        "0",
        "90",
        "2",
        "70",
        "0",
        "10",
        "1",
        "20",
        "2",
        "10",
        "4",
        "20",
        "2",
    ];
    let text = [
        "0", "TEXT", "8", "0", "10", "40", "20", "0", "30", "0", "40", "1", "1", "AB",
    ];
    let door_block = door_block();
    let doc = dxf(
        LAYERS,
        &[&door_block],
        &[&line, &circle, &arc, &door, &mirrored, &polyline, &text],
    );
    let ids = doc.entities().iter().map(|e| e.id()).collect();
    (doc, ids)
}

fn of(grips: &[Grip], id: EntityId) -> Vec<(GripKind, (f64, f64))> {
    grips
        .iter()
        .filter(|g| g.entity == id)
        .map(|g| {
            let round = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
            (g.kind, (round(g.point.x()), round(g.point.y())))
        })
        .collect()
}

#[test]
fn every_kind_shows_its_grips() {
    use GripKind::*;
    let (doc, ids) = sample();
    let grips = doc.grips(&ids);
    assert_eq!(
        of(&grips, ids[0]),
        [(End, (0.0, 0.0)), (Mid, (5.0, 0.0)), (End, (10.0, 0.0))]
    );
    assert_eq!(
        of(&grips, ids[1]),
        [
            (Center, (20.0, 0.0)),
            (Quadrant, (22.0, 0.0)),
            (Quadrant, (20.0, 2.0)),
            (Quadrant, (18.0, 0.0)),
            (Quadrant, (20.0, -2.0)),
        ]
    );
    let half = 2.0 * std::f64::consts::FRAC_1_SQRT_2;
    let round = |v: f64| (v * 1e9).round() / 1e9;
    assert_eq!(
        of(&grips, ids[2]),
        [
            (End, (32.0, 0.0)),
            (Mid, (round(30.0 + half), round(half))),
            (End, (30.0, 2.0)),
            (Center, (30.0, 0.0)),
        ]
    );
    assert_eq!(of(&grips, ids[3]), [(Insertion, (10.0, 5.0))]);
    assert_eq!(
        of(&grips, ids[4]),
        [(Insertion, (-10.0, 0.0))],
        "a mirrored OCS puts it where it is drawn"
    );
    assert_eq!(
        of(&grips, ids[5]),
        [(Vertex, (1.0, 2.0)), (Vertex, (4.0, 2.0))]
    );
    let vertices: Vec<_> = grips
        .iter()
        .filter(|g| g.entity == ids[5])
        .map(|g| g.vertex_index)
        .collect();
    assert_eq!(vertices, [Some(0), Some(1)]);
    assert_eq!(of(&grips, ids[6]), [], "a text has no grips yet");
}

#[test]
fn grips_follow_the_order_given_without_repeats() {
    let (doc, ids) = sample();
    let grips = doc.grips(&[ids[3], ids[0], ids[3], 999_999]);
    let entities: Vec<EntityId> = grips.iter().map(|g| g.entity).collect();
    assert_eq!(entities, [ids[3], ids[0], ids[0], ids[0]]);
    assert_eq!(GripKind::Quadrant.name(), "QUA");
}
