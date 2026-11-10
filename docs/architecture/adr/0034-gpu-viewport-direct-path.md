# 0034. The GPU viewport: drawing straight into the widget

Date: 2026-11-10 · Status: Accepted (2026-11-09, PR #147)

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

## Implementation notes

### D6.1, presenting (2026-11-10)

- **Precision by region, not by two `f32`.** The geometry kept on the GPU
  covers a region around the view, the view and four more on each side. It
  is clipped to that region in `f64` and kept relative to the region's
  center, so a single `f32` keeps survey coordinates exact. This also solves
  what two `f32` would not: a long line seen 10⁶ times closer has no
  endpoint millions of pixels away to round. Panning out of the region, or
  zooming past the band (×2), builds it again. Tests check survey
  coordinates and a diagonal at 10⁶ zoom against the CPU.
- **`unsafe` stays at the boundary.** Making a surface from native handles
  needs `unsafe`, which the workspace forbids. `arq-python`, which PyO3
  already exempts, makes the surface from Qt's handles. `arq-render`
  receives it through a safe constructor and re-exports `wgpu`.
- **Small things show.** A line that can be shorter than a pixel within
  the band, or a circle or arc smaller than one, is also drawn as a point,
  since a line that short makes no fragment. Without multisampling, every
  segment's end is drawn as a point too, since OpenGL leaves out a line's
  last pixel (the diamond-exit rule), which the CPU draws. One point per
  chord of a curve would light whole areas of a dense drawing, so it is one
  per small shape.
- **Antialiasing is optional and off by default.** 4× multisampling doubles
  a frame on the GT 730 (33 ms instead of 17), so the viewport asks for it
  only when the Preferences of D6.3 do. It is used on hardware adapters
  only: lavapipe drops pixels of multisampled lines (a 5-pixel line drawn as
  2). The equivalence tests check both modes.
- **The comparison found a CPU defect.** tiny-skia, clipping a hairline
  that starts above its canvas, drew it a pixel lower. The CPU rasterizer
  now paints on a canvas 8 pixels larger all round, beyond the trace's
  4-pixel clip, and cuts the view out, so tiny-skia never clips. The cost
  is 6 % on the 100,000-entity drawing (99 → 105 ms) and 14 % on a 1 %
  view (13.4 → 15.3 ms). A regression test keeps it fixed.
- **Errors, not panics.** wgpu's uncaptured errors are collected and
  returned. A failed frame detaches the window, and the viewport goes back
  to the CPU rasterizer in the same widget.
- **Opt-in until D6.3:** `ARQ_GPU_VIEWPORT=opengl` (or another backend)
  makes viewports present with the GPU on X11. The overlays are not drawn in
  that mode until D6.2. The surface is released before Qt destroys the
  native window: on `DeferredDelete`, on quitting, and when the window is
  recreated (`WinIdChange`).

Measured on the reference machine, a viewport of 1200 × 650 on the
100,000-entity drawing (lines, circles and arcs), the median frame while
panning and zooming:

| Path | Pan | Zoom |
|---|---|---|
| CPU rasterizer | 45 ms | 63 ms |
| GPU, no antialiasing | 17 ms (vsync) | 17 ms (vsync) |
| GPU, 4× antialiasing | 32 ms | 33 ms |

At a working zoom the GPU's and the CPU's images are the same shapes in the
same places.

### D6.2, the overlays (2026-11-10)

- **One description.** `arqstudiocad.ui.overlay` describes what is drawn
  over the drawing, in logical pixels and in painting order: strokes (width,
  dashed, closed, crisp), translucent fills and labels. The viewport builds
  it once a frame (`Viewport.overlay()`). The CPU path paints it with
  `QPainter` (`overlay.paint`) and the GPU path hands it to
  `CadView.present`. The old painting methods are gone, and with them the
  chance of the two paths drifting apart.
- **Strokes as triangles.** `arq-render/src/overlay.rs` turns strokes into
  quads on the CPU, a few hundred vertices a frame. Their edges fade over
  half a pixel in the shader, by the distance to the stroke's middle, so
  they are smooth without multisampling. Crisp strokes (the crosshair)
  cover whole pixels. Dashes follow Qt's `DashLine` (four widths on, two
  off), running on across corners.
- **Texts stay Qt's.** Snap names and the drop note are painted by Qt into
  small premultiplied images at physical pixels, then drawn as textured
  quads, so `tr()`, the font and the text direction are Qt's.
- **Selection by outlines, not by the retained geometry.** The selection
  and the highlight are the entities' outlines (`CadView.outlines`), as the
  CPU path draws them. Redrawing the retained segments in another width (D3)
  can come when a measurement asks for it.
- **Ends, not joints.** Without multisampling only the real ends of lines
  and arcs are drawn as extra pixels, not every joint between an arc's
  chords. Those made dense views look heavier than they should.
