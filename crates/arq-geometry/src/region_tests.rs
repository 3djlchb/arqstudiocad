//! Tests of the regions and their booleans (a submodule of `region`).

use proptest::prelude::*;

use super::{Containment, Region};
use crate::{GeometryError, Point2, Tolerance};

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// The rectangle between two corners, counterclockwise.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region {
    Region::new(vec![vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]]).unwrap()
}

/// A square of side `side` centred at `(cx, cy)`, turned by `angle`.
fn turned_square(cx: f64, cy: f64, side: f64, angle: f64) -> Region {
    let (s, c) = angle.sin_cos();
    let h = side / 2.0;
    let corners = [(-h, -h), (h, -h), (h, h), (-h, h)]
        .iter()
        .map(|&(x, y)| p(cx + c * x - s * y, cy + s * x + c * y))
        .collect();
    Region::new(vec![corners]).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn a_contour_needs_three_corners() {
    assert_eq!(
        Region::new(vec![vec![p(0.0, 0.0), p(1.0, 0.0)]]),
        Err(GeometryError::ShortContour)
    );
    assert!(Region::new(Vec::new()).unwrap().is_empty());
}

#[test]
fn the_area_counts_holes_against_the_outside() {
    let ring = Region::new(vec![
        vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        vec![p(1.0, 1.0), p(1.0, 3.0), p(3.0, 3.0), p(3.0, 1.0)],
    ])
    .unwrap();
    assert!(close(ring.area(), 12.0));
    assert_eq!(ring.contains(p(0.5, 0.5), TOL), Containment::Inside);
    assert_eq!(ring.contains(p(2.0, 2.0), TOL), Containment::Outside);
    assert_eq!(ring.contains(p(1.0, 2.0), TOL), Containment::Boundary);
    assert_eq!(ring.contains(p(5.0, 2.0), TOL), Containment::Outside);
}

#[test]
fn two_overlapping_squares() {
    let (a, b) = (rect(0.0, 0.0, 2.0, 2.0), rect(1.0, 1.0, 3.0, 3.0));
    let union = a.union(&b, TOL);
    assert_eq!(union.contours().len(), 1);
    assert_eq!(union.contours()[0].len(), 8);
    assert!(close(union.area(), 7.0));
    let both = a.intersection(&b, TOL);
    assert_eq!(both.contours().len(), 1);
    assert_eq!(both.contours()[0].len(), 4);
    assert!(close(both.area(), 1.0));
    let rest = a.difference(&b, TOL);
    assert_eq!(rest.contours()[0].len(), 6);
    assert!(close(rest.area(), 3.0));
}

#[test]
fn squares_apart_stay_apart() {
    let (a, b) = (rect(0.0, 0.0, 1.0, 1.0), rect(3.0, 0.0, 4.0, 1.0));
    assert_eq!(a.union(&b, TOL).contours().len(), 2);
    assert!(a.intersection(&b, TOL).is_empty());
    assert_eq!(a.difference(&b, TOL), a);
}

#[test]
fn a_square_inside_another_makes_a_hole() {
    let (big, small) = (rect(0.0, 0.0, 4.0, 4.0), rect(1.0, 1.0, 2.0, 2.0));
    let ring = big.difference(&small, TOL);
    assert_eq!(ring.contours().len(), 2);
    assert!(close(ring.area(), 15.0));
    let pieces = ring.pieces(TOL);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].contours().len(), 2);
    assert!(pieces[0].contours()[0].len() == 4 && close(pieces[0].area(), 15.0));
    assert_eq!(big.union(&small, TOL).area(), 16.0);
    assert!(close(big.intersection(&small, TOL).area(), 1.0));
    assert!(small.difference(&big, TOL).is_empty());
}

#[test]
fn squares_sharing_an_edge_unite_without_a_seam() {
    let (a, b) = (rect(0.0, 0.0, 1.0, 1.0), rect(1.0, 0.0, 2.0, 1.0));
    let union = a.union(&b, TOL);
    assert_eq!(union.contours().len(), 1);
    // The corners where the squares met are on a straight run: dropped.
    assert_eq!(union.contours()[0].len(), 4);
    assert!(close(union.area(), 2.0));
    assert!(a.intersection(&b, TOL).is_empty());
    assert_eq!(a.difference(&b, TOL).area(), 1.0);
}

#[test]
fn equal_regions() {
    let a = rect(0.0, 0.0, 1.0, 1.0);
    assert!(close(a.union(&a, TOL).area(), 1.0));
    assert!(close(a.intersection(&a, TOL).area(), 1.0));
    assert!(a.difference(&a, TOL).is_empty());
}

