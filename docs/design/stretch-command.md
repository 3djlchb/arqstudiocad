# STRETCH command — initial 2D delivery

Milestone 02.14, group 3. Implemented on the `feature/stretch` working copy
from main `50a4c18`, on Linux. This is a documented subset, not a claim of
complete AutoCAD compatibility.

## Reference and scope

Behavioral reference: [AutoCAD 2021 STRETCH command](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-F000A502-D39E-4D31-A8E2-4A626473FB72.htm).
The implementation is original; no Autodesk code, artwork or documentation
text is copied. AutoCAD 2021 is the baseline for this and later commands;
newer behavior must be identified explicitly.

- `STRETCH`, alias `S`, Spanish `ESTIRA`; menu, Classic toolbar and Ribbon.
- Crossing rectangles, drawn right-to-left or entered with `C`, select
  line endpoints and lightweight-polyline vertices. Multiple windows combine.
- Individually selected objects and `W` window selections move whole.
  `ALL`, `L` and `P` use the existing whole-object selection behavior.
- Existing selection is accepted; otherwise Enter ends object selection.
  Shift removes selected objects and their recorded crossing windows.
- Base point and second point determine the offset; Enter at the second
  prompt uses the first point as displacement. `D` or Enter at the base
  prompt asks for displacement, whose empty answer uses the previous value
  (initially zero). Existing Cartesian, relative, polar and direct-distance
  point input applies. This delivery accepts XY coordinates only.
- Circles move by their centers, block references by their insertion points
  and texts by their anchors when those lie in a crossing window.
- Polyline bulges, widths, closure, elevation, thickness and normal are
  retained, using the existing vertex writer. Both upward and downward
  plan normals are covered. Solids are left unchanged.
- Locked layers are filtered with the existing translated message.
- Rust owns every geometric decision and mutation. One atomic transaction
  reports its change set, updates the spatial index and supports undo/redo.
  Cancellation before the final point makes no document changes.

## Explicit limits

Only the Top view is accepted; the command asks the user to use PLAN in
other views. A live partial-geometry preview is implemented on `feature/stretch-live-preview` for the currently supported shapes. A partially selected standalone arc with just one endpoint
inside is refused atomically, rather than given an unverified reshape;
whole-object arc moves use the existing transformation. Tilted polylines
and other unsupported entities are not given invented stretch behavior.
These limits keep milestone 02.14's STRETCH entry in progress.

The codec, file formats and dependency versions are unchanged. Local tests
exercise geometry, existing metadata, bindings, selection gestures, command
input, cancellation, rollback, undo/redo and translations. Public sample
round-trip tests are internal regression checks. No independent ProgeCAD
validation is claimed: that work is available on Windows only. No task
requiring a codec change or independent external validation is included.

## Acceptance example

Draw a line from `(0, 0)` to `(10, 0)`. Run `STRETCH`, choose `C`, give
corners `(9, -1)` and `(11, 1)`, press Enter, then give base `(0, 0)` and
second point `(2, 3)`. The start remains `(0, 0)`, the end becomes `(12, 3)`.
One UNDO restores the original and REDO restores the stretch.

## Local verification

Run the standard checks from AGENTS.md, after fetching the public samples.
Focused checks are `cargo test -p arq-document stretch --lib`,
`cargo test -p arq-core stretch --lib` and
`QT_QPA_PLATFORM=offscreen uv run pytest tests/python/test_stretch.py`.

## Verification results (Linux development machine)

- Focused command/selection/i18n/workspace suite: 81 passed.
- `cargo test`: non-GPU suites pass, but the GPU renderer process terminates
  with SIGSEGV. The original main commit `50a4c18`, exported separately to
  `/tmp` and compiled without this change, reproduces that native failure.
  An OpenGL-only diagnostic run also reports device loss. Renderer sources
  are unchanged; no GPU fix is included in this task.
- `cargo test -- --skip gpu::tests`: 845 passed, 20 ignored, four GPU tests
  explicitly filtered. The nine public-codec corpus tests also pass when
  requested explicitly. Private/native external corpora are not claimed.
- Full Python suite: 1046 passed, five skipped and one failure: the
  auto-selected GPU application subprocess exits with SIGSEGV on shutdown.
  Repeating with only that case explicitly deselected gives 1046 passed,
  five skipped and one deselected. This is not an all-green full suite.
- Cargo fmt, workspace/all-target Clippy with warnings denied, Ruff lint and
  format, strict mypy, stubtest and cargo-deny pass. Cargo-deny emits existing
  informational warnings, including an unmatched libfuzzer license exception.
