//! Tests of the rasterizer (a submodule of `raster`).
//!
//! Pixel positions are worked out by hand from the camera: screen
//! x = (wx - cx) * scale + width / 2 and y = height / 2 - (wy - cy) * scale.
//! A hairline lights the pixel row its screen y falls in, so lines are put at
//! world positions that land on a pixel's middle (y = 50.5 is row 50).

use arq_core::{CadDocument, Color, Document};
use arq_geometry::{Point2, Vector2};

use super::*;
use crate::scene::Scene;

const DARK: Style = Style {
    background: [33, 40, 48],
};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn pixel(img: &Image, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
}

fn is_background(img: &Image, x: u32, y: u32, style: Style) -> bool {
    pixel(img, x, y) == style.background
}

/// A 100 x 100 view of the origin at one pixel per unit.
fn camera() -> Camera {
    Camera::new(100, 100)
}

fn scene_of(cad: &CadDocument) -> Scene {
    Scene::build(cad.document())
}

#[test]
fn a_line_lands_on_its_pixels() {
    let mut cad = CadDocument::new();
    // World y = -0.5 -> screen y = 50 + 0.5 = 50.5: row 50. x from -30 to 30
    // -> columns 20 to 80.
    cad.add_line(p(-30.0, -0.5), p(30.0, -0.5)).unwrap();
    let img = render(&scene_of(&cad), &camera(), DARK);
    assert_eq!(
        (img.width, img.height, img.pixels.len()),
        (100, 100, 40_000)
    );
    assert_eq!(
        pixel(&img, 50, 50),
        [255, 255, 255],
        "ACI 7 is white on dark"
    );
    assert_eq!(pixel(&img, 25, 50), [255, 255, 255]);
    assert!(is_background(&img, 50, 45, DARK), "rows away stay empty");
    assert!(is_background(&img, 10, 50, DARK), "beyond its end");
    assert!(is_background(&img, 90, 50, DARK), "beyond its other end");
}

#[test]
fn colors_and_a_light_background() {
    let mut cad = CadDocument::new();
    let (red, _) = cad.add_line(p(-30.0, 10.5), p(30.0, 10.5)).unwrap(); // row 39
    cad.set_entity_color(&[red], Color::Index(1)).unwrap();
    let (rgb, _) = cad.add_line(p(-30.0, -10.5), p(30.0, -10.5)).unwrap(); // row 60
    cad.set_entity_color(
        &[rgb],
        Color::Rgb {
            r: 10,
            g: 200,
            b: 30,
        },
    )
    .unwrap();
    cad.add_line(p(-30.0, -20.5), p(30.0, -20.5)).unwrap(); // row 70, ACI 7
    let light = Style {
        background: [250, 250, 250],
    };
    let img = render(&scene_of(&cad), &camera(), light);
    assert_eq!(pixel(&img, 50, 39), [255, 0, 0]);
    assert_eq!(pixel(&img, 50, 60), [10, 200, 30]);
    assert_eq!(pixel(&img, 50, 70), [0, 0, 0], "ACI 7 is black on light");
}

#[test]
fn circles_and_arcs() {
    let mut cad = CadDocument::new();
    // Circle of radius 20.5 around (0,0): its top is screen y = 50 - 20.5
    // = 29.5 (row 29) at column 50; its right side x = 70.5 (column 70).
    cad.add_circle(p(0.0, 0.0), 20.5).unwrap();
    let img = render(&scene_of(&cad), &camera(), DARK);
    assert_ne!(pixel(&img, 50, 29), DARK.background);
    assert_ne!(pixel(&img, 70, 50), DARK.background);
    assert!(is_background(&img, 50, 50, DARK), "hollow");

    // Upper half only: the bottom (row 70) stays empty.
    let mut cad = CadDocument::new();
    cad.add_arc(p(0.0, 0.0), 20.5, 0.0, std::f64::consts::PI)
        .unwrap();
    let img = render(&scene_of(&cad), &camera(), DARK);
    assert_ne!(pixel(&img, 50, 29), DARK.background);
    assert!(is_background(&img, 50, 70, DARK));
}

#[test]
fn survey_coordinates_keep_their_precision() {
    let mut cad = CadDocument::new();
    let (x0, y0) = (500_000.0, 9_977_000.0);
    cad.add_line(p(x0 - 30.0, y0 - 0.5), p(x0 + 30.0, y0 - 0.5))
        .unwrap();
    let mut cam = camera();
    cam.fit(arq_geometry::Bounds2::new(
        p(x0 - 45.0, y0 - 45.0),
        p(x0 + 45.0, y0 + 45.0),
    ));
    assert_eq!(cam.scale(), 1.0);
    let img = render(&scene_of(&cad), &cam, DARK);
    assert_eq!(
        pixel(&img, 50, 50),
        [255, 255, 255],
        "same row as at the origin"
    );
    assert!(is_background(&img, 50, 49, DARK) && is_background(&img, 50, 51, DARK));
}

