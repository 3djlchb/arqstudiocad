//! The drawing kept on the GPU (ADR 0034 D2): a scene's segments in world
//! coordinates, so that panning and zooming change only the camera.
//!
//! The geometry covers a *region* around the view: the view and four more
//! on each side. It is clipped to that region in `f64` and kept relative to
//! the region's center, so its `f32` coordinates stay small. Survey
//! coordinates keep their precision, and a long line seen a million times
//! closer has no endpoint millions of pixels away to round. Arcs are
//! flattened for a *band* of zoom, finely enough for twice the scale built
//! for. The geometry is built again when the scene changes, the background
//! changes, the zoom leaves the band (a factor of two either way), or the
//! view leaves the region.
//!
//! Colors, the arc rule, points and dashes are the CPU rasterizer's, so
//! both draw the same thing (ADR 0013 D5.8).

use arq_core::Shape;
use arq_geometry::{Bounds2, Point2, Tolerance};

use crate::camera::Camera;
use crate::raster::{ARC_TOLERANCE_PX, POINT_PX, Style, clip, ink};
use crate::scene::{Scene, shape_bounds};

/// How many views the region reaches beyond the view on each side.
const REGION_VIEWS: f64 = 4.0;
/// The zoom may change by this factor either way before arcs are
/// flattened again.
const BAND: f64 = 2.0;
/// `along` of a solid segment: not dashed.
pub(crate) const SOLID: f32 = -1.0;

/// One vertex of a line, as the GPU reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub(crate) struct Vertex {
    /// World position relative to the region's center.
    pub(crate) at: [f32; 2],
    /// Added on screen after the camera, in pixels (y down): a point's
    /// cross.
    pub(crate) offset: [f32; 2],
    /// World distance from the segment's start, for dashes; [`SOLID`] when
    /// not dashed.
    pub(crate) along: f32,
    /// The color, opaque.
    pub(crate) rgba: [u8; 4],
}

/// What the geometry was built for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    revision: u64,
    background: [u8; 3],
    /// The scale arcs were flattened for.
    scale: f64,
    region: Bounds2,
}

/// A scene's segments around a view, relative to the region's center.
#[derive(Clone, Debug)]
pub(crate) struct Retained {
    key: Key,
    /// The region's center, in world coordinates.
    pub(crate) origin: (f64, f64),
    /// Line-list vertices: two per segment.
    pub(crate) vertices: Vec<Vertex>,
    /// Point-list vertices: one pixel for each line that can be shorter than
    /// a pixel within the band, and each circle or arc smaller than one. A
    /// line that short makes no fragment on the GPU, while the CPU still
    /// shows it as a dot.
    pub(crate) dots: Vec<Vertex>,
}

impl Retained {
    /// The geometry of `scene` around what `camera` shows, in `style`'s
    /// colors. `None` when the camera shows nothing finite.
    pub(crate) fn build(scene: &Scene, camera: &Camera, style: Style) -> Option<Self> {
        let view = camera.visible()?;
        let region = around(view)?;
        let center = region.center();
        let key = Key {
            revision: scene.revision(),
            background: style.background,
            scale: camera.scale(),
            region,
        };
        let mut builder = Builder {
            origin: (center.x(), center.y()),
            region,
            fine_scale: camera.scale() * BAND,
            coarse_scale: camera.scale() / BAND,
            rgba: [0, 0, 0, 255],
            vertices: Vec::new(),
            dots: Vec::new(),
        };
        let tol = Tolerance::default();
        for entry in scene.entries() {
            if !entry.bounds.is_some_and(|b| b.intersects(region, tol)) {
                continue;
            }
            for item in &entry.items {
                if !shape_bounds(item.shape).intersects(region, tol) {
                    continue;
                }
                let [r, g, b] = ink(item.color, style);
                builder.rgba = [r, g, b, 255];
                builder.add(item.shape);
            }
        }
        Some(Self {
            key,
            origin: builder.origin,
            vertices: builder.vertices,
            dots: builder.dots,
        })
    }

    /// Whether this geometry still serves `scene` seen through `camera`.
    pub(crate) fn serves(&self, scene: &Scene, camera: &Camera, style: Style) -> bool {
        let key = &self.key;
        let scale = camera.scale();
        key.revision == scene.revision()
            && key.background == style.background
            && scale <= key.scale * BAND
            && scale >= key.scale / BAND
            && camera
                .visible()
                .is_some_and(|view| contains(key.region, view))
    }

    /// The camera's center relative to the region's center.
    pub(crate) fn center(&self, camera: &Camera) -> (f64, f64) {
        let (x, y) = camera.center();
        (x - self.origin.0, y - self.origin.1)
    }
}

/// The view and [`REGION_VIEWS`] more on each side.
fn around(view: Bounds2) -> Option<Bounds2> {
    let (w, h) = (view.width(), view.height());
    view.expanded(REGION_VIEWS * w.max(h)).ok()
}

fn contains(outer: Bounds2, inner: Bounds2) -> bool {
    outer.min().x() <= inner.min().x()
        && outer.min().y() <= inner.min().y()
        && outer.max().x() >= inner.max().x()
        && outer.max().y() >= inner.max().y()
}

