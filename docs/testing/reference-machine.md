# Reference machine measurements

The reference machine is the project's performance **floor**
([ADR 0007](../architecture/adr/0007-graphics-api.md),
[ADR 0013 D5.8](../architecture/adr/0013-application-shell.md)): what must
run well, runs well there, and every faster machine scales up from it.
Its measurements, kept here, are the shared reference every development
machine compares against.

## The machine

| Part | |
|---|---|
| Graphics card | GeForce GT 730 (GK208B, `10de:1287`, chip NV106) on nouveau |
| Graphics stack | Mesa 25.0.7 (`25.0.7-2+deb13u1`): OpenGL 4.3, OpenGL ES 3.2; Vulkan instance 1.4.309 with llvmpipe only (software) |
| Processor | Intel Core i7-2600 (2011), 4 cores / 8 threads, AVX but no AVX2 or FMA |
| Memory | 16 GB |
| System | LMDE 7, kernel 6.12.118 since 2026-11-22 (6.12.117 from 2026-11-11, 6.12.116 before), X11 (Cinnamon) |
| Screen | 1920 × 1080 at 60 Hz over HDMI since 2026-11-19 (a new monitor); 1366 × 768 until then |

The application draws there with OpenGL ES on the GT 730; Vulkan and the
software backend are both llvmpipe, passed over.

## How it is measured

Each dated folder below holds the text reports of one session; no images
and no drawings. The screen's size counts: the desktop composes every
frame of it on the same card, and at 1920 × 1080 the GPU viewport's 95th
percentile is about twice what it is at 1366 × 768 (2026-11-19,
[resolution](reference-machine/2026-11-19/resolution-and-startup.txt)).
`reference_machine_report.sh` records the screens and the mode in use. The desktop's load counts too: with other programs drawing, the GPU
viewport's 95th percentile at 1920 × 1080 goes from about 17 to about
30 ms (2026-11-22, [desktop load](reference-machine/2026-11-22/gpu-desktop-load.txt)).
Measure with the desktop idle, and the report records which drawing
programs were open and how many nouveau GPU traps the boot had: traps in
Xorg can leave the X server refusing new windows until a reboot. Every run of the application uses a fresh
`XDG_CONFIG_HOME`, so no saved setting or crash sentinel counts, and
`LANG=C.UTF-8`. Run `uv run python tools/fetch_codec_samples.py` first, or
the tests that read the public samples skip.

1. **GPU information:** `glxinfo -B` and `vulkaninfo --summary`, without
   the `deviceUUID` and `driverUUID` lines.
2. **`bash tools/reference_machine_report.sh`,** without `ARQ_CORPUS_DIR`:
   build, Rust and Python tests with `ARQ_GPU_TESTS=require`, the
   self-check, and timings on generated drawings of 10,000 and 100,000
   lines.
3. **`uv run python tools/gpu_viewport_check.py`,** the mouse left still:
   the probes, the viewport's pan and zoom on the GPU and the CPU (median
   and 95th percentile), and the GPU's overlays against the CPU's.
4. **The cards' survey:** each `--gpu-adapters <api>` and
   `--gpu-probe <api> --adapter N` child process, and
   `renderer.survey(renderer_settings()[2:])`, timed 5 times, the median,
   with the CPU they take.
5. **Startup:** a script that builds the application as `application.run`
   does, 5 starts, the median of: the window shown, the renderer chosen,
   the card survey done, and a generated 100,000-line DXF asked for as the
   window shows and opened by the same background path as OPEN. The
   [startup timing](reference-machine/2026-11-11/startup-timing.txt) of
   2026-11-11 gives the drawing's generator and the method in full.
6. **Options › Graphics,** opened in the application: what "This session"
   says, which cards and APIs "Choice" offers, and why the rest cannot be
   chosen.
7. **Opening in the idle application:** the application built as in step
   5, left idle, then the same generated DXF opened five times by OPEN's
   path; the medians from the ask to `DocumentOpened`, to OPEN's end and
   to the first frame, and the GUI thread's cProfile over it. The
   [method](reference-machine/2026-11-15/startup-timing.txt) of 2026-11-15
   gives it in full.
