//! The CPU rasterizer: a scene seen through a camera, painted into an RGBA
//! image with tiny-skia (ADR 0013, D5.3). It needs no GPU and no Qt, and is
//! the guaranteed fallback of every renderer to come.
//!
//! Shapes outside the view are skipped by their boxes; segments are clipped
//! to the view in `f64` before becoming `f32` pixels, so far-away geometry
//! never reaches the painter with huge coordinates. Consecutive shapes of
//! one color are stroked as one path.

use arq_core::{Color, Shape};
use arq_geometry::Tolerance;
use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

use crate::aci::aci_rgb_on;
use crate::camera::Camera;
use crate::scene::{Scene, shape_bounds};

/// How far a flattened arc may stray from the true curve, in pixels.
const ARC_TOLERANCE_PX: f64 = 0.25;
/// Half the size of a point's cross, in pixels.
const POINT_PX: f64 = 2.0;

/// How the view looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Background color.
    pub background: [u8; 3],
}

impl Default for Style {
    /// AutoCAD's dark model-space background.
    fn default() -> Self {
        Self {
            background: [33, 40, 48],
        }
    }
}

/// A rendered view: `width` × `height` pixels, RGBA, premultiplied alpha
/// (Qt's `Format_RGBA8888_Premultiplied`), rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

/// Paints `scene` as `camera` sees it.
pub fn render(scene: &Scene, camera: &Camera, style: Style) -> Image {
    let (width, height) = camera.size();
    let mut pixmap = Pixmap::new(width, height).expect("a camera is at least 1x1");
    let [r, g, b] = style.background;
    pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));

    if let Some(view) = camera.visible() {
        // A little beyond the edges, so strokes touching them are drawn.
        let margin = 2.0 / camera.scale();
        let view = view.expanded(margin).unwrap_or(view);
        let tol = Tolerance::default();
        let mut batch = Batch::new(style);
        for entry in scene.entries() {
            if !entry.bounds.is_some_and(|b| b.intersects(view, tol)) {
                continue;
            }
            for item in &entry.items {
                if !shape_bounds(item.shape).intersects(view, tol) {
                    continue;
                }
                batch.set_color(
                    &mut pixmap,
                    item.color,
                    matches!(item.shape, Shape::Placeholder(_)),
                );
                batch.add(camera, item.shape);
            }
        }
        batch.flush(&mut pixmap);
    }
    Image {
        width,
        height,
        pixels: pixmap.take(),
    }
}

/// Shapes of one color and style, collected into one path.
struct Batch {
    style: Style,
    rgb: Option<[u8; 3]>,
    dashed: bool,
    path: PathBuilder,
    width: f64,
    height: f64,
}

impl Batch {
    fn new(style: Style) -> Self {
        Self {
            style,
            rgb: None,
            dashed: false,
            path: PathBuilder::new(),
            width: 0.0,
            height: 0.0,
        }
    }

    /// Starts a new path when the color or the dash changes.
    fn set_color(&mut self, pixmap: &mut Pixmap, color: Color, dashed: bool) {
        let rgb = match color {
            Color::Rgb { r, g, b } => [r, g, b],
            Color::Index(i) => aci_rgb_on(i, self.style.background),
            _ => aci_rgb_on(7, self.style.background),
        };
        if self.rgb != Some(rgb) || self.dashed != dashed {
            self.flush(pixmap);
            self.rgb = Some(rgb);
            self.dashed = dashed;
        }
        self.width = f64::from(pixmap.width());
        self.height = f64::from(pixmap.height());
    }

    fn add(&mut self, camera: &Camera, shape: Shape) {
        let screen = |p: arq_geometry::Point2| camera.to_screen(p.x(), p.y());
        match shape {
            Shape::Line(l) => self.segment(screen(l.start()), screen(l.end())),
            Shape::Circle(c) => {
                self.arc(camera, c.center(), c.radius(), 0.0, std::f64::consts::TAU);
            }
            Shape::Arc(a) => self.arc(camera, a.center(), a.radius(), a.start_angle(), a.sweep()),
            Shape::Point(p) => {
                let (x, y) = screen(p);
                self.segment((x - POINT_PX, y), (x + POINT_PX, y));
                self.segment((x, y - POINT_PX), (x, y + POINT_PX));
            }
            Shape::Text(b) | Shape::Placeholder(b) => {
                let [p0, p1, p2, p3] = b.corners().map(screen);
                self.segment(p0, p1);
                self.segment(p1, p2);
                self.segment(p2, p3);
                self.segment(p3, p0);
            }
        }
    }

    /// An arc as a polyline fine enough for its size on screen.
    fn arc(
        &mut self,
        camera: &Camera,
        center: arq_geometry::Point2,
        radius: f64,
        start: f64,
        sweep: f64,
    ) {
        let r_px = radius * camera.scale();
        let (cx, cy) = (center.x(), center.y());
        if r_px < 0.5 {
            let p = camera.to_screen(cx, cy);
            return self.segment(p, (p.0 + 0.5, p.1));
        }
        // Largest angle step whose chord stays within the tolerance.
        let step = 2.0 * (1.0 - (ARC_TOLERANCE_PX / r_px).min(1.0)).acos();
        let n = ((sweep / step).ceil() as usize).clamp(2, 4096);
        let point = |i: usize| {
            let a = start + sweep * (i as f64) / (n as f64);
            camera.to_screen(cx + radius * a.cos(), cy + radius * a.sin())
        };
        let mut previous = point(0);
        for i in 1..=n {
            let next = point(i);
            self.segment(previous, next);
            previous = next;
        }
    }

    /// A segment in screen pixels, clipped to the view first.
    fn segment(&mut self, a: (f64, f64), b: (f64, f64)) {
        let pad = 4.0;
        if let Some((a, b)) = clip(a, b, -pad, -pad, self.width + pad, self.height + pad) {
            self.path.move_to(a.0 as f32, a.1 as f32);
            self.path.line_to(b.0 as f32, b.1 as f32);
        }
    }

    fn flush(&mut self, pixmap: &mut Pixmap) {
        let builder = std::mem::replace(&mut self.path, PathBuilder::new());
        let (Some(path), Some([r, g, b])) = (builder.finish(), self.rgb) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: 0.0, // hairline: one pixel at any zoom, the fastest path
            dash: if self.dashed {
                StrokeDash::new(vec![4.0, 3.0], 0.0)
            } else {
                None
            },
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

/// Liang–Barsky: the part of segment `a`–`b` inside the rectangle, if any.
fn clip(
    a: (f64, f64),
    b: (f64, f64),
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> Option<((f64, f64), (f64, f64))> {
    if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) {
        return None;
    }
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.0 - x0),
        (dx, x1 - a.0),
        (-dy, a.1 - y0),
        (dy, y1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((
        (a.0 + t0 * dx, a.1 + t0 * dy),
        (a.0 + t1 * dx, a.1 + t1 * dy),
    ))
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
