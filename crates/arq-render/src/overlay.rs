//! The overlays drawn over the drawing (ADR 0034 D3, D4): the selection,
//! the highlight, grips, rubber bands, snap markers, the crosshair and the
//! texts, as the viewport describes them once for both renderers.
//!
//! The viewport paints this description with `QPainter` over the CPU's
//! image, or hands it here for the GPU. Strokes and fills become triangles
//! on the CPU (a few hundred a frame), their edges antialiased in the
//! shader by their distance to the stroke's middle, so no multisampling is
//! needed. Texts come as small images Qt painted, drawn as textured quads.
//!
//! Coordinates are the widget's logical pixels, origin top-left, y down;
//! the device pixel ratio scales them to the GPU's physical pixels.

/// One thing drawn over the drawing, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayItem {
    /// A polyline.
    Stroke {
        /// Its points, at least two to draw anything.
        points: Vec<[f32; 2]>,
        /// Color, straight (not premultiplied) alpha.
        rgba: [u8; 4],
        /// Width in logical pixels.
        width: f32,
        /// Dashed as Qt's `DashLine`: four widths on, two off, continuing
        /// along the polyline.
        dashed: bool,
        /// The last point joins the first.
        closed: bool,
        /// Crisp: whole pixels, no antialiasing (the crosshair).
        crisp: bool,
    },
    /// A filled axis-aligned rectangle.
    Fill {
        /// One corner.
        min: [f32; 2],
        /// The opposite corner.
        max: [f32; 2],
        /// Color, straight alpha.
        rgba: [u8; 4],
    },
}

/// A text Qt painted, to be drawn as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayLabel {
    /// Its top-left corner, in physical pixels.
    pub at: [i32; 2],
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, RGBA, premultiplied alpha.
    pub pixels: Vec<u8>,
}

/// Everything drawn over the drawing in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// Strokes and fills, in the order they are painted.
    pub items: Vec<OverlayItem>,
    /// Texts, over everything else.
    pub labels: Vec<OverlayLabel>,
    /// Physical pixels per logical pixel.
    pub ratio: f32,
}

impl Overlay {
    /// Nothing over the drawing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.labels.is_empty()
    }
}

/// A triangle vertex: where, in physical pixels, its color, and how far it
/// lies across the stroke from its middle, against the stroke's half
/// width, so the fragment shader can fade the edges.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub(crate) struct OverlayVertex {
    pub(crate) at: [f32; 2],
    pub(crate) rgba: [u8; 4],
    /// Signed distance across the stroke, in physical pixels.
    pub(crate) across: f32,
    /// The stroke's half width; negative for what is never faded (fills,
    /// crisp strokes).
    pub(crate) half: f32,
}

/// The overlay's strokes and fills as a triangle list, in order.
pub(crate) fn triangles(overlay: &Overlay) -> Vec<OverlayVertex> {
    let ratio = if overlay.ratio > 0.0 {
        overlay.ratio
    } else {
        1.0
    };
    let mut out = Vec::new();
    for item in &overlay.items {
        match item {
            OverlayItem::Fill { min, max, rgba } => {
                let (a, b) = (scale(*min, ratio), scale(*max, ratio));
                let corners = [[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]];
                quad(&mut out, corners, *rgba, [0.0; 4], -1.0);
            }
            OverlayItem::Stroke {
                points,
                rgba,
                width,
                dashed,
                closed,
                crisp,
            } => {
                let width = (width * ratio).max(1.0);
                let mut path: Vec<[f32; 2]> = points
                    .iter()
                    .map(|&p| {
                        let p = scale(p, ratio);
                        if *crisp { pixel_center(p) } else { p }
                    })
                    .collect();
                if *closed && path.len() > 2 {
                    path.push(path[0]);
                }
                let pieces = if *dashed {
                    dashes(&path, 4.0 * width, 2.0 * width)
                } else {
                    vec![path]
                };
                for piece in pieces {
                    for pair in piece.windows(2) {
                        segment(&mut out, pair[0], pair[1], *rgba, width, *crisp);
                    }
                }
            }
        }
    }
    out
}

fn scale(p: [f32; 2], ratio: f32) -> [f32; 2] {
    [p[0] * ratio, p[1] * ratio]
}

/// The middle of the pixel `p` falls in: a one-pixel line through it fills
/// exactly that column or row.
fn pixel_center(p: [f32; 2]) -> [f32; 2] {
    [p[0].floor() + 0.5, p[1].floor() + 0.5]
}

/// A segment as a quad. Antialiased strokes reach half a pixel beyond their
/// width, where the shader fades them; crisp ones cover whole pixels and
/// stretch to cover their end pixels too.
fn segment(
    out: &mut Vec<OverlayVertex>,
    a: [f32; 2],
    b: [f32; 2],
    rgba: [u8; 4],
    width: f32,
    crisp: bool,
) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    if length == 0.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let half = width / 2.0;
    let (reach, extend, fade) = if crisp {
        (half, half, -1.0)
    } else {
        (half + 0.5, 0.0, half)
    };
    let (nx, ny) = (-uy * reach, ux * reach);
    let (ex, ey) = (ux * extend, uy * extend);
    let (a, b) = ([a[0] - ex, a[1] - ey], [b[0] + ex, b[1] + ey]);
    let corners = [
        [a[0] + nx, a[1] + ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] - nx, a[1] - ny],
    ];
    quad(out, corners, rgba, [reach, reach, -reach, -reach], fade);
}

/// Two triangles over four corners in order.
fn quad(
    out: &mut Vec<OverlayVertex>,
    corners: [[f32; 2]; 4],
    rgba: [u8; 4],
    across: [f32; 4],
    half: f32,
) {
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(OverlayVertex {
            at: corners[i],
            rgba,
            across: across[i],
            half,
        });
    }
}

/// The dashes of a polyline, `on` drawn and `off` skipped, the pattern
/// running on from one segment to the next as Qt's does.
fn dashes(path: &[[f32; 2]], on: f32, off: f32) -> Vec<Vec<[f32; 2]>> {
    let mut pieces = Vec::new();
    let mut current: Vec<[f32; 2]> = Vec::new();
    // Whether the pattern is drawing now, and how much of that part is left.
    let (mut drawing, mut left) = (true, on);
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let at = |d: f32| {
            let t = if length > 0.0 { d / length } else { 0.0 };
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        if drawing && current.is_empty() {
            current.push(a);
        }
        let mut d = 0.0;
        while length - d > left {
            d += left;
            let p = at(d);
            if drawing {
                current.push(p);
                pieces.push(std::mem::take(&mut current));
            } else {
                current = vec![p];
            }
            drawing = !drawing;
            left = if drawing { on } else { off };
        }
        left -= length - d;
        if drawing {
            current.push(b);
        }
    }
    if current.len() > 1 {
        pieces.push(current);
    }
    pieces
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
