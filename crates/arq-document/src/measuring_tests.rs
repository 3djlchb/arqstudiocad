//! Tests of measuring a drawing's solids (a submodule of `measuring`):
//! AcadRust's ACIS primitives, alone, together, and inside block
//! references placed, turned, scaled, mirrored and arrayed.

use std::f64::consts::{FRAC_PI_2, PI};

use acadrust::entities::Solid3D;
use acadrust::entities::acis::primitives;
use acadrust::entities::acis::types::SatDocument;
use acadrust::{CadDocument, EntityType};
use arq_geometry::{Geometry2, Line, Point2, Point3};

use super::{SolidMeasures, Unmeasured};
use crate::document::Document;
use crate::entity::{EntityId, PlanGeometry};
use crate::error::DocumentError;
use crate::solid::Untranslated;
use crate::translate::handle;

fn solid(sat: &SatDocument) -> EntityType {
    EntityType::Solid3D(Solid3D::from_sat(&sat.to_sat_string()))
}

/// A drawing holding `entities`, and their ids.
fn drawing(entities: Vec<EntityType>) -> (Document, Vec<EntityId>) {
    let mut cad = CadDocument::new();
    let ids = entities
        .into_iter()
        .map(|e| cad.add_entity(e).unwrap().value())
        .collect();
    (Document::from_engine(cad), ids)
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * (1.0 + b.abs()), "{a} != {b}");
}

#[track_caller]
fn at(p: Point3, x: f64, y: f64, z: f64) {
    close(p.x(), x);
    close(p.y(), y);
    close(p.z(), z);
}

fn measured(doc: &Document, ids: &[EntityId]) -> SolidMeasures {
    doc.measure_solids(ids).unwrap()
}

#[test]
fn one_solid() {
    // A 2 × 2 × 2 box centered on (1, 0, 0).
    let (doc, ids) = drawing(vec![solid(&primitives::build_box(
        [1.0, 0.0, 0.0],
        2.0,
        2.0,
        2.0,
    ))]);
    let m = measured(&doc, &ids);
    assert_eq!((m.solids, m.refused.clone()), (1, vec![]));
    let total = m.total.unwrap();
    close(total.volume, 8.0);
    close(total.area, 24.0);
    at(total.centroid, 1.0, 0.0, 0.0);
    at(total.bounds.min(), 0.0, -1.0, -1.0);
    at(total.bounds.max(), 2.0, 1.0, 1.0);
}

#[test]
fn several_solids_together() {
    let (doc, ids) = drawing(vec![
        solid(&primitives::build_box([1.0, 0.0, 0.0], 2.0, 2.0, 2.0)),
        solid(&primitives::build_sphere([10.0, 0.0, 0.0], 1.0)),
    ]);
    let m = measured(&doc, &[ids[0], ids[1], ids[0]]);
    assert_eq!(m.solids, 2, "each id counts once");
    let total = m.total.unwrap();
    let ball = 4.0 / 3.0 * PI;
    close(total.volume, 8.0 + ball);
    close(total.area, 24.0 + 4.0 * PI);
    // The centroid weighted by volume.
    at(
        total.centroid,
        (8.0 * 1.0 + ball * 10.0) / (8.0 + ball),
        0.0,
        0.0,
    );
    at(total.bounds.min(), 0.0, -1.0, -1.0);
    at(total.bounds.max(), 11.0, 1.0, 1.0);
}

#[test]
fn what_is_not_a_solid_is_refused() {
    let mut cad = CadDocument::new();
    let ball = cad
        .add_entity(solid(&primitives::build_sphere([0.0, 0.0, 0.0], 1.0)))
        .unwrap()
        .value();
    let broken = cad
        .add_entity(EntityType::Solid3D(Solid3D::from_sat("not ACIS")))
        .unwrap()
        .value();
    let mut doc = Document::from_engine(cad);
    let segment = Line::new(
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(1.0, 0.0).unwrap(),
    )
    .unwrap();
    let layer = doc.current_layer().id();
    let line = doc
        .add_entity(PlanGeometry::new(Geometry2::Line(segment)), layer)
        .unwrap();
    let m = measured(&doc, &[line, broken, ball]);
    assert_eq!(m.solids, 1);
    assert_eq!(
        m.refused,
        vec![
            (line, Unmeasured::NotASolid),
            (broken, Unmeasured::Untranslated(Untranslated::Unreadable)),
        ]
    );
    let none = measured(&doc, &[line]);
    assert!(none.total.is_none() && none.solids == 0);
    assert!(matches!(
        doc.measure_solids(&[999_999]),
        Err(DocumentError::EntityNotFound(999_999))
    ));
}

/// A drawing with block PIEZA, a 2 × 2 × 2 box centered on its base point
/// (1, 0), and one reference to it; its id.
fn with_block() -> (Document, EntityId) {
    let (mut doc, ids) = drawing(vec![solid(&primitives::build_box(
        [1.0, 0.0, 0.0],
        2.0,
        2.0,
        2.0,
    ))]);
    let first = doc
        .create_block("PIEZA", Point2::new(1.0, 0.0).unwrap(), &ids)
        .unwrap();
    // The reference left where the box was.
    let m = measured(&doc, &[first]);
    close(m.total.unwrap().volume, 8.0);
    let placed = doc
        .insert_block("PIEZA", Point2::new(10.0, 5.0).unwrap(), 2.0, FRAC_PI_2)
        .unwrap();
    (doc, placed)
}

