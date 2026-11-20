# Independent polar arrays

Milestone 02.14; scope approved by the maintainer, 2026-11-20.

## Behavior

`ARRAYPOLAR` (`MATRIZPOLAR` in Spanish) creates independent 2D copies.
The rectangular `ARRAY`, `AR` and `-ARRAY` aliases keep their existing meaning.
The command announces its non-associative scope, requires Top view and uses
ordinary/current selection, including Previous and window selection. Existing
selection rules filter locked layers. Success clears selection and remembers
it as Previous.

Prompts: center, item count including the originals (2–100000), signed fill
angle, and Rotate items Yes/No. Positive angles run counterclockwise;
negative angles run clockwise. Zero and angles beyond one full turn are
refused. A full turn divides by the item count, avoiding a duplicate at the
source position; partial fills divide by count minus one, including the last
angle. Source geometry stays unchanged. Rotation Yes turns each copy around
the center; No asks for an explicit group base point, then translates the
whole group so that base orbits the center without turning any object.
This approved explicit point avoids dependence on selection order.

Initial defaults are four items, 360 degrees and Yes. Only successful values
are remembered for the session. Cancellation/failure does not mutate geometry,
history or remembered defaults. UI angles are degrees; geometry/API angles
are radians. There is no associative array entity, path, row, level, custom
3D rotation axis, item editing or persistent array metadata in this delivery.

## Ownership and validation

Geometry computes placement transforms, including a cancellation-resistant
translation formula for small angles. `CadDocument::polar_array` deduplicates
source IDs, validates every source and applies existing copy/transform support
in one ARRAYPOLAR transaction. The result contains only new IDs, ordered by
item then source. Styles, block references and polyline geometry use existing
copy semantics; copies can subsequently be edited independently.

Counts, finite angles and offsets are checked before mutation. Finite source
bounds and checked transformed corners guard against coordinate overflow,
including intermediate arithmetic, before each copy. A late failure rolls
back all earlier copies. This is conservative at extreme coordinates: an
unrepresentable bounding corner can reject otherwise representable geometry.
No new dependency, CPU instruction requirement or serialization type is added.
Python only coordinates prompts and publishes the native change set.

## Reference and validation

Behavioral reference: [AutoCAD 2021 -ARRAY](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-8336B4CD-5375-4290-BD08-7D9E022741F6.htm),
its non-associative polar workflow, signed fill and Rotate option. Original
implementation with hand-derived expected coordinates; no Autodesk source,
manual prose or artwork copied. The explicit No-rotation base prompt and fixed
100000-position limit are deliberate scope choices, not full ARRAY parity.

Rust tests exercise placement, small-angle/survey precision, errors, atomic
rollback, styles, ordering, index and undo/redo. Binding tests cover blocks,
bulged polylines and synthetic DWG/DXF round trips. Command/Qt tests cover
degrees, prompts, cancellation, session defaults, Top guard, Spanish catalog
and the shared menu/ribbon/toolbar action. New tests are sabotage-verified.
Linux checks are internal validation, not independent ProgeCAD acceptance.

The approved development-host diagnostic compares 100 synthetic lines in
100 positions over 360 degrees, with and without rotation, against equivalent
grouped COPY/ROTATE: two warmups and ten samples per case. Exact commits,
environment and ratios belong in [testing and performance](../TESTING_AND_PERFORMANCE.md).
Generating many copies also adds redraw work. Interactive floor acceptance
belongs on the actual i7-2600/GT 730, using `tools/reference_machine_report.sh`
and `uv run python tools/gpu_viewport_check.py`; development timings cannot
establish that acceptance.

## Local verification (2026-11-20)

All mandatory checks passed: Cargo format; workspace/all-target Clippy with
warnings denied; 966 Rust tests (20 ignored); 1226 Python/Qt tests (5 skipped);
Ruff lint/format; mypy (202 source files); stubtest (one native module);
cargo-deny; and all nine explicitly requested public-codec corpus tests.
Rust tests used `RUST_TEST_THREADS=1` for the known NVIDIA parallel-test
issue. Python used `LC_ALL=C.UTF-8` and `QT_QPA_PLATFORM=offscreen`. These
settings do not establish that the known es_EC locale failures or real-X11
Vulkan shutdown failure are fixed. Cargo-deny emitted existing informational
warnings; Qt tests emitted existing deprecation warnings.

The eight new Rust and seventeen Python/Qt cases passed after restoration.
Deliberate placement, validation, overflow-guard, premature-default, forgotten
default, failed-edit state, Top-guard and missing-menu mutations produced
behavioral test failures. Exact saved bytes were restored and touched, native
code rebuilt and affected Python bytecode caches removed. Temporary scripts,
backups and logs remain outside the repository. No private DWG, real QSettings
or system/desktop configuration was used or changed.