struct Builder {
    origin: (f64, f64),
    region: Bounds2,
    /// Arcs are flattened for this scale, the band's finest.
    fine_scale: f64,
    /// The band's smallest scale, where segments are shortest on screen.
    coarse_scale: f64,
    rgba: [u8; 4],
    vertices: Vec<Vertex>,
    dots: Vec<Vertex>,
}

impl Builder {
    fn add(&mut self, shape: Shape) {
        match shape {
            Shape::Line(l) => {
                self.segment(l.start(), l.end(), false);
                self.dot_if_tiny(l.start(), l.end());
            }
            Shape::Circle(c) => self.arc(c.center(), c.radius(), 0.0, std::f64::consts::TAU),
            Shape::Arc(a) => self.arc(a.center(), a.radius(), a.start_angle(), a.sweep()),
            Shape::Point(p) => {
                let d = POINT_PX as f32;
                self.mark(p, [-d, 0.0], [d, 0.0]);
                self.mark(p, [0.0, -d], [0.0, d]);
            }
            // A text's letters come as lines; its box only picks it.
            Shape::Text(_) => {}
            Shape::Placeholder(b) => {
                let [p0, p1, p2, p3] = b.corners();
                self.segment(p0, p1, true);
                self.segment(p1, p2, true);
                self.segment(p2, p3, true);
                self.segment(p3, p0, true);
            }
        }
    }

    /// An arc as a polyline fine enough for the band's largest scale, or a
    /// dot when it is smaller than a pixel there, as the CPU draws it.
    fn arc(&mut self, center: Point2, radius: f64, start: f64, sweep: f64) {
        // Smaller than a pixel within the band: one dot, as the CPU's
        // faint smudge; never a dot per chord, which would light the whole
        // neighbourhood of a drawing full of small circles.
        if 2.0 * radius * self.coarse_scale < 1.0
            && self.region.contains_point(center, Tolerance::default())
        {
            self.dot((center.x(), center.y()));
        }
        let r_px = radius * self.fine_scale;
        if r_px < 0.5 {
            return;
        }
        // Largest angle step whose chord stays within the tolerance.
        let step = 2.0 * (1.0 - (ARC_TOLERANCE_PX / r_px).min(1.0)).acos();
        let n = ((sweep / step).ceil() as usize).clamp(2, 4096);
        let (cx, cy) = (center.x(), center.y());
        let point = |i: usize| {
            let a = start + sweep * (i as f64) / (n as f64);
            Point2::new(cx + radius * a.cos(), cy + radius * a.sin()).ok()
        };
        let mut previous = point(0);
        for i in 1..=n {
            let next = point(i);
            if let (Some(a), Some(b)) = (previous, next) {
                self.segment(a, b, false);
            }
            previous = next;
        }
    }

    /// A segment clipped to the region in `f64`.
    fn segment(&mut self, a: Point2, b: Point2, dashed: bool) {
        let (min, max) = (self.region.min(), self.region.max());
        let Some((a, b)) = clip(
            (a.x(), a.y()),
            (b.x(), b.y()),
            min.x(),
            min.y(),
            max.x(),
            max.y(),
        ) else {
            return;
        };
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        let (start, end) = if dashed {
            (0.0, length as f32)
        } else {
            (SOLID, SOLID)
        };
        self.push(a, [0.0, 0.0], start);
        self.push(b, [0.0, 0.0], end);
    }

    /// A dot at the middle of a line that can be shorter than a pixel
    /// within the band, when that middle is in the region.
    fn dot_if_tiny(&mut self, a: Point2, b: Point2) {
        let mid = Point2::new((a.x() + b.x()) / 2.0, (a.y() + b.y()) / 2.0);
        if let Ok(mid) = mid
            && a.distance_to(b) * self.coarse_scale < 1.0
            && self.region.contains_point(mid, Tolerance::default())
        {
            self.dot((mid.x(), mid.y()));
        }
    }

    /// One pixel at `p`.
    fn dot(&mut self, (x, y): (f64, f64)) {
        let vertex = self.vertex((x, y), [0.0, 0.0], SOLID);
        self.dots.push(vertex);
    }

    /// A short mark of fixed size on screen at `p`: a point's arm.
    fn mark(&mut self, p: Point2, from: [f32; 2], to: [f32; 2]) {
        if !self.region.contains_point(p, Tolerance::default()) {
            return;
        }
        self.push((p.x(), p.y()), from, SOLID);
        self.push((p.x(), p.y()), to, SOLID);
    }

    fn push(&mut self, at: (f64, f64), offset: [f32; 2], along: f32) {
        let vertex = self.vertex(at, offset, along);
        self.vertices.push(vertex);
    }

    fn vertex(&self, (x, y): (f64, f64), offset: [f32; 2], along: f32) -> Vertex {
        Vertex {
            at: [(x - self.origin.0) as f32, (y - self.origin.1) as f32],
            offset,
            along,
            rgba: self.rgba,
        }
    }
}

#[cfg(test)]
#[path = "retained_tests.rs"]
mod tests;
