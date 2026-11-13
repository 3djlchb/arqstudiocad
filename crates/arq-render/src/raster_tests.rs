//! Tests of the rasterizer (a submodule of `raster`).
//!
//! Pixel positions are worked out by hand from the camera: screen
//! x = (wx - cx) * scale + width / 2 and y = height / 2 - (wy - cy) * scale.
//! A hairline lights the pixel row its screen y falls in, so lines are put at
//! world positions that land on a pixel's middle (y = 50.5 is row 50).

use arq_core::{CadDocument, Color, Document};
use arq_geometry::{Line, Point2, Vector2};

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

#[test]
fn a_preview_is_inked_on_transparency_and_fitted() {
    use arq_core::DrawItem;
    let item = |a: (f64, f64), b: (f64, f64)| DrawItem {
        shape: Shape::Line(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap()),
        color: Color::Index(1),
    };
    // A square far from the origin: it fills the preview whatever its place.
    let square = [
        item((1000.0, 1000.0), (1010.0, 1000.0)),
        item((1010.0, 1000.0), (1010.0, 1010.0)),
        item((1010.0, 1010.0), (1000.0, 1010.0)),
        item((1000.0, 1010.0), (1000.0, 1000.0)),
    ];
    let ink = [200, 210, 220];
    let image = crate::preview(&square, 40, [33, 40, 48], Some(ink));
    assert_eq!(
        (image.width, image.height, image.pixels.len()),
        (40, 40, 40 * 40 * 4)
    );
    let at = |x: usize, y: usize| {
        let i = (y * 40 + x) * 4;
        <[u8; 4]>::try_from(&image.pixels[i..i + 4]).unwrap()
    };
    assert_eq!(at(20, 20)[3], 0, "transparent inside the square");
    assert_eq!(at(0, 0)[3], 0, "and around it");
    // The square's left side, 5 % in: about two pixels from the edge.
    let left: Vec<[u8; 4]> = (1..4).map(|x| at(x, 20)).collect();
    assert!(
        left.iter().any(|px| px[3] > 0),
        "the left side is drawn: {left:?}"
    );
    for px in image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[3] == 255)
    {
        assert_eq!(&px[..3], ink, "every opaque pixel is the ink");
    }
    assert!(
        crate::preview(&[], 8, [33, 40, 48], Some(ink))
            .pixels
            .iter()
            .all(|&b| b == 0),
        "nothing: empty"
    );
}

#[test]
fn a_preview_keeps_each_shape_color_on_its_background() {
    use arq_core::DrawItem;
    let line = |y: f64, color: Color| DrawItem {
        shape: Shape::Line(Line::new(p(0.0, y), p(10.0, y)).unwrap()),
        color,
    };
    let items = [line(0.0, Color::Index(1)), line(10.0, Color::Index(7))];
    // The colors of the strokes, back from premultiplied alpha.
    let colors = |background: [u8; 3]| -> Vec<[u8; 3]> {
        let image = crate::preview(&items, 40, background, None);
        let (pixels, _) = image.pixels.as_chunks::<4>();
        pixels
            .iter()
            .filter(|px| px[3] >= 96)
            .map(|px| {
                let a = f64::from(px[3]);
                [0, 1, 2].map(|i| (f64::from(px[i]) * 255.0 / a).round().min(255.0) as u8)
            })
            .collect()
    };
    let near = |found: &[[u8; 3]], want: [u8; 3]| {
        found
            .iter()
            .any(|c| (0..3).all(|i| c[i].abs_diff(want[i]) <= 6))
    };
    let dark = colors([33, 40, 48]);
    assert!(near(&dark, [255, 0, 0]), "red, as its layer says");
    assert!(near(&dark, [255, 255, 255]), "ACI 7 white on dark");
    let light = colors([255, 255, 255]);
    assert!(near(&light, [255, 0, 0]));
    assert!(near(&light, [0, 0, 0]), "ACI 7 black on light");
    assert!(!near(&light, [255, 255, 255]));
}

