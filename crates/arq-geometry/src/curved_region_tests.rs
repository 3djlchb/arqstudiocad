//! Tests of curved regions and their booleans (a submodule of
//! `curved_region`).

use std::f64::consts::{PI, TAU};

use proptest::prelude::*;

use super::{CurvedRegion, Trim};
use crate::{Containment, GeometryError, Point2, Region, Tolerance, Vector2};

const TOL: Tolerance = Tolerance::DEFAULT;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

/// A disc of radius `r` round `(cx, cy)`: one counterclockwise circle.
fn disc(cx: f64, cy: f64, r: f64) -> CurvedRegion {
    let circle = Trim::Ellipse {
        center: p(cx, cy),
        a: v(r, 0.0),
        b: v(0.0, r),
        start: 0.0,
        sweep: TAU,
    };
    CurvedRegion::new(vec![vec![circle]], TOL).unwrap()
}

/// The rectangle between two corners, counterclockwise.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> CurvedRegion {
    let square = Region::new(vec![vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]]).unwrap();
    CurvedRegion::from(&square)
}

/// The area two discs of radius `r`, `d` apart, share.
fn lens(r: f64, d: f64) -> f64 {
    2.0 * r * r * (d / (2.0 * r)).acos() - d / 2.0 * (4.0 * r * r - d * d).sqrt()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn contours_must_close() {
    let open = Trim::Ellipse {
        center: p(0.0, 0.0),
        a: v(1.0, 0.0),
        b: v(0.0, 1.0),
        start: 0.0,
        sweep: PI,
    };
    assert_eq!(
        CurvedRegion::new(vec![vec![open]], TOL),
        Err(GeometryError::OpenContour)
    );
    assert_eq!(
        CurvedRegion::new(vec![vec![]], TOL),
        Err(GeometryError::ShortContour)
    );
}

#[test]
fn a_disc_and_a_square_measure_exactly() {
    assert!(close(disc(3.0, -2.0, 1.5).area(), PI * 2.25));
    assert!(close(rect(0.0, 0.0, 2.0, 3.0).area(), 6.0));
    let d = disc(0.0, 0.0, 1.0);
    assert_eq!(d.contains(p(0.2, 0.3), TOL), Containment::Inside);
    assert_eq!(d.contains(p(0.0, 1.0), TOL), Containment::Boundary);
    assert_eq!(d.contains(p(0.8, 0.8), TOL), Containment::Outside);
}

#[test]
fn two_overlapping_discs() {
    let (a, b) = (disc(0.0, 0.0, 1.0), disc(1.2, 0.0, 1.0));
    let shared = lens(1.0, 1.2);
    let both = a.intersection(&b, TOL);
    assert_eq!(both.contours().len(), 1);
    assert_eq!(both.contours()[0].len(), 2, "two arcs");
    assert!(close(both.area(), shared));
    let union = a.union(&b, TOL);
    assert_eq!(union.contours().len(), 1);
    assert!(close(union.area(), 2.0 * PI - shared));
    let rest = a.difference(&b, TOL);
    assert!(close(rest.area(), PI - shared));
}

#[test]
fn a_disc_inside_a_square_makes_a_round_hole() {
    let ring = rect(-2.0, -2.0, 2.0, 2.0).difference(&disc(0.5, 0.0, 1.0), TOL);
    assert!(close(ring.area(), 16.0 - PI));
    let pieces = ring.pieces(TOL);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].contours().len(), 2);
    assert!(
        rect(0.0, 0.0, 1.0, 1.0)
            .difference(&disc(0.5, 0.5, 2.0), TOL)
            .is_empty()
    );
}

#[test]
fn a_bar_across_a_disc_cuts_it_in_two() {
    let parts = disc(0.0, 0.0, 1.0).difference(&rect(-2.0, -0.25, 2.0, 0.25), TOL);
    let pieces = parts.pieces(TOL);
    assert_eq!(pieces.len(), 2);
    // A segment of height h of the unit circle: acos(1 − h) − (1 − h)√(2h − h²).
    let h: f64 = 0.75;
    let cap = (1.0 - h).acos() - (1.0 - h) * (2.0 * h - h * h).sqrt();
    assert!(close(parts.area(), 2.0 * cap));
}

