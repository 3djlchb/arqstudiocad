//! Tests of REVOLVE in the document (a submodule of `revolving`): closed
//! polylines and circles turned about an axis in plan, whole or in part,
//! either way; the rest refused with why.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use acadrust::EntityType;
use acadrust::entities::{Circle, Line as CodecLine, LwPolyline};
use acadrust::types::{Vector2, Vector3 as CodecVector};

use super::*;

fn drawing(entities: Vec<EntityType>) -> (Document, Vec<EntityId>) {
    let mut cad = acadrust::CadDocument::new();
    let ids = entities
        .into_iter()
        .map(|e| cad.add_entity(e).unwrap().value())
        .collect();
    (Document::from_engine(cad), ids)
}

fn square(x0: f64, x1: f64) -> EntityType {
    let mut p = LwPolyline::new();
    for (x, y) in [(x0, 0.0), (x1, 0.0), (x1, 1.0), (x0, 1.0)] {
        p.add_point(Vector2::new(x, y));
    }
    p.is_closed = true;
    p.common.layer = "PIEZAS".into();
    EntityType::LwPolyline(p)
}

fn q(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn measured(doc: &Document, id: EntityId) -> arq_kernel::MassProperties {
    doc.solid(id).unwrap().unwrap().mass_properties().unwrap()
}

#[track_caller]
fn about(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} != {b}");
}

#[test]
fn a_square_beside_the_axis_turns_into_a_tube() {
    // From x 1 to 2, turned about the Y axis: a tube 1 high.
    let (mut doc, ids) = drawing(vec![square(1.0, 2.0)]);
    doc.create_layer("PIEZAS").ok();
    let out = doc
        .revolve_entities(&ids, q(0.0, 0.0), q(0.0, 1.0), TAU)
        .unwrap();
    assert_eq!(out.refused, []);
    let m = measured(&doc, out.solids[0]);
    about(m.volume, 3.0 * PI);
    about(m.bounds.min().z(), -2.0);
    about(m.bounds.max().z(), 2.0);
    assert_eq!(doc.entity(out.solids[0]).unwrap().layer(), "PIEZAS");
    assert!(doc.entity(ids[0]).is_none(), "the profile is erased");
}

#[test]
fn a_part_turn_goes_one_way_or_the_other() {
    // Counterclockwise about +Y, X turns toward -Z; the other way, +Z.
    for (angle, below) in [(FRAC_PI_2, true), (-FRAC_PI_2, false)] {
        let (mut doc, ids) = drawing(vec![square(1.0, 2.0)]);
        let out = doc
            .revolve_entities(&ids, q(0.0, 0.0), q(0.0, 1.0), angle)
            .unwrap();
        let m = measured(&doc, out.solids[0]);
        about(m.volume, 3.0 * PI / 4.0);
        if below {
            about(m.bounds.max().z(), 0.0);
            about(m.bounds.min().z(), -2.0);
        } else {
            about(m.bounds.min().z(), 0.0);
            about(m.bounds.max().z(), 2.0);
        }
    }
}

#[test]
fn a_profile_on_either_side_and_a_circle() {
    // On the axis's other side, and the axis drawn downward: the same.
    let (mut doc, ids) = drawing(vec![square(-2.0, -1.0)]);
    let out = doc
        .revolve_entities(&ids, q(0.0, 5.0), q(0.0, -5.0), TAU)
        .unwrap();
    about(measured(&doc, out.solids[0]).volume, 3.0 * PI);
    // A circle 3 from the axis, radius 1, at elevation 2: a torus there.
    let mut circle = Circle::new();
    circle.center = CodecVector::new(3.0, 0.0, 2.0);
    circle.radius = 1.0;
    let (mut doc, ids) = drawing(vec![EntityType::Circle(circle)]);
    let out = doc
        .revolve_entities(&ids, q(0.0, -1.0), q(0.0, 1.0), TAU)
        .unwrap();
    let m = measured(&doc, out.solids[0]);
    about(m.volume, 6.0 * PI * PI);
    about(m.centroid.z(), 2.0);
}

#[test]
fn what_cannot_be_turned_is_left_with_why() {
    let line = CodecLine::from_points(
        CodecVector::new(0.0, 0.0, 0.0),
        CodecVector::new(1.0, 0.0, 0.0),
    );
    let (mut doc, ids) = drawing(vec![square(-1.0, 1.0), EntityType::Line(line)]);
    let out = doc
        .revolve_entities(&ids, q(0.0, 0.0), q(0.0, 1.0), TAU)
        .unwrap();
    assert_eq!(out.solids, []);
    assert_eq!(
        out.refused,
        [
            (ids[0], Unrevolved::CrossesAxis),
            (ids[1], Unrevolved::NotAProfile)
        ]
    );
    assert!(matches!(
        doc.revolve_entities(&ids, q(0.0, 0.0), q(0.0, 1.0), 0.0),
        Err(DocumentError::Geometry(_))
    ));
    assert!(matches!(
        doc.revolve_entities(&ids, q(1.0, 1.0), q(1.0, 1.0), TAU),
        Err(DocumentError::Geometry(_))
    ));
}

