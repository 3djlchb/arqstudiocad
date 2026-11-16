# Constant polyline widths in DXF: codec correction

Found by the [rectangular ARRAY comparison](array-rectangular-windows.md#our-dxf-polyline-widths)
in ProgeCAD on 2026-11-15; corrected the same day in our AcadRust fork
(ADR 0017 D2), at the maintainer's request.

## The defect

The DXF writer wrote an LWPOLYLINE's groups 40 and 41 at every vertex. For a
polyline with a constant width, every vertex width is 0, so the entity held
group 43 = 0.25 and, at each vertex, 40 = 0 and 41 = 0. The DXF reference
does not use group 43 when 40/41 are given:
- **ProgeCAD 2026** read every vertex of such a polyline at width 0.
- **Our own reader** takes group 43, so our round trips could not show the defect.

**What the other writers do:**
- **Our DWG writer** writes vertex widths only when some vertex has one (its widths flag).
- **Our R12 writer** writes them only for the vertices that have them.
- **ProgeCAD's own DXF** writes group 43 alone for a constant width.

## The correction

| | |
|---|---|
| Fork commit | `81b8de1` |
| Immutable tag | `arqstudiocad/on-ea66abe-lwpolyline-dxf-width` |
| Base | stacked on `f10cc74`, the previous pin; every earlier correction kept |
| Red test | `9a6d8ef` |

The writer now writes groups 40/41 only when some vertex has a width of its own, as the DWG writer encodes them. A constant width is group 43 alone. A polyline with vertex widths keeps 40/41 at every vertex.

## Validation

**Fork.**
- `tests/lwpolyline_dxf_width.rs`, three tests:
  - a constant width is written as 43 alone and read back;
  - vertex widths are written at every vertex and read back;
  - no width writes no width groups.
- On the previous commit the first and third failed, as the red commit shows.
- The full default suite passes: 202 groups, 2,458 tests.
- `cargo fmt` reports 49 formatting differences in files this correction does not touch; they predate it and are left as they were. Both changed files are formatted.

**Application.** `dxf_width_tests.rs` in `arq-document`: a polyline drawn with a constant width and saved as DXF holds group 43 alone, and keeps it when read back and saved again. On the previous pin it failed with the zero vertex widths; on `81b8de1` it passes.

**ProgeCAD.** `tools/progecad_array_ours.py` wrote the seven ARRAY cases again with the new pin, as DWG and as DXF. ProgeCAD 2026 reopened all fourteen files.
- **All match ProgeCAD's own arrays,** entity for entity, layers, colors and widths included.
- **Case C4's DXF, the one that differed before,** now reports width 0.25 at every vertex, as ProgeCAD's own drawing does.
