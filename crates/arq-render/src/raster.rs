//! The CPU rasterizer: a scene seen through a camera, painted into an RGBA
//! image with tiny-skia (ADR 0013, D5.3). It needs no GPU and no Qt, and is
//! the guaranteed fallback of every renderer to come.
//!
//! Shapes outside the view are skipped by their boxes; segments are clipped
//! to the view in `f64` before becoming `f32` pixels, so far-away geometry
//! never reaches the painter with huge coordinates. Consecutive shapes of
//! one color are stroked as one path.
//!
//! What is drawn ([`trace`]) is kept apart from who paints it (a [`Sink`]),
//! so the GPU renderer draws the very same segments.

use arq_core::{Color, DrawItem, Shape};
use arq_geometry::Tolerance;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

use crate::aci::aci_rgb_on;
use crate::camera::Camera;
use crate::scene::{Scene, shape_bounds};

/// How far a flattened arc may stray from the true curve, in pixels.
pub(crate) const ARC_TOLERANCE_PX: f64 = 0.25;
/// Half the size of a point's cross, in pixels.
pub(crate) const POINT_PX: f64 = 2.0;
/// An entity not drawn yet is outlined dashed: on, off, in pixels.
pub(crate) const DASH_PX: [f32; 2] = [4.0, 3.0];
/// Beyond the view, segments are clipped this far out, in pixels.
const PAD_PX: f64 = 4.0;
/// The CPU's canvas reaches this far beyond the view, past the pad and a
/// stroke's antialiasing, so tiny-skia never clips.
const MARGIN_PX: f32 = 8.0;

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
    let [r, g, b] = style.background;
    paint(
        scene,
        camera,
        style,
        tiny_skia::Color::from_rgba8(r, g, b, 255),
    )
}

/// A picture of `items` fitted to a `size` × `size` square on a transparent
/// background: a block's preview in the Blocks palette (ADR 0025 D3). Each
/// shape keeps its own color, as the viewport draws it over `background`
/// (ACI 7 white on dark, black on light), unless `ink` paints them all.
pub fn preview(items: &[DrawItem], size: u32, background: [u8; 3], ink: Option<[u8; 3]>) -> Image {
    let shown = items
        .iter()
        .map(|item| DrawItem {
            shape: item.shape,
            color: ink.map_or(item.color, |[r, g, b]| Color::Rgb { r, g, b }),
        })
        .collect();
    let scene = Scene::from_items(shown);
    let size = size.max(1);
    let mut camera = Camera::new(size, size);
    if let Some(bounds) = scene.bounds() {
        camera.fit(bounds);
    }
    paint(
        &scene,
        &camera,
        Style { background },
        tiny_skia::Color::TRANSPARENT,
    )
}

/// Paints `scene` as `camera` sees it over `fill`.
fn paint(scene: &Scene, camera: &Camera, style: Style, fill: tiny_skia::Color) -> Image {
    let (width, height) = camera.size();
    // Painted with a margin all round, wider than the trace's clipping pad,
    // then cut out: tiny-skia never clips a hairline itself, which shifted
    // lines entering at the top edge by a pixel (found against the GPU).
    let margin = MARGIN_PX as u32;
    let mut pixmap =
        Pixmap::new(width + 2 * margin, height + 2 * margin).expect("a camera is at least 1x1");
    pixmap.fill(fill);
    let mut sink = PixmapSink {
        pixmap,
        path: PathBuilder::new(),
        fill: PathBuilder::new(),
        rgb: None,
        dashed: false,
    };
    trace(scene, camera, style, &mut sink);
    sink.flush();
    let full = sink.pixmap.take();
    let (row, stride) = (width as usize * 4, (width + 2 * margin) as usize * 4);
    let mut pixels = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        let start = (y + margin as usize) * stride + margin as usize * 4;
        pixels.extend_from_slice(&full[start..start + row]);
    }
    Image {
        width,
        height,
        pixels,
    }
}

/// Where traced segments go: tiny-skia here, the GPU renderer's vertices
/// there, so every renderer draws the very same segments (ADR 0013 D5.8).
pub(crate) trait Sink {
    /// The segments from here on are drawn in `rgb`, dashed or not. Called
    /// only when either changes.
    fn style(&mut self, rgb: [u8; 3], dashed: bool);
    /// A segment in screen pixels, already clipped to near the view.
    fn segment(&mut self, a: (f32, f32), b: (f32, f32));
    /// A filled triangle in screen pixels, already clipped to near the view
    /// (ADR 0038). Every fill comes before every segment.
    fn triangle(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32));
}

/// Walks what `scene` shows through `camera` into `sink`, as segments in
/// screen pixels: shapes outside the view skipped by their boxes, arcs
/// flattened for their size on screen, segments clipped in `f64`. The
/// fills come first, under every stroke (ADR 0038), so a wide polyline
/// never hides the lines crossing it.
pub(crate) fn trace(scene: &Scene, camera: &Camera, style: Style, sink: &mut impl Sink) {
    let Some(view) = camera.visible() else {
        return;
    };
    // A little beyond the edges, so strokes touching them are drawn.
    let margin = 2.0 / camera.scale();
    let view = view.expanded(margin).unwrap_or(view);
    let tol = Tolerance::default();
    let (width, height) = camera.size();
    let mut tracer = Tracer {
        style,
        sink,
        current: None,
        width: f64::from(width),
        height: f64::from(height),
    };
    for fills in [true, false] {
        for entry in scene.entries() {
            if !entry.bounds.is_some_and(|b| b.intersects(view, tol)) {
                continue;
            }
            for item in &entry.items {
                let is_fill = matches!(item.shape, Shape::Triangle(_));
                if is_fill != fills || !shape_bounds(item.shape).intersects(view, tol) {
                    continue;
                }
                tracer.set_color(item.color, matches!(item.shape, Shape::Placeholder(_)));
                tracer.add(camera, item.shape);
            }
        }
    }
}

