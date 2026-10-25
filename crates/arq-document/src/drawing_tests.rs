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
    // (-1,0)-(-3,0), for a light polyline and for an old-style one, then
    // OCS (5,0)-(7,0) is world (-5,0)-(-7,0).
    let doc = dxf(
        LAYERS,
        &[],
        &[
            &[
                "0", "POLYLINE", "8", "0", "66", "1", "70", "0", "210", "0", "220", "0", "230",
                "-1", "0", "VERTEX", "8", "0", "10", "5", "20", "0", "0", "VERTEX", "8", "0", "10",
                "7", "20", "0", "0", "SEQEND", "8", "0",
            ],
            &[
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
            ],
        ],
    );
    assert_eq!(
        lines(&shapes(&doc)),
        [((-5.0, 0.0), (-7.0, 0.0)), ((-1.0, 0.0), (-3.0, 0.0))]
    );
}

#[test]
fn a_solid_is_drawn_by_its_outline() {
    // Corners 1 (0,0), 2 (2,0), 3 (0,1), 4 (2,1) in a mirrored OCS: world
    // x is -x. The outline runs 1-2-4-3: (0,0) (-2,0) (-2,1) (0,1).
    let doc = dxf(
        LAYERS,
        &[],
        &[&[
            "0", "SOLID", "8", "0", "10", "0", "20", "0", "11", "2", "21", "0", "12", "0", "22",
            "1", "13", "2", "23", "1", "210", "0", "220", "0", "230", "-1",
        ]],
    );
    assert_eq!(
        lines(&shapes(&doc)),
        [
            ((0.0, 0.0), (-2.0, 0.0)),
            ((-2.0, 0.0), (-2.0, 1.0)),
            ((-2.0, 1.0), (0.0, 1.0)),
            ((0.0, 1.0), (0.0, 0.0)),
        ]
    );
}

/// A solid-filled hatch on layer MUROS with `paths` (group codes from 91 on).
fn hatch(normal_z: &'static str, paths: &[&'static str]) -> Document {
    let mut codes = vec![
        "0", "HATCH", "8", "MUROS", "10", "0", "20", "0", "30", "0", "210", "0", "220", "0", "230",
        normal_z, "2", "SOLID", "70", "1", "71", "0",
    ];
    codes.extend_from_slice(paths);
    codes.extend_from_slice(&["75", "0", "76", "1", "98", "0"]);
    dxf(LAYERS, &[], &[&codes])
}

#[test]
fn a_hatch_is_drawn_by_its_boundary() {
    // Two loops: a polyline square (0,0)-(2,2), its last side bulging out
    // (bulge 1 from (0,2) back to (0,0): a half circle around (0,1), run
    // counterclockwise, so through (-1,1)); and edges: a LINE (5,0)-(7,0), an
    // ARC around (6,0), radius 1, from 0° to 180° counterclockwise, and one
    // around (10,0) from 90° to 0° clockwise, stored mirrored as 270° to
    // 360°: the quarter through 45°, (10.7071,0.7071).
    let doc = hatch(
        "1",
        &[
            "91", "2", //
            "92", "2", "72", "1", "73", "1", "93", "4", "10", "0", "20", "0", "42", "0", "10", "2",
            "20", "0", "42", "0", "10", "2", "20", "2", "42", "0", "10", "0", "20", "2", "42", "1",
            "97", "0", //
            "92", "1", "93", "3", "72", "1", "10", "5", "20", "0", "11", "7", "21", "0", "72", "2",
            "10", "6", "20", "0", "40", "1", "50", "0", "51", "180", "73", "1", "72", "2", "10",
            "10", "20", "0", "40", "1", "50", "270", "51", "360", "73", "0", "97", "0",
        ],
    );
    let items = shapes(&doc);
    assert_eq!(
        lines(&items),
        [
            ((0.0, 0.0), (2.0, 0.0)),
            ((2.0, 0.0), (2.0, 2.0)),
            ((2.0, 2.0), (0.0, 2.0)),
            ((5.0, 0.0), (7.0, 0.0)),
        ]
    );
    let middles: Vec<Point2> = items
        .iter()
        .filter_map(|i| match i.shape {
            Shape::Arc(a) => Some(a.midpoint()),
            _ => None,
        })
        .collect();
    let h = std::f64::consts::FRAC_1_SQRT_2;
    let expected = [p(-1.0, 1.0), p(6.0, 1.0), p(10.0 + h, h)];
    assert_eq!(middles.len(), 3, "{items:?}");
    for (found, wanted) in middles.iter().zip(expected) {
        assert!(near(*found, wanted), "{found:?} is not {wanted:?}");
    }
    // Drawn as the hatch is: on MUROS, colored by layer (1).
    assert!(items.iter().all(|i| i.color == Color::Index(1)));
}

