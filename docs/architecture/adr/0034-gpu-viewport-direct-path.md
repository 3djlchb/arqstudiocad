# 0034. The GPU viewport: drawing straight into the widget

Date: 2026-11-10 · Status: Proposed

## Context

ADR 0007 decides one wgpu renderer with its backend chosen at run time,
and the GT 730 on LMDE 7 as the release gate. ADR 0013 D5.8 makes the CPU
rasterizer the guaranteed floor. Milestone 02.7 has the probe, the spatial
index and an offscreen `GpuRenderer` (PR #146). That renderer draws the
rasterizer's very segments and is checked against it.

Measured on the reference machine (GT 730, nouveau, Mesa 25.0.7, X11):

| What | 1366 × 696 | 1920 × 1080 |
|---|---|---|
| CPU rasterizer, the 100,000-entity drawing | | 99 ms |
| CPU rasterizer, a 1 % view of it | | 13 ms |
| Tracing that drawing into segments on the CPU | | 30 ms |
| Reading a frame back from the GPU | about 20 ms | 20–40 ms |
| Clearing a frame | 5 ms | 10 ms |
| One full-frame textured quad | 6 ms | 7 ms |
| Uploading a full-frame RGBA image | 13–18 ms | 13–24 ms |
| 100,000 lines kept on the GPU, drawn straight into the widget | under 16.6 ms (60 fps, vsync) | |
| 1,000,000 such lines | 35 ms | |

The [spike](../../testing/wgpu-qt-spike.md) and ADR 0007's last section
have the method. The upload and quad times come from a further
measurement that wrote a full-frame texture and composited two textured
quads, waiting for the GPU.

So on the floor machine:

- **Reading back costs more than it saves.** The offscreen path is no gain,
  and the maintainer chose the direct path (ADR 0007, 2026-11-09).
- **Tracing every frame costs as much as drawing.** The drawing must stay
  on the GPU, so that panning and zooming change only the camera.
- **Lines are cheap, full frames are not.** Painting the overlay with
  `QPainter` into a full-frame image and compositing it would cost about
  25 ms per mouse move at 1366 × 696, worse than today, where moving the
  mouse only repaints the overlay over a kept image. The overlays have to be
  geometry too.

Today the viewport paints the CPU image with `QPainter`, then over it
(ADR 0016 D4):

- the selection, two pixels wide, and the highlight under the cursor,
  three pixels wide;
- the grips, five-pixel squares, the hot one red;
- the window and crossing rectangles, translucent blue and green, the
  crossing dashed;
- the rubber band from the last point, dashed;
- the snap marker, AutoCAD's glyph, with its translated name beside it;
- the outlines dragged in a preview, transformed;
- the frame and note of a block dropped from the palette;
- the crosshair (ADR 0029).

## Decision

### D1 — wgpu presents straight to the viewport's window

- **The drawing area becomes a native child of the viewport**
  (`WA_NativeWindow`, `WA_PaintOnScreen`, no paint engine). Python passes
  its native handles to Rust through the PyO3 API, as integers. No crate
  depends on Qt, and PySide6 never calls Rust directly.
- **Surfaces, platform by platform:**

  | Platform | Handles | wgpu needs | State |
  |---|---|---|---|
  | X11 | `winId()` as the Xlib window, and Qt's own `Display*` from `nativeInterface().display()` | the display handle when the instance is created; Vulkan needs Qt's real `Display*` | tried on the reference machine (spike) |
  | Windows | `winId()` as the `HWND` | Win32 handle | to try |
  | macOS | `winId()` as the `NSView*` | AppKit handle, Metal layer | to try |
  | Wayland | Qt's `wl_display` and the window's `wl_surface` | Wayland handles | to try. If PySide6 does not expose `wl_surface`, Qt runs on XWayland (`QT_QPA_PLATFORM=xcb`) when the GPU is on |

- **Presenting:** `Fifo` (vsync), which every backend offers. A frame is
  drawn only when something changed, never in a loop.
- **The widget's size** follows Qt's resize in physical pixels, as the CPU
  image does (device pixel ratio).

### D2 — The drawing lives on the GPU

- **Retained geometry.** The scene goes to the GPU once, and again only
  for what a change set touches. Panning, zooming, hovering and selecting
  change uniforms, not buffers.
- **Arcs and circles** are flattened on the CPU with the rasterizer's
  quarter-pixel rule, for a band of zoom. They are flattened again only
  when the zoom leaves the band, a factor of two either way, and never when
  panning.
- **Precision.** Coordinates are kept relative to the scene's origin, each
  split into two `f32` (high and low). The camera's center is split the same
  way, and the vertex shader subtracts the high and low parts separately.
  This keeps survey coordinates (ADR 0013) exact at the rasterizer's 10⁶
  zoom without `f64` on the GPU, within the downlevel limits.
- **Colors** are resolved per vertex as the rasterizer resolves them (ACI
  on the background). A background change rewrites only colors.
- **The rasterizer's equivalence test stays the referee.** The same
  retained geometry drawn offscreen (#146's image) is compared with the CPU
  rasterizer on the same input (ADR 0013 D5.8).

### D3 — The overlays are geometry drawn by wgpu

Each overlay of ADR 0016 D4, 0029 and 0030 is drawn by wgpu over the
drawing, in the same frame:

- **Wide lines** (selection two pixels, highlight three) as quads made in
  the vertex shader. A line wider than one pixel does not exist in OpenGL
  ES.
- **Selection and highlight** draw the entity's retained segments again, in
  the selection color and width, so selecting uploads nothing.
- **Grips, rectangles, rubber band, crosshair and snap glyphs:** a few
  hundred vertices built each frame, translucent fills blended.
- **Dashes** are cut along each segment from its start, as the rasterizer
  and #146's renderer cut them.
- **Texts** (the snap's name, the drop note) are painted by Qt, so `tr()`,
  the font and right-to-left text keep working, into small images uploaded
  only when the text changes and drawn as quads where Qt places them.
  Nothing full-frame is uploaded or blended.

### D4 — One description of the overlays, two painters

The viewport keeps deciding *what* is overlaid: selection, highlight,
grips, band, snap, crosshair, drop. It turns that into one description,
passed to the view, which draws it with wgpu. The CPU path paints the same
description with `QPainter`. Today's painting code becomes that painter,
so the two paths cannot drift apart. ADR 0016 D4's look is unchanged.

### D5 — Choosing, falling back, being told

- **At start** the probe of ADR 0007 runs, each backend in a child
  process. The viewport uses the first **hardware** adapter that passes. A
  CPU adapter (llvmpipe, lavapipe, WARP) is never used for the viewport: the
  CPU rasterizer is the fallback (ADR 0013 D5.8). Lavapipe drew the
  100,000-line drawing in 63–71 ms using every core.
- **Crash sentinel:** a file written before the GPU starts and removed
  after its first presented frame. If the file is there at the next start,
  the GPU is skipped, and the user is told why and how to try again.
- **Preferences:** Renderer, Automatic (the default), a named backend, or
  CPU. **`--safe-mode`** forces the CPU rasterizer.
- **At run time** a lost device or surface switches that viewport to the
  CPU rasterizer for the session, in the same widget. The command window
  says so. Nothing is lost: the document never lives in the renderer.

### D6 — The order of work

Each step is its own pull request, every one with the GPU off by default
until the last:

1. **Present:** a surface for the viewport on X11, the drawing retained
   with the precision of D2, pan and zoom by uniform. Measured on the
   reference machine.
2. **Overlays:** the description of D4, drawn by wgpu and painted by
   `QPainter`.
3. **Choosing and falling back:** D5, then the GPU on by default where it
   passes.
4. **The other platforms:** Windows, macOS and Wayland tried and recorded.
   02.7 is complete when the reference machine passes its release gate
   (ADR 0007).

## Consequences

- **The drawing and the overlays come from the GPU in one frame.** Hovering
  and selecting change uniforms and a few hundred vertices, still never
  re-rendering the drawing (ADR 0016 D4's intent, now on the GPU).
- **The overlay code moves:** from painting directly to describing (D4).
  Its look and behavior do not change, and the CPU path keeps them.
- **Native child widgets** bring the usual Qt caveats. Nothing else can
  paint over that area, so tooltips and menus must be top-level windows, as
  they already are. A screenshot of the window comes from the surface, not
  from `QWidget.grab()`.
- **Text stays Qt's**, at the cost of small uploads when a label changes.
- **Two renderers are kept on purpose**, the CPU rasterizer and the GPU,
  over one trace and one overlay description, compared by tests.
- **Amends ADR 0016 D4** (the overlays are drawn on the GPU when it is on,
  painted over the CPU image otherwise) and ADR 0007's offscreen fallback
  (the CPU rasterizer is the fallback, not offscreen readback).
