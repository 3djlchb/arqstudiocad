# Independent rectangular arrays

Milestone 02.14; approved by the maintainer, 2026-11-15. First delivery:
non-associative copies. Associative editing and 3D levels remain separate work.
Independent [polar arrays](polar-array.md) and [path arrays](path-array.md)
are delivered separately.

## Behavior

`ARRAYRECT`, with `ARRAY`, `AR` and `-ARRAY` aliases, creates a rectangular
array. Spanish names: `MATRIZRECTANG`, `MATRIZ` and `-MATRIZ`. All aliases
explicitly announce independent rectangular copies; they do not claim the
full associative ARRAY workflow.

The command accepts the current selection or ordinary selection (including
Window, Crossing, Crossing polygon, All, Last and Previous). Locked-layer
objects are filtered with the existing message. Top view is required.

It asks for rows, columns, signed row/column spacing on repeated axes, and
orientation. Counts are positive integers; the product must be 2–100000
cells, including the originals. Zero spacing is refused on repeated axes.
Defaults start at four rows/columns, one model unit spacing and zero degrees;
successful values are remembered for this session. Cancellation or failure
never updates these defaults or changes the document.

The original selection is cell (0,0). Positive column spacing runs along the
oriented X axis, positive row spacing along its perpendicular Y axis; negative
values reverse these directions. Orientation rotates grid axes only, not
object geometry. The command reads degrees or an angle picked from the world
origin; the API receives radians. Unlike SNAPANG in legacy AutoCAD, this
orientation is an explicit command parameter, not a drawing snap setting.
Spacing is between corresponding points in neighboring cells, not the clear
gap between object bounding boxes. Unit-cell corner picking is not included.

After success the ordinary selection is cleared and remembered as Previous.
All generated entities are independent, retaining the source properties and
block definitions; modifying one copy leaves its source and siblings alone.

## Ownership and validation

`CadDocument::rectangular_array` owns the loops and one ARRAYRECT transaction.
It deduplicates source IDs and returns only new IDs, ordered by row, column,
then source. The existing copy/transform capability determines supported
kinds. There is no new persistent array entity or serialized array metadata.

Finite spacings/angles, checked cell counts and finite offsets are validated
before mutation. The transaction checks every source and locked layer, and
rolls back even late coordinate overflow. The public PyO3 method and stub
expose the same operation; Python coordinates prompts and publishes its
change set. No library or CPU baseline change is needed.

## Reference and validation

Public behavioral reference: [AutoCAD 2021 -ARRAY](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-8336B4CD-5375-4290-BD08-7D9E022741F6.htm).
Original implementation; no proprietary source, artwork or manual text copied.
The documented default 100000-element ceiling is a fixed cell limit here;
there is no MAXARRAY setting yet.

Rust tests cover hand-derived placement, source styles, deduplication,
change sets, one-step undo/redo, invalid parameters and transaction rollback.
Python tests cover binding conversions, index updates, block/polyline copies,
DWG/DXF persistence, command prompts, cancellation, Spanish names and the
shared menu/ribbon/toolbar action. New tests are sabotage-verified.

Linux validation does not constitute an independent AutoCAD/ProgeCAD
comparison. The [Windows comparison](../testing/array-rectangular-windows.md) was made in ProgeCAD on
2026-11-15: the same copies in DWG and DXF; zero spacing and more than 100,000
cells refused here as designed, made by ProgeCAD. Large generated drawings
may incur redraw costs; reference-floor interactive acceptance belongs on
the actual i7-2600/GT 730 machine, not this development host. Measured there
on 2026-11-19 ([report](../testing/reference-machine/2026-11-19/array.txt)): the diagnostic's ratio 0.723,
10,000 copies on screen in about 0.1 s and the 100,000 ceiling in about
0.6 s, then panned at the display's refresh on the GPU.

## Local verification (2026-11-15)

All mandatory local checks passed: cargo fmt, workspace/all-target Clippy
with warnings denied, cargo test (881 passed, 20 ignored), full pytest
(1112 passed, 5 skipped), Ruff lint/format, strict mypy, stubtest and
cargo-deny (existing informational warnings). All nine explicitly requested
public-codec corpus tests passed after fetching the samples.

All three new Rust cases and all twelve new Python/Qt cases were observed
failing under deliberate placement/count/error-handling, premature mutation,
view-guard or translation mutations. Exact saved bytes were restored and
touched, the extension rebuilt, and all fifteen passed again. Temporary
sabotage/measurement scripts and logs stayed outside the repository.
The approved diagnostic comparison is recorded in
[testing and performance](../TESTING_AND_PERFORMANCE.md).