/// The box of everything drawn.
fn extent(items: &[DrawItem]) -> Bounds2 {
    items
        .iter()
        .map(|i| i.shape.bounds())
        .reduce(|a, b| a.union(b))
        .unwrap()
}

#[test]
fn a_hatch_with_ellipse_and_spline_edges() {
    // A whole ellipse around (0,0), semi-axes 2 (along x) and 1: the box
    // (-2,-1)-(2,1). A quadratic spline from (0,0) to (4,0) pulled up by
    // (2,2) (knots 0 0 0 1 1 1): it peaks at (2,1). Its box is (0,0)-(4,1).
    let ellipse = hatch(
        "1",
        &[
            "91", "1", "92", "1", "93", "1", "72", "3", "10", "0", "20", "0", "11", "2", "21", "0",
            "40", "0.5", "50", "0", "51", "360", "73", "1", "97", "0",
        ],
    );
    // Drawn as segments between points on it: within 0.005 of its box.
    let b = extent(&shapes(&ellipse));
    assert!(within(b, (-2.0, -1.0), (2.0, 1.0), 0.005), "{b:?}");
    let spline = hatch(
        "1",
        &[
            "91", "1", "92", "1", "93", "2", "72", "4", "94", "2", "73", "0", "74", "0", "95", "6",
            "96", "3", "40", "0", "40", "0", "40", "0", "40", "1", "40", "1", "40", "1", "10", "0",
            "20", "0", "10", "2", "20", "2", "10", "4", "20", "0", "72", "1", "10", "4", "20", "0",
            "11", "0", "21", "0", "97", "0",
        ],
    );
    let b = extent(&shapes(&spline));
    assert!(within(b, (0.0, 0.0), (4.0, 1.0), 0.01), "{b:?}");
}

/// Whether box `b` runs from `lo` to `hi`, give or take `tolerance`.
fn within(b: Bounds2, lo: (f64, f64), hi: (f64, f64), tolerance: f64) -> bool {
    let close = |a: Point2, (x, y): (f64, f64)| {
        (a.x() - x).abs() < tolerance && (a.y() - y).abs() < tolerance
    };
    close(b.min(), lo) && close(b.max(), hi)
}

#[test]
fn ellipses_and_3d_faces_are_drawn() {
    // Half an ellipse around (0,0), semi-axes 2 and 1, parameters 0 to π:
    // the upper half, box (-2,0)-(2,1); with normal (0,0,-1) the minor axis
    // turns down, so the lower half, box (-2,-1)-(2,0). From 3π/2 on past
    // 2π to π/2: the right half, box (0,-1)-(2,1). A 3DFACE
    // (5,0)-(7,0)-(7,1)-(5,1): its four sides.
    let ellipse = |normal_z: &'static str, start: &'static str, end: &'static str| {
        dxf(
            LAYERS,
            &[],
            &[&[
                "0", "ELLIPSE", "8", "0", "10", "0", "20", "0", "30", "0", "11", "2", "21", "0",
                "31", "0", "210", "0", "220", "0", "230", normal_z, "40", "0.5", "41", start, "42",
                end,
            ]],
        )
    };
    let (pi, half_pi) = ("3.141592653589793", "1.5707963267948966");
    let b = extent(&shapes(&ellipse("1", "0", pi)));
    assert!(within(b, (-2.0, 0.0), (2.0, 1.0), 0.005), "{b:?}");
    let b = extent(&shapes(&ellipse("-1", "0", pi)));
    assert!(within(b, (-2.0, -1.0), (2.0, 0.0), 0.005), "{b:?}");
    let b = extent(&shapes(&ellipse("1", "4.71238898038469", half_pi)));
    assert!(within(b, (0.0, -1.0), (2.0, 1.0), 0.005), "{b:?}");
    let face = dxf(
        LAYERS,
        &[],
        &[&[
            "0", "3DFACE", "8", "0", "10", "5", "20", "0", "30", "0", "11", "7", "21", "0", "31",
            "0", "12", "7", "22", "1", "32", "0", "13", "5", "23", "1", "33", "0",
        ]],
    );
    assert_eq!(
        lines(&shapes(&face)),
        [
            ((5.0, 0.0), (7.0, 0.0)),
            ((7.0, 0.0), (7.0, 1.0)),
            ((7.0, 1.0), (5.0, 1.0)),
            ((5.0, 1.0), (5.0, 0.0)),
        ]
    );
}