- No ProgeCAD/AutoCAD execution, Windows validation, reference-machine GPU
  gate or release CPU-floor check is claimed. This is not a release.

## Selection overhead measurement

Baseline: Session from main `50a4c18`, executed with the same installed
extension, interpreter and EventBus as the changed Session. Development CPU:
AMD Ryzen 9 5900X. Fifteen alternating batches of 100 calls, warmed up,
GC disabled; compare medians on this machine only. The scenario is adding
exactly the same tuple of IDs to an existing selection: a repeated click
or selection must not publish another selection event. The new fast path
avoids redundant processing when there are no crossing windows to override.

| Already-selected IDs | Before, ms/call | After, ms/call | After / before |
|---|---|---|---|
| 1 | 0.000519 | 0.000175 | 0.337 |
| 100 | 0.018409 | 0.000251 | 0.014 |
| 1000 | 1.782305 | 0.000934 | 0.001 |

This narrow check does not establish whole-application performance or
reference-machine acceptance. Reproduce from the task working copy:

```sh
uv run python - <<'PYBENCH'
import gc
import statistics
import subprocess
import sys
import time
import types

from arqstudiocad.events import EventBus
from arqstudiocad.session import Session

baseline = types.ModuleType('stretch_baseline_session')
sys.modules[baseline.__name__] = baseline
source = subprocess.check_output(['git', 'show', '50a4c18:python/arqstudiocad/session.py'], text=True)
exec(compile(source, 'baseline-session.py', 'exec'), baseline.__dict__)

def measure(cls, count):
    session = cls(EventBus())
    doc = session.new_document()
    ids = tuple(range(count))
    session.select(doc, ids)
    def run():
        start = time.perf_counter_ns()
        for _ in range(100):
            session.select(doc, ids)
        return (time.perf_counter_ns()-start)/100/1e6
    return session, doc, run

gc.disable()
for count in (1, 100, 1000):
    old, old_doc, before = measure(baseline.Session, count)
    new, new_doc, after = measure(Session, count)
    before(); after()
    a=[];b=[]
    for _ in range(15):
        a.append(before()); b.append(after())
    prior=statistics.median(a);current=statistics.median(b)
    print(f'existing selection {count}: before={prior:.6f} ms after={current:.6f} ms ratio={current/prior:.3f}')
    old.close(old_doc);new.close(new_doc)
PYBENCH
```


## Crossing polygons (2026-11-15)

The `feature/stretch-crossing-polygon` delivery adds `CP` (Crossing polygon)
at STRETCH's object-selection prompt. Pick corners through the existing point
input (cursor/snaps or coordinates), use `U` to remove the last added corner,
and Enter to close the polygon. Enter again ends object selection. The
initial corner is retained when there is nothing left to undo. During input,
the contour and its closing side follow the current point as a dashed crossing
overlay. It uses the existing shared Stroke description for CPU and GPU, and
is cleared when input ends or is cancelled. A partial-geometry displacement
preview remains future work.

A valid polygon has at least three corners, no degenerate edges and no
self-crossings or self-touching. Concave contours and either winding work;
the final repeated first corner is optional. Invalid contours leave selection
and geometry unchanged. Rust validates the contour before any edit and reuses
`Region` for boundary-inclusive containment in a local coordinate frame.

The spatial index supplies broad-phase candidates. Exact drawn curves,
points, existing text/placeholder boxes and filled triangles determine which
visible entities meet the polygon. Its enclosing box alone never determines
selection. The same polygon determines which line endpoints and polyline
vertices move. Rectangles and polygons combine without moving any point twice;
whole-object selection takes precedence. Deselecting/clearing/closing removes
polygon provenance. The existing three-argument `stretch_entities` API stays
valid; the optional fourth argument carries `(entity_id, polygon_points)`
pairs. `select_crossing_polygon(points)` is a read-only query.

The existing document writer, properties, normal/elevation handling, locked
layer filtering, atomic history and partial-arc limitations are unchanged.
Existing commands that do not enable crossing-window prompts gain no CP option.
No codec, dependency, startup, GPU backend or new rendering path is introduced.
While CP is active, the shared overlay describes its polygon edges; idle views
have no polygon work.
Polygon validation checks edge pairs; very large contours cost more than a
rectangle. Reference-floor performance acceptance remains a separate check on
the actual reference machine; no development timing is claimed as acceptance.

