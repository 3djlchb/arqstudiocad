# wgpu in a Qt widget: the integration spike on the reference machine

ADR 0007 leaves one risk open. wgpu must draw into a Qt widget through its
native window on Windows, macOS, X11 and Wayland. Its fallback is to render
offscreen with wgpu and let Qt show the image. This page records the first
spike, run on the reference machine on 2026-11-09. It covers X11 only.

## How

An isolated spike, not in the repository: a `cdylib` with `wgpu` =30.0.1
(`gles`, `vulkan`, `wgsl`), `pyo3` 0.29.3 and `raw-window-handle` 0.6.2,
the workspace's versions, plus a PySide6 6.12.0 script on Python 3.13. It
stays outside the repository because only `crates/arq-python` may depend on
`pyo3` (AGENTS.md).

- **Scene:** N short random segments (`LineList`, one `Float32x2` vertex
  buffer). A uniform zooms the scene every frame, so every frame redraws
  every line. One colour, no depth.
- **Limits:** the device is requested with
  `Limits::downlevel_webgl2_defaults()`, ADR 0007's downlevel rule.
- **Path A, direct:** a `QWidget` with `WA_NativeWindow`,
  `WA_PaintOnScreen` and `WA_NoSystemBackground`, whose `paintEngine()`
  returns `None`. Its `winId()` is the X11 window. Rust builds the surface
  from `RawWindowHandle::Xlib` and presents each frame. A `QTimer` drives
  the frames.
- **Path B, offscreen:** wgpu draws to an `Rgba8Unorm` texture. The pixels
  are copied to a mapped buffer, unpadded and returned as bytes. Qt wraps
  them in a `QImage` and paints it with `QPainter`, like today's CPU
  rasterizer.
- **Checks in every run:**
  - the window is resized halfway through, and drawing goes on;
  - after the run, the window's area is grabbed from the screen and the
    lines' red is counted, so a black or stale window would fail;
  - frame times are taken until the GPU is done (`PollType::Wait`).
- **Machine:** the reference machine of ADR 0007 (GT 730, nouveau, Mesa
  25.0.7, LMDE 7, X11, one 1366 × 768 monitor at 60 Hz), so the windows are
  1366 × 696 at device pixel ratio 1.0.

## What X11 needs

- **The display handle.** wgpu's GL backend picks its EGL platform when it
  creates the instance, so the instance needs the display handle too:
  `InstanceDescriptor::new_with_display_handle`. Without it, wgpu uses
  Mesa's surfaceless platform, which has no window surfaces. The surface's
  display handle must then be the same one.
- **Qt's own `Display*` for Vulkan.** With a null Xlib display, wgpu's GL
  backend opens the default display and works. Vulkan panics ("Display
  pointer is not set"). PySide6 gives Qt's `Display*` as an integer:
  `QGuiApplication.nativeInterface().display()`, a
  `QNativeInterface.QX11Application`. Passing it works for both backends,
  and it is what a product renderer should do.
- **Present modes.** The GL surface offers only `Fifo` (vsync). Lavapipe's
  Vulkan surface offers `Immediate`, `Mailbox` and `Fifo`.

## Measured

Frames per second and median / 95th-percentile frame times, 6-second runs:

| Path | Backend | Lines | fps | Median | p95 |
|---|---|---|---|---|---|
| A direct | GL, GT 730 | 10,000 | 60.3 | 16.6 ms | 16.8 ms |
| A direct | GL, GT 730 | 100,000 | 60.3 | 16.6 ms | 20.1 ms |
| A direct | GL, GT 730 | 1,000,000 | 27.7 | 35.1 ms | 42.3 ms |
| A direct | Vulkan, lavapipe | 100,000 | 15.7 | 63.0 ms | 68.3 ms |
| B offscreen + `QImage` | GL, GT 730 | 10,000 | 53.8 | 18.5 ms | 22.8 ms |
| B offscreen + `QImage` | GL, GT 730 | 100,000 | 59.0 | 16.6 ms | 18.6 ms |
| B offscreen + `QImage` | GL, GT 730 | 1,000,000 | 20.7 | 48.2 ms | 50.3 ms |
| B offscreen + `QImage` | Vulkan, lavapipe | 100,000 | 13.2 | 66.1 ms | 70.1 ms |

At 60 fps, path A is held by vsync, so its real cost is lower. Lavapipe
without vsync (`Immediate`) gives the same 15.7 fps: it is bound by the CPU.

Path B's render plus readback alone, without Qt, with the texture and buffer
kept between frames (190 frames each):

| Backend | Size | Lines | Median | p95 |
|---|---|---|---|---|
| GL, GT 730 | 1366 × 696 | 10,000 | 20.3 ms | 25.5 ms |
| GL, GT 730 | 1366 × 696 | 100,000 | 16.0 ms | 30.1 ms |
| GL, GT 730 | 1920 × 1080 | 100,000 | 27.3 ms | 39.9 ms |
| GL, GT 730 | 1920 × 1080 | 1,000,000 | 65.4 ms | 69.9 ms |
| Vulkan, lavapipe | 1920 × 1080 | 100,000 | 71.1 ms | 84.2 ms |

These times are noisy (nouveau runs Kepler at low clocks), and 10,000 lines
cost as much as 100,000. The readback through OpenGL on nouveau has a fixed
cost of roughly 15–25 ms per frame, larger at full HD. Creating the texture
and buffer every frame made little difference.

For comparison, today's CPU rasterizer on the same machine (`cargo bench -p
arq-render --bench raster`, 100,000 entities at 1920 × 1080): render all
97.7 ms, a 1 % view 13.5 ms, building the scene from the document 76.1 ms.

## What it shows

> **Correction (2026-11-09):** path B's times below leave out the trace, the
> CPU work that turns the drawing into segments each frame (30 ms for this
> drawing on this machine). With it, the offscreen renderer takes 86 ms for
> the whole drawing against the CPU's 99, and 44 ms for a 1 % view against
> 13. The direct path was chosen instead ([ADR
> 0007](../architecture/adr/0007-graphics-api.md)).

- **Both paths work on X11** with the GT 730 and with lavapipe: the lines on
  screen, resizes, no errors. The risk of ADR 0007 is retired for X11 on
  the reference machine.
- **Path B is 3–6 times faster than the CPU rasterizer** for a whole
  100,000-entity drawing (16–27 ms against 98 ms), and it keeps today's
  painting unchanged. The viewport paints the drawing's image, then its
  overlays (grips, rubber bands, snap markers, the crosshair) with
  `QPainter`, and redraws the image only when the view or the drawing
  changes, not when the cursor moves.
- **Path A is faster still** (60 fps held by vsync at 100,000 lines;
  1,000,000 at 28 fps against path B's 21). But `QPainter` cannot paint on a
  `WA_PaintOnScreen` window, so the overlays would have to be drawn by wgpu
  too.
- **Lavapipe is no better than the CPU rasterizer** (63–71 ms against 98 ms
  at 100,000 lines, while taking the CPU's cores). This supports ADR 0013
  D5.8: the CPU rasterizer is the fallback before a software adapter.

## Not covered

- **Wayland:** the machine runs Cinnamon on X11 and has no Wayland
  compositor. Qt's `wl_surface` comes through its native interface, not
  `winId()`.
- **Windows** (DX12, Vulkan, GL into an `HWND`) and **macOS** (Metal into an
  `NSView`).
- **High-DPI:** the only monitor here has a device pixel ratio of 1.0. The
  spike sizes its surfaces in physical pixels, as the viewport does.
- **Real drawings:** arcs, texts, colours, line widths and selection
  highlight, which the product renderer will draw.
