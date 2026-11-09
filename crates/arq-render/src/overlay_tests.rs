//! Tests of the overlay's triangles (a submodule of `overlay`), on the CPU.

use super::*;

const BLUE: [u8; 4] = [30, 144, 255, 255];

fn stroke(points: &[[f32; 2]], width: f32, dashed: bool, crisp: bool) -> OverlayItem {
    OverlayItem::Stroke {
        points: points.to_vec(),
        rgba: BLUE,
        width,
        dashed,
        closed: false,
        crisp,
    }
}

fn overlay(items: Vec<OverlayItem>, ratio: f32) -> Overlay {
    Overlay {
        items,
        labels: Vec::new(),
        ratio,
    }
}

fn bounds(vs: &[OverlayVertex]) -> ([f32; 2], [f32; 2]) {
    let xs = vs.iter().map(|v| v.at[0]);
    let ys = vs.iter().map(|v| v.at[1]);
    (
        [
            xs.clone().fold(f32::MAX, f32::min),
            ys.clone().fold(f32::MAX, f32::min),
        ],
        [xs.fold(f32::MIN, f32::max), ys.fold(f32::MIN, f32::max)],
    )
}

#[test]
fn a_fill_is_two_triangles_never_faded() {
    let o = overlay(
        vec![OverlayItem::Fill {
            min: [10.0, 20.0],
            max: [30.0, 25.0],
            rgba: [30, 144, 255, 40],
        }],
        1.0,
    );
    let t = triangles(&o);
    assert_eq!(t.len(), 6);
    assert_eq!(bounds(&t), ([10.0, 20.0], [30.0, 25.0]));
    assert!(t.iter().all(|v| v.half < 0.0 && v.rgba[3] == 40));
}

#[test]
fn a_smooth_stroke_reaches_half_a_pixel_beyond_its_width() {
    // Horizontal, 2 wide: one pixel each side, plus half a pixel to fade.
    let t = triangles(&overlay(
        vec![stroke(&[[10.0, 50.0], [90.0, 50.0]], 2.0, false, false)],
        1.0,
    ));
    assert_eq!(t.len(), 6);
    assert_eq!(bounds(&t), ([10.0, 48.5], [90.0, 51.5]));
    assert!(t.iter().all(|v| v.half == 1.0 && v.across.abs() == 1.5));
}

#[test]
fn a_crisp_stroke_covers_whole_pixels() {
    // From pixel (10, 50) to pixel (19, 50): exactly that row, ends included.
    let t = triangles(&overlay(
        vec![stroke(&[[10.2, 50.7], [19.9, 50.1]], 1.0, false, true)],
        1.0,
    ));
    assert_eq!(bounds(&t), ([10.0, 50.0], [20.0, 51.0]));
    assert!(t.iter().all(|v| v.half < 0.0), "never faded");
}

#[test]
fn the_device_pixel_ratio_scales_places_and_widths() {
    let t = triangles(&overlay(
        vec![stroke(&[[10.0, 50.0], [90.0, 50.0]], 2.0, false, false)],
        2.0,
    ));
    assert_eq!(bounds(&t), ([20.0, 97.5], [180.0, 102.5]));
}

#[test]
fn dashes_run_on_across_corners() {
    // Width 1: four on, two off. An L of 5 then 5: drawn 0-4, skipped 4-6
    // (the corner at 5), drawn 6-10.
    let pieces = dashes(&[[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]], 4.0, 2.0);
    assert_eq!(
        pieces,
        vec![vec![[0.0, 0.0], [4.0, 0.0]], vec![[5.0, 1.0], [5.0, 5.0]]]
    );
    // Long enough for the pattern to repeat on one segment.
    let pieces = dashes(&[[0.0, 0.0], [13.0, 0.0]], 4.0, 2.0);
    assert_eq!(
        pieces,
        vec![
            vec![[0.0, 0.0], [4.0, 0.0]],
            vec![[6.0, 0.0], [10.0, 0.0]],
            vec![[12.0, 0.0], [13.0, 0.0]],
        ]
    );
}

#[test]
fn a_closed_stroke_comes_back_to_its_start() {
    let square = OverlayItem::Stroke {
        points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        rgba: BLUE,
        width: 1.0,
        dashed: false,
        closed: true,
        crisp: false,
    };
    assert_eq!(
        triangles(&overlay(vec![square], 1.0)).len(),
        4 * 6,
        "four sides"
    );
}

#[test]
fn nothing_degenerate_is_drawn() {
    let t = triangles(&overlay(
        vec![
            stroke(&[[5.0, 5.0], [5.0, 5.0]], 2.0, false, false),
            stroke(&[[1.0, 1.0]], 2.0, false, false),
        ],
        1.0,
    ));
    assert!(t.is_empty());
}