#[test]
fn squares_touching_at_a_corner_stay_two_contours() {
    let (a, b) = (rect(0.0, 0.0, 1.0, 1.0), rect(1.0, 1.0, 2.0, 2.0));
    let union = a.union(&b, TOL);
    assert_eq!(union.contours().len(), 2);
    assert!(union.contours().iter().all(|c| c.len() == 4));
    assert_eq!(union.pieces(TOL).len(), 2);
    assert!(a.intersection(&b, TOL).is_empty());
}

#[test]
fn a_bar_across_a_square_cuts_it_in_two() {
    let (a, bar) = (rect(0.0, 0.0, 3.0, 3.0), rect(-1.0, 1.0, 4.0, 2.0));
    let rest = a.difference(&bar, TOL);
    assert_eq!(rest.pieces(TOL).len(), 2);
    assert!(close(rest.area(), 6.0));
}

#[test]
fn an_island_in_a_hole_is_its_own_piece() {
    let ring = rect(0.0, 0.0, 6.0, 6.0).difference(&rect(1.0, 1.0, 5.0, 5.0), TOL);
    // The island has a hole of its own: inside both outside contours, it
    // belongs to the smaller.
    let island = rect(2.0, 2.0, 4.0, 4.0).difference(&rect(2.5, 2.5, 3.5, 3.5), TOL);
    let all = ring.union(&island, TOL);
    let mut pieces = all.pieces(TOL);
    pieces.sort_by(|a, b| a.area().total_cmp(&b.area()));
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].contours().len(), 2);
    assert!(close(pieces[0].area(), 3.0));
    assert_eq!(pieces[1].contours().len(), 2);
    assert!(close(pieces[1].area(), 20.0));
}

#[test]
fn edges_in_any_order_make_a_region() {
    let corners = [p(0.0, 0.0), p(2.0, 0.0), p(2.0, 1.0), p(0.0, 1.0)];
    let mut edges: Vec<_> = (0..4).map(|k| (corners[k], corners[(k + 1) % 4])).collect();
    edges.swap(0, 2);
    // A zero-length edge and a run back and forth from a corner (a slit)
    // are nothing.
    edges.push((p(0.5, 0.5), p(0.5, 0.5)));
    edges.push((p(0.0, 0.0), p(1.0, 0.5)));
    edges.push((p(1.0, 0.5), p(0.0, 0.0)));
    let region = Region::from_edges(&edges, TOL);
    assert_eq!(region.contours().len(), 1);
    assert!(close(region.area(), 2.0));
}

#[test]
fn far_coordinates_keep_their_precision() {
    // Survey coordinates (ADR 0010): the frame starts at the first corner.
    let (x, y) = (612_345.678, 4_123_456.789);
    let tol = Tolerance::new(1e-6, 1e-10).unwrap();
    let a = rect(x, y, x + 2.0, y + 2.0);
    let b = rect(x + 1.0, y + 1.0, x + 3.0, y + 3.0);
    assert!((a.union(&b, tol).area() - 7.0).abs() < 1e-6);
    assert!((a.intersection(&b, tol).area() - 1.0).abs() < 1e-6);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    // Squares turned at random cross in general position; aligned ones
    // share edges and corners: both obey the area identities.
    #[test]
    fn the_areas_add_up(
        ax in -3.0..3.0f64, ay in -3.0..3.0f64, aside in 0.5..4.0f64, aangle in 0.0..1.6f64,
        bx in -3.0..3.0f64, by in -3.0..3.0f64, bside in 0.5..4.0f64, bangle in 0.0..1.6f64,
        aligned in any::<bool>(),
    ) {
        let (aangle, bangle) = if aligned { (0.0, 0.0) } else { (aangle, bangle) };
        let snap = |v: f64| if aligned { (v * 2.0).round() / 2.0 } else { v };
        let a = turned_square(snap(ax), snap(ay), snap(aside), aangle);
        let b = turned_square(snap(bx), snap(by), snap(bside), bangle);
        let (union, both, rest) = (a.union(&b, TOL), a.intersection(&b, TOL), a.difference(&b, TOL));
        prop_assert!(close(union.area(), a.area() + b.area() - both.area()));
        prop_assert!(close(rest.area(), a.area() - both.area()));
        prop_assert!(both.area() >= -1e-12 && both.area() <= a.area().min(b.area()) + 1e-9);
        // Every result's pieces carry its whole area.
        for r in [&union, &both, &rest] {
            let total: f64 = r.pieces(TOL).iter().map(Region::area).sum();
            prop_assert!(close(total, r.area()));
        }
    }
}
