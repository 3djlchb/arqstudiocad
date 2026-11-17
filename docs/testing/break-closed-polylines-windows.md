# BREAK on closed polylines: Windows comparison

Milestone 02.14. Our initial BREAK refused closed paths until a Windows comparison
showed how they behave ([design](../design/break-command.md); maintainer,
2026-11-15). This page is that comparison. The maintainer approved the rule
it shows the same day, ProgeCAD's own, so the same BREAK gives the same
drawing in both programs ([rule](#rule-approved-2026-11-15)).

## Observations (2026-11-15, Windows agent)

**Environment.**
- ProgeCAD 2026 Professional ESN (`ACADVER` 19.0i) on Windows 11 Pro.
- The development laptop, with the maintainer's permission to automate ProgeCAD.
- A blank drawing from ProgeCAD's own `pcadISO (Metric).dwt`: `INSUNITS` 4, mm.
- World UCS, Top view, `OSMODE`, `ORTHOMODE` and `SNAPMODE` 0 for the run and restored after it.

**Method.**
- `tools/progecad_command_compare.lsp` builds each closed LWPOLYLINE with ActiveX.
- It runs ProgeCAD's own `BREAK`: the polyline is selected as an entity, then `F` (First point), the first point and the second point.
- It writes every vertex with its bulge and widths, the closed flag, elevation, constant width and handle.
- One case at a time on a cleared model space.

**Saving and reopening.** Each case was saved before and after as DWG 2018 (AC1032) and DXF 2018, and reopened as described in [the STRETCH comparison](stretch-arcs-windows.md#reopening).

The square is (0,0), (10,0), (10,10), (0,10), closed, in that (counterclockwise) order unless the table says otherwise. A point's **parameter** p is its place along the vertices: vertex i is p = i, and the middle of segment i is i + 0.5.

| Case | Points (first → second) | Parameters | After: one open LWPOLYLINE, same handle |
|---|---|---|---|
| B1 | (10,5) → (5,10) | 1.5, 2.5 | (5,10), (0,10), (0,0), (10,0), (10,5) |
| B2 | (5,10) → (10,5) | 2.5, 1.5 | (5,10), (0,10), (0,0), (10,0), (10,5): as B1 |
| B3 | (0,5) → (5,0), around vertex 0 | 3.5, 0.5 | (0,5), (0,0), (5,0) |
| B4, clockwise square (0,0), (0,10), (10,10), (10,0) | (10,5) → (5,10) | 2.5, 1.5 | (10,5), (10,0), (0,0), (0,10), (5,10) |
| B5 | (10,5) → `@` | 1.5, 1.5 | (10,5), (10,10), (0,10), (0,0), (10,0), (10,5): opened, nothing removed |
| B6, a semicircle on top: (0,0), (10,0), (10,5) with bulge 1, (0,5) | (10,2) → (5,10), the arc's top | 1.4, 2.5 | (5,10) bulge 0.4142135624, (0,5), (0,0), (10,0), (10,2) |
| B7, constant width 0.5 | (10,5) → (5,10) | 1.5, 2.5 | as B1; every segment 0.5 wide |
| B8 | (2,0) → (8,0), one segment | 0.2, 0.8 | (8,0), (10,0), (10,10), (0,10), (0,0), (2,0) |
| B9 | (8,0) → (2,0) | 0.8, 0.2 | as B8 |
| B10 | (10,5) → (10,10), a vertex | 1.5, 2 | (10,10), (0,10), (0,0), (10,0), (10,5) |
| B11 | (10,0) → (0,10), two vertices | 1, 3 | (0,10), (0,0), (10,0) |

**What these show:**
- **What is removed.** The span between the smaller and the larger parameter, along the vertices. The removed span never contains vertex 0: in B3 the points around vertex 0 remove the long side, from 0.5 to 3.5.
- **The order of the points does not matter** (B1 and B2, B8 and B9). The winding does not matter either, except through the parameters (B4).
- **The result** is the same entity, opened: one open LWPOLYLINE with the source's handle. It starts at the larger parameter's point, runs along the vertices through vertex 0, and ends at the smaller parameter's point.
  - A break point on a vertex adds no vertex (B10, B11).
  - A cut arc keeps its circle, with its bulge for the part left (B6: a quarter turn, tan(90°/4) = 0.4142135624).
  - Widths stay with their segments (B7). The last vertex of the open polyline has width 0, and `ConstantWidth` then reads 0.
- **A single point** (`@`, or two equal parameters) opens the polyline there and removes nothing. The point is both its first and its last vertex (B5).

### Rule approved (2026-11-15)

For a closed LWPOLYLINE given to BREAK with a first and a second point:

1. Project both points onto the polyline and take their parameters p₁ and p₂ along the vertices, from vertex 0.
2. Remove the span from min(p₁, p₂) to max(p₁, p₂). The order of the points does not matter.
3. Keep the rest as the same entity, now open, from max to min through vertex 0. Add vertices only where a break point is not already a vertex. Keep each kept part of a bulged segment on its circle, and keep each segment's widths.
4. With p₁ = p₂, open the polyline at that point without removing anything.

This differs from our BREAK on circles, which removes the counterclockwise span from the first point to the second. ProgeCAD's closed polylines follow the parameters instead. The table's rows are the expected coordinates for the Rust tests.

### A ProgeCAD DXF defect

Reopened in ProgeCAD, 43 of the 44 BREAK files matched what was recorded when each case ran ([reopening](stretch-arcs-windows.md#reopening)). The exception is B7 after the break, saved as DXF: its widths were gone.

**Before the break,** the width was constant. ProgeCAD's DXF holds it as group 43 (0.5), and it reopened with that width.

**After the break,** the widths are per vertex: 0.5 on every segment, 0 on the open polyline's last vertex. ProgeCAD's DXF writer then writes neither group 43 nor the vertices' groups 40/41, so the widths are lost. The same case saved as DWG kept them.

So a ProgeCAD DXF cannot serve as the reference for polylines with varying widths: compare those through DWG. For our own writer, groups 40/41 must be written whenever the widths are not all equal.

## Implementation regression coverage (2026-11-17)

The approved rule is implemented on `feature/break-closed-polylines`.
Geometry tests use B1-B11's explicit expected vertex order and bulges.
Document/core tests additionally check effective widths, mirrored plan
normals, identity, atomicity, index maintenance and undo/redo. Python tests
save and reopen DWG and DXF and inspect the resulting per-vertex widths,
including the B7 normalization. This does not constitute a new ProgeCAD
run: external observations above remain dated 2026-11-15.