#[test]
fn equal_discs_and_halves() {
    let d = disc(0.0, 0.0, 2.0);
    assert!(close(d.union(&d, TOL).area(), 4.0 * PI));
    assert!(d.difference(&d, TOL).is_empty());
    // Two half discs sharing their diameter unite into one circle.
    let half = |start: f64| {
        let arc = Trim::Ellipse {
            center: p(0.0, 0.0),
            a: v(2.0, 0.0),
            b: v(0.0, 2.0),
            start,
            sweep: PI,
        };
        let (from, to) = (arc.end(), arc.start());
        CurvedRegion::new(vec![vec![arc, Trim::Segment { from, to }]], TOL).unwrap()
    };
    let whole = half(0.0).union(&half(PI), TOL);
    assert_eq!(whole.contours().len(), 1);
    assert_eq!(whole.contours()[0].len(), 1, "one circle, no seam");
    assert!(close(whole.area(), 4.0 * PI));
}

/// The region under `y = num / den` over `x` in `[0, 2π]`, down to `y = 0`.
fn under(num: [f64; 3], den: [f64; 3]) -> CurvedRegion {
    let top = Trim::Graph {
        num,
        den,
        from: TAU,
        to: 0.0,
    };
    let (left, right) = (top.end(), top.start());
    CurvedRegion::new(
        vec![vec![
            Trim::Segment {
                from: p(0.0, 0.0),
                to: p(TAU, 0.0),
            },
            Trim::Segment {
                from: p(TAU, 0.0),
                to: right,
            },
            top,
            Trim::Segment {
                from: left,
                to: p(0.0, 0.0),
            },
        ]],
        TOL,
    )
    .unwrap()
}

