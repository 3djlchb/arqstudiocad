// Ported from tiny-skia 0.12.0, src/scan/hairline_aa.rs, under its license:
//
// Copyright (c) 2011 Google Inc. All rights reserved.
// Copyright (c) 2020 Yevhenii Reizner All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
//   * Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in
//     the documentation and/or other materials provided with the
//     distribution.
//
//   * Neither the name of the copyright holder nor the names of its
//     contributors may be used to endorse or promote products derived
//     from this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// The port's changes are ArqStudioCAD's, GPL-3.0-or-later.

//! Antialiased hairlines painted straight into premultiplied RGBA pixels.
//!
//! The CPU rasterizer's strokes are one-pixel hairlines in one opaque
//! color. tiny-skia draws them well but slowly: every two-pixel step of a
//! line runs its whole raster pipeline, which cost about two thirds of a
//! frame on the floor machine (2026-11-20 and 2026-11-23 reports). This is
//! tiny-skia's own antialiased hairline (`scan/hairline_aa.rs`, from
//! Skia's `SkScan_Antihair.cpp`) with the pipeline replaced by the very
//! blend it computes for an opaque color, so the pixels are the same, bit
//! for bit (`hairline_tests.rs` holds it to that).
//!
//! Ported from tiny-skia 0.12 (BSD-3-Clause, its notice above; see
//! `THIRD_PARTY.md`).
//! What is left out never happens here: dashes (tiny-skia still draws
//! those), caps other than butt, and segments outside the canvas. The
//! trace clips every segment to near the view, inside the canvas's margin;
//! a pixel that would still fall outside is dropped, never written.

/// 26.6 fixed point.
type FDot6 = i32;
/// 16.16 fixed point.
type FDot16 = i32;

const ONE_16: FDot16 = 1 << 16;
const HALF_16: FDot16 = ONE_16 / 2;
/// tiny-skia's fixed-point range: the caller clips to it first.
const MAX_COORD: f32 = 32767.0;

/// Hairlines of one opaque color onto `width` × `height` RGBA pixels.
pub(crate) struct Hairlines<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
    rgba: [u8; 4],
}

impl<'a> Hairlines<'a> {
    /// `pixels` is `width * height * 4` premultiplied bytes, rows top to
    /// bottom, as a tiny-skia `Pixmap` holds them.
    pub(crate) fn new(pixels: &'a mut [u8], width: u32, height: u32, rgb: [u8; 3]) -> Self {
        debug_assert_eq!(pixels.len(), width as usize * height as usize * 4);
        let [r, g, b] = rgb;
        Self {
            pixels,
            width,
            height,
            rgba: [r, g, b, 255],
        }
    }

    /// The segment from `a` to `b`, in pixels, as tiny-skia strokes a
    /// hairline: antialiased, butt ends.
    pub(crate) fn line(&mut self, a: (f32, f32), b: (f32, f32)) {
        let inside = |v: f32| v.abs() <= MAX_COORD; // false for NaN too
        if !(inside(a.0) && inside(a.1) && inside(b.0) && inside(b.1)) {
            return;
        }
        let dot6 = |v: f32| (v * 64.0) as FDot6;
        self.anti_hairline(dot6(a.0), dot6(a.1), dot6(b.0), dot6(b.1));
    }

    fn anti_hairline(&mut self, mut x0: FDot6, mut y0: FDot6, mut x1: FDot6, mut y1: FDot6) {
        if (x1 - x0).abs() > 511 * 64 || (y1 - y0).abs() > 511 * 64 {
            // Halved as tiny-skia does, each end shifted on its own.
            let hx = (x0 >> 1) + (x1 >> 1);
            let hy = (y0 >> 1) + (y1 >> 1);
            self.anti_hairline(x0, y0, hx, hy);
            self.anti_hairline(hx, hy, x1, y1);
            return;
        }
        let horizontal = (x1 - x0).abs() > (y1 - y0).abs();
        if horizontal {
            if x0 > x1 {
                (x0, y0, x1, y1) = (x1, y1, x0, y0);
            }
        } else {
            if y0 > y1 {
                (x0, y0, x1, y1) = (x1, y1, x0, y0);
            }
            if x0 == x1 && y0 == y1 {
                return; // nothing to draw
            }
        }
        // Along the major axis (u) and across it (v).
        let (u0, v0, u1, v1) = if horizontal {
            (x0, y0, x1, y1)
        } else {
            (y0, x0, y1, x1)
        };
        let istart = u0 >> 6;
        let istop = (u1 + 63) >> 6;
        let mut fstart = left_shift(v0, 10);
        let straight = v0 == v1;
        let slope = if straight {
            0
        } else {
            let slope = left_shift(v1 - v0, 16) / (u1 - u0);
            fstart += (slope * (32 - (u0 & 63)) + 32) >> 6;
            slope
        };
        let (scale_start, scale_stop) = if istop - istart == 1 {
            (u1 - u0, 0) // within a single pixel
        } else {
            (64 - (u0 & 63), u1 & 63)
        };
        if istart < 0 {
            return; // off the canvas: the trace never sends one
        }
        let kind = match (horizontal, straight) {
            (true, true) => Kind::HLine,
            (true, false) => Kind::Horish,
            (false, true) => Kind::VLine,
            (false, false) => Kind::Vertish,
        };
        let (istart, istop) = (istart as u32, istop as u32);
        fstart = self.cap(kind, istart, fstart, slope, scale_start);
        let first = istart + 1;
        let full = istop - first - u32::from(scale_stop > 0);
        if full > 0 {
            fstart = self.run(kind, first, first + full, fstart, slope);
        }
        if scale_stop > 0 {
            self.cap(kind, istop - 1, fstart, slope, scale_stop);
        }
    }