#[test]
fn a_block_reference_measures_what_it_holds_where_it_puts_it() {
    let (doc, placed) = with_block();
    let m = measured(&doc, &[placed]);
    assert_eq!((m.solids, m.refused.clone()), (1, vec![]));
    let total = m.total.unwrap();
    // Scaled by 2 and turned a quarter: 4 × 4 × 4 around the insertion point.
    close(total.volume, 64.0);
    close(total.area, 96.0);
    at(total.centroid, 10.0, 5.0, 0.0);
    at(total.bounds.min(), 8.0, 3.0, -2.0);
    at(total.bounds.max(), 12.0, 7.0, 2.0);
}

/// The block reference `id`, changed by `change`.
fn change_insert(
    doc: &mut Document,
    id: EntityId,
    change: impl FnOnce(&mut acadrust::entities::Insert),
) {
    let Some(EntityType::Insert(ins)) = doc.cad_mut().get_entity_mut(handle(id)) else {
        panic!("a block reference");
    };
    change(ins);
}

#[test]
fn a_mirrored_reference_measures_its_mirror_image() {
    // A box off its base point, mirrored across the reference's y axis.
    let (mut doc, ids) = drawing(vec![solid(&primitives::build_box(
        [3.0, 0.0, 0.0],
        2.0,
        2.0,
        2.0,
    ))]);
    doc.create_block("PIEZA", Point2::new(0.0, 0.0).unwrap(), &ids)
        .unwrap();
    let placed = doc
        .insert_block("PIEZA", Point2::new(0.0, 0.0).unwrap(), 1.0, 0.0)
        .unwrap();
    change_insert(&mut doc, placed, |ins| ins.set_x_scale(-1.0));
    let m = measured(&doc, &[placed]);
    assert_eq!(m.refused, vec![]);
    let total = m.total.unwrap();
    close(total.volume, 8.0);
    close(total.area, 24.0);
    at(total.centroid, -3.0, 0.0, 0.0);
    at(total.bounds.min(), -4.0, -1.0, -1.0);
}

#[test]
fn an_unevenly_scaled_reference_is_refused() {
    let (mut doc, placed) = with_block();
    change_insert(&mut doc, placed, |ins| ins.set_y_scale(3.0));
    let m = measured(&doc, &[placed]);
    assert_eq!(
        (m.solids, m.refused),
        (0, vec![(placed, Unmeasured::UnevenScale)])
    );
    assert!(m.total.is_none());
}

#[test]
fn an_array_counts_every_copy() {
    let (mut doc, placed) = with_block();
    change_insert(&mut doc, placed, |ins| {
        ins.row_count = 2;
        ins.column_count = 3;
        ins.row_spacing = 10.0;
        ins.column_spacing = 20.0;
    });
    let m = measured(&doc, &[placed]);
    assert_eq!(m.solids, 6);
    close(m.total.unwrap().volume, 6.0 * 64.0);
}

#[test]
fn a_block_inside_a_block() {
    let (mut doc, placed) = with_block();
    let outer = doc
        .create_block("CONJUNTO", Point2::new(10.0, 5.0).unwrap(), &[placed])
        .unwrap();
    let twice = doc
        .insert_block("CONJUNTO", Point2::new(0.0, 0.0).unwrap(), 0.5, 0.0)
        .unwrap();
    let m = measured(&doc, &[outer, twice]);
    assert_eq!(m.solids, 2);
    // Full size where it was, half size (4 × 4 × 4 halved) at the origin.
    let total = m.total.unwrap();
    close(total.volume, 64.0 + 8.0);
    at(total.bounds.min(), -1.0, -1.0, -2.0);
    at(total.bounds.max(), 12.0, 7.0, 2.0);
}

#[test]
fn a_skewed_nesting_is_refused() {
    // A block turned 45° inside one stretched twice along x: its axes end
    // up equally long but no longer square.
    let (mut doc, ids) = drawing(vec![solid(&primitives::build_box(
        [0.0, 0.0, 0.0],
        2.0,
        2.0,
        2.0,
    ))]);
    doc.create_block("PIEZA", Point2::new(0.0, 0.0).unwrap(), &ids)
        .unwrap();
    let turned = doc
        .insert_block("PIEZA", Point2::new(0.0, 0.0).unwrap(), 1.0, PI / 4.0)
        .unwrap();
    doc.create_block("GRUPO", Point2::new(0.0, 0.0).unwrap(), &[turned])
        .unwrap();
    let outer = doc
        .insert_block("GRUPO", Point2::new(0.0, 0.0).unwrap(), 1.0, 0.0)
        .unwrap();
    // The z axis as long as the others: only their angle gives it away.
    change_insert(&mut doc, outer, |ins| {
        ins.set_x_scale(2.0);
        ins.set_z_scale(2.5f64.sqrt());
    });
    let m = measured(&doc, &[outer]);
    assert_eq!(
        (m.solids, m.refused),
        (0, vec![(outer, Unmeasured::UnevenScale)])
    );
}

#[test]
fn a_curved_solid_scales_with_its_reference() {
    // A ball of radius 1 on the base point, its reference scaled by 3.
    let (mut doc, ids) = drawing(vec![solid(&primitives::build_sphere([0.0, 0.0, 0.0], 1.0))]);
    doc.create_block("BOLA", Point2::new(0.0, 0.0).unwrap(), &ids)
        .unwrap();
    let placed = doc
        .insert_block("BOLA", Point2::new(5.0, 0.0).unwrap(), 3.0, 0.0)
        .unwrap();
    let total = measured(&doc, &[placed]).total.unwrap();
    close(total.volume, 4.0 / 3.0 * PI * 27.0);
    close(total.area, 4.0 * PI * 9.0);
    at(total.bounds.max(), 8.0, 3.0, 3.0);
}
