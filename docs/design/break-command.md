# BREAK command — initial 2D delivery

Milestone 02.14, group 3. Maintainer-approved scope: lines, arcs and circles,
with AutoCAD 2021 as the baseline. Open lightweight polylines are implemented on `feature/break-polylines`; closed paths await Windows comparison, as approved by the maintainer.

## Behavior

- `BREAK`, alias `BR`, Spanish command `PARTE`, available in Modify menus,
  the Classic toolbar and the Ribbon through one shared action.
- Select one object with the cursor or a typed point. The pick supplies the
  first break point. `F` (First point) replaces it, then a second point defines
  the gap. Existing Cartesian, relative, polar and snap input applies.
- Rust projects both points onto the bounded curve. Lines and arcs lose the
  span between those points, independent of their order. Equal interior
  points split an open curve without a gap; `@` repeats the first point.
- Circles lose the counterclockwise span from first to second and become an
  arc. Coincident projected circle points are refused; no full-circle arc is
  invented. Equal points at an open curve's endpoint are also refused as a
  no-op. Points beyond open-curve endpoints can remove an end or the whole
  curve. Pieces no longer than the existing linear tolerance are omitted.
- One atomic operation, one `BREAK` undo step, one change set; cancellation
  at any prompt and failed edits leave the document unchanged. Locked layers
  cannot be edited. The first surviving open-curve piece keeps its ID; other
  pieces copy the source properties. A converted circle receives an arc ID,
  following the existing TRIM writer. Undo restores the original IDs.
- The command currently requires Top view. Tilted/non-editable curves and
  other entity kinds are unsupported. No new entity type, codec behavior,
  dependency, renderer implementation or compile-time CPU feature is added.

## Reference and validation

Behavioral reference: [AutoCAD 2021 BREAK command](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-36A1CDE0-3871-4B25-AC98-93235FA83863.htm).
The implementation is original; no Autodesk source or artwork is copied.
The public reference is a behavioral specification, not external validation.

For example, breaking `(0,0)`–`(10,0)` at `(3,0)` and `(7,0)` leaves
`(0,0)`–`(3,0)` and `(7,0)`–`(10,0)`. Undo restores the original line.
Breaking a radius-5 circle centered at the origin from `(5,0)` to `(0,5)`
leaves the three-quarter arc starting at `(0,5)` and ending at `(5,0)`.

Tests belong to geometry (projection, direction, tolerances, coordinate
precision), core (atomicity, properties, change sets, spatial index and
undo/redo), bindings/integration (non-finite input, synthetic DWG/DXF
round trips) and Python/Qt (prompts, cancellation, viewport clicks, shared
UI actions and Spanish text). New tests are sabotage-verified, restoring
saved bytes exactly. Settings and drawings used by tests are temporary.

Validation on LMDE 7 is internal only. No AutoCAD/ProgeCAD execution or
independent Windows interoperability acceptance is claimed. No startup or
renderer algorithm is changed; the command adds one ordinary UI action and
uses incremental document change reporting. Reference-machine performance
acceptance remains separate from development-machine checks.

## Local verification (2026-11-15)

On the LMDE 7 development host: 856 Rust tests passed with serial execution,
including GPU tests; nine explicit public-codec corpus tests passed. Python
with `LC_ALL=C.UTF-8`, offscreen Qt and required public samples: 1073 passed,
five platform/adapter skips and the previously documented GPU application
shutdown failure (`test_the_app_chooses_the_graphics_card_by_itself`). That
failure was not reinvestigated and is not counted as passing.

Cargo fmt, workspace/all-target Clippy with warnings denied, Ruff lint and
format, strict mypy, stubtest, cargo-deny and changed-file pre-commit hooks
passed. Sabotage caused both geometry tests, both core tests, the binding
integration test and all nine command/UI cases to fail. After exact-byte
restoration and extension rebuild, all four focused Rust tests and all ten
focused Python cases passed. The sabotage exposed and corrected two false
positives in cancellation tests before the final verification.