8. **Criterion benchmarks:** `cargo bench --locked -p arq-core --bench
   query` and `cargo bench --locked -p arq-render --bench raster`, their
   fixtures generated in the benchmark; a change's own benchmark is run
   on its base too, the same harness copied in, for a ratio
   ([2026-11-15](reference-machine/2026-11-15/benchmarks.txt)).
9. **A change's own scenario:** when a pull request's development
   diagnostic left the floor's check pending, the same scenario at the same
   base and candidate commits, each built in a copy of its own, alternating
   them, with a monitor for heavy processes; and, for commands drawn as the
   cursor moves, cursor frames in the main window on the CPU and the GPU
   ([2026-11-19](reference-machine/2026-11-19/operations.txt)).

## Adding a row

Measure on the reference machine with the steps above. Put the reports in
`docs/testing/reference-machine/<date>/`, without personal data: `~` for the
home folder, no host name, UUIDs, addresses or serial numbers, and no file
names of private drawings. Add one row to the table, the newest last.

## Comparing another machine

A development machine runs steps 2, 3 and 5 on the same commit. Its figures
are not compared one by one with these: a faster machine is expected to be
faster. Compare **ratios** instead (GPU against CPU frame, a drawing opened
at startup against opened alone, `main` against the version before a
change). When a change makes a ratio worse on the development machine,
measure it here before it ships: only this machine says whether the floor
still holds.

## Measurements

Times in ms. Window, survey and open are medians of 5 starts, measured from
the process's start; "open at startup" is a 100,000-line drawing asked for
as the window shows. Pan and zoom are a 1200 × 650 viewport with 100,000
entities, median (95th percentile).