## Live orientation preview (2026-11-20)

The maintainer approved a first preview delivery at the Orientation prompt,
after rows, columns and spacings are known. The remembered/default angle
is shown before any mouse move; the cursor then chooses the angle using
the existing world-origin reference. A click accepts that angle, typed
degrees override it, and Enter accepts the remembered default. Grid axes
turn; source objects keep their orientation. Earlier prompts are unchanged.

`RectangularGrid` in arq-geometry owns signed offsets in row/column order,
excluding the original cell. The commit and read-only preview plan share it.
Finite corners are checked before lazy traversal, so unseen overflowing
offsets are rejected without enumerating a large grid per cursor move.
Existing public edit error categories and messages are retained. The core
validates/deduplicates source IDs without beginning a transaction; commit
still owns exactly one atomic transaction.

`CadView.rectangular_array_outlines` reads the cached Top-view scene and
returns copied contours plus explicit shown/total copied-cell counts.
`arq-render` tessellates each source once per query, translates contours in
native code and bounds both output and source tessellation. Diagnostic
budgets are 1000 copied cells and 4000 output vertices. If one
complete source cell cannot fit, no incomplete cell is displayed. The prefix
is in row/column order; the ordinary selected originals remain visible.
A single source shape may need its normal bounded tessellation before a
vertex-budget rejection. ID validation is proportional to the selection.
These limits bound multiplicative preview work, not all existing selection
overlay costs or a universal cursor-frame duration.

The overlay discloses partial results with translated shown/total counts.
Confirmation creates every requested copy, independently of display budgets.
Cancellation, completion and errors clear the transient display; a query does
not create entities, update the spatial index, dirty the document or add undo
steps. Physical native pixels become logical overlay pixels once, including
high-DPI displays. The existing overlay description serves CPU and GPU; no
GPU backend, dependency, serialization or CPU baseline changes.

Approval includes a development-host diagnostic: 100 synthetic lines, a
10×10 array, CPU viewport 800×600, five warmups and 100 samples. Compare
commit cost against the exact base and measure native preview query, overlay
construction and CPU painting separately. Use identical fixtures/settings,
isolated configuration and a quiet window, recording exact commits and
ratios. Results cannot establish minimum performance: run actual cursor
interaction on the i7-2600/GT 730 using `tools/reference_machine_report.sh`
and `uv run python tools/gpu_viewport_check.py` before floor acceptance.
No ProgeCAD behavior decision is needed for this read-only visualization of
the existing operation.

An initial 20000-vertex diagnostic displayed all 99 copied cells of the
approved fixture (9900 paths, 19800 vertices), taking about 40 ms to build
and paint on the development host. The vertex budget was reduced to 4000
before final measurement; this is a work bound, not reference-floor
acceptance. The edit retains its original preallocated offset buffer.

Final local checks after rebasing onto main at
`7487c3257a1b6d86280c5dc9f29628669cfc79cf`: Cargo format, workspace/all-target
Clippy with warnings denied, 999 Rust tests (20 ignored), 1249 Python/Qt
tests (5 skipped), Ruff lint/format, mypy, native stubtest, cargo-deny and
all nine explicit public-codec cases passed. Own crate artifacts were
invalidated and the extension rebuilt to exclude stale pre-rebase binaries;
the new ellipse-curve Rust and Python snap tests ran in this verification.
Rust used serial execution for the known NVIDIA parallel-test defect; Python
used C.UTF-8 and Qt offscreen. This does not establish that the known es_EC
locale or real-X11 Vulkan shutdown defects are fixed.

Six new Rust and seven Python/Qt cases were sabotage-verified with behavioral
failures for placement, unseen overflow, deduplication, cell/vertex budgets,
incomplete source cells, wrong pixel translation, missing Top guard and
missing overlay. Exact saved bytes were restored and touched; the native
extension rebuilt after native mutations, including arq-python restoration,
and affected Python bytecode caches removed. The final 4000-vertex budget
was independently sabotage-verified after the rebase. Temporary scripts,
logs and public fixtures stayed outside tracked source; no private drawing,
real QSettings or system/desktop setting was changed.

Final diagnostic: edit after/before 0.992421; bounded query 0.329887 ms;
overlay construction 1.481141 ms, CPU painting 6.808924 ms and combined
build+paint 8.389221 ms for the approved fixture. The final budget shows
20 of 99 copied cells (2000 paths/4000 vertices) with an explicit notice.
See [testing and performance](../TESTING_AND_PERFORMANCE.md) for exact
commits, environment, repetitions, the initial 40.011402 ms full-preview
measurement and its different work budget. Reference interaction acceptance
remains pending.