#[test]
fn a_hatch_with_rational_and_mirrored_curved_edges() {
    // A rational spline edge, control points (1,0) (1,1) (0,1) weighted
    // 1, √2/2, 1: exactly a quarter of the unit circle around (0,0), every
    // point drawn at distance 1 (unweighted, it would bulge to 1.06).
    let spline = hatch(
        "1",
        &[
            "91",
            "1",
            "92",
            "1",
            "93",
            "2",
            "72",
            "4",
            "94",
            "2",
            "73",
            "1",
            "74",
            "0",
            "95",
            "6",
            "96",
            "3",
            "40",
            "0",
            "40",
            "0",
            "40",
            "0",
            "40",
            "1",
            "40",
            "1",
            "40",
            "1",
            "10",
            "1",
            "20",
            "0",
            "42",
            "1",
            "10",
            "1",
            "20",
            "1",
            "42",
            "0.7071067811865476",
            "10",
            "0",
            "20",
            "1",
            "42",
            "1",
            "72",
            "1",
            "10",
            "0",
            "20",
            "1",
            "11",
            "1",
            "21",
            "0",
            "97",
            "0",
        ],
    );
    let curve: Vec<Point2> = lines(&shapes(&spline))
        .into_iter()
        .filter(|((ax, ay), (bx, by))| (ax + ay - 1.0).abs() > 1e-9 || (bx + by - 1.0).abs() > 1e-9)
        .flat_map(|(a, b)| [p(a.0, a.1), p(b.0, b.1)])
        .collect();
    assert!(curve.len() > 4, "{curve:?}");
    for q in &curve {
        let r = (q.x() * q.x() + q.y() * q.y()).sqrt();
        assert!((r - 1.0).abs() < 1e-3, "{q:?} at {r}");
    }
    // A mirrored hatch's elliptic edge: around OCS (3,0), semi-axes 2 and
    // 1, 0° to 180° (the upper half), so in the world around (-3,0), box
    // (-5,0)-(-1,1).
    let mirrored = hatch(
        "-1",
        &[
            "91", "1", "92", "1", "93", "2", "72", "3", "10", "3", "20", "0", "11", "2", "21", "0",
            "40", "0.5", "50", "0", "51", "180", "73", "1", "72", "1", "10", "1", "20", "0", "11",
            "5", "21", "0", "97", "0",
        ],
    );
    let b = extent(&shapes(&mirrored));
    assert!(within(b, (-5.0, 0.0), (-1.0, 1.0), 0.005), "{b:?}");
}

#[test]
fn a_mirrored_hatch_is_drawn_where_it_is() {
    // OCS LINE (1,0)-(3,0) and ARC around (6,0) from 0° to 90°, through
    // OCS (6.7071,0.7071), and a polyline loop (1,2)-(3,2); mirrored: world
    // (-1,0)-(-3,0), an arc around (-6,0) through (-6.7071,0.7071) and
    // (-1,2)-(-3,2).
    let doc = hatch(
        "-1",
        &[
            "91", "2", "92", "1", "93", "2", "72", "1", "10", "1", "20", "0", "11", "3", "21", "0",
            "72", "2", "10", "6", "20", "0", "40", "1", "50", "0", "51", "90", "73", "1", "97",
            "0", //
            "92", "2", "72", "0", "73", "0", "93", "2", "10", "1", "20", "2", "10", "3", "20", "2",
            "97", "0",
        ],
    );
    let items = shapes(&doc);
    assert_eq!(
        lines(&items),
        [((-1.0, 0.0), (-3.0, 0.0)), ((-1.0, 2.0), (-3.0, 2.0))]
    );
    let Some(Shape::Arc(arc)) = items
        .iter()
        .map(|i| i.shape)
        .find(|s| matches!(s, Shape::Arc(_)))
    else {
        panic!("{items:?}")
    };
    let h = std::f64::consts::FRAC_1_SQRT_2;
    assert!(near(arc.midpoint(), p(-6.0 - h, h)), "{:?}", arc.midpoint());
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

#[test]
fn external_references_draw_filename_strokes_without_loading_or_mutating() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xrefs");
    for file in ["attachment.dxf", "overlay.dxf", "missing.dxf", "nested.dxf"] {
        let doc = Document::open(root.join(file)).unwrap();
        let before = doc.snapshot();
        let drawing = shapes(&doc);
        assert!(
            drawing.iter().any(|i| matches!(i.shape, Shape::Text(_))),
            "{file}"
        );
        assert!(!lines(&drawing).is_empty(), "{file}");
        assert!(
            !drawing
                .iter()
                .any(|i| matches!(i.shape, Shape::Placeholder(_)))
        );
        assert!(doc.validate_roundtrip(&before).is_empty());
        assert_eq!(doc.cad().entities().count(), 1);
    }
}

