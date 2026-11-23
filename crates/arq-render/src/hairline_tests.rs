//! `Hairlines` paints the very pixels tiny-skia's hairline stroke paints.

use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};

use super::Hairlines;

const WIDTH: u32 = 700;
const HEIGHT: u32 = 600;

/// A small deterministic generator: the same cases on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    /// In `lo..hi`, at any fraction of a pixel.
    fn coord(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() as f32 / (1u32 << 31) as f32)
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 8) as u8
    }
}

type Segment = ((f32, f32), (f32, f32));

/// Both painters over the same `background`, one color per batch.
fn compare(background: tiny_skia::Color, batches: &[([u8; 3], Vec<Segment>)]) {
    let mut expected = Pixmap::new(WIDTH, HEIGHT).expect("size");
    expected.fill(background);
    let mut actual = expected.clone();
    for (rgb, segments) in batches {
        let mut path = PathBuilder::new();
        for &(a, b) in segments {
            path.move_to(a.0, a.1);
            path.line_to(b.0, b.1);
        }
        let mut paint = Paint::default();
        paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: 0.0,
            ..Stroke::default()
        };
        if let Some(path) = path.finish() {
            expected.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        let mut lines = Hairlines::new(actual.data_mut(), WIDTH, HEIGHT, *rgb);
        for &(a, b) in segments {
            lines.line(a, b);
        }
    }
    let different = expected
        .data()
        .chunks(4)
        .zip(actual.data().chunks(4))
        .enumerate()
        .filter(|(_, (e, a))| e != a)
        .map(|(i, (e, a))| (i as u32 % WIDTH, i as u32 / WIDTH, e.to_vec(), a.to_vec()))
        .take(5)
        .collect::<Vec<_>>();
    assert!(
        different.is_empty(),
        "(x, y, tiny-skia, ours): {different:?}"
    );
    // And something was drawn: a comparison of two blank canvases tests nothing.
    let mut blank = Pixmap::new(WIDTH, HEIGHT).expect("size");
    blank.fill(background);
    assert_ne!(expected.data(), blank.data());
}

fn dark() -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(33, 40, 48, 255)
}

#[test]
fn random_segments_match_tiny_skia_bit_for_bit() {
    let mut rng = Lcg(7);
    let batches = (0..40)
        .map(|_| {
            let rgb = [rng.byte(), rng.byte(), rng.byte()];
            let segments = (0..200)
                .map(|_| {
                    let a = (rng.coord(4.0, 696.0), rng.coord(4.0, 596.0));
                    // Mostly short, as a flattened arc's chords; some long.
                    let reach = if rng.next().is_multiple_of(8) {
                        600.0
                    } else {
                        6.0
                    };
                    let b = (
                        (a.0 + rng.coord(-reach, reach)).clamp(4.0, 696.0),
                        (a.1 + rng.coord(-reach, reach)).clamp(4.0, 596.0),
                    );
                    (a, b)
                })
                .collect();
            (rgb, segments)
        })
        .collect::<Vec<_>>();
    compare(dark(), &batches);
    compare(tiny_skia::Color::WHITE, &batches);
    compare(tiny_skia::Color::TRANSPARENT, &batches);
}

#[test]
fn level_plumb_tiny_and_long_lines_match() {
    let mut segments: Vec<Segment> = vec![
        ((10.0, 10.0), (400.0, 10.0)),     // level, on a pixel edge
        ((10.0, 20.5), (400.0, 20.5)),     // level, through pixel centers
        ((10.3, 30.7), (11.1, 30.7)),      // level, within a pixel or so
        ((50.0, 40.0), (50.0, 500.0)),     // plumb
        ((60.5, 40.0), (60.5, 41.0)),      // plumb, one pixel
        ((70.2, 40.2), (70.4, 40.3)),      // a speck
        ((80.0, 80.0), (80.0, 80.0)),      // nothing at all
        ((5.0, 5.0), (690.0, 590.0)),      // long enough to be halved
        ((690.0, 8.0), (6.0, 580.0)),      // long, the other way
        ((100.0, 100.0), (101.0, 300.0)),  // nearly plumb
        ((100.0, 300.0), (600.0, 301.0)),  // nearly level
        ((200.25, 200.0), (250.0, 250.0)), // a diagonal
    ];
    // Crossing, so later lines blend over earlier ones.
    segments.extend((0..20).map(|i| {
        let t = i as f32 * 0.37;
        ((300.0 + t, 120.0), (340.0 - t, 160.0 + t))
    }));
    compare(dark(), &[([255, 255, 255], segments.clone())]);
    compare(dark(), &[([0, 200, 80], segments)]);
}