Behavioral reference: [AutoCAD 2021 SELECT, CPolygon](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-0DD5DA73-9DC5-4424-8FED-7BBE3BE52A4D.htm).
This is an original implementation and internally tested subset, not an
independent AutoCAD/ProgeCAD validation or a claim of full STRETCH parity.

Example: draw lines `(0,0)`–`(10,0)` and `(0,5)`–`(10,5)`. Run `STRETCH`,
then `CP`, and enter `(9,-1)`, `(11,-1)`, `(11,6)`, `(10.5,6)`, `(10.5,1)`,
`(9,1)`. Press Enter twice, use base `(0,0)` and displacement `(2,3)`.
Only the lower line's end becomes `(12,3)`; the upper line stays unchanged
even though its end lies in the polygon's enclosing rectangle. One UNDO
restores the drawing.

### Crossing-polygon verification

LMDE 7 development host (2026-11-15): 860 Rust tests passed in serial mode,
including the GPU tests, and nine explicit public-codec corpus tests passed.
Python with `LC_ALL=C.UTF-8`, offscreen Qt and required public samples:
1082 passed, five platform/adapter skips and the documented GPU application
shutdown failure. That known failure was not reopened or counted as passing.
Cargo fmt, workspace/all-target Clippy, Ruff lint/format, strict mypy,
stubtest and cargo-deny passed.

New coverage: two geometry tests, one document-selection test, one core
transaction test and nine Python cases. Sabotage of polygon construction
failed all four Rust tests and seven Python cases; removing CP failed the
four cancellation stages; hiding the contour failed the cursor/overlay case.
After exact-byte restoration and extension rebuild, all four focused Rust
tests and 27 Python cases (new plus existing STRETCH) passed.

While CP is active, overlay work grows with its corner count. Before claiming
reference-floor interactive performance, run `tools/reference_machine_report.sh`
and `uv run python tools/gpu_viewport_check.py` on the actual GT 730/i7-2600
machine and exercise both short and long CP contours. No timing measured on
the development machine is substituted for that acceptance.

## Live preview (2026-11-15)

The displacement and second-point prompts carry the captured per-entity
crossing rectangles/polygons in a STRETCH drag. Moving the cursor displays
centerline contours of the exact supported result, including bulged
polylines and whole block/text moves. It uses the common overlay description
for CPU and GPU presentation; it does not rerender or mutate the document.

The document computes one read-only stretch plan shared by preview and
commit. The core applies the same ID/locked-layer checks and area union.
CadView flattens only the requested transient shapes with the existing
outline tolerance. No complete document clone, all-entity drawing query,
new dependency or compile-time CPU requirement is introduced. Cancellation
clears the preview; invalid geometry shows no candidate and leaves history
unchanged. Polyline width fills are not previewed, as with ordinary outlines.

Partial standalone arcs remain refused in both paths until the agreed
[Windows comparison](../testing/stretch-arcs-windows.md) establishes the
behavior. The maintainer chose this order over inventing an arc rule.

Pointer overlay work increases with selected geometry and polygon corner
count. Actual interaction/CPU-floor acceptance remains on the i7-2600/GT 730
host using `tools/reference_machine_report.sh` and
`uv run python tools/gpu_viewport_check.py`; development diagnostics are not
a substitute. Startup, codec and GPU-probe implementations are unchanged.

### Live-preview local verification

All required local checks passed on LMDE 7: fmt, workspace/all-target
Clippy with warnings denied, cargo test (881 passed, 20 ignored), pytest
(1103 passed, 5 skipped), Ruff lint/format, mypy, stubtest and cargo-deny
(existing informational warnings). All nine explicit public-codec corpus
cases passed after fetching the samples in the new working copy.

The two new document cases, new core case and three binding/Qt cases were
observed failing under deliberately omitted plans, missing block translation,
empty queries, omitted overlays and ignored displacement. Exact bytes were
restored and touched; uv sync rebuilt the restored native extension.
The focused restored suites passed (4 document, 6 core, 21 Python cases).
Temporary scripts and logs remain outside the repository.

A proposed development-host diagnostic (100 selected lines in 100000
synthetic lines; before/after edit cost and separate preview cost) awaits
maintainer scenario approval before measurement. No timing or floor
acceptance is claimed yet.

The public contour query also refuses non-Top views, rather than returning
XY contours through a camera for another plane. Removing that guard caused
the binding's Front-view assertion to fail; exact-byte restoration, touch,
uv sync and all mandatory local checks passed again afterwards.
