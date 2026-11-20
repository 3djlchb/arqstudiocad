//! The curve TRIM, EXTEND and BREAK edit, ellipses included (ADR 0041):
//! a submodule of `entities`.

use acadrust::EntityType;
use arq_geometry::{Geometry2, Line, Point2, Tolerance, extend};

use crate::document::Document;
use crate::drawing::Shape;
use crate::test_dxf::{LAYERS, dxf};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn near(a: Point2, b: Point2) -> bool {
    a.distance_to(b) < 1e-9
}

/// An elliptical arc about (10,0), semi-axes 4 along x and 2, parameters
/// 0 to π/2, at height 3, seen from below (normal -Z): from (14,0) round
/// to (10,-2) in the world.
fn from_below() -> Document {
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
            "3",
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
            "-1",
            "40",
            "0.5",
            "41",
            "0",
            "42",
            "1.5707963267948966",
        ]],
    )
}

fn drawn(doc: &Document, id: u64) -> arq_geometry::Ellipse {
    let items = doc.drawing(id).unwrap();
    let [item] = &items[..] else {
        panic!("{items:?}")
    };
    let Shape::Ellipse(e) = item.shape else {
        panic!("{item:?}")
    };
    e
}

#[test]
fn an_ellipse_seen_from_below_is_edited_in_the_world_and_kept_below() {
    let mut doc = from_below();
    let id = doc.entities()[0].id();
    assert!(
        doc.entity(id).unwrap().plan().is_none(),
        "no plan geometry yet"
    );
    let plan = doc.curve_plan(id).unwrap();
    assert!((plan.elevation() - 3.0).abs() < 1e-12);
    let Geometry2::Ellipse(e) = plan.geometry() else {
        panic!()
    };
    let (a, b) = e.ends().unwrap();
    assert!(
        near(a, p(10.0, -2.0)) && near(b, p(14.0, 0.0)),
        "{a:?} {b:?}"
    );
    // Grown at (10,-2), counterclockwise backwards in the world, to x = 7.
    let wall: Geometry2 = Line::new(p(7.0, -9.0), p(7.0, 9.0)).unwrap().into();
    let grown = extend(plan.geometry(), &[wall], p(10.0, -2.0), Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    doc.set_curve_plan(id, plan.with_geometry(grown)).unwrap();
    let y = -2.0 * (1.0 - 9.0 / 16.0f64).sqrt();
    let (a, b) = drawn(&doc, id).ends().unwrap();
    assert!(near(a, p(7.0, y)) && near(b, p(14.0, 0.0)), "{a:?} {b:?}");
    let EntityType::Ellipse(el) = doc.cad().get_entity(crate::translate::handle(id)).unwrap()
    else {
        panic!()
    };
    assert!(
        el.normal.z < 0.0 && (el.center.z - 3.0).abs() < 1e-12,
        "{el:?}"
    );
    assert_eq!(doc.curve_plan(id).unwrap().geometry(), grown);
}

#[test]
fn a_fence_meets_an_ellipse_and_other_entities_have_no_curve() {
    let mut doc = from_below();
    let id = doc.entities()[0].id();
    // x = 12 crosses the arc below, at (12, -√3).
    let hits = doc.fence_candidates(p(12.0, -5.0), p(12.0, 5.0), &[id]);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(near(hits[0].1, p(12.0, -3f64.sqrt())), "{hits:?}");
    // A text has no curve, and takes none.
    let text = doc.add_text("x", p(0.0, 0.0), 1.0, 0.0).unwrap();
    assert!(doc.curve_plan(text).is_none());
    let plan = doc.curve_plan(id).unwrap();
    assert!(doc.set_curve_plan(text, plan).is_err());
}

#[test]
fn a_tilted_ellipse_has_no_curve_and_takes_none() {
    // Its normal along y: it stands upright, out of the plan.
    let mut doc = dxf(
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
            "1",
            "230",
            "0",
            "40",
            "0.5",
            "41",
            "0",
            "42",
            "1.5707963267948966",
        ]],
    );
    let tilted = doc.entities()[0].id();
    assert!(doc.curve_plan(tilted).is_none());
    let plan = from_below()
        .curve_plan(from_below().entities()[0].id())
        .unwrap();
    let before = doc.drawing(tilted).unwrap();
    assert!(doc.set_curve_plan(tilted, plan).is_err());
    assert_eq!(doc.drawing(tilted).unwrap(), before, "unchanged");
}
