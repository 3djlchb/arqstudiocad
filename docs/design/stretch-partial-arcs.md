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
