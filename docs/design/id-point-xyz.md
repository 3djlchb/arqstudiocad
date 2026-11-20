# IDPOINT: read-only world XYZ identification

Maintainer-approved scope, 2026-11-17: ID/IDPOINT (Spanish IDPUNTO) must report
Z as well as X/Y, for typed coordinates and exact compatible model references,
including isometric views. No ProgeCAD is required for internal validation.

## Input and capture

- `x,y,z` preserves all three finite coordinates; `x,y` and existing absolute
  polar input identify the world XY plane, Z=0. Invalid or non-finite Z is
  rejected and the prompt remains active. ID has no previous-point base.
- A dedicated spatial flag on point prompts/events is used only by ID.
  Other commands retain XY input, ORTHO, snaps and editing behavior.
- END, MID, CEN and QUA capture exact compatible geometry: sloping LINEs,
  POINTs, CIRCLEs/ARCs in their OCS, lightweight polyline vertices and curved
  segments at their elevation, visible nested block contents through their
  full affine placements, and topological vertices of translated ACIS bodies.
  Solid vertices use END; isolines and sampled wireframe points are never
  treated as geometric reference points. Unsupported kinds contribute none.
- Running snaps and one-shot overrides are respected. ID explicitly refuses
  unsupported INT/PER/TAN/NEA overrides; general 3D intersections and
  cursor-dependent curve snaps are future work. Center can be captured by
  pointing at its curve, as before. Off/frozen/invisible geometry is excluded.
- Reference points are compared in physical screen pixels. The closest wins;
  coincident screen positions prefer the point nearest the viewer. The snap
  marker is projected at the actual XYZ height, on both overlay painters.
- A free click meets the world XY plane, Z=0. In Front/Back/Left/Right, a
  geometric reference is still capturable; a free point cannot meet the
  edge-on XY plane and remains refused, asking for typed coordinates.
- The result uses the existing four decimals, now for X/Y/Z. No entities,
  history, drawing settings or dirty state change. Esc clears the spatial
  prompt so subsequent 2D input remains unaffected.

## Layers and candidate query

The document adapter computes exact reference points from entity data and
owned B-rep topology. Its existing block/visibility traversal is reused;
reference-mode traversal transforms exact points rather than deforming a
circle record under a nonuniform block scale. Codec types stay inside
arq-document.

`CadDocument.query_point_ray` intersects the infinite sight line's XY
projection with the spatial-index root envelope, expanded by the aperture.
Top is the existing small indexed box. Other directions query 64 consecutive
conservative rectangles along the clipped line and deduplicate candidates.
This is an index query, not 64 tessellation segments: no approximation removes
a possible point. Non-finite intermediate bounds fall back to all indexed
entities. Exact XYZ and final screen-distance filtering happen afterward.
This avoids relying on projected tessellated-curve bounds at high zoom, or
tracing every entity on each cursor move. Broad candidates can remain numerous
for dense drawings, very long entities or large apertures.

The read-only `CadView.snap_point3` binding returns the snap kind, exact world
XYZ and physical screen position. No persistent API state or new dependency
is introduced. ADR 0030's general plan-only editing snaps remain unchanged.

## Verification and performance

Rust tests belong to arq-document (exact source points/blocks/visibility and
solid topology) and arq-core (conservative ray candidates and invalid input).
Python tests cover typed XYZ in English/Spanish, invalid Z, capture in Top,
SW isometric and Front, marker projection, high zoom, overlapping solid
vertices, free XY clicks and cancellation. Existing ID tests now expect Z=0
for planar points. Sabotage verification removed Z from exact source points, emptied the ray
candidates, replaced XYZ parsing with a constant point, dropped Z from the
click signal, changed a free click to Z=7 and corrupted the unsupported-snap
message. Each affected test group failed as expected. Exact saved source bytes
were restored and Rust dependencies touched before `uv sync` rebuilt the
extension. Python bytecode caches for restored modules were invalidated too,
so same-second/same-size mutations could not survive restoration.

The maintainer approved a development diagnostic of 100 captures in Top and
SW isometric on the same synthetic 100000-line drawing, comparing the former
XY query with the XYZ query at exact recorded commits. It measures query
cost, not whole-frame latency or minimum-machine acceptance. The reference
host must check cursor interaction and redraw with
`tools/reference_machine_report.sh` and `uv run python tools/gpu_viewport_check.py`.

## Delivery evidence

Integrated with main at `6f9379d` (MATCHPROP): 931 Rust tests, 1181 Python
tests and the nine explicit codec corpus tests passed. Three Python tests
were skipped for platform availability (two PowerShell cases and a Windows
OpenGL card-selection case). Formatting, workspace clippy with warnings
denied, Ruff lint/format, strict mypy, stubtest and cargo-deny passed.
[Diagnostic measurements](../TESTING_AND_PERFORMANCE.md#id-xyz-capture-diagnostic-2026-11-17)
record the exact base/candidate commits. The reference host measured them
on 2026-11-19 ([reports](../testing/reference-machine/2026-11-19/)): SW isometric capture +0.14 ms, Top
unchanged, and cursor frames at ID's prompt the same before and after on
the CPU and the GPU. The supported command behavior needs no ProgeCAD validation.
