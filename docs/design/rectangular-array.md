# Independent rectangular arrays

Milestone 02.14; approved by the maintainer, 2026-11-15. First delivery:
non-associative copies. Associative editing, polar/path arrays and 3D levels
remain separate work.

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
comparison. A Windows comparison remains pending. Large generated drawings
may incur redraw costs; reference-floor interactive acceptance belongs on
the actual i7-2600/GT 730 machine, not this development host.

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
