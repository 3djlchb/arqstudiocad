# World-coordinate indicator

Maintainer-approved scope, 2026-11-17: show positive X, Y and Z in red,
green and blue, following OpenCADStudio as a visual reference. The maintainer
specified anchoring the overlay at the universal origin (0, 0, 0).

## Behavior and boundary

- Always shown in model viewports, including an empty drawing. The indicator
  follows the projected world origin through pan, zoom and resize; it is not
  moved to a corner when that origin leaves the viewport.
- Arrows and X/Y/Z labels follow all ten preset views. The longest projected
  arm is 38 logical pixels, independent of zoom and device pixel ratio.
  Foreshortening is preserved. An axis seen end-on has a small ring and a
  label instead of an arrow of zero length.
- Colors are X (230,56,56), Y (56,217,56), Z (56,115,230), fully opaque.
  On a light background Y uses (56,153,56) for contrast.
- Rust supplies the existing view basis and the world origin's physical
  screen position through the read-only `CadView.world_axes` property.
  Python lays out screen glyphs and translated labels. The shared Overlay
  description feeds both QPainter and wgpu (ADR 0034 D4).
- The indicator is presentation data: no entity, selection target, snap,
  extent, history entry, dirty flag or file-format setting is introduced.
  This does not implement editable UCS, UCSICON options or a view cube.
- Original project implementation; reference provenance is in THIRD_PARTY.md.

## Verification

Tests in `tests/python/test_world_axes.py` cover all preset bases, pan/zoom,
origin placement, DPI 1/2, colors on dark/light backgrounds, end-on axes,
CPU painting and conversion of the same overlay to GPU geometry/text.
Sabotage verification completed: reversing the screen-up projection failed
all ten orientation cases; relocating the indicator to a corner failed both
DPI cases; changing X to yellow failed both background/color cases; omitting
the indicator failed the real CPU/GPU screenshot comparison on `world X`.
Exact saved bytes were restored, native sources touched and `uv sync` rebuilt
the extension. The restored tests passed. CPU screenshots were inspected in
Top and SW isometric views. The real overlay comparison passed on Vulkan;
OpenGL did not attach on this NVIDIA development host and is not claimed as
validated here.

The maintainer approved a development-machine diagnostic: CPU viewport
800 x 600, 100 synthetic lines, Top and SW isometric, five warm-ups and
100 timed overlay construction + painting repetitions per view. Record
exact base/candidate commits, equal fixtures/settings and environment.
Reference-machine acceptance remains separate; run
`tools/reference_machine_report.sh` and `uv run python tools/gpu_viewport_check.py`
on the i7-2600/GT 730 before asserting minimum-machine frame performance.
ProgeCAD is not required.
