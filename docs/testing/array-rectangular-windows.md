# Rectangular ARRAY: Windows comparison

Milestone 02.14. The [rectangular ARRAY](../design/rectangular-array.md) was
validated on Linux only; its Windows comparison was pending. This page is
that comparison, made in ProgeCAD on 2026-11-15.

## Method

**Environment.**
- ProgeCAD 2026 Professional ESN (`ACADVER` 19.0i) on Windows 11 Pro.
- The development laptop, with the maintainer's permission to automate ProgeCAD.
- A blank drawing from ProgeCAD's own `pcadISO (Metric).dwt`.
- World UCS, Top view, `OSMODE`, `ORTHOMODE` and `SNAPMODE` 0 for the run and restored after it.

**Both sides, the same nine cases:**
- **ProgeCAD:** `tools/progecad_command_compare.lsp` (`arq:compare-array`) builds synthetic sources and runs ProgeCAD's own `-ARRAY`, Rectangular, with `ARRAYASSOCIATIVITY` 0 (restored afterwards). It saves each result as DWG 2018.
- **Ours:** `tools/progecad_array_ours.py` builds the same sources through our core and runs `rectangular_array` with the same counts, spacings and angle. It saves each result as DWG and as DXF.

**Comparing.** ProgeCAD reopened every file, its own and ours, one per instance, from its command line. It described each entity with the same fields, layer and color included (`arq:dump-full`). The two sides were then compared as multisets of entities: without handles or creation order, and with coordinates to 10⁻⁹.

**Repeatable.** Both tools, run a second time from this branch, gave the same results.

## Results

| Case | Sources | Rows × columns, row / column spacing, angle | ProgeCAD | Ours, DWG | Ours, DXF |
|---|---|---|---|---|---|
| C1 | line (0,0)–(1,0.5) | 3 × 4, 2 / 3, 0° | 12 lines | same | same |
| C2 | the line | 3 × 4, −2 / −3, 0° | 12 lines | same | same |
| C3 | the line | 3 × 4, 2 / 3, 30° | 12 lines | same | same |
| C4 | the line on layer `ARRAY-TEST` in red; a circle; an arc; a closed polyline 0.25 wide | 2 × 3, 4 / 15, 0° | 24 entities | same, layers and colors included | same, except the polyline widths ([below](#our-dxf-polyline-widths)) |
| C5 | the line | 1 × 5, column spacing 3 | 5 lines | same | same |
| C6 | the line | 5 × 1, row spacing 2 | 5 lines | same | same |
| C7 | a closed square at elevation 2 | 2 × 2, 3 / 3, 0° | 4 polylines | same, elevation 2 kept | same |
| C8 | the line | 3 × 3, 0 / 0 | 9 lines, stacked at the source | **refused**: repeated axes need nonzero spacing | refused |
| C9 | the line | 317 × 316 = 100,172 cells | 100,172 entities | **refused**: 2 to 100,000 cells | refused |

C1–C7: **ours is the same as ProgeCAD's**, entity for entity, in DWG and in DXF, except the DXF widths of C4.

## What differs, and why

**Associativity.**
- **ProgeCAD's default** is an associative array: `ARRAYASSOCIATIVITY` is 1, and even `-ARRAY` makes a single block reference.
- **Ours** makes independent copies, as the design chose for this first delivery.
- **With `ARRAYASSOCIATIVITY` 0,** ProgeCAD also makes independent copies and keeps the source with its handle, as ours does. Exploding its associative array gave the same twelve lines as C1.

**Orientation.**
- **ProgeCAD** turns the grid by the drawing's `SNAPANG`; **ours** takes the angle as a parameter of the command.
- At 30° (C3) both turn the grid's axes and not the objects, with the same result: columns along 30°, rows along 120°.

**Zero spacing (C8).**
- **ProgeCAD** stacks nine copies on the source, which leaves duplicates invisible on screen.
- **Ours** refuses, as designed.

**More than 100,000 cells (C9).**
- **ProgeCAD** has no `MAXARRAY` variable and makes all 100,172.
- **Ours** keeps AutoCAD 2021's documented default ceiling of 100,000, as designed.

**Order of creation.**
- **ProgeCAD** creates the copies column by column; **ours** creates them row by row, as designed.
- The handles therefore differ; the geometry does not.

These differences are as designed; this comparison does not ask to change them.

## Our DXF: polyline widths

In C4, the closed polyline 0.25 wide reopened from **our DXF** with its constant width 0.25, but every vertex reported width 0. From our DWG, and from ProgeCAD's own drawing, every vertex reported 0.25.

**The cause is in our DXF writer, not in ARRAY:**
- **Ours** writes group 43 = 0.25 and, at every vertex, groups 40 and 41 = 0.
- **ProgeCAD's DXF** for a constant width writes group 43 only (see [the BREAK comparison](break-closed-polylines-windows.md#a-progecad-dxf-defect), case B7 before the break).
- **The DXF reference** says group 43 is not used when variable widths (40 and/or 41) are given.

So a reader that follows the reference takes our constant-width polylines as having width 0. ProgeCAD's bounding boxes leave widths out, so the run did not show how it draws them.

**This affects every constant-width polyline we save as DXF, not only arrays.** It is fixed in the DXF writer of our AcadRust fork (ADR 0017 D2), which now writes 40/41 only when some vertex has a width of its own, as its DWG writer does ([correction and validation](lwpolyline-dxf-width.md), 2026-11-15).

**The drawings themselves are not committed**, as for the other comparisons: the two tools reproduce them.