/// The trace in progress: the current color and the view's size.
struct Tracer<'a, S: Sink> {
    style: Style,
    sink: &'a mut S,
    current: Option<([u8; 3], bool)>,
    width: f64,
    height: f64,
}

impl<S: Sink> Tracer<'_, S> {
    /// Tells the sink when the color or the dash changes.
    fn set_color(&mut self, color: Color, dashed: bool) {
        let rgb = ink(color, self.style);
        if self.current != Some((rgb, dashed)) {
            self.current = Some((rgb, dashed));
            self.sink.style(rgb, dashed);
        }
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
            // A text's letters come as lines; its box only picks it.
            Shape::Text(_) => {}
            Shape::Placeholder(b) => {
                let [p0, p1, p2, p3] = b.corners().map(screen);
                self.segment(p0, p1);
                self.segment(p1, p2);
                self.segment(p2, p3);
                self.segment(p3, p0);
            }
            Shape::Triangle(t) => {
                let pad = PAD_PX;
                let corners = t.map(screen);
                let kept = clip_polygon(&corners, -pad, -pad, self.width + pad, self.height + pad);
                let f = |p: (f64, f64)| (p.0 as f32, p.1 as f32);
                for i in 1..kept.len().saturating_sub(1) {
                    self.sink.triangle(f(kept[0]), f(kept[i]), f(kept[i + 1]));
                }
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
        let pad = PAD_PX;
        if let Some((a, b)) = clip(a, b, -pad, -pad, self.width + pad, self.height + pad) {
            self.sink
                .segment((a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32));
        }
    }
}

/// The color a shape is drawn in on `style`'s background: ACI 7 is white
/// on dark and black on light, as the rest of the ACI table adapts.
pub(crate) fn ink(color: Color, style: Style) -> [u8; 3] {
    match color {
        Color::Rgb { r, g, b } => [r, g, b],
        Color::Index(i) => aci_rgb_on(i, style.background),
        _ => aci_rgb_on(7, style.background),
    }
}

/// Segments of one color and style, collected into one tiny-skia path;
/// triangles of that color into another, filled.
struct PixmapSink {
    pixmap: Pixmap,
    path: PathBuilder,
    fill: PathBuilder,
    rgb: Option<[u8; 3]>,
    dashed: bool,
}

impl Sink for PixmapSink {
    fn style(&mut self, rgb: [u8; 3], dashed: bool) {
        self.flush();
        self.rgb = Some(rgb);
        self.dashed = dashed;
    }

    fn segment(&mut self, a: (f32, f32), b: (f32, f32)) {
        self.path.move_to(a.0, a.1);
        self.path.line_to(b.0, b.1);
    }

    fn triangle(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
        self.fill.move_to(a.0, a.1);
        self.fill.line_to(b.0, b.1);
        self.fill.line_to(c.0, c.1);
        self.fill.close();
    }
}

impl PixmapSink {
    fn flush(&mut self) {
        let fill = std::mem::replace(&mut self.fill, PathBuilder::new());
        if let (Some(path), Some([r, g, b])) = (fill.finish(), self.rgb) {
            let mut paint = Paint::default();
            paint.set_color_rgba8(r, g, b, 255);
            // Pixel centers decide, as the GPU's triangles: both the same.
            paint.anti_alias = false;
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::from_translate(MARGIN_PX, MARGIN_PX),
                None,
            );
        }
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
                StrokeDash::new(DASH_PX.to_vec(), 0.0)
            } else {
                None
            },
            ..Stroke::default()
        };
        self.pixmap.stroke_path(
            &path,
            &paint,
            &stroke,
            Transform::from_translate(MARGIN_PX, MARGIN_PX),
            None,
        );
    }
}

/// Sutherland–Hodgman: the part of a convex polygon inside the rectangle,
/// in `f64`, its corners in order; fewer than three when nothing is left.
pub(crate) fn clip_polygon(
    polygon: &[(f64, f64)],
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> Vec<(f64, f64)> {
    type Inside = fn((f64, f64), f64) -> bool;
    type Cross = fn((f64, f64), (f64, f64), f64) -> (f64, f64);
    let at_x: Cross = |a, b, x| (x, a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0));
    let at_y: Cross = |a, b, y| (a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1), y);
    let edges: [(Inside, Cross, f64); 4] = [
        (|p, x| p.0 >= x, at_x, x0),
        (|p, x| p.0 <= x, at_x, x1),
        (|p, y| p.1 >= y, at_y, y0),
        (|p, y| p.1 <= y, at_y, y1),
    ];
    let mut out = polygon.to_vec();
    for (inside, cross, limit) in edges {
        let input = std::mem::take(&mut out);
        let Some(&last) = input.last() else {
            break;
        };
        let mut previous = last;
        for &p in &input {
            match (inside(p, limit), inside(previous, limit)) {
                (true, true) => out.push(p),
                (true, false) => {
                    out.push(cross(previous, p, limit));
                    out.push(p);
                }
                (false, true) => out.push(cross(previous, p, limit)),
                (false, false) => {}
            }
            previous = p;
        }
    }
    out
}

/// Liang–Barsky: the part of segment `a`–`b` inside the rectangle, if any.
pub(crate) fn clip(
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