#[test]
fn a_sine_band_as_a_plane_leaves_it_on_a_cylinder() {
    // Under y = 2 + sin x: ∫ = 4π.
    let band = under([2.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    assert!(close(band.area(), 4.0 * PI));
    // Cut at y = 2, where the graph crosses at x = π (and meets at 0, 2π).
    let low = band.intersection(&rect(-1.0, -1.0, 7.0, 2.0), TOL);
    assert!(close(low.area(), 4.0 * PI - 2.0));
    let high = band.difference(&rect(-1.0, -1.0, 7.0, 2.0), TOL);
    assert!(close(high.area(), 2.0));
}

#[test]
fn a_rational_graph_as_a_plane_leaves_it_on_a_cone() {
    // Under y = 1 / (2 + cos x): ∫ over a turn = 2π / √3.
    let under_cone = under([1.0, 0.0, 0.0], [2.0, 1.0, 0.0]);
    assert!(close(under_cone.area(), TAU / 3f64.sqrt()));
    let halves = under_cone.difference(&rect(PI, -1.0, 7.0, 2.0), TOL);
    assert!(close(halves.area(), PI / 3f64.sqrt()));
}

#[test]
fn polygons_carry_over() {
    let square = rect(0.0, 0.0, 2.0, 2.0);
    let other = rect(1.0, 1.0, 3.0, 3.0);
    assert!(close(square.union(&other, TOL).area(), 7.0));
    assert!(close(square.intersection(&other, TOL).area(), 1.0));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // Discs and rectangles at random, often overlapping: the areas add up.
    #[test]
    fn the_areas_add_up(
        ax in -1.5..1.5f64, ay in -1.5..1.5f64, ar in 0.3..2.0f64,
        bx in -1.5..1.5f64, by in -1.5..1.5f64, bw in 0.3..3.0f64, bh in 0.3..3.0f64,
        round in any::<bool>(),
    ) {
        let a = disc(ax, ay, ar);
        let b = if round { disc(bx, by, bw / 2.0) } else { rect(bx, by, bx + bw, by + bh) };
        let (union, both, rest) = (a.union(&b, TOL), a.intersection(&b, TOL), a.difference(&b, TOL));
        prop_assert!(close(union.area(), a.area() + b.area() - both.area()));
        prop_assert!(close(rest.area(), a.area() - both.area()));
        for r in [&union, &both, &rest] {
            let total: f64 = r.pieces(TOL).iter().map(CurvedRegion::area).sum();
            prop_assert!(close(total, r.area()));
        }
    }
}

fn circle(cx: f64, cy: f64, r: f64, start: f64, sweep: f64) -> Trim {
    Trim::Ellipse {
        center: p(cx, cy),
        a: v(r, 0.0),
        b: v(0.0, r),
        start,
        sweep,
    }
}

#[test]
fn a_neighbour_s_shared_side_is_kept_by_a_difference() {
    // The squares share a side, run opposite ways: the difference keeps it.
    let rest = rect(0.0, 0.0, 1.0, 1.0).difference(&rect(1.0, 0.0, 2.0, 1.0), TOL);
    assert_eq!(rest.contours().len(), 1);
    assert!(close(rest.area(), 1.0));
    // So does a half disc less the other half.
    let half = |start: f64| {
        let arc = circle(0.0, 0.0, 1.0, start, PI);
        let (from, to) = (arc.end(), arc.start());
        CurvedRegion::new(vec![vec![arc, Trim::Segment { from, to }]], TOL).unwrap()
    };
    assert!(close(half(0.0).difference(&half(PI), TOL).area(), PI / 2.0));
}

#[test]
fn arcs_running_along_each_other_are_not_crossings() {
    // A disc and the half of it above its diameter share an arc.
    let d = disc(0.0, 0.0, 1.0);
    let arc = circle(0.0, 0.0, 1.0, 0.0, PI);
    let (from, to) = (arc.end(), arc.start());
    let top = CurvedRegion::new(vec![vec![arc, Trim::Segment { from, to }]], TOL).unwrap();
    let both = d.intersection(&top, TOL);
    assert_eq!(both.contours().len(), 1);
    assert_eq!(both.contours()[0].len(), 2, "the arc and the diameter");
    assert!(close(both.area(), PI / 2.0));
    let low = d.difference(&top, TOL);
    assert_eq!(low.contours()[0].len(), 2);
    assert!(close(low.area(), PI / 2.0));
}

#[test]
fn pieces_touching_at_points_stay_apart() {
    // A square less the disc inscribed in it: four corners, each touching
    // the next where the disc touches the square.
    let corners = rect(-1.0, -1.0, 1.0, 1.0).difference(&disc(0.0, 0.0, 1.0), TOL);
    let pieces = corners.pieces(TOL);
    assert_eq!(pieces.len(), 4);
    for piece in &pieces {
        assert!(close(piece.area(), (4.0 - PI) / 4.0));
    }
}

#[test]
fn tangent_curves_leaving_a_corner_are_told_apart_by_curvature() {
    // Two circles through the origin, both leaving it along +x: the smaller
    // bends left sooner, so it is the leftmost turn. Arriving by the small
    // one, it closes on itself; arriving by the large one, the small one is
    // taken before closing.
    let small = circle(0.0, 1.0, 1.0, -PI / 2.0, TAU);
    let large = circle(0.0, 2.0, 2.0, -PI / 2.0, TAU);
    let from_small = CurvedRegion::from_trims(&[small, large], TOL);
    assert_eq!(from_small.contours().len(), 2);
    let from_large = CurvedRegion::from_trims(&[large, small], TOL);
    assert_eq!(from_large.contours().len(), 1);
    assert_eq!(from_large.contours()[0].len(), 2);
    // Either way the same area: five discs' worth of the unit one.
    assert!(close(from_small.area(), 5.0 * PI) && close(from_large.area(), 5.0 * PI));
}

#[test]
fn arcs_are_joined_across_a_hair_s_breadth_either_way() {
    // Each arc starts a hair after the one before ends, at both joins: read
    // the other way round the circle, a gap of just under a turn.
    let first = circle(0.0, 0.0, 1.0, 1e-14, PI - 1e-14);
    let second = circle(0.0, 0.0, 1.0, PI + 1e-14, PI - 1e-14);
    let region = CurvedRegion::from_trims(&[first, second], TOL);
    assert_eq!(region.contours().len(), 1);
    assert_eq!(region.contours()[0].len(), 1);
    assert!(close(region.area(), PI));
}

#[test]
fn a_circle_and_its_reverse_cancel() {
    let ccw = circle(0.0, 0.0, 1.0, 0.0, TAU);
    let cw = circle(0.0, 0.0, 1.0, TAU, -TAU);
    assert!(CurvedRegion::from_trims(&[ccw, cw], TOL).is_empty());
    assert_eq!(CurvedRegion::from_trims(&[ccw], TOL).contours().len(), 1);
}

#[test]
fn an_overlap_hidden_between_samples_is_found() {
    // The circle starts off its axes, so no sample sits at its rightmost
    // point; the square reaches 1e-4 into the bulge there.
    let r = 1.0;
    let d = CurvedRegion::new(vec![vec![circle(0.0, 0.0, r, 0.03, TAU)]], TOL).unwrap();
    let h = 1e-4;
    let square = rect(r - h, -1.0, r + 1.0, 1.0);
    let cap = r * r * ((r - h) / r).acos() - (r - h) * (2.0 * r * h - h * h).sqrt();
    let both = d.intersection(&square, TOL);
    assert!(
        (both.area() - cap).abs() <= 1e-9 * cap,
        "{} {cap}",
        both.area()
    );
}

#[test]
fn a_sliver_thinner_than_the_tolerance_is_nothing() {
    // A triangle a unit long and 5e-10 high: its corners lie apart, but it
    // encloses nothing the tolerance can tell.
    let corners = [p(0.0, 0.0), p(1.0, 0.0), p(0.5, 5e-10)];
    let sides: Vec<Trim> = (0..3)
        .map(|k| Trim::Segment {
            from: corners[k],
            to: corners[(k + 1) % 3],
        })
        .collect();
    assert!(CurvedRegion::from_trims(&sides, TOL).is_empty());
    // Twice as high as the tolerance it stays.
    let corners = [p(0.0, 0.0), p(1.0, 0.0), p(0.5, 4e-9)];
    let sides: Vec<Trim> = (0..3)
        .map(|k| Trim::Segment {
            from: corners[k],
            to: corners[(k + 1) % 3],
        })
        .collect();
    assert_eq!(CurvedRegion::from_trims(&sides, TOL).contours().len(), 1);
}

#[test]
fn trims_running_along_each_other_up_to_near_their_end() {
    // A band on a cylinder's window and the section of a box on it share
    // the window's edges for part of their height: near where that shared
    // stretch ends, one side of a point is still on the other trim, the
    // other is not, which is no crossing (a box's corner on a turned
    // column, ADR 0033 delivery 2).
    let seg = |a: (f64, f64), b: (f64, f64)| Trim::Segment {
        from: p(a.0, a.1),
        to: p(b.0, b.1),
    };
    let g = |n: [f64; 3], d: f64, from: f64, to: f64| Trim::Graph {
        num: n,
        den: [d, 0.0, 0.0],
        from,
        to,
    };
    let x0 = 1.5707963267948948;
    let x1 = 7.853981633974481;
    let face = CurvedRegion::from_trims(
        &[
            seg((x1, 0.0), (x1, 2.0936845230923717)),
            seg((x1, 2.0936845230923717), (x0, 2.0936845230923717)),
            seg((x0, 2.0936845230923717), (x0, 0.0)),
            seg((1.570796326794895, 0.0), (x1, 0.0)),
        ],
        TOL,
    );
    let up = [1.8084534125830893, 0.23948569142680448, 0.0880508571760062];
    let low = [0.3624455631677497, 0.23948569142680448, 0.0880508571760062];
    let side = [0.9362696760424247, -0.4334092459614225, 0.4247908597671816];
    let section = CurvedRegion::from_trims(
        &[
            g(up, 0.9108513749041844, 5.095363254311353, x0),
            seg((x0, 2.0821226404347204), (x0, 0.49458828603200555)),
            g(low, 0.9108513749041844, x0, x1),
            seg((x1, 0.49458828603200555), (x1, 2.0821226404347204)),
            g(up, 0.9108513749041844, x1, 6.073275949587764),
            g(
                side,
                0.19071602562375906,
                6.073275949587763,
                5.09536325431138,
            ),
        ],
        TOL,
    );
    let both = face.intersection(&section, TOL);
    assert_eq!(both.contours().len(), 1);
    assert!(both.area() > 9.0 && both.area() < section.area());
    let union = face.union(&section, TOL);
    assert!(close(
        union.area(),
        face.area() + section.area() - both.area()
    ));
    let rest = face.difference(&section, TOL);
    assert!(close(rest.area(), face.area() - both.area()));
}
