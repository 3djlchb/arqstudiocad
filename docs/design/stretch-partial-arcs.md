# Partial standalone arc STRETCH implementation

## Reservation (2026-11-16)

Codex, Windows development role. Implement the maintainer-approved sagitta
rule recorded in [the native comparison](../testing/stretch-arcs-windows.md).
Base: `e0da06c2914ce082f4d6b88e27dd53a9f50cfe73`.

Scope: a crossing rectangle or polygon selecting one standalone ARC endpoint;
shared read-only preview and commit, preserved entity properties and plane,
whole-arc translation controls, atomic refusal of a collapsed chord, and
undo/redo. Keep the existing Top-view restriction. No new command, dependency,
codec change or GPU path. Do not use ProgeCAD while another agent owns it.

Expected areas: arq-geometry arc reconstruction, arq-document stretch planning,
layer-appropriate geometry/document/core/binding/command/preview tests, the
STRETCH design and roadmap. New visible messages require Spanish translation.

Validation: measured native coordinates already recorded for 90/180/270-degree
arcs; local geometry and plane controls; preview/commit equivalence; history,
rollback and DWG/DXF safe-save round trips; exact-byte sabotage verification;
all mandatory local checks and the explicit public-codec corpus. Existing
native evidence describes ProgeCAD's behavior, not independent acceptance of
our new outputs. Any new external check and reference-floor interaction or
performance acceptance remain separate, explicitly reported follow-ups.

Dependencies: no unmerged PR. Surface PR #183 remains paused separately. Check
GitHub reservations again before publication and keep this PR a draft until
implementation and local checks are complete.

## Implementation (2026-11-20)

`Arc::with_endpoints_preserving_sagitta` reconstructs the circular arc in
arq-geometry. With sweep theta and radius r, the retained height is
`h = 2 r sin(theta/4)^2`, avoiding subtraction of nearly equal cosines.
For a new half-chord q, the center is displaced from the chord midpoint
along its left perpendicular by `(q^2/h - h)/2`, and the radius is
`hypot(q, offset)`. This preserves minor and major arcs, including a
semicircle changing to either side of 180 degrees. Angles remain radians.
The center and angles are computed in a local frame before translation.

The document's shared `stretch_plan` supplies both preview and commit.
One selected endpoint moves once through the union of rectangles/polygons;
no selected endpoint leaves the arc unchanged; both endpoints or a whole
object selection translate the arc. Existing plane conversion writes back
upward/downward OCS without replacing the entity or its metadata. Tilted
planes and ellipses keep their existing restrictions.

Chords within the document's existing linear tolerance of zero are refused.
Nonfinite intermediates and results whose reconstructed endpoints cannot
meet that tolerance are refused too, rather than silently drifting. Very
shallow arcs with large endpoint changes can reach this representability
limit. Failure preserves the entire transaction and its history. Python
only translates the invalid-geometry refusal; no geometry is computed there.

The tests compare the six recorded ProgeCAD cases A1-A4b, properties and
planes, read-only preview, undo/redo, rollback, spatial queries, command
input/cancellation and synthetic DWG/DXF reopenings. See
[Windows validation](../testing/stretch-partial-arcs-windows.md) for actual
results, sabotage checks and limitations. No new codec or dependency is used.