#[test]
fn a_line_entering_at_the_top_edge_is_drawn_where_it_is() {
    // Found against the GPU renderer: tiny-skia, clipping a hairline that
    // starts above its canvas, drew it a pixel lower. The same line drawn
    // ten pixels lower, wholly inside, must give the same pixels shifted.
    let cam = Camera::new(160, 120);
    let rows = |shift: f64| {
        let mut cad = CadDocument::new();
        let (a, b) = ((148.8, -1.19 + shift), (160.17, 3.81 + shift));
        // Screen to world at one pixel per unit: x - 80, 60 - y.
        cad.add_line(p(a.0 - 80.0, 60.0 - a.1), p(b.0 - 80.0, 60.0 - b.1))
            .unwrap();
        let img = render(&scene_of(&cad), &cam, DARK);
        let top = shift as u32;
        (top..top + 3)
            .map(|y| (140..160).map(|x| pixel(&img, x, y)).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let (entering, inside) = (rows(0.0), rows(10.0));
    assert!(
        inside.iter().flatten().any(|&c| c != DARK.background),
        "the line is drawn"
    );
    assert_eq!(entering, inside);
}

/// A 60 × 20 rectangle about the origin, its sides 6 wide (ADR 0038).
fn wide_frame(cad: &mut CadDocument, width: f64) -> arq_core::EntityId {
    let ring = [(-30.0, -10.0), (30.0, -10.0), (30.0, 10.0), (-30.0, 10.0)];
    let vertices: Vec<_> = ring.iter().map(|&(x, y)| (p(x, y), 0.0)).collect();
    let style = arq_core::PolylineStyle {
        width,
        ..Default::default()
    };
    cad.add_polyline_with(&vertices, true, style).unwrap().0
}

#[test]
fn a_wide_polyline_fills_its_band() {
    let mut cad = CadDocument::new();
    wide_frame(&mut cad, 6.0);
    let img = render(&scene_of(&cad), &camera(), DARK);
    // Its bottom side runs on y = -10 (row 60), 6 wide: rows 57 to 62.
    for row in [58, 60, 62] {
        assert_eq!(pixel(&img, 50, row), [255, 255, 255], "row {row} is filled");
    }
    assert!(is_background(&img, 50, 65, DARK) && is_background(&img, 50, 54, DARK));
    assert!(
        is_background(&img, 50, 50, DARK),
        "the frame's middle stays empty"
    );
    // The mitered corner is filled out to (32, -12): column 82, row 62.
    assert_eq!(pixel(&img, 81, 61), [255, 255, 255]);
}

/// Records what a trace sends, in order.
#[derive(Default)]
struct Recorder {
    calls: Vec<char>,
}

impl Sink for Recorder {
    fn style(&mut self, _: [u8; 3], _: bool) {}
    fn segment(&mut self, _: (f32, f32), _: (f32, f32)) {
        self.calls.push('s');
    }
    fn triangle(&mut self, _: (f32, f32), _: (f32, f32), _: (f32, f32)) {
        self.calls.push('t');
    }
}

#[test]
fn fills_come_before_every_stroke() {
    let mut cad = CadDocument::new();
    // A line drawn before the wide polyline still ends on top of its band.
    cad.add_line(p(0.0, -40.0), p(0.0, 40.0)).unwrap();
    wide_frame(&mut cad, 6.0);
    let mut sink = Recorder::default();
    trace(&scene_of(&cad), &camera(), DARK, &mut sink);
    let first_segment = sink.calls.iter().position(|&c| c == 's').unwrap();
    let last_triangle = sink.calls.iter().rposition(|&c| c == 't').unwrap();
    assert!(last_triangle < first_segment, "{:?}", sink.calls);
    // Without width there is nothing to fill.
    let mut cad = CadDocument::new();
    wide_frame(&mut cad, 0.0);
    let mut sink = Recorder::default();
    trace(&scene_of(&cad), &camera(), DARK, &mut sink);
    assert!(!sink.calls.contains(&'t') && sink.calls.contains(&'s'));
}

#[test]
fn a_band_seen_a_million_times_closer_is_still_filled() {
    let mut cad = CadDocument::new();
    wide_frame(&mut cad, 6.0);
    let mut cam = camera();
    // Deep inside the bottom side: every pixel is in the band.
    cam.zoom_at(50.0, 60.0, 1e6);
    let img = render(&scene_of(&cad), &cam, DARK);
    for (x, y) in [(0, 0), (50, 50), (99, 99)] {
        assert_eq!(pixel(&img, x, y), [255, 255, 255], "({x}, {y})");
    }
}

#[test]
fn a_polygon_is_clipped_to_the_rectangle() {
    // A triangle with a corner outside on the right: a quadrilateral left.
    let kept = clip_polygon(
        &[(0.0, 0.0), (20.0, 0.0), (0.0, 10.0)],
        -1.0,
        -1.0,
        10.0,
        11.0,
    );
    assert_eq!(kept.len(), 4, "{kept:?}");
    assert!(
        kept.iter()
            .all(|&(x, y)| (-1.0..=10.0).contains(&x) && (-1.0..=11.0).contains(&y))
    );
    assert!(kept.contains(&(10.0, 0.0)) && kept.contains(&(10.0, 5.0)));
    // Wholly inside: unchanged; wholly outside: nothing.
    let inside = [(1.0, 1.0), (2.0, 1.0), (1.0, 2.0)];
    assert_eq!(clip_polygon(&inside, 0.0, 0.0, 5.0, 5.0), inside.to_vec());
    assert!(clip_polygon(&inside, 10.0, 10.0, 20.0, 20.0).is_empty());
    // A huge one round the rectangle: the rectangle itself.
    let huge = [(-1e9, -1e9), (1e9, -1e9), (0.0, 1e9)];
    let kept = clip_polygon(&huge, 0.0, 0.0, 4.0, 3.0);
    assert_eq!(kept.len(), 4, "{kept:?}");
}
