//! Tests of the drawing primitives (a submodule of `drawing`).
//!
//! Each drawing is a DXF written here; every expected position is worked out
//! by hand in the comments, not with the code under test.

use std::f64::consts::PI;

use arq_geometry::{Point2, Tolerance};

use super::*;
use crate::test_dxf::{LAYERS, door_block, dxf};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.approx_eq(b, Tolerance::default())
}

/// Every shape of the whole drawing, in order.
fn shapes(doc: &Document) -> Vec<DrawItem> {
    doc.drawing_all()
        .into_iter()
        .flat_map(|(_, items)| items)
        .collect()
}

/// The ends of every line, rounded for comparison.
fn lines(items: &[DrawItem]) -> Vec<((f64, f64), (f64, f64))> {
    let r = |v: f64| (v * 1e6).round() / 1e6 + 0.0;
    items
        .iter()
        .filter_map(|i| match i.shape {
            Shape::Line(l) => Some((
                (r(l.start().x()), r(l.start().y())),
                (r(l.end().x()), r(l.end().y())),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn polylines_become_lines_and_arcs() {
    let doc = dxf(
        LAYERS,
        &[],
        &[
            // A closed square.
            &[
                "0",
                "LWPOLYLINE",
                "8",
                "0",
                "90",
                "4",
                "70",
                "1", //
                "10",
                "0",
                "20",
                "0",
                "10",
                "2",
                "20",
                "0",
                "10",
                "2",
                "20",
                "2",
                "10",
                "0",
                "20",
                "2",
            ],
            // (0,0) to (2,0) with bulge 1: a half circle around (1,0), run
            // counterclockwise from (0,0), so through (1,-1).
            &[
                "0",
                "LWPOLYLINE",
                "8",
                "0",
                "90",
                "2",
                "70",
                "0",
                "10",
                "0",
                "20",
                "0",
                "42",
                "1",
                "10",
                "2",
                "20",
                "0",
            ],
        ],
    );
    let items = shapes(&doc);
    assert_eq!(
        lines(&items),
        [
            ((0.0, 0.0), (2.0, 0.0)),
            ((2.0, 0.0), (2.0, 2.0)),
            ((2.0, 2.0), (0.0, 2.0)),
            ((0.0, 2.0), (0.0, 0.0)),
        ]
    );
    let arcs: Vec<Arc> = items
        .iter()
        .filter_map(|i| match i.shape {
            Shape::Arc(a) => Some(a),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 1);
    let arc = arcs[0];
    assert!(near(arc.center(), p(1.0, 0.0)) && (arc.radius() - 1.0).abs() < 1e-9);
    assert!(near(arc.midpoint(), p(1.0, -1.0)), "{:?}", arc.midpoint());
}

#[test]
fn a_mirrored_polyline_is_drawn_where_it_is() {
    // Normal (0,0,-1): OCS x is world -x, so OCS (1,0)-(3,0) is world
    // (-1,0)-(-3,0). The codec's explode gives OCS coordinates on LINEs.
    let doc = dxf(
        LAYERS,
        &[],
        &[&[
            "0",
            "LWPOLYLINE",
            "8",
            "0",
            "90",
            "2",
            "70",
            "0",
            "210",
            "0",
            "220",
            "0",
            "230",
            "-1", //
            "10",
            "1",
            "20",
            "0",
            "10",
            "3",
            "20",
            "0",
        ]],
    );
    assert_eq!(lines(&shapes(&doc)), [((-1.0, 0.0), (-3.0, 0.0))]);
}

#[test]
fn an_insert_is_its_block_moved_rotated_and_scaled() {
    // INSERT at (10,5), scale 2, rotation 90°, on A-PUERTAS, color 1 (red).
    // Point q of the block goes to (10,5) + rot90(2 (q - (1,1))), where
    // rot90(x,y) = (-y,x):
    //   (1,1)->(10,5)   (2,1)->(10,7)   (1,2)->(8,5)
    let block = door_block();
    let doc = dxf(
        LAYERS,
        &[&block],
        &[&[
            "0",
            "INSERT",
            "8",
            "A-PUERTAS",
            "62",
            "1",
            "2",
            "PUERTA", //
            "10",
            "10",
            "20",
            "5",
            "30",
            "0",
            "41",
            "2",
            "42",
            "2",
            "43",
            "2",
            "50",
            "90",
        ]],
    );
    let items = shapes(&doc);
    assert_eq!(
        lines(&items),
        [((10.0, 5.0), (10.0, 7.0)), ((10.0, 5.0), (8.0, 5.0))]
    );
    // Layer 0 + ByBlock takes the insert's color; MUROS keeps its own.
    assert_eq!(items[0].color, Color::Index(1));
    assert_eq!(items[1].color, Color::Index(1), "MUROS is color 1 too");
}

#[test]
fn layer_zero_and_by_layer_follow_the_insert() {
    // Same block, inserted at the origin on A-PUERTAS (color 5) with its own
    // color ByLayer: the layer-0 line becomes A-PUERTAS, so color 5.
    let block = door_block();
    let doc = dxf(
        LAYERS,
        &[&block],
        &[&[
            "0",
            "INSERT",
            "8",
            "A-PUERTAS",
            "62",
            "256",
            "2",
            "PUERTA",
            "10",
            "0",
            "20",
            "0",
            "30",
            "0",
        ]],
    );
    let items = shapes(&doc);
    assert_eq!(
        lines(&items),
        [((0.0, 0.0), (1.0, 0.0)), ((0.0, 0.0), (0.0, 1.0))]
    );
    assert_eq!(
        items[0].color,
        Color::Index(5),
        "ByBlock -> insert ByLayer -> A-PUERTAS"
    );
    assert_eq!(items[1].color, Color::Index(1), "MUROS");
}

#[test]
fn nested_and_arrayed_inserts() {
    // OUTER (base 0,0) holds PUERTA inserted at (100,0). OUTER is inserted
    // at (0,100) as a 2-column array, spacing 50: copies at x = 0 and 50.
    // PUERTA's first line (1,1)-(2,1) -> (0,0)-(1,0) -> +(100,0) -> +(0,100)
    // -> (100,100)-(101,100), and +(50,0) for the second column.
    let block = door_block();
    let outer: &[&str] = &[
        "0", "BLOCK", "8", "0", "2", "OUTER", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "OUTER", //
        "0", "INSERT", "8", "0", "2", "PUERTA", "10", "100", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let doc = dxf(
        LAYERS,
        &[&block, outer],
        &[&[
            "0", "INSERT", "8", "0", "2", "OUTER", "10", "0", "20", "100", "30", "0", //
            "70", "2", "71", "1", "44", "50", "45", "0",
        ]],
    );
    let found = lines(&shapes(&doc));
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(
        found.contains(&((100.0, 100.0), (101.0, 100.0))),
        "{found:?}"
    );
    assert!(
        found.contains(&((150.0, 100.0), (151.0, 100.0))),
        "{found:?}"
    );
}

#[test]
fn a_block_inside_itself_stops() {
    let selfish: &[&str] = &[
        "0", "BLOCK", "8", "0", "2", "BUCLE", "70", "0", "10", "0", "20", "0", "30", "0", "3",
        "BUCLE", //
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "1", "21", "0", "31",
        "0", //
        "0", "INSERT", "8", "0", "2", "BUCLE", "10", "5", "20", "0", "30", "0", //
        "0", "ENDBLK", "8", "0",
    ];
    let doc = dxf(
        LAYERS,
        &[selfish],
        &[&[
            "0", "INSERT", "8", "0", "2", "BUCLE", "10", "0", "20", "0", "30", "0",
        ]],
    );
    assert_eq!(
        lines(&shapes(&doc)),
        [((0.0, 0.0), (1.0, 0.0))],
        "drawn once, no endless loop"
    );
}

#[test]
fn an_insert_with_a_mirrored_normal() {
    // Normal (0,0,-1): the insertion point (10,0) is OCS, world (-10,0);
    // the block's (1,1)-(2,1) -> (0,0)-(1,0) -> OCS (10,0)-(11,0) ->
    // world (-10,0)-(-11,0).
    let block = door_block();
    let doc = dxf(
        LAYERS,
        &[&block],
        &[&[
            "0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "0", "30", "0", "210", "0",
            "220", "0", "230", "-1",
        ]],
    );
    let found = lines(&shapes(&doc));
    assert!(found.contains(&((-10.0, 0.0), (-11.0, 0.0))), "{found:?}");
}

#[test]
fn hidden_layers_are_not_drawn() {
    let block = door_block();
    let line = |layer: &'static str| -> Vec<&'static str> {
        vec![
            "0", "LINE", "8", layer, "10", "0", "20", "0", "30", "0", "11", "1", "21", "1", "31",
            "0",
        ]
    };
    let (off, frozen) = (line("APAGADA"), line("CONGELADA"));
    let doc = dxf(
        LAYERS,
        &[&block],
        &[
            &off,
            &frozen,
            // Insert on a frozen layer: nothing of it.
            &[
                "0",
                "INSERT",
                "8",
                "CONGELADA",
                "2",
                "PUERTA",
                "10",
                "0",
                "20",
                "0",
                "30",
                "0",
            ],
            // Insert on an off layer: its layer-0 line is hidden, its MUROS
            // line still shows.
            &[
                "0", "INSERT", "8", "APAGADA", "2", "PUERTA", "10", "50", "20", "0", "30", "0",
            ],
        ],
    );
    assert_eq!(lines(&shapes(&doc)), [((50.0, 0.0), (50.0, 1.0))]);
}

#[test]
fn texts_points_and_unknown_ids() {
    let doc = dxf(
        LAYERS,
        &[],
        &[
            &[
                "0", "TEXT", "8", "0", "10", "20", "20", "5", "30", "0", "40", "2.5", "1", "NOTA",
            ],
            &["0", "POINT", "8", "MUROS", "10", "3", "20", "4", "30", "0"],
            &[
                "0", "CIRCLE", "8", "0", "10", "0", "20", "0", "30", "0", "40", "2",
            ],
            &[
                "0", "ARC", "8", "0", "10", "0", "20", "0", "30", "0", "40", "1", "50", "0", "51",
                "90",
            ],
        ],
    );
    let all = shapes(&doc);
    match all[0].shape {
        // Its box picks it; "NOTA" at height 2.5 has no descenders.
        Shape::Text(b) => assert!(near(b.min(), p(20.0, 5.0)) && b.height() > 1.0, "{b:?}"),
        other => panic!("not a text box: {other:?}"),
    }
    // Then its letters, in the built-in font (text_layout.rs).
    let strokes = all[1..]
        .iter()
        .take_while(|i| matches!(i.shape, Shape::Line(_)))
        .count();
    assert!(strokes > 10, "{strokes} strokes");
    let items = &all[strokes..];
    assert_eq!(items[1].shape, Shape::Point(p(3.0, 4.0)));
    assert_eq!(items[1].color, Color::Index(1), "MUROS");
    assert!(matches!(items[2].shape, Shape::Circle(c) if (c.radius() - 2.0).abs() < 1e-12));
    match items[3].shape {
        Shape::Arc(a) => assert!((a.end_angle() - PI / 2.0).abs() < 1e-12),
        other => panic!("not an arc: {other:?}"),
    }
    assert_eq!(all[0].color, Color::Index(7));
    let id = doc.entities()[0].id();
    assert_eq!(doc.drawing(id).unwrap(), all[..=strokes].to_vec());
    assert!(doc.drawing(999_999).is_none());
}
