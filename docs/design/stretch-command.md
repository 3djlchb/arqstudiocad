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
other views. Crossing polygons and a live partial-geometry preview remain
future work. A partially selected standalone arc with just one endpoint
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
