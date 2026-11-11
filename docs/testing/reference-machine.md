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
| System | LMDE 7, kernel 6.12.116, X11 (Cinnamon), 1366 × 768 |

The application draws there with OpenGL ES on the GT 730; Vulkan and the
software backend are both llvmpipe, passed over.

## How it is measured

Each dated folder below holds the text reports of one session; no images
and no drawings. Every run of the application uses a fresh
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