#[test]
fn far_and_huge_geometry() {
    let mut cad = CadDocument::new();
    // Far outside the view: nothing drawn.
    cad.add_circle(p(10_000.0, 10_000.0), 1.0).unwrap();
    // Zoomed in a million times on a line crossing the view: clipped, drawn.
    cad.add_line(p(-1000.0, 0.0000005), p(1000.0, 0.0000005))
        .unwrap();
    let mut cam = camera();
    cam.zoom_at(50.0, 50.0, 1e6);
    let img = render(&scene_of(&cad), &cam, DARK);
    // y = 0.0000005 -> 50 - 0.5 = 49.5: row 49.
    assert_eq!(pixel(&img, 0, 49), [255, 255, 255]);
    assert_eq!(pixel(&img, 99, 49), [255, 255, 255]);
    let drawn = (0..100 * 100)
        .filter(|&i| !is_background(&img, i % 100, i / 100, DARK))
        .count();
    assert_eq!(drawn, 100, "only that row");
}

#[test]
fn the_scene_follows_changes() {
    let mut cad = CadDocument::new();
    let mut scene = Scene::build(cad.document());
    assert_eq!(scene.shape_count(), 0);
    let (id, made) = cad.add_line(p(-30.0, -0.5), p(30.0, -0.5)).unwrap();
    assert!(scene.update(cad.document(), &made));
    assert_eq!(scene.shape_count(), 1);

    let moved = cad
        .move_entities(&[id], Vector2::new(0.0, 10.0).unwrap())
        .unwrap();
    scene.update(cad.document(), &moved);
    let img = render(&scene, &camera(), DARK);
    assert!(is_background(&img, 50, 50, DARK), "left its old row");
    assert_eq!(pixel(&img, 50, 40), [255, 255, 255], "now on row 40");

    let zero = cad.document().current_layer().id();
    let recolored = cad.set_layer_color(zero, Color::Index(5)).unwrap();
    scene.update(cad.document(), &recolored);
    let img = render(&scene, &camera(), DARK);
    assert_eq!(
        pixel(&img, 50, 40),
        [0, 0, 255],
        "ByLayer follows the layer"
    );

    let hidden = cad.set_layer_visible(zero, false).unwrap();
    scene.update(cad.document(), &hidden);
    assert_eq!(scene.shape_count(), 0, "off layer");

    let shown = cad.set_layer_visible(zero, true).unwrap();
    scene.update(cad.document(), &shown);
    let gone = cad.delete_entities(&[id]).unwrap();
    scene.update(cad.document(), &gone);
    assert_eq!(scene.shape_count(), 0);
    assert!(scene.bounds().is_none());
    let nothing = cad.delete_entities(&[]).unwrap();
    assert!(!scene.update(cad.document(), &nothing), "no change");
}

#[test]
fn texts_are_drawn_as_their_letters() {
    let mut doc = Document::new();
    doc.insert_text_for_tests("NOTA", 0.0, 0.0);
    let cad = CadDocument::from_document(doc);
    let scene = scene_of(&cad);
    let bounds = scene.bounds().expect("the text has a box");
    let mut cam = camera();
    cam.fit(bounds);
    let img = render(&scene, &cam, DARK);
    let drawn = (0..100 * 100)
        .filter(|&i| !is_background(&img, i % 100, i / 100, DARK))
        .count();
    assert!(drawn > 50, "its letters are drawn ({drawn} pixels)");
    // "NOTA" in the built-in font: dozens of strokes, plus its picking box.
    assert!(scene.shape_count() > 20, "{} shapes", scene.shape_count());
}

#[test]
fn clipping() {
    // Inside, crossing, outside, degenerate, non-finite.
    let r = |a, b| clip(a, b, 0.0, 0.0, 10.0, 10.0);
    assert_eq!(r((1.0, 1.0), (2.0, 2.0)), Some(((1.0, 1.0), (2.0, 2.0))));
    assert_eq!(r((-5.0, 5.0), (15.0, 5.0)), Some(((0.0, 5.0), (10.0, 5.0))));
    assert_eq!(r((-5.0, -5.0), (-1.0, 20.0)), None);
    assert_eq!(r((3.0, 3.0), (3.0, 3.0)), Some(((3.0, 3.0), (3.0, 3.0))));
    assert_eq!(r((f64::NAN, 0.0), (1.0, 1.0)), None);
}

#[test]
fn a_diagonal_stays_exact_when_zoomed_in_a_million_times() {
    // y = x + 0.0000005 at 1e6 pixels per unit: screen y = 49.5 - (sx - 50),
    // so it lights (50, 49), (60, 39), (40, 59). Its ends are ±1e9 pixels
    // away, where f32 cannot tell pixels apart: only clipping in f64 before
    // converting keeps it on those pixels.
    let mut cad = CadDocument::new();
    cad.add_line(
        p(-1000.0, -1000.0 + 0.0000005),
        p(1000.0, 1000.0 + 0.0000005),
    )
    .unwrap();
    let mut cam = camera();
    cam.zoom_at(50.0, 50.0, 1e6);
    let img = render(&scene_of(&cad), &cam, DARK);
    for (x, y) in [(50, 49), (60, 39), (40, 59)] {
        assert_ne!(pixel(&img, x, y), DARK.background, "({x}, {y})");
    }
}