| Date | Commit | Mesa / OpenGL | Start → window | Start → survey done | Open at startup: main / before #161 | Pan GPU | Zoom GPU | Pan CPU | Zoom CPU | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| 2026-11-10 | `a2b6a72` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 16.8 (20.4) | 16.9 (21.3) | 44.1 (50.8) | 60.6 (67.0) | ADR 0007's release gate; report 907 passed, 2 skipped; DXF 100k opens in 337 ms |
| 2026-11-11 | `06132ad` | 25.0.7 / 4.3, ES 3.2 | 589 | 905 | 835 / 545 | 16.6 (22.3) | 16.6 (22.4) | 44.5 (49.8) | 61.9 (69.8) | [Reports](reference-machine/2026-11-11/). Opening right at startup is 290 ms slower since #161: the card survey holds one of the 2 background threads. The survey costs 331 ms (three child processes at ~70% of a core). Report 961 passed, 3 skipped; DXF 100k opens in 353 ms |
| 2026-11-15 | `611fd1c` | 25.0.7 / 4.3, ES 3.2 | 599 | 1385 | 565 (with #172; 2026-11-11's 835 gone) | 16.7 (16.8) | 16.7 (16.9) | 43.9 (48.9) | 58.8 (65.1) | [Reports](reference-machine/2026-11-15/). OPEN counts its entities instead of listing them on the GUI thread (81 ms): opening in the idle application, ask to first frame 689 → 582 ms (0.85). Quick TRIM/EXTEND (#188) validated here: 124 ms → 10 µs and 136 ms → 7 µs with 100,000 lines. Report 1094 passed, 3 skipped; DXF 100k opens in 335 ms |
| 2026-11-19 | `992acf8` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 16.7 (25.1) | 16.6 (16.8) | 46.0 (50.1) | 61.1 (67.2) | [Reports](reference-machine/2026-11-19/). The floor checks left pending by #192, #205, #208 and #209, at their own commits: STRETCH edit 0.361 → 0.360 ms (0.997), its preview query 0.117 ms and 2.7-3.5 ms more per CPU cursor frame with 100 lines stretched, within the frame; closed BREAK: open 0.706 → 0.716 ms (1.014), closed 0.710 ms; ID capture in SW isometric +0.14 ms; world XYZ overlay +0.28 ms a CPU frame. On the GPU every cursor frame stays at the display's 16.4-16.7 ms. The GPU viewport after #203: overlays 0 pixels apart. Report 1199 passed, 3 skipped; DXF 100k opens in 349 ms |
| 2026-11-19 | `ab94cc8` | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | — | — | Same session, after #211. Rectangular ARRAY (#191): the diagnostic 56.1 → 40.6 ms (0.723); in the main window 10,000 copies on screen in 98 ms, 100,000 in 609 ms, then panned at 16.6 ms on the GPU and 33 ms on the CPU ([array](reference-machine/2026-11-19/array.txt)). STRETCH's remaining cases ([stretch cases](reference-machine/2026-11-19/stretch-cases.txt)): zoom/pan during the prompt +3 ms on the CPU; 100 bulged polylines 8.8 → 24 ms a CPU cursor frame (12 ms once the overlay is painted without a QPointF a point); a 402-corner crossing polygon 4 → 252 ms (242 on the GPU), its validation repeated once per selected entity (fixed by #213, which validates it once: 16 ms on the CPU and 11 ms on the GPU) |
| 2026-11-19 | `c19705b` | 25.0.7 / 4.3, ES 3.2 | 577 | 1299 | 569 | 16.6 (28.3) | 16.7 (31.9) | 44.6 (50.4) | 58.6 (65.9) | [Resolution and startup](reference-machine/2026-11-19/resolution-and-startup.txt). The new monitor, 1920 × 1080: the GPU's 95th percentile 23.6-31.9 ms at that size, 16.8-16.9 ms switched to 1368 × 768, medians the refresh at both; the CPU and startup the same at both sizes. Startup against 2026-11-15: window about 570 ms, survey about 1280 ms, opening in the idle application, ask to first frame, 582 → about 520 ms (a rewritten harness) |
| 2026-11-20 | `763c69c` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 18.0 (32.2) | 17.9 (32.0) | 47.3 (52.1) | 61.1 (67.1) | [Viewport report](reference-machine/2026-11-20/viewport.txt). Medians of three passes at 1920 × 1080 and 60 Hz. The GPU took 2.63 times less time to pan and 3.41 times less time to zoom by median repaint time. The overlay comparison found zero unmatched pixels in all three passes. |
| 2026-11-20 | `c19705b` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 28.1 (51.8) | 29.5 (47.2) | 46.7 (53.5) | 62.6 (69.1) | [Alternating comparison](reference-machine/2026-11-20/gpu-frame-timing.txt). Three passes per commit in one X11 session at 1920 × 1080 and 60 Hz. High GPU frame times reproduced on both commits. |
| 2026-11-20 | `763c69c` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 30.4 (49.2) | 29.4 (52.3) | 47.8 (53.7) | 62.7 (70.2) | Same alternating comparison as the preceding row. All six overlay comparisons found zero unmatched pixels. In a separate instrumented pass, median time inside `queue.present` was about 98% of median Rust frame time among frames exceeding 25 ms. |
| 2026-11-20 | `89f6ebd` | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | 48.5 (55.1) | 63.8 (72.6) | [CPU stage timings](reference-machine/2026-11-20/cpu-frame-timing.txt). Medians from three instrumented passes. Tiny-skia took about 32.5 ms for pan and 39.1 ms for zoom. Qt took less than 1 ms in each phase. |
| 2026-11-20 | `763c69c` | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | — | — | [Large drawing open](reference-machine/2026-11-20/large-drawing-open.txt). For a generated 100,000-line DXF, median request to first paint was 573.5 ms with idle OpenGL, 513.1 ms with idle CPU and 760.3 ms when requested at startup. Opening the document on the worker took about 372 ms in the idle sessions. |
| 2026-11-22 | `0da68ac` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 16.6 (16.8–17.3) | 16.6 (16.8–17.2) | 43.3–45.5 (48.9–53.7) | — | [Desktop load](reference-machine/2026-11-22/gpu-desktop-load.txt). Kernel 6.12.118. At 1920 × 1080 with the desktop idle every GPU frame is the refresh; with the usual programs open (Firefox, VLC) the GPU's 95th percentile rises to 27–32 ms and the CPU's does not. The surface offers only Fifo, and frame latency 1 or 2 changes nothing: no code change. nouveau GPU traps in Xorg on earlier boots explain the days the X server refused new windows |
| 2026-11-22 | `957366b` | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | — | — | [Open phases](reference-machine/2026-11-22/open-phases.txt). Opening a 100,000-line drawing: the engine's decode is 74% of a DXF (309 ms) and 55% of a DWG (133 ms); our document 29–30 ms; the spatial index 76–83 ms. Sizing the index's maps once and sharing the file's bytes: the index 80.6 → 67.2 ms (0.834), the whole DWG open about 0.95 |
| 2026-11-22 | `cc116a7` (fork) | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | — | — | [Engine DXF read](reference-machine/2026-11-22/engine-dxf-read.txt). Profiled with perf: the engine's DXF reader copied each value line twice and searched it three times. Fixed in the fork: reading the 100,000-line DXF takes 14% fewer instructions (8.84 → 7.61 G for five reads); the open from Python about 390 → 344 ms with #236. The DWG read has no such single cause (its bit reader) |
| 2026-11-23 | `cf0db83` + #248 | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | 18.5 (22.3) | 29.0 (32.5) | [CPU hairlines](reference-machine/2026-11-23/cpu-hairlines.txt). tiny-skia ran its whole pipeline for every two pixels of an antialiased line; its hairline is now ported with the blend written straight into the pixels, bit for bit the same. Alternated with the base (45.2 and 59.9 ms): pan 0.41, zoom 0.48; Criterion's render all 100.1 → 44.8 ms |
| 2026-11-23 | `763c69c`, `bc1f6a9` | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | — | — | [STRETCH partial arcs](reference-machine/2026-11-23/stretch-partial-arcs.txt), the acceptance #200 asked for, base `626a78f`. 100 arcs with one end selected preview at 5.3 ms a CPU cursor frame and at the display's rate on the GPU; whole arcs and lines within 3.1% of the base; the commit moves only the selected end and undo restores. Accepted |
| 2026-11-23 | `bc1f6a9` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 16.6 (16.7) | 16.6 (16.7) | 10.7 (18.0) | 3.8 (5.8) | [Path ARRAY](reference-machine/2026-11-23/path-array.txt), the acceptance #220 left pending. The diagnostic: ARRAY / grouped COPY-ROTATE 0.507 aligned, 0.748 kept (0.451 and 0.692 on the development host). In the main window 10,000 copies on screen in 129-141 ms and 100,000 in 613-697 ms; pan and zoom columns over the 100,000. Accepted |
| 2026-11-23 | `b8014d0` (fork) | 25.0.7 / 4.3, ES 3.2 | — | — | — | — | — | — | — | [Engine DWG read](reference-machine/2026-11-23/engine-dwg-read.txt). Profiled with perf: the DWG reader's bounds check was never inlined (8%) and AC18 pages were decompressed a byte at a time (5%). Fixed in the fork: 14% fewer instructions reading the 100,000-line DWG, the read 128 → 122 ms, the open from Python about 219 → 211 ms. The rest is spread out: per-entity allocations, page faults, the builder's passes |
| 2026-11-26 | `3abe09b` | 25.0.7 / 4.3, ES 3.2 | — | — | — | 16.6 (16.7) | 16.6 (16.8) | 11.6 (18.6) | 8.0 (8.9) | [Polar ARRAY and ARRAYRECT's preview](reference-machine/2026-11-26/arrays.txt), the acceptances #218 (polar) and #222 (the preview) left pending. Polar diagnostic 0.434 rotating, 0.736 kept (0.393 and 0.704 on the development host); 9,900 copies on screen in 123-130 ms and 99,000 in 657-707 ms, pan and zoom columns over the 99,000. The orientation preview of a 10 x 10 array of 100 lines in a 100,000-line drawing: 27 ms a CPU cursor frame (p95 28.7, limit 33), the display's rate on the GPU. Accepted |

## Renderer and safe mode checks

- 2026-11-20, `763c69c`. Automatic used OpenGL on the GT 730. Safe mode used the CPU with default settings and with OpenGL saved in Preferences. Both paths opened, edited, saved and reopened the generated DXF. [Validation report](reference-machine/2026-11-20/renderer-safe-mode.txt)
