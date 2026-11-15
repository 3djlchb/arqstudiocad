# BREAK command — initial 2D delivery

Milestone 02.14, group 3. Maintainer-approved scope: lines, arcs and circles,
with AutoCAD 2021 as the baseline. Polylines are a separate future delivery.

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