- **Dense views differ.** Where about one entity falls on each pixel (the
  100,000-entity drawing seen whole), the CPU's antialiasing averages the
  geometry into a grey and the GPU without antialiasing draws whole pixels,
  brighter. Antialiasing (D6.3's preference) makes it grey too. At a
  working zoom, the same selection, highlight, grips, window, drop note and
  crosshair captured on screen from both paths differ in no pixel
  (`test_the_gpu_draws_the_overlays_the_cpu_paints`).

Measured on the reference machine with the crosshair and 200 entities
selected, the median frame of a 1200 × 650 viewport on the 100,000-entity
drawing: 20 ms panning and zooming on the GPU, 54 ms panning and 71 ms
zooming on the CPU.

### D6.3, choosing and falling back (2026-11-10)

- **Chosen without waiting.** `arqstudiocad.renderer.plan` decides at once
  whether a GPU may be tried, from the Renderer setting, `--safe-mode`, the
  crash sentinel and the platform (X11 until D6.4), and which backends to
  probe. `probe` then runs them on the window's background worker, each in
  a child process (ADR 0007), and takes the first **hardware** adapter;
  lavapipe, llvmpipe and WARP never draw a viewport. The viewports draw on
  the CPU meanwhile and switch when the answer comes (`Viewport.set_gpu`),
  about a second after the window shows on the reference machine, where
  the two probes take 0.3 s.
- **On by default.** Renderer is Automatic unless chosen otherwise:
  the GPU draws wherever a hardware adapter passes. `ARQ_GPU_VIEWPORT`
  stays as a developer's override that skips the choice.
- **The crash sentinel** (`gpu-starting`, beside the settings in portable
  mode, otherwise in the application's local data folder) is written
  before a viewport attaches the GPU and removed at its first frame, when
  the GPU stops cleanly, and on quitting. Found at the next start, it keeps
  the CPU and says why. Choosing a renderer in Options clears it and tries
  again.
- **Options › Display › Graphics:** Renderer (Automatic, CPU, or each
  hardware backend of the platform) and antialiasing, with a line saying
  what draws now. A change applies at once, without restarting.
- **Told when there is news.** The command window says what draws at start
  when it is the graphics card ("Drawing with the graphics card: NV106
  (OpenGL).") or when the CPU draws for a reason worth knowing: safe mode, a
  past crash, no card passing the check. When the GPU fails while drawing,
  it says every viewport draws on the CPU until the next start. Where the
  CPU is expected (another platform until D6.4) or chosen, nothing is said
  at start; Options › Graphics says it in every case.

### D6.4, the other platforms (2026-11-10)

- **Windows and macOS present.** The surface is made from the viewport's
  `winId()`: an `HWND` on Windows (DirectX 12, Vulkan or OpenGL), an
  `NSView*` on macOS, to which wgpu gives a Metal layer. The window system
  comes from Qt's platform (`xcb`, `windows`, `cocoa`). CI builds and tests
  it on Windows and macOS, but its runners have no real graphics card: only
  WARP and llvmpipe, which the probe refuses on purpose.
- **Automatic waits for a try on real hardware** (maintainer, 2026-11-10).
  Where the graphics card has not yet been seen drawing (`renderer.VALIDATED`,
  X11 alone so far), Automatic keeps the CPU without saying so at start. A
  backend chosen in Options › Graphics is probed and used, with the crash
  sentinel and the fallback. A platform joins `VALIDATED` once its report
  is recorded here.
- **Wayland runs on XWayland.** PySide6 gives Qt's `wl_display` but not a
  window's `wl_surface`, so in a Wayland session with an X server the
  application starts Qt on `xcb` before the window exists
  (`application.qt_platform_for_gpu`), unless the CPU draws, safe mode is
  on, or the user chose a platform with `QT_QPA_PLATFORM`.
- **`tools/gpu_viewport_check.py`** is the check to run on a machine to try.
  It probes every backend, then opens a real viewport with each hardware
  backend that passes and with the CPU, times panning and zooming the
  100,000-entity drawing, and compares the overlays' pixels with the CPU's.
  It writes `target/gpu-viewport-report-<date>.txt`. On the reference
  machine: OpenGL on the GT 730, 21.7 ms panning and 22.5 ms zooming
  against the CPU's 48.0 and 65.2, the overlays the same to the pixel.

**Wayland, tried (2026-11-10)** in weston 14 nested in the reference
machine's X11 desktop, with XWayland:

- In a Wayland session with an X server, the application started Qt on
  `xcb` by itself and chose the GT 730. Its first frame disarmed the
  sentinel, and the command window said so.
- Captured by `weston-screenshooter`, the composed screen showed the
  overlay scene as the CPU paints it, with no pixel apart under the tests'
  comparison. Panning changed what is on screen.
- Without an X server, Qt ran on Wayland and the CPU drew, quietly.
- Reading a window's pixels from X gives nothing under XWayland, which
  composes nothing, so the tests that compare captured windows skip in a
  Wayland session; the composed screen was checked with weston instead.
- weston crashed more than once ("double free"). The main window alone
  brings it down on XWayland, with the CPU drawing as much as the GPU: a
  weston bug the GPU viewport does not cause, to check under GNOME's and
  KDE's compositors.

Still to try: Windows on the development laptop (GTX 1050 Ti, UHD 630;
DirectX 12, Vulkan, OpenGL), and macOS when a Mac is at hand.