    /// A partly covered end pixel at `u` along the line, its coverage
    /// scaled by `mod64` sixty-fourths.
    fn cap(&mut self, kind: Kind, u: u32, f: FDot16, slope: FDot16, mod64: i32) -> FDot16 {
        let f = (f + HALF_16).max(0);
        let v = (f >> 16) as u32;
        let a = (f >> 8) as u8;
        let near = small_scale(255 - a, mod64);
        let far = small_scale(a, mod64);
        match kind {
            Kind::HLine => {
                self.blend(u, v, far);
                if let Some(above) = v.checked_sub(1) {
                    self.blend(u, above, near);
                }
                f - HALF_16
            }
            Kind::VLine => {
                self.blend(v, u, far);
                self.blend(v.max(1) - 1, u, near);
                f - HALF_16
            }
            Kind::Horish => {
                let top = v.max(1) - 1;
                self.blend(u, top, near);
                self.blend(u, top + 1, far);
                f + slope - HALF_16
            }
            Kind::Vertish => {
                let left = v.max(1) - 1;
                self.blend(left, u, near);
                self.blend(left + 1, u, far);
                f + slope - HALF_16
            }
        }
    }

    /// The fully covered pixels from `u` to `stop`, two across each.
    fn run(&mut self, kind: Kind, u: u32, stop: u32, f: FDot16, slope: FDot16) -> FDot16 {
        debug_assert!(u < stop);
        match kind {
            Kind::HLine | Kind::VLine => {
                let f = (f + HALF_16).max(0);
                let v = (f >> 16) as u32;
                let a = (f >> 8) as u8;
                for u in u..stop {
                    if kind == Kind::HLine {
                        self.blend(u, v, a);
                        if let Some(above) = v.checked_sub(1) {
                            self.blend(u, above, 255 - a);
                        }
                    } else {
                        self.blend(v, u, a);
                        self.blend(v.max(1) - 1, u, 255 - a);
                    }
                }
                f - HALF_16
            }
            Kind::Horish | Kind::Vertish => {
                let mut f = f + HALF_16;
                for u in u..stop {
                    f = f.max(0);
                    let v = (f >> 16) as u32;
                    let a = (f >> 8) as u8;
                    let low = v.max(1) - 1;
                    if kind == Kind::Horish {
                        self.blend(u, low, 255 - a);
                        self.blend(u, low + 1, a);
                    } else {
                        self.blend(low, u, 255 - a);
                        self.blend(low + 1, u, a);
                    }
                    f += slope;
                }
                f - HALF_16
            }
        }
    }

    /// The color over pixel (`x`, `y`) at coverage `a`, source-over, as
    /// tiny-skia's low-precision pipeline computes it (`lerp_u8`).
    #[inline]
    fn blend(&mut self, x: u32, y: u32, a: u8) {
        if a == 0 || x >= self.width || y >= self.height {
            return; // a transparent coverage leaves the pixel as it is
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let pixel = &mut self.pixels[i..i + 4];
        if a == 255 {
            pixel.copy_from_slice(&self.rgba);
            return;
        }
        // Source-over an opaque color is the color itself, then lerped
        // toward it by the coverage.
        let (a, inv) = (u16::from(a), u16::from(255 - a));
        for (d, &s) in pixel.iter_mut().zip(&self.rgba) {
            *d = div255(u16::from(*d) * inv + u16::from(s) * a) as u8;
        }
    }
}

/// Which way a line runs: tiny-skia's four hairline blitters.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    HLine,
    Horish,
    VLine,
    Vertish,
}

/// tiny-skia's `div255`: `(v + 255) / 256`, close to `v / 255` and, for an
/// opaque coverage, exact.
#[inline]
fn div255(v: u16) -> u16 {
    (v + 255) >> 8
}

fn small_scale(value: u8, dot6: i32) -> u8 {
    debug_assert!((0..=64).contains(&dot6));
    ((i32::from(value) * dot6) >> 6) as u8
}

fn left_shift(value: i32, shift: u32) -> i32 {
    ((value as u32) << shift) as i32
}

#[cfg(test)]
#[path = "hairline_tests.rs"]
mod tests;
