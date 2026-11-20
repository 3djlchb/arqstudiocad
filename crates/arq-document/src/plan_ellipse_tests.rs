//! An ellipse lying in the plan as plan geometry (ADR 0041 D4): read,
//! written in its own sense, owned by the model, kept through a save. A
//! submodule of `entities`.

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
    let entity = doc.entity(id).unwrap();
    assert!(entity.is_editable() && entity.is_transformable());
    let plan = entity.plan().unwrap();
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
    doc.set_plan_geometry(id, plan.with_geometry(grown))
        .unwrap();
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
    assert_eq!(doc.entity(id).unwrap().plan().unwrap().geometry(), grown);
}

#[test]
fn a_fence_meets_an_ellipse_and_a_text_takes_no_plan_geometry() {
    let mut doc = from_below();
    let id = doc.entities()[0].id();
    // x = 12 crosses the arc below, at (12, -√3).
    let hits = doc.fence_candidates(p(12.0, -5.0), p(12.0, 5.0), &[id]);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(near(hits[0].1, p(12.0, -3f64.sqrt())), "{hits:?}");
    // A text has no curve, and takes none.
    let text = doc.add_text("x", p(0.0, 0.0), 1.0, 0.0).unwrap();
    assert!(doc.entity(text).unwrap().plan().is_none());
    let plan = doc.entity(id).unwrap().plan().unwrap();
    assert!(doc.set_plan_geometry(text, plan).is_err());
}

#[test]
fn a_tilted_ellipse_has_no_plan_geometry_and_takes_none() {
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
    assert!(doc.entity(tilted).unwrap().plan().is_none());
    assert!(!doc.entity(tilted).unwrap().is_transformable());
    let below = from_below();
    let plan = below
        .entity(below.entities()[0].id())
        .unwrap()
        .plan()
        .unwrap();
    let before = doc.drawing(tilted).unwrap();
    assert!(doc.set_plan_geometry(tilted, plan).is_err());
    assert_eq!(doc.drawing(tilted).unwrap(), before, "unchanged");
}

#[test]
fn the_model_owns_an_ellipse_in_the_plan_and_a_save_keeps_it() {
    let doc = from_below();
    let id = doc.entities()[0].id();
    let owned = doc.owned_view(id).expect("owned by the model");
    assert_eq!(owned.kind(), "ELLIPSE");
    assert_eq!(owned.plan(), doc.entity(id).unwrap().plan());
    // A save, read back and checked against the model, in DWG and DXF.
    let dir = tempfile::tempdir().unwrap();
    for name in ["e.dwg", "e.dxf"] {
        doc.save(dir.path().join(name)).unwrap();
    }
    let back = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    let again = back.entity(back.entities()[0].id()).unwrap();
    let (a, b) = (
        doc.entity(id).unwrap().plan().unwrap().geometry(),
        again.plan().unwrap().geometry(),
    );
    let (Geometry2::Ellipse(a), Geometry2::Ellipse(b)) = (a, b) else {
        panic!()
    };
    let (ea, eb) = (a.ends().unwrap(), b.ends().unwrap());
    assert!(near(ea.0, eb.0) && near(ea.1, eb.1) && (a.ratio() - b.ratio()).abs() < 1e-12);
}

#[test]
fn an_ellipse_written_at_another_height_moves_there() {
    let mut doc = from_below();
    let id = doc.entities()[0].id();
    let plan = doc.entity(id).unwrap().plan().unwrap();
    let raised = crate::PlanGeometry::at_elevation(plan.geometry(), 8.0).unwrap();
    doc.set_plan_geometry(id, raised).unwrap();
    let back = doc.entity(id).unwrap().plan().unwrap();
    assert!((back.elevation() - 8.0).abs() < 1e-12);
    assert_eq!(back.geometry(), plan.geometry(), "the same ellipse, higher");
}

/// A drawing of one ellipse about (10,0), its major semi-axis `major`,
/// minor to major `ratio`, from parameter `start` to `end`.
fn one_ellipse(major: (f64, f64), ratio: f64, start: f64, end: f64) -> Document {
    let mut doc = Document::new();
    let e = arq_geometry::Ellipse::new(
        p(10.0, 0.0),
        arq_geometry::Vector2::new(major.0, major.1).unwrap(),
        ratio,
        start,
        end,
    )
    .unwrap();
    doc.add_ellipse(e, 0.0).unwrap();
    doc
}

#[test]
fn the_check_after_saving_sees_an_ellipse_changed_within_the_same_box() {
    // Each pair has the same box, so only the ellipse's own data tells
    // them apart: its major axis along the other diagonal; other semi-axes
    // with a² + b² kept (5,3 and √29,√5 along the diagonal); an arc all
    // but a gap away from its extremes against the whole; the same gap
    // moved.
    let d = std::f64::consts::FRAC_1_SQRT_2;
    let a2 = 29f64.sqrt();
    let tau = std::f64::consts::TAU;
    let pairs = [
        (
            ((5.0 * d, 5.0 * d), 0.6, 0.0, 0.0),
            ((5.0 * d, -5.0 * d), 0.6, 0.0, 0.0),
        ),
        (
            ((5.0 * d, 5.0 * d), 0.6, 0.0, 0.0),
            ((a2 * d, a2 * d), 5f64.sqrt() / a2, 0.0, 0.0),
        ),
        (
            ((5.0, 0.0), 0.6, 0.0, 0.0),
            ((5.0, 0.0), 0.6, 0.3, 0.1 + tau),
        ),
        (((5.0, 0.0), 0.6, 0.3, 0.1), ((5.0, 0.0), 0.6, 0.25, 0.05)),
    ];
    for ((m0, r0, s0, e0), (m1, r1, s1, e1)) in pairs {
        let (a, b) = (one_ellipse(m0, r0, s0, e0), one_ellipse(m1, r1, s1, e1));
        let (ia, ib) = (a.entities()[0].id(), b.entities()[0].id());
        assert_eq!(ia, ib);
        let bounds = |doc: &Document| doc.entity(ia).unwrap().bounds().unwrap();
        assert!(bounds(&a).approx_eq(bounds(&b), arq_geometry::Tolerance::DEFAULT));
        assert!(
            a.validate_roundtrip(&one_ellipse(m0, r0, s0, e0))
                .is_empty()
        );
        assert!(
            !a.validate_roundtrip(&b).is_empty(),
            "{m1:?} {r1} {s1} {e1}"
        );
    }
    // And at another height.
    let a = one_ellipse((5.0, 0.0), 0.6, 0.0, 0.0);
    let mut higher = one_ellipse((5.0, 0.0), 0.6, 0.0, 0.0);
    let id = higher.entities()[0].id();
    let plan = higher.entity(id).unwrap().plan().unwrap();
    higher
        .set_plan_geometry(
            id,
            crate::PlanGeometry::at_elevation(plan.geometry(), 4.0).unwrap(),
        )
        .unwrap();
    assert!(!a.validate_roundtrip(&higher).is_empty());
}