#[test]
fn xref_filename_uses_both_path_separators_and_respects_layer_visibility() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/xrefs/attachment.dxf");
    let mut doc = Document::open(path).unwrap();
    let expected = shapes(&doc);
    for path in ["C:\\site\\child.dxf", "/site/child.dxf"] {
        doc.cad_mut()
            .block_records
            .get_mut("SITE")
            .unwrap()
            .xref_path = path.into();
        assert_eq!(shapes(&doc), expected);
    }
    let layer = doc.layer_by_name("0").unwrap().id();
    doc.set_layer_visible(layer, false).unwrap();
    assert!(shapes(&doc).is_empty());
}

#[test]
fn xref_marker_transforms_strokes_instead_of_replacing_them_with_a_box() {
    let mut local = Vec::new();
    xref_marker(
        "child.dwg",
        Vector3::new(0.0, 0.0, 0.0),
        &Transform::identity(),
        Color::Index(3),
        &mut local,
    );
    let t = Transform::from_scaling(Vector3::new(-2.0, 3.0, 1.0))
        .then(&Transform::from_rotation(
            Vector3::new(0.0, 0.0, 1.0),
            PI / 2.0,
        ))
        .then(&Transform::from_translation(Vector3::new(10.0, 20.0, 0.0)));
    let mut moved = Vec::new();
    xref_marker(
        "child.dwg",
        Vector3::new(0.0, 0.0, 0.0),
        &t,
        Color::Index(3),
        &mut moved,
    );
    let (a, b) = (lines(&local), lines(&moved));
    assert!(!a.is_empty());
    assert_eq!(a.len(), b.len());
    for ((a1, a2), (b1, b2)) in a.into_iter().zip(b) {
        for ((x, y), (xx, yy)) in [(a1, b1), (a2, b2)] {
            assert!((xx - (10.0 - 3.0 * y)).abs() < 1e-5);
            assert!((yy - (20.0 - 2.0 * x)).abs() < 1e-5);
        }
    }
    assert!(moved.iter().all(|i| i.color == Color::Index(3)));
}

#[test]
fn unloaded_and_pathless_xrefs_still_have_a_marker() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/xrefs/attachment.dxf");
    let mut doc = Document::open(path).unwrap();
    let expected = shapes(&doc);
    doc.cad_mut()
        .block_records
        .get_mut("SITE")
        .unwrap()
        .flags
        .is_xref_unloaded = true;
    assert_eq!(shapes(&doc), expected);
    doc.cad_mut()
        .block_records
        .get_mut("SITE")
        .unwrap()
        .xref_path
        .clear();
    assert!(
        !lines(&shapes(&doc)).is_empty(),
        "the block name is the fallback label"
    );
}

#[test]
fn xref_arrays_place_one_filename_at_each_instance() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/xrefs/attachment.dxf");
    let mut doc = Document::open(path).unwrap();
    let id = doc.cad().entities().next().unwrap().common().handle;
    let EntityType::Insert(i) = doc.cad_mut().get_entity_mut(id).unwrap() else {
        panic!("expected INSERT")
    };
    i.row_count = 2;
    i.column_count = 3;
    i.row_spacing = 100.0;
    i.column_spacing = 100.0;
    let drawing = shapes(&doc);
    let bounds: Vec<_> = drawing
        .iter()
        .filter_map(|item| match item.shape {
            Shape::Text(b) => Some(b),
            _ => None,
        })
        .collect();
    assert_eq!(bounds.len(), 6);
    // The source's 30-degree rotation applies to array offsets, after scaling.
    assert!((bounds[1].min().x() - bounds[0].min().x() - 100.0 * (PI / 6.0).cos()).abs() < 1e-9);
    assert!((bounds[1].min().y() - bounds[0].min().y() - 50.0).abs() < 1e-9);
    assert!((bounds[3].min().x() - bounds[0].min().x() + 50.0).abs() < 1e-9);
}
