# Independent path arrays

Milestone 02.14. The maintainer authorized starting this task on 2026-11-20.
Placement, alignment, corner handling and the diagnostic scenario were
explicitly approved on the same date.

## Approved scope

- A non-associative 2D ARRAYPATH command; Spanish catalog MATRIZCAMINO.
- Source selection, explicit group base point, a path entity, item count
  and optional alignment, with one atomic undo/redo step.
- Path kinds: lines, arcs and open lightweight polylines, including bulges.
- Independent copies retaining existing copy/transform properties.
- Synthetic tests, DWG/DXF round trips and sabotage verification in Linux.
- No associative metadata, closed paths, spacing-based Measure mode, rows,
  levels, arbitrary 3D paths or new dependency in this delivery.

## Approved placement and measurement

1. Preserve all source objects and the guide and generate N new
   items, including the first at the guide start. The count excludes the
   originals: they can be far from the guide's beginning.
2. Divide over the entire open path, including both endpoints;
   preserve the first item's source orientation and rotate later items by
   their tangent change from the start. Without alignment use translations
   only. At an exact interior corner use the outgoing tangent; at the final
   endpoint use the last segment's tangent (explicit maintainer decision).
3. Approved performance diagnostic: 100 synthetic lines, 100 positions along
   an open mixed straight/curved polyline, alignment on/off, atomic operation
   versus equivalent grouped COPY/ROTATE, two warmups and ten samples each.
   Exact commits and a quiet measurement window are required before timing.

## Implementation boundaries

`arq-geometry` owns stationing by true curve length, tangent calculations,
finite-coordinate checks and placement transforms. `arq-core` owns source
validation, one transaction, deduplication, returned IDs and rollback.
PyO3 belongs only in arq-python; Python coordinates prompts, localization
and change-set publication. Menu/ribbon/toolbar use one shared action.

Existing `Document::join_path` already extracts world-plan vertices and
signed bulges for lines, arcs and open lightweight polylines, rejecting
closed/tilted paths. This delivery reuses that read-only adapter without changing the
other agent's ellipse-curve implementation. Line and Arc already expose
length and distance sampling; negative bulges must retain traversal order.
Do not infer direction from rendered/tessellated chords or implement geometry
in Python. Numerical overflow and late failure must leave the document intact.

Geometry tests cover length, stationing, signed bulges, endpoints and survey
precision. Core tests cover source/guide ownership, order, styles, index,
invalid sources, late failure and undo/redo. Binding/integration tests cover
conversions, blocks/polylines and synthetic persistence. Command/Qt tests cover
selection, cancellation at every prompt, failed edits, remembered defaults,
Top guard, Spanish catalog and shared actions. Every new test needs a
behavioral sabotage failure followed by exact-byte restoration and rebuild.

Development-host timings are diagnostic only. Interactive minimum-performance
acceptance belongs on the actual i7-2600/GT 730 reference machine. No private
fixture, real QSettings or system configuration may be used or changed.

## Behavioral reference

[AutoCAD 2021 ARRAYPATH](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-Core/files/GUID-D36C46CD-4E17-4A16-A387-C0B158EA5A9E.htm)
and its [Spanish command name](https://help.autodesk.com/cloudhelp/2021/ESP/AutoCAD-Core/files/GUID-D36C46CD-4E17-4A16-A387-C0B158EA5A9E.htm).
The reference documents independent items, Divide/Measure, explicit base
points and relative alignment. It also documents DELOBJ-controlled source
retention. These approved scope choices must be distinguished from full
AutoCAD compatibility. No proprietary source, prose or artwork is copied;
record the behavioral reference in THIRD_PARTY.md with implementation.

## Command and API

`ARRAYPATH` / `MATRIZCAMINO` requires Top view, takes ordinary/current
selection, an explicit group base point, one guide, 2–100000 items and
Align Yes/No. Initial session defaults are four items and Yes; only success
remembers them. Unsupported guide kinds are reprompted; closed/tilted or
degenerate guides are refused by the native operation without mutation.
Selection is cleared after success and remembered as Previous. The original
guide and sources remain; N counts the newly generated groups, not originals.
A guide may be read on a locked layer, but sources follow existing unlocked
copy rules. Sources/guide elevations are not used to move copies in Z; this
delivery uses world XY stationing and retains source elevation.

`CadDocument::path_array(ids, guide, base, count, align)` returns only new
IDs, ordered by station and deduplicated input order, in one ARRAYPATH
transaction. `arq_geometry::path_array_transforms` computes the true lengths
and placements. Existing `Transform2::checked_apply_point` guards source
bounding corners before each copy. As in polar ARRAY this is conservative
at extreme coordinates; unrepresentable intermediate frames/bounds refuse
the complete operation. Zero-length constituent segments are refused.

Direction is the guide's stored world-plan traversal, independent of the
screen pick location. Width is not an offset to the guide's centerline.
Straight/curved polyline lengths are accumulated once; station segment lookup
uses binary search. Negative bulges keep their directed sense. There is no
preview or later array editing in this first non-associative delivery.

Station lookup allows only accumulated floating-point roundoff, capped by
`Tolerance::DEFAULT.linear()`, when deciding whether a station reaches a
vertex. This keeps decimal-length corner alignment deterministic without
introducing a fixed large snap radius. The first station always remains
the stored first endpoint, including paths with very short initial segments.

## Local verification (2026-11-20)

All required local checks passed: Cargo format; workspace/all-target Clippy
with warnings denied; 973 Rust tests (20 ignored); 1241 Python/Qt tests
(5 skipped); Ruff lint/format; mypy; native stubtest; cargo-deny; and all nine
explicit public-codec corpus cases after fetching the public samples.
Rust ran serially for the known NVIDIA parallel-test defect. Python ran
with `LC_ALL=C.UTF-8` and `QT_QPA_PLATFORM=offscreen`; this does not establish
that the known es_EC locale or real-X11 Vulkan shutdown defects are fixed.
Existing Qt deprecation/cargo-deny informational warnings remain unrelated.

Seven new Rust and fifteen Python/Qt cases passed after sabotage restoration.
Omitting the first station, accepting invalid counts, choosing incoming corner
tangents, disabling overflow checks, remembering defaults prematurely, losing
defaults on failure, ignoring remembered count/Top guard and removing the menu
action produced behavioral failures. A decimal-corner regression failed before
the roundoff fix and when the fix was disabled. Exact bytes were restored,
files touched, native code rebuilt and affected Python bytecode caches removed.
All temporary scripts, backups and logs stayed outside the repository.

The approved development diagnostic measured after/before ratios of 0.451227
with alignment and 0.692498 without, with no monitored heavy-process overlap.
Exact commits, fixture, environment and limits are recorded in
[testing and performance](../TESTING_AND_PERFORMANCE.md). These operation-only
results left real reference-machine interaction acceptance pending. It was
done on 2026-11-23 with the plan agreed first: the diagnostic gave 0.507
aligned and 0.748 kept on the floor, and in the main window 10,000 copies
were on screen in 129-141 ms and 100,000 in 613-697 ms, panned and zoomed
at the display's rate on the GPU and within 18 ms on the CPU
([report](../testing/reference-machine/2026-11-23/path-array.txt)).