fn bulged(points: &[(f64, f64, f64)]) -> EntityType {
    let mut p = LwPolyline::new();
    for &(x, y, bulge) in points {
        p.add_point_with_bulge(Vector2::new(x, y), bulge);
    }
    p.is_closed = true;
    EntityType::LwPolyline(p)
}

#[test]
fn arcs_on_the_axis_s_left_turn_the_right_way() {
    // A half disc left of the axis, its arc through (-1, 0): a ball. And
    // a square grooved by a half circle round (-3, 1), on that side too.
    let (mut doc, ids) = drawing(vec![
        bulged(&[(0.0, 1.0, 1.0), (0.0, -1.0, 0.0)]),
        bulged(&[
            (-1.0, 0.0, 0.0),
            (-3.0, 0.0, 1.0),
            (-3.0, 2.0, 0.0),
            (-1.0, 2.0, 0.0),
        ]),
    ]);
    let out = doc
        .revolve_entities(&ids, q(0.0, -5.0), q(0.0, 5.0), TAU)
        .unwrap();
    assert_eq!(out.refused, []);
    about(measured(&doc, out.solids[0]).volume, 4.0 / 3.0 * PI);
    about(
        measured(&doc, out.solids[1]).volume,
        16.0 * PI - 3.0 * PI * PI + 4.0 * PI / 3.0,
    );
}

#[test]
fn revolved_solids_are_written_and_read_back_the_same() {
    use crate::acis_write::sat_of;
    use crate::solid::translate;
    // A cone drawn clockwise, the groove, a ball's lune and a baluster's
    // belly (a lemon), whole and in part.
    let profiles = || {
        vec![
            bulged(&[(0.0, 0.0, 0.0), (0.0, 4.0, 0.0), (3.0, 0.0, 0.0)]),
            bulged(&[
                (1.0, 0.0, 0.0),
                (3.0, 0.0, -1.0),
                (3.0, 2.0, 0.0),
                (1.0, 2.0, 0.0),
            ]),
            bulged(&[(0.0, 1.0, -1.0), (0.0, -1.0, 0.0)]),
            bulged(&[
                (1.0, 0.0, 0.0),
                (2.0, 0.0, 0.0),
                (2.0, 2.0, 0.0),
                (0.0, 2.0, 0.0),
                (0.0, 1.0, -(PI / 8.0).tan()),
            ]),
            bulged(&[
                (0.0, 0.0, 0.0),
                (1.2, 0.0, 0.0),
                (1.2, 0.3, 0.0),
                (0.6, 0.6, 0.6),
                (0.5, 3.0, 0.0),
                (0.9, 3.3, 0.0),
                (0.9, 3.6, 0.0),
                (0.0, 3.6, 0.0),
            ]),
        ]
    };
    for angle in [TAU, 1.5 * PI] {
        let (mut doc, ids) = drawing(profiles());
        let out = doc
            .revolve_entities(&ids, q(0.0, 0.0), q(0.0, 1.0), angle)
            .unwrap();
        assert_eq!(out.refused, []);
        for id in out.solids {
            let body = doc.solid(id).unwrap().unwrap();
            let m = body.mass_properties().unwrap();
            let sat = sat_of(&body).unwrap();
            let back = translate(
                &acadrust::entities::acis::types::SatDocument::parse(&sat.to_sat_string()).unwrap(),
            )
            .unwrap();
            back.check(arq_geometry::Tolerance::DEFAULT).unwrap();
            // A sphere keeps its pole, the axis: along plan Y.
            for (f, g) in body.faces().iter().zip(back.faces()) {
                if let (arq_kernel::Surface::Sphere(s), arq_kernel::Surface::Sphere(r)) =
                    (&f.surface, &g.surface)
                {
                    assert!((s.pole() - r.pole()).length() < 1e-12);
                    assert!((s.pole().y().abs() - 1.0).abs() < 1e-12);
                }
            }
            let n = back.mass_properties().unwrap();
            assert!((m.volume - n.volume).abs() < 1e-9 * m.volume, "{angle}");
            assert!((m.area - n.area).abs() < 1e-9 * m.area, "{angle}");
        }
    }
}
