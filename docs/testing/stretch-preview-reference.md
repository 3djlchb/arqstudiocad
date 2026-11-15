# STRETCH live preview: reference-machine handoff

Required after the approved development diagnostic (2026-11-15) produced
candidate/base edit ratios above one: initial 1.06330; five alternating
batches pooled 1.00521, overlapping batch ranges. The preview query itself
was 0.061386 ms on the Ryzen host. These are not reference-floor results.

## Scope and revisions

Use the actual i7-2600/GT 730, 16 GB, LMDE 7 Cinnamon/X11 reference machine.
Follow the agent workflow and reserve a distinct measurement branch first;
leave other agents' worktrees alone. The PR is #192; the maintainer merges
manually. No ProgeCAD is needed for this work.

Measured development source: base `755c67c`, candidate `bf2cbe9`, both retained
in the public history. Later main includes OPEN and ARRAY (#190/#191).
For current acceptance compare updated main (at least `66c0125`) against the
current #192 head, recording exact full SHAs actually built. The historical
pair may also be repeated separately; do not attribute its timings to the
newer integrated branch without measurement. Keep base and candidate environments separate; use the same
public/synthetic fixture, default release profile and repetitions. Do not
raise target-cpu, AVX2/FMA requirements or simulate performance by limits.

## Reproduce the diagnostic

The full fixture, calls and timing boundaries are specified in
[testing and performance](../TESTING_AND_PERFORMANCE.md), STRETCH preview
diagnostic. Use 100000 synthetic unit horizontal lines on a 3-unit grid,
100 selected endpoints, crossing boxes ±0.1, offset (2,3), no polygons and
an 800×600 Top view. Construct/open the fixture and view outside timing.

Measure the unchanged `stretch_entities` operation on each revision: three
warm-ups and twenty samples per batch, five batches alternating B/C and
C/B. Use a monotonic high-resolution timer, check the first moved endpoint,
undo outside timing and verify restoration. Keep all samples and report
batch medians, pooled medians and same-machine ratio. Monitor for competing
builds/tests and repeat a contaminated window, explicitly recording why.

Measure `CadView.stretch_outlines` separately on the candidate with the
same fixture: three warm-ups and twenty samples. Assert contour count,
screen endpoints and unchanged geometry/history on every call. Do not
confuse this pure query with a full interactive frame.

## Cursor and presentation

Collect `tools/reference_machine_report.sh` and
`uv run python tools/gpu_viewport_check.py` reports, following the reference
page's isolation and privacy rules. Exercise real STRETCH cursor movement
after a base point and at the Displacement prompt, with CPU rendering and
with the normally selected GPU path when available. Include straight and
bulged polylines, short and long crossing polygons, zoom/pan and cancellation.
Check the preview against committed geometry and ensure one undo step.
Record viewport dimensions, display scaling, settings, backend, selection
sizes, sample counts and actual observed frame/input latency. Use temporary
app configuration; no global desktop keys or real QSettings modifications.

Store sanitized reference reports as specified by
[the reference-machine page](reference-machine.md). Add actual observations
and ratios to its table and the PR; never substitute the historical timing
table for current candidate validation. No fixed time threshold or multiplier
is inferred from the development host. The maintainer reviews the evidence
before accepting the floor and marking #192 ready.

Standalone partial arcs remain refused: their Windows behavioral comparison
is a different pending task, described in [stretch-arcs-windows.md](stretch-arcs-windows.md).