## Open lightweight polylines (2026-11-15)

BREAK now accepts open LWPOLYLINE objects as well as the initial curves.
The original pick, First point override and second point follow the same
command flow. Both points are projected onto the bounded path; the interval
between them is removed regardless of pick order. A repeated interior point
splits without a gap; equal projected endpoints are refused as a no-op.
Picks beyond both ends can remove the entire path. Straight and positive or
negative bulge segments are handled in the geometry layer.

The existing segment-parameter and subpath helpers are reused. BREAK also
records each output segment's original segment index and fractional range.
The document interpolates start/end widths from this provenance, rather than
reprojecting geometry at self-crossings or discarding variable widths.
Constant width, elevation, thickness, normal, layer, color and other source
properties stay with every piece. Both upward and downward plan normals
are supported. The first surviving piece keeps its ID; other pieces copy
the complete source before any source path is rewritten.

One BREAK transaction updates the index and undoes/redoes all pieces.
The Python API signature is unchanged. Tilted and legacy heavyweight/3D
polylines remain unsupported. Closed paths are currently refused explicitly;
the maintainer approved the open-path delivery and chose Windows confirmation
before implementing closed-path traversal (2026-11-15). No behavior is
invented for that case. This is Linux internal validation, not independent ProgeCAD
acceptance. The codec, rendering and CPU baseline are unchanged.

### Open-polyline local verification

All mandatory local checks passed on LMDE 7: fmt, workspace/all-target
Clippy with warnings denied, cargo test (882 passed, 20 ignored), pytest
(1103 passed, 5 skipped), Ruff lint/format, mypy, stubtest and cargo-deny
(existing informational warnings). All nine explicit public-codec corpus
cases passed after the new worktree fetched the public samples.

Every new case (2 geometry, 1 document, 1 core, 3 binding/command cases)
and the modified unsupported-object case was observed failing under
intentional empty results, width loss, omitted dispatch or incorrect command
acceptance. Exact bytes were restored/touched, the native extension rebuilt,
and all focused checks passed again (4 new Rust cases; 12 Python cases
including existing BREAK regressions). Temporary scripts/logs remained
outside the repository. No private drawing, real app setting or system
configuration was changed.

The shared TRIM subpath wrapper does not collect width provenance; its
existing geometry cases pass. No diagnostic timing, independent ProgeCAD
comparison or reference-floor performance acceptance is claimed in this
first open-path delivery.

### Closed-path follow-up approved by the maintainer

The current delivery is open paths only. On Windows, create a synthetic
closed square in both vertex orders, then BREAK at two interior edge points.
Record the exact picks and which directed span remains, resulting vertex
order, closure flag, properties and undo/redo. Repeat across the closing
segment, with a bulged segment and with coincident picks. Save/reopen DWG
and DXF and record ProgeCAD's version. Use only synthetic/public data and
report observations through GitHub before approving the Rust rule; do not
infer it from the circle's counterclockwise convention.

After retaining the maintainer's remote branch updates and incorporating
main through ARRAY #191 (`66c0125`), all mandatory local checks passed
again: 885 Rust cases (20 ignored), 1116 Python cases (5 skipped), all nine
explicit public-codec corpus cases, fmt, Clippy, Ruff, mypy, stubtest and
deny. The approved scope is open paths; closed traversal remains the
separate Windows follow-up above.

### Integration after STRETCH #192 merged

Updated with main `5ac83f4` on 2026-11-15. The only merge conflicts were
appended core tests and the long milestone 02.14 row. All test functions
from both sides were retained, and the roadmap keeps both command deliveries.
No operation semantics or approved scope changed. Every required local gate
passed on the combined tree: 888 Rust cases (20 ignored), 1119 Python cases
(5 skipped), all nine explicit codec corpus cases, fmt, Clippy, Ruff, mypy,
stubtest and deny. The actual reference-machine STRETCH check remains a
separate follow-up; closing its implementation PR does not establish a
performance result.
