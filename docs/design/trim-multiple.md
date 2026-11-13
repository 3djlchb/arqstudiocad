# Multiple-object TRIM and EXTEND

Milestone 02.14. Behavioral reference:
[AutoCAD 2021 TRIM](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-B1A185EF-07C6-4C53-A76F-05ADE11F5C32.htm)
and [EXTEND](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-89DD7B0F-F4F1-410D-9A3A-5847CA5F8744.htm).
Original implementation; no Autodesk code, artwork or documentation passages
are copied. Work starts from main `d5a2fc8`, after STRETCH's PR #180.

## Interaction

At the TRIM or EXTEND object prompt:

- An ordinary click still picks one object immediately.
- Press the left button in empty space, hold it and draw a path across
  several objects; release to apply the operation. The path appears as a
  dashed overlay and changes no geometry before release.
- Alternatively, click an empty point and then another point to use the
  segment between them as a two-point fence.
- Hold Shift during the gesture to extend in TRIM, or trim in EXTEND.
- `F` (Fence) accepts a sequence of points; Enter completes the fence.
- `C` (Crossing) accepts opposite corners of a rectangle and applies the
  operation immediately after the second corner.
- Quick and Standard retain their current meaning and selected boundaries.
  Enter finishes the command; Esc discards an unfinished fence.

The direct cursor gestures operate in the Top view. Other views retain
single-object picking; no arbitrary-UCS or projected 3D fence is claimed.
Fence and crossing prompts use the existing XY point-input convention.

## Selection and editing contract

`CadDocument.fence_picks(points, crossing=False)` is a read-only Rust query.
The spatial index supplies candidates; the existing exact line/circle/arc
intersection algorithms locate the first hit on each entity. Polylines use
their exact drawn curves, including bulges, rather than their bounding boxes
or filled-width triangles. Hidden and frozen layers are excluded.

A fence follows its point order. A crossing rectangle follows its perimeter
clockwise from the first corner; curves wholly enclosed are included with
a point on the curve. Each entity occurs once per gesture, at its first
encounter, including when the fence loops back. Multiple separate trim
portions on a single object need separate gestures. Supported target kinds
remain plan lines, arcs, circles and lightweight polylines; unsupported
objects are not silently erased by the selection query.

Python coordinates the existing per-object TRIM/EXTEND operations; no new
geometry mutation or codec path is added. Locked-layer targets are reported
and skipped. A target which cannot be extended or trimmed reports the
existing diagnostic; other targets continue. Each successful target remains
one atomic undo step, as in the existing command; `U` removes the last
successful target, not the entire fence. This delivery does not claim
complete AutoCAD batch undo equivalence. Single-object prompts of other
commands do not accept fences.

`Prompt.multiple` and the matching `CommandPrompt` field explicitly enable
cursor fences only for these commands. Qt supplies world-plan points to
Python, which passes them through the existing PyO3 boundary. Rust owns the
hit decisions. The preview uses the existing shared Stroke overlay; no GPU
shader, backend, codec or dependency change is included.

## Acceptance example

Draw horizontal lines `(0,0)-(10,0)`, `(0,5)-(10,5)` and `(0,10)-(10,10)`,
then a vertical cutting line `(5,-5)-(5,15)`. Start TRIM and drag from
`(8,-2)` to `(8,12)` starting in empty space. All three right-hand portions
are removed, leaving their ends at x = 5. Add a boundary at x = 15 and use
Shift with the same gesture to extend the lines to it. `U` reverses the
last individual edit; Esc during an unfinished gesture changes nothing.

## Local verification

Focused suites cover mouse dragging, two-click fences, Shift, cancellation,
Fence/Crossing prompts, Quick/Standard boundaries, hidden/locked layers,
curved polylines, rectangle order, exact hits, invalid coordinates and
isolation of other entity prompts. Rust tests cover both the document
query and the spatial-index facade, including edits and undo between
queries. Full-suite outcomes and the known local GPU issue are recorded
with the PR. Validation is internal on Linux; no independent ProgeCAD or
AutoCAD execution is claimed. No release or reference-machine gate is
claimed.

## Selection cost

Development CPU: AMD Ryzen 9 5900X. Compare the existing per-object pick
queries with the new fence query for the same ordered IDs, without changing
the drawing; medians of 15 repetitions, one warm-up correctness check.
The comparison measures selection only, not mutation or end-to-end UI time.

| Targets | Existing individual picks, ms | Fence, ms | Fence / individual |
|---|---|---|---|
| 100 | 0.052 | 0.044 | 0.850 |
| 1000 | 0.573 | 0.475 | 0.829 |

These are same-machine ratios, not a claim about the reference floor.
Reproduction from the task working copy:

```sh
uv run python - <<'PYBENCH'
import statistics
import time
from arqstudiocad.core import CadApplication
for count in (100,1000):
 doc=CadApplication().create_document('Fence timing')
 ids=[doc.add_line((0,y),(10,y))[0] for y in range(count)]
 def manual():
  return [doc.pick((8,y),0.1) for y in range(count)]
 def fence():
  return [i for i,_ in doc.fence_picks(((8,-1),(8,count)))]
 assert manual()==fence()==ids
 samples=[[],[]]
 for _ in range(15):
  for n,fn in enumerate((manual,fence)):
   start=time.perf_counter();fn();samples[n].append((time.perf_counter()-start)*1000)
 before,after=(statistics.median(x) for x in samples)
 print(f'{count} targets: manual picks {before:.3f} ms, fence {after:.3f} ms, ratio {after/before:.3f}')
 doc.close()
PYBENCH
```

## Verification results

- Focused command, cursor-input, overlay, selection and i18n suites: 80 passed.
- Full Python suite: 1059 passed, five skipped, one failure in the existing
  automatic GPU application subprocess (SIGSEGV).
- Full Rust run also encounters the existing GPU renderer SIGSEGV, already
  reproduced on unmodified main during PR #180. The run with only
  `gpu::tests` explicitly excluded passes: 848 passed, 20 ignored and four
  filtered GPU tests. Nine explicitly requested public-codec corpus tests
  also pass. This is not a claim that the
  default full suite is green locally.
- Cargo fmt, workspace/all-target Clippy with warnings denied, Ruff,
  strict mypy, stubtest and cargo-deny pass.
- The shared overlay primitives are reused. GPU presentation on the
  reference machine and independent external CAD validation are not claimed.
