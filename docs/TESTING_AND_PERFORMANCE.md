# ArqStudioCAD — Testing and performance

How the project proves it works and how fast it is. Master plan §10, §14;
the binding minimum is the technical contract §12. A check that inspects
nothing is a failure, not a pass.

## The test pyramid, today

| Level | Where | Run with |
|---|---|---|
| 1. Rust unit tests | beside each crate (`src/*_tests.rs`, `#[cfg(test)]`) | `cargo test` |
| 2. Rust–Python integration | `tests/python/test_core_*.py`, `stubtest` | `uv run pytest`, `uv run stubtest arqstudiocad._core` |
| 3. Interface and user flows | `tests/python/test_*.py` with pytest-qt, offscreen | `uv run pytest` |
| 4. Format reading, writing and round trip | `crates/arq-core/tests/corpus.rs`: generated drawings in every version (always), the engine's samples and the maintainer's drawings (`--ignored`); `tools/dwg-bench` across libraries | `cargo test`; `ARQ_CORPUS_DIR=… cargo test -p arq-core --test corpus -- --ignored` |
| 5. Performance and memory | `crates/arq-core/benches`, `crates/arq-render/benches`, `tools/dwg-bench` (time and peak memory per operation) | `cargo bench`; the bench's report |
| 6. Cross-platform and installation | CI on every pull request and push to `main`; packaging workflow; `tools/check_cpu_floor.sh` | GitHub Actions; WSL |
| 7. Regression with real files | the maintainer's drawings (never committed), regression drawings per fix (ADR 0017 D5.4) | corpus |

Every change is sabotage-checked: the behavior is broken on purpose and a
test must fail; a behavior no test catches either gets a test or is
removed.

## Qt settings isolation

The Python test harness redirects implicit `QSettings()` instances to a fresh
INI directory for each test, for both user and system scopes. This also avoids
the native Windows registry and prevents system-preference fallback. The paths
remain temporary through widget destruction and process shutdown. Tests using
explicit settings files (including portable mode) must supply a temporary path;
do not use explicit native/organization-based constructors, which bypass Qt's
default format. Child processes do not inherit Qt's in-process redirection and
must receive their own temporary settings file (as the GPU application probe does).
`test_settings_isolation.py` verifies the application factory, system fallback,
and separation between tests, checking containment before writing.

## Measured so far

| Measure | Result | Source |
|---|---|---|
| Spatial query, 100,000 entities | Indexed: 1% window ≈ 35.7 µs; all entities ≈ 6.95 ms. Previous bounding-box scan: ≈ 18 ms | `cargo bench --locked -p arq-core --bench query`, Windows NT 10.0.26300 AMD64, Intel family 6 model 158, release, Criterion 20 samples (2026-10-17) |
| Pick, window/crossing selection, object snap; 100,000 entities | Indexed: pick ≈ 1.18 µs (hit), window ≈ 520 µs, crossing ≈ 535 µs, snap ≈ 1.55 µs (hit). Previous full-drawing scan: ≈ 54–56 ms each | Same benchmark and environment; cursor placed on drawn geometry (2026-10-17) |
| One recorded change plus its undo; 100,000 entities | ≈ 5.38 µs (index update and undo); previous baseline ≈ 1.5 µs | Same benchmark and environment (2026-10-17) |
| Scene build / render, 100,000 entities at 1080p (CPU) | 59 ms / 69 ms, 1 % redraw 10 ms | `cargo bench -p arq-render`, ADR 0013 |
| Read DWG/DXF, 201 drawings, median / p95 | 12.6 ms / 38.5 ms; peak memory median 11 MiB | `tools/dwg-bench` |
| Read a 100,000-entity DXF | 0.37 s, 84 MiB | `tools/dwg-bench` |
| TRIM Quick, 100,000 unrelated lines + target + 2 cutters | Median 58.041 ms → 4.099 µs; after/before 0.0000706 | LMDE 7 development host, Ryzen 9 5900X; native Criterion benchmark, optimized Cargo bench profile, 20 samples (2026-11-15); method below |
| EXTEND Quick, same fixture size | Median 60.694 ms → 2.735 µs; after/before 0.0000451 | Same host, profile and repetitions; method below |
| Rectangular ARRAY, 100 lines × 10 rows × 10 columns | Grouped COPY median 27.539 ms → atomic ARRAY 19.784 ms; after/before 0.718 | LMDE 7 development host, Ryzen 9 5900X; release PyO3 extension, Python 3.13.5, 2 warm-ups + 10 samples (2026-11-15); diagnostic only; method below |
| STRETCH edit, 100 selected lines in 100000 lines | Initial median 0.168242 → 0.178892 ms (1.063); five alternating batches pooled 0.168732 → 0.169612 ms (1.005) | LMDE 7 development host, Ryzen 9 5900X; release PyO3 extension; 3 warm-ups + 20 samples per run (2026-11-15); diagnostic only; reference check pending |
| STRETCH read-only preview, same selection | Median 0.061386 ms per contour query | Same host/profile; 3 warm-ups + 20 samples; excludes Qt overlay construction, painting and frame scheduling |
| STRETCH preview, one 402-corner crossing polygon over 100 of 100000 lines | `stretch_outlines` 240.5 → 8.0 ms (0.033); Criterion: validating per pair 235.94 ms, once for all 3.08 ms | Reference machine, i7-2600, 2026-11-19; three alternating runs of 3 warm-ups + 20 samples; `cargo bench -p arq-core --bench query`; method below |
| STRETCH preview cursor frame on the CPU, 100 two-arc polylines among 100000 entities | 22.88, 23.23 → 12.42, 11.84 ms; the overlay painted 16.12 → 7.50 ms | Reference machine, i7-2600, 2026-11-19; two alternating runs of 60 synthetic cursor moves after 3 warm-ups; method below |
| World XYZ overlay, 800 x 600 CPU viewport, 100 lines | Top 0.011100 → 0.121881 ms (10.980x); SW isometric 0.010960 → 0.105006 ms (9.581x) | LMDE 7 development host, Ryzen 9 5900X; release extension, Python 3.13.5, 5 warm-ups + 100 samples per view (2026-11-17); overlay-only diagnostic; reference acceptance pending |
| ID capture, 100000 elevated lines, 800 x 600 view | Top XY → XYZ: 0.001710 → 0.001540 ms (0.901x); SW isometric plane intersection → XYZ reference: 0.000420 → 0.071691 ms (170.693x) | LMDE 7 development host, Ryzen 9 5900X; release extension, 5 warm-ups + 100 captures per view (2026-11-17); different isometric capabilities, query-only diagnostic; reference acceptance pending |
| BREAK, 1000-vertex path in 100000 unrelated lines | Open pooled median 0.347343 → 0.348968 ms (1.00468x); new closed case 0.376989 ms | LMDE 7 development host, Ryzen 9 5900X; release extension, 5 alternating pairs, each with 5 warm-ups + 100 samples (2026-11-17); operation-only diagnostic; reference acceptance pending |
| Polar ARRAY, 100 lines × 100 positions over 360 degrees | Rotate: grouped COPY/ROTATE 50.053234 → atomic ARRAY 19.694723 ms (0.393476x); no rotation: grouped COPY 28.168532 → 19.834554 ms (0.704139x) | LMDE 7 development host, Ryzen 9 5900X; release PyO3 extension, Python 3.13.5, 2 warmups + 10 samples per case (2026-11-20); operation-only diagnostic; reference interaction pending |
| Path ARRAY, 100 lines × 100 positions on a mixed open path | Align: grouped COPY/ROTATE 40.513941 → atomic ARRAY 18.280985 ms (0.451227x); no alignment: grouped COPY 26.347081 → 18.245295 ms (0.692498x) | LMDE 7 development host, Ryzen 9 5900X; release PyO3, Python 3.13.5, 2 warmups + 10 samples per case (2026-11-20); operation-only diagnostic; reference interaction pending |

### Quick TRIM/EXTEND diagnostic comparison (2026-11-15)

Base production code: `a2236f4deb5a4a168acfba2ad66c628076a10ba4`, with the
same added benchmark harness as candidate `fff33a0bcd07312e4c7ac975ccfc1a9d0d39ad82`.
Harness: `crates/arq-core/benches/query.rs`, SHA-256
`f1572ee4c33a7526ab3a94563b06fd453369531373cd6aa4c8d396c302705b45`.
Rust 1.99.0; LMDE 7, Linux 6.12.117+deb13-amd64, Ryzen 9 5900X (24 logical
CPUs); repository-default optimized bench profile, no raised CPU baseline.
These are same-machine diagnostics, not reference-floor acceptance.

Each fixture has N distant short lines, target `(0,0)`–`(10,0)` and two
vertical cutters: x=3/7 for TRIM, x=20/30 for EXTEND. Total entities N+3.
The target, picks and unrelated grid are identical before and after. Criterion
uses 20 samples, 1 s warm-up and 3 s requested measurement time per case.
`iter_custom` accumulates operation time; fixture construction and undo/reset
are outside that interval. Every iteration checks the edited endpoint and
restored original geometry. Undo preserves normal lifetime ID allocation;
the drawing geometry and entity count are restored, not the ID allocator.
The measured operation includes its normal transaction/history bookkeeping.

| Unrelated lines | Operation | Base median | Candidate median | After / before |
|---|---|---|---|---|
| 10,000 | TRIM | 4.867 ms | 4.050 µs | 0.000832 |
| 10,000 | EXTEND | 4.792 ms | 2.696 µs | 0.000563 |
| 100,000 | TRIM | 58.041 ms | 4.099 µs | 0.0000706 |
| 100,000 | EXTEND | 60.694 ms | 2.735 µs | 0.0000451 |

Commands: `cargo bench --locked -p arq-core --bench query -- "TRIM EXTEND Quick" --save-baseline before`
on the base, then the identical harness with `--baseline before` on the
candidate. Values are Criterion's median point estimates. A first base run
was discarded after detecting a concurrent agent build; the base was repeated
in an idle window. Candidate measurement monitored heavy build/test processes
throughout and detected no overlap. Measurements were separate from checks
and compilation. No desktop/system setting or CPU feature was changed.

The distant-grid scenario demonstrates candidate pruning, not a universal
speedup. Dense/overlapping drawings and broad arc-circle envelopes can retain
many candidates. Python command dispatch, viewport redraw and full fence
latency are not measured here. Validate actual interaction on the i7-2600 /
GT 730 reference machine before asserting minimum supported performance;
never convert these numbers using a CPU multiplier. No ProgeCAD is required.

On the reference machine (2026-11-15), the same harness on the same base
and on `main` at `22fe1e4`: TRIM 124.25 ms → 10.276 µs and EXTEND
135.87 ms → 7.376 µs with 100,000 unrelated lines (main / base 0.0000827
and 0.0000543), and with 10,000 lines 10.964 ms → 9.474 µs and 11.912 ms
→ 7.026 µs: the cost no longer grows with the drawing on the floor either
([benchmarks](testing/reference-machine/2026-11-15/benchmarks.txt)).
The core operation is validated there; Python dispatch, the viewport's
redraw and full fence latency still are not measured.

### Rectangular ARRAY diagnostic comparison (2026-11-15)

Maintainer-approved scenario: 100 synthetic lines replicated in a 10 × 10
grid. Base `755c67c9ec6ddf11bb421f9026119a0b1c7e2a94` uses 99 existing
`copy_entities` calls inside one `doc.group("ARRAYRECT")`; candidate
`7e76d2d90f9e685bc8edfa357904f2de103f0521` uses one
`rectangular_array(ids, 10, 10, 20, 20, 0)` call. Both preserve originals,
produce 10000 entities and undo in one step. This compares equivalent copy
workflows, not an earlier ARRAY implementation, which did not exist.

Host: LMDE 7, Linux 6.12.117+deb13-amd64, Ryzen 9 5900X; Python 3.13.5,
Rust 1.99.0, the `uv sync` editable extension built in the repository-default
Cargo release profile on each commit, no CPU baseline/feature changes.
No viewport is constructed, so there is no viewport-size or GPU setting.
No QSettings, system preference or private file is accessed. Both runs use
the same script and synthetic coordinates. Heavy build/test processes were
monitored every 250 ms throughout both runs; none were detected.

Fixture: source line i (0–99) runs from `(i % 10, i // 10)` to
`(i % 10 + 0.5, i // 10)`. Every cell uses offset `(column * 20, row * 20)`;
cell (0,0) is skipped. Garbage collection is disabled during the 12 calls;
the first two are warm-ups and ten elapsed values are sampled using
`time.perf_counter_ns()`. Time includes Python API dispatch, mutation,
transaction/history and index maintenance. Fixture creation, assertions
and undo are outside timing. Every call checks entity count, the last
copy's endpoints `(189,189)`–`(189.5,189)`, the ARRAYRECT history entry and
restoration to 100 entities after undo. Normal ID allocation continues
between iterations; source geometry and count are restored.

| Workflow | Median, ms | Candidate / base |
|---|---|---|
| Base grouped COPY | 27.5389385 | 1.000 |
| Candidate atomic ARRAY | 19.7838075 | 0.718 |

These short same-machine diagnostics do not measure command prompts,
redraw, interactive latency or minimum-machine acceptance. The extra
geometry can cost redraw time: exercise representative arrays on the actual
i7-2600/GT 730 host with `tools/reference_machine_report.sh` and
`uv run python tools/gpu_viewport_check.py` before asserting floor performance.
Do not convert these timings using a CPU multiplier. ProgeCAD is unnecessary
for this diagnostic; external behavioral validation remains pending.

**On the reference machine (2026-11-19, the same commits):** five
alternating runs, pooled 56.1196 → 40.5945 ms (0.7234). In the main window
on `ab94cc8`, ARRAY 10 × 10 of 100 lines shows its 10,000 entities 98 ms
after the command is typed on the CPU (91 ms on the GPU); of 1,000 lines,
100,000 entities, 609 ms (658 ms). Then the GPU pans and zooms them at the
display's 16.6 ms, the CPU at 33 and 19 ms in a 1098 × 347 viewport.
Details in [the array report](testing/reference-machine/2026-11-19/array.txt).

### STRETCH preview diagnostic (2026-11-15)

Maintainer-approved scenario: 100 selected lines in 100000 synthetic lines;
compare committing the same stretch before/after, and measure the new
read-only contour query separately. Measured base
`755c67c9ec6ddf11bb421f9026119a0b1c7e2a94`; measured candidate
`bf2cbe975f4dc4e7b39e1be1d35c84a8d9fcd7c4`. Both used their existing
`uv sync` release extensions, Rust 1.99.0, Python 3.13.5 on LMDE 7,
Linux 6.12.117+deb13-amd64, Ryzen 9 5900X (24 logical CPUs). No raised
CPU baseline, feature flags, system preferences or private drawings.
These values were collected before integrating main's later OPEN and ARRAY
changes. They describe the exact historical revisions above, not a timing
claim for the newer integrated branch. Current reference-floor validation
must record its actual updated base and candidate commits separately.

Fixture: a temporary ASCII DXF with 100000 LINE entities; source i has start
`((i % 1000)*3, (i // 1000)*3)` and end `(start.x+1, start.y)`. Query box
`(-0.1,-0.1)`–`(298.1,0.1)` selects the first 100. Each has a crossing box
of ±0.1 around its endpoint. Displacement is `(2,3)`; no polygons. A Top-view
CadView, 800×600 physical pixels, fits the same complete drawing once.
Fixture writing/opening, view construction and all assertions are outside
operation timing. No GPU is attached or probed, and no Qt settings are read.

Each call is timed with `time.perf_counter_ns()`, garbage collection disabled
only during sampling: three warm-ups, then twenty samples. Commit uses
`doc.stretch_entities(ids, (2,3), areas, [])` in both versions; after each
sample it checks the first endpoint `(3,3)`, undoes outside timing and checks
restoration to `(1,0)`. Preview uses
`view.stretch_outlines(ids, (2,3), areas, [])`; every call checks 100 contours,
the first screen endpoints, unchanged history and original endpoint.
Time includes API dispatch and normal operation bookkeeping; preview time
includes conversion to screen contours, not the Qt overlay loop or drawing.

The initial sequential run found base 0.1682415 ms, candidate 0.1788915 ms
(ratio 1.06330), and preview 0.061386 ms. Candidate samples had a burst of
higher values, so the edit comparison was repeated in five batches with
order B/C, C/B, B/C, C/B, B/C. Every batch recreates the identical fixture
and uses the same three warm-ups and twenty samples. No samples were dropped.

| Batch | Base median, ms | Candidate median, ms |
|---|---|---|
| 1 | 0.167392 | 0.167972 |
| 2 | 0.167422 | 0.170452 |
| 3 | 0.1683415 | 0.172142 |
| 4 | 0.173342 | 0.169807 |
| 5 | 0.167031 | 0.1669715 |
| Pooled 100 samples per version | 0.168732 | 0.1696115 |

Pooled candidate/base is 1.00521; batch ranges overlap. These short
measurements establish neither an improvement nor a conclusive regression.
The initial and pooled ratios are both above one, so the reference-machine
check remains required by the project's performance rule. A process monitor
polled heavy build/test processes every 250 ms throughout both measurement
windows; none were detected. Temporary scripts and raw logs stayed outside
the repository.

Follow [the reference handoff](testing/stretch-preview-reference.md) before
claiming floor acceptance. In particular, 0.061386 ms is not a complete
cursor-frame time; curved/dense selections and long crossing polygons are
not represented. Do not convert these development timings with a fixed
CPU multiplier.

**On the reference machine (2026-11-19, the integrated pair `66c0125` →
`5ac83f4`):** the same edit, five alternating batches, pooled 0.361476 →
0.360268 ms (0.99666); the preview query 0.117402 ms. In the main window,
synthetic cursor moves at the displacement prompt with 100 lines stretched
cost 4.18 → 6.88 ms a CPU frame (95th percentile 10.02 ms) in a
1098 × 347 viewport and 4.39 → 7.84 ms at 800 × 600; on the GPU every
frame stays at the display's 16.4-16.7 ms. Cancelling leaves the history
unchanged and the committed stretch is one undo step. The cases left out
at first, measured the same day ([stretch cases](testing/reference-machine/2026-11-19/stretch-cases.txt)): zoom
and pan during the prompt, CPU 6.07 → 8.98 ms, GPU unchanged; 100 two-arc
polylines, CPU 8.83 → 24.06 ms (95th percentile 27.05 ms), GPU 9.29 ms
(12 ms on the CPU since; below, "Overlay painting without a QPointF a point");
a 402-corner crossing polygon, 4.00 → 252.35 ms on the CPU and 241.79 ms
on the GPU. The polygon's cost is its validation, repeated for each of the
100 entities: `stretch_outlines` takes 248.741 ms with the polygon given
per entity, 0.114 ms with rectangles, and one validation plus a selection
query 4.122 ms; #213 validates it once (below, "Shared crossing polygon
validation"). Not covered: a hand on a real mouse. Details in the [2026-11-19 reports](testing/reference-machine/2026-11-19/).

## Metrics to keep

Opening and saving time by file size and entity count; peak memory and its
growth while editing; selection, regeneration and spatial-query time;
import/export time and exchange fidelity; cost of transactions, undo/redo
and model updates; scaling on several cores; start-up time and base
consumption.

## Rules for benchmarks

Each names its input files, environment (machine, OS, build profile) and
repetitions, and is reproducible; figures from incompatible machines are not
compared. The **floor** is the reference machine (GeForce GT 730, i7-2600,
16 GB, LMDE 7, ADR 0013 D5.8): what must run there is measured there or
under its emulated CPU (`tools/check_cpu_floor.sh`). Its measurements,
the reference every development machine compares against, are in
[testing/reference-machine.md](testing/reference-machine.md).

## Engineering results

Structural, energy, hydraulic and other calculations are accepted only with
reference cases, units, numerical tolerances and independent verification.

### World-coordinate overlay diagnostic (2026-11-17)

Maintainer-approved scenario: CPU viewport 800 x 600 logical/physical pixels
(DPI 1), 100 synthetic lines, Top and SW isometric, five warm-ups followed by
100 samples per view. Base `78ae515c0295f1ad4caee730054c875aa5bfa2be`;
candidate `8274d9075e4e51414219959ce388672630a39515`. Both use their own
`uv sync` release extensions, Python 3.13.5, PySide6 6.12.0, default Qt font,
LMDE 7/Linux 6.12.117+deb13-amd64, Ryzen 9 5900X. No build baseline or system
configuration changes; no private drawing. The later documentation commit
does not change the measured implementation.

Fixture: for i = 0..99, source start `(i % 10 - 5, i // 10 - 5)` and end
`(start.x + 0.5, start.y + 0.25)`. For each view, fit normally and pan so the
projected origin is `(400,300)`. Use `QT_QPA_PLATFORM=offscreen`, no GPU
attachment/probe, no cursor, selection or active command; settings paths are
redirected to a temporary INI directory. An 800 x 600 RGB32 QImage is filled
with the model background outside each timed interval.

Measure `Viewport.overlay()`, QPainter construction, `overlay.paint` and
`QPainter.end()` with `time.perf_counter_ns()`, garbage collection disabled
only during sampling. Assert 100 entities, nonempty render geometry and a
complete render outside timing, and check every overlay has XYZ labels on
the candidate and no labels on the base. Fixture creation, model rendering,
assertions and image filling are outside timing. One identical temporary
harness was used on both revisions, SHA-256
`6f12fcc54a38b621c38252ada1199847087ba4f1669039b7b3a1157c08910eed`.

| View | Base median, ms | Candidate median, ms | After / before | Added, ms |
|---|---|---|---|---|
| Top | 0.011100 | 0.121881 | 10.980 | 0.110781 |
| SW isometric | 0.010960 | 0.105006 | 9.581 | 0.094046 |

The base overlay in this scenario is empty; the candidate paints the new
arrows, ring and labels. These ratios describe added overlay work, not a
whole-frame slowdown. They exclude model rasterization, scheduling, native
GPU presentation and GPU label uploads. Heavy build/test processes were
monitored every 10 ms across both runs; none were detected. Scripts and raw
samples stayed outside the repository. No QSettings or system preference was
changed outside the temporary configuration.

Both ratios exceed one, so reference-machine validation remains required.
Run `tools/reference_machine_report.sh` and
`uv run python tools/gpu_viewport_check.py` on the actual i7-2600/GT 730, and
repeat this overlay scenario at identical commits/settings before claiming
floor acceptance. Functional CPU/GPU overlay equivalence passed locally on
Vulkan, but that is not minimum-machine performance acceptance. No ProgeCAD
is needed.

**On the reference machine (2026-11-19, the same commits):** Top 0.045464
→ 0.323360 ms (7.11), SW isometric 0.035770 → 0.298076 ms (8.33), about
0.28 ms added a frame. The GPU viewport check on `main` at `992acf8`, with
the axes, pans at 16.7 ms and zooms at 16.6 ms on the GT 730, its overlays
0 pixels apart from the CPU's. Details in the [2026-11-19 reports](testing/reference-machine/2026-11-19/).

### ID XYZ capture diagnostic (2026-11-17)

Maintainer-approved scenario: 100 captures in Top and SW isometric over the
same 100000 synthetic lines. Base `6f9379da2080f211f69a9cf15cc725a04cf3020f`
(including MATCHPROP); candidate `ced90bab1893e342a24dd6389760d14663cd10fb`.
Both revisions use their own `uv sync` repository-default release extension,
Python 3.13.5, Rust 1.99.0, LMDE 7/Linux 6.12.117+deb13-amd64, Ryzen 9 5900X.
The documentation-only follow-up does not change the measured implementation.
No CPU baseline/feature changes, private files or system settings.

The same temporary ASCII DXF has 100000 LINEs. Source i (0..99999) starts at
`((i % 1000)*3, (i // 1000)*3, 7)` and ends one unit along +X, at the same Y/Z.
Use an 800 x 600 physical-pixel CadView. In each view, zoom to 20 pixels per
model unit and pan so source 50050's midpoint `(150.5,150,7)` projects to
`(400,300)`. Aperture is 7 physical pixels; modes END/MID/CEN/INT in both
versions. No Qt application, widget, native GPU probe, rendering or QSettings
is constructed/accessed by this query diagnostic.

Five warm-ups precede 100 samples timed with `time.perf_counter_ns()` and GC
disabled only around each call. In Top, base calls
`document.snap(view.to_world(*screen), 7/view.scale, modes)`; candidate calls
`view.snap_point3(*screen, 7, modes)`. In SW isometric, the base ID previously
called only `view.plane_point(*screen)`, without a geometry snap; the candidate
uses the XYZ reference query. Fixture loading, scene/index construction,
view setup and assertions are outside timing. Every call checks the source
midpoint (XY on the base Top, XYZ on the candidate), or the round-trip screen
projection of the base isometric XY-plane intersection. Entity count and
unchanged dirty/history state are checked too.

| View / capability | Base median, ms | Candidate median, ms | After / before | Added, ms |
|---|---|---|---|---|
| Top, planar reference → XYZ reference | 0.001710 | 0.001540 | 0.900585 | -0.000170 |
| SW isometric, XY-plane intersection → XYZ reference | 0.000420 | 0.071691 | 170.692857 | 0.071271 |

The isometric ratio compares different capabilities: the old call could not
identify the elevated reference. It is added feature cost, not a whole-frame
slowdown. The very small Top values from one short run do not establish a
universal speedup. These measurements exclude Python command dispatch, Qt
markers, GPU text uploads and actual cursor-frame scheduling.

An initial attempt was blocked before timing because another build was
running. The valid measurement window monitored heavy build/test processes
every 10 ms; none were detected. Temporary harness SHA-256:
`318ec5ee784939bbb4b4a4839863c200ac4e7dd27d75657c94256ad74934f973`. Raw samples/scripts and the generated DXF remained outside
the repository. Both extensions keep the normal release profile and ISA floor.

Reference-machine acceptance remains pending because ID now adds a geometric
query in non-plan views, and reference traversal adds a branch to drawing
visits. On the actual i7-2600/GT 730, repeat the query scenario with these exact
commits/settings and exercise ID cursor capture in Top/SW isometric, CPU and
GPU, at 800 x 600 and the reference report's normal viewport size. Also run
`tools/reference_machine_report.sh` and `uv run python tools/gpu_viewport_check.py`
for redraw checks. Do not turn these development timings into reference-host
times using a CPU multiplier. No ProgeCAD is required.

**On the reference machine (2026-11-19, the same commits):** Top 0.004282
→ 0.003193 ms; SW isometric 0.000653 → 0.142791 ms, the added 3D reference
search. Synthetic cursor moves at ID's prompt in the main window cost the
same before and after: about 1.3-2.2 ms a CPU frame and the display's
16.4-16.7 ms on the GPU, in Top and SW isometric, at 800 × 600 and
1098 × 347. Details in the [2026-11-19 reports](testing/reference-machine/2026-11-19/).

### Closed BREAK diagnostic (2026-11-17)

Maintainer-approved scenario: a synthetic 1000-vertex polyline inside a drawing
of 100000 unrelated lines. Compare the existing open operation before/after,
and measure the newly supported closed operation separately. Base
`d83b8610846d1aed7113dce096293d4f0032b58c`; implementation candidate
`aabdeace0d4abd8f056d767b151936ed244e89e2`. The documentation-only follow-up
does not change the measured implementation. Both use their own `uv sync`
release extension, Python 3.13.5, Rust 1.99.0, LMDE 7/Linux
6.12.117+deb13-amd64, Ryzen 9 5900X. No raised CPU baseline or feature flags.

Fixture vertex i is `(100*cos(2*pi*i/1000), 100*sin(2*pi*i/1000))`, bulge zero.
Separate open/closed ASCII DXFs differ only in the closed flag. Unrelated line
j runs from `(10000+(j%1000)*3, 10000+(j//1000)*3)` one unit along +X. There are
100001 entities before each call. Pick vertices 100 and 400. The open result
is two polylines with 101 and 600 vertices; the closed result is one open
polyline with 701 vertices, from vertex 400 through vertex zero to vertex 100.
All fixtures are synthetic and generated outside the repository.

Five warm-ups precede 100 timed `document.break_entity` calls per case per
run, with `time.perf_counter_ns()` and GC disabled only during each call.
Time includes normal API dispatch, mutation, history and index maintenance.
Fixture creation/loading, entity-count/geometry/change-set assertions and
undo are outside timing. Every call checks the surviving ID, piece/vertex
counts, cut endpoints and BREAK history entry; undo restores the original ID
and its 1000 vertices and 100001 total entities. Normal ID allocation continues
for open-path copies. No Qt application, viewport, GPU probe or QSettings is
used, so model redraw and cursor-frame latency are not measured.

The first pair showed a 1.2594 open-operation ratio. Reversing order did not
reproduce it, so the same scenario was repeated in five alternating pairs
(B/C, C/B, B/C, C/B, B/C), preserving all samples:

| Pair | Base open median, ms | Candidate open median, ms | Candidate closed median, ms |
|---|---|---|---|
| 1 | 0.3475140 | 0.4376590 | 0.3771240 |
| 2 | 0.3802985 | 0.3474180 | 0.3814740 |
| 3 | 0.3489280 | 0.3433130 | 0.3778235 |
| 4 | 0.3413180 | 0.3473285 | 0.3754335 |
| 5 | 0.3458680 | 0.3417530 | 0.3754030 |
| Pooled 500 samples per case | 0.3473430 | 0.3489680 | 0.3769885 |

Pooled open after/before is 1.004678. The initial 26% increase was not
reproduced consistently; the short runs show variability, not a demonstrated
universal speedup or equivalence guarantee. The new closed measurement has no
successful old closed-operation counterpart and is not a before/after ratio.
All windows monitored heavy build/test processes every 10 ms; none overlapped.
The identical temporary measurement harness has SHA-256
`ed9b3e3c71ab457dfeff536be95dc63f657e6fdb9facd8718c4b0639ece0e677`.
Raw logs, scripts and synthetic files stayed outside the repository; no private
drawing, real app preference or system setting was changed.

The pooled ratio is above one, so actual reference-machine validation remains
required before floor acceptance: repeat these operations on the i7-2600/GT 730
and exercise the resulting paths interactively with
`tools/reference_machine_report.sh` and `uv run python tools/gpu_viewport_check.py`.
No time conversion using a CPU multiplier, minimum-machine claim or new
ProgeCAD execution is implied.

**On the reference machine (2026-11-19, the same commits):** five
alternating pairs, open pooled 0.706338 → 0.715927 ms (1.01358), the pair
medians overlapping; closed 0.709716 ms. Details in the [2026-11-19 reports](testing/reference-machine/2026-11-19/).

### Shared crossing polygon validation (2026-11-19)

Found on the reference machine while checking STRETCH's preview with a
crossing polygon (#212, its `stretch-cases.txt`): a 402-corner polygon over 100 selected lines previewed at about
250 ms a cursor move. The viewport gives the selection's polygon once per
entity, and `CadView.stretch_outlines` and `CadDocument.stretch_entities`
validated each pair's copy with `SelectionPolygon::new`, whose
self-intersection check compares every edge with every other. Now both go
through `arq_geometry::selection_polygons`, which validates each distinct
contour once (keyed by its exact coordinates) and shares it, keeping the
pairs' order and the first invalid contour's error.

Measured on the reference machine (i7-2600, LMDE 7, Rust 1.99.0, Python
3.13.5), base `main` at `ab94cc81ad2f02fd41a3865809580e62c9271050`
against this change, each in its own `uv sync` build. The harness
(polygon_probe.py, kept outside the repository like the others) opens
100000 generated lines (line i from `((i % 1000)*3, (i // 1000)*3)` one
unit along +X), an 800 × 600 Top `CadView` fitted to them, and a comb
polygon of 402 corners with a 0.2-wide tooth around each of the first 100
lines' ends; three warm-ups and twenty samples a call, three runs
alternating base and fix, medians:

| Call | Base, ms | Fix, ms |
|---|---|---|
| `stretch_outlines`, the polygon given for each of the 100 entities | 241.150, 237.400, 240.540 | 7.987, 8.101, 8.048 |
| `stretch_outlines`, a rectangle per entity instead | 0.111, 0.121, 0.110 | 0.117, 0.109, 0.114 |
| `select_crossing_polygon` (one validation and a query) | 4.082, 4.190, 4.067 | 4.097, 4.143, 4.128 |

`cargo bench --locked -p arq-core --bench query -- "STRETCH crossing polygon"`
(the new group, the same 402-corner polygon and 100 of 100000 lines, built
in the benchmark) on the same machine, Criterion's [low median high]:

| Benchmark | Time |
|---|---|
| validate per pair (402 corners x 100) | [233.31 ms 235.94 ms 238.91 ms] |
| validate shared (402 corners x 100) | [3.0540 ms 3.0818 ms 3.1259 ms] |
| preview, polygons validated | [2.6878 ms 2.7036 ms 2.7197 ms] |

Of the fix's 8 ms, one validation is about 3 ms and the preview itself
2.7 ms; the rest is converting the 100 copies of the polygon's points from
Python and comparing them. Rectangles and single polygons cost what they
did. The commit (`stretch_entities`) takes the same path.

In the main window, as in #212's `stretch-cases.txt` (the 1366 × 740
window, a 1098 × 347 viewport, synthetic cursor moves at the displacement
prompt with that polygon over the 100 lines; harness SHA-256
`d68e18a970f670d7d41fcf4fde00b2f03ba4610a7b4f93e73b2fc30e4c51ac49`), two
alternating runs each, median (95th percentile) per cursor move:

| Renderer | Base, ms | Fix, ms |
|---|---|---|
| CPU (safe mode) | 260.74 (270.14), 264.83 (286.90) | 15.63 (16.64), 16.28 (18.45) |
| GPU (OpenGL ES on the GT 730) | 248.71 (257.77), 256.18 (266.98) | 11.11 (12.62), 11.08 (15.16) |

From about four frames a second to the display's rate on the GPU and
about one 60 Hz frame on the CPU.

### Overlay painting without a QPointF a point (2026-11-19)

Found on the reference machine with STRETCH's preview of 100 two-arc
polylines (#212, its `stretch-cases.txt`): a CPU cursor frame of 24 ms,
over one 60 Hz frame, against 8.8 ms without the preview. A frame's parts,
timed one by one in the main window with the prompt up (median of 50,
harness `frame_breakdown.py`, SHA-256
`73e1fe03f967309b82de5ae1b6e873f16a35e50213e411ee5320e673973a1024`):
the overlay had 409 strokes and 8,418 points; `stretch_outlines` took
1.45 ms, building the overlay 6.46 ms (the preview's strokes 4.2 ms, the
selection's 2.0 ms) and painting it with QPainter 16.12 ms. The core's
tessellation is the one the drawing uses (0.25 pixel); a stretched arc
grows, and its points with it.

Painting cost one `QPointF` made in Python for each point, and a pen,
brush and antialiasing set for each stroke. A bench of 400 strokes and
8,400 points on an offscreen image (`paint_bench.py`, SHA-256
`4e39c996f05c0f8776cbf9461b2bc8f420001e54d3c8b5ca98d61754abf1da25`):

| Way | Median, ms |
|---|---|
| as before: state per stroke, a `QPointF` per point | 14.44 |
| the state set once | 10.18 |
| building the polygons only, a `QPointF` per point | 8.12 |
| drawing polygons already built | 1.42 |
| the state set once, each polygon copied in one block | 6.10 |

Now `overlay.paint` sets the painter's state only when the style changes
(and again after a fill), and builds each polygon by copying its points
into the polygon's storage in one block (`QPolygonF.resize`, then
`shiboken6.VoidPtr` over `data()`), the technique pyqtgraph uses under
PySide6 (THIRD_PARTY.md). At import it checks that a copied polygon
equals one built point by point, and builds them point by point if not.
The viewport hands the core's outlines over as they come when the
display ratio is one, instead of dividing each point by it. A test paints
an overlay of mixed styles both ways and compares the images pixel by
pixel.

Measured on the reference machine, base `main` at
`fc5d76bd0f1890919846856fd997f43aca091667` against this change, each
built by its own `uv sync`. The main window and the method of
`stretch-cases.txt` (`stretch_cases.py`, SHA-256
`d68e18a970f670d7d41fcf4fde00b2f03ba4610a7b4f93e73b2fc30e4c51ac49`; the
lines with `cursor_frames.py`,
`4b01c469cba2dc10246c8066ae1e5d30d039e27a8eb2d9d7acbcfc51679b2482`), two
runs alternating base and fix, median a cursor move at STRETCH's
displacement prompt, no heavy process during any run:

| CPU (safe mode) | Base, ms | Fix, ms |
|---|---|---|
| 100 two-arc polylines | 22.88, 23.23 | 12.42, 11.84 |
| 402-corner crossing polygon | 15.39, 15.42 | 13.32, 15.27 |
| 100 lines, zoom and pan with each move | 8.87, 8.53 | 6.41, 6.41 |
| 100 lines | 6.55, 6.44 | 4.36, 4.36 |

The frame's parts with the two-arc polylines: building the overlay 6.46 →
3.23 ms, painting it 16.12 → 7.50 ms. On the GPU nothing is painted with
QPainter; the overlay and `to_core` took 6.56 → 3.37 ms a frame there,
and the frames stay at the display's refresh (a repeated `repaint`
16.59 → 16.43 ms). Its cursor-move medians in this series (bulged 9.5 →
13.0 ms, lines 15.4 → 16.0 ms) follow when each move meets the next
refresh, not the work. `gpu_viewport_check.py`, twice each, alternating:
the overlays 0 pixels apart; pan 17.2 and 15.8 ms on the base, 18.0 and
16.1 ms with the fix, both with a 95th percentile near 32 ms that the base
shows too in this session, after a reboot. That percentile is the new
monitor's 1920 × 1080: switched to 1368 × 768 it is the refresh again
([resolution and startup](testing/reference-machine/2026-11-19/resolution-and-startup.txt)).

### Polar ARRAY diagnostic comparison (2026-11-20)

Maintainer-approved scenario: 100 synthetic lines, 100 positions including
the source, signed fill +360 degrees, rotation on and off. Base production
commit `ded603746a7ba25ff4248d0365d6d69ec57cc740` was extracted with
`git archive` into a temporary directory and built with `uv sync`. Candidate
`49b89aa4cf92277c5b094c17641e00760acd09f4` uses the new atomic operation.
Both use the same lockfile and repository-default release PyO3 profile,
Python 3.13.5, LMDE 7, Linux 6.12.117+deb13-amd64, Ryzen 9 5900X,
x86-64-v2 baseline. No compiler CPU feature or desktop/system setting changed.

Each independent drawing has lines indexed i=0..99 from (10+i%10, i//10)
to (10.5+i%10, i//10), center (0,0), and explicit base (10,0) when not
rotating. Item j=1..99 uses angle `tau * (j/100)`. The base wraps calls in
`doc.group("ARRAYPOLAR")`: with rotation, COPY at zero offset followed by
ROTATE of the new IDs; without rotation, COPY at the base's orbital offset
`(-20*sin(angle/2)**2, 10*sin(angle))`. The candidate calls
`polar_array(ids, (0,0), 100, tau, rotate, base)` once.

Two warmups followed by ten samples per case, baseline then candidate;
`perf_counter_ns`, GC collected before and disabled during timing. Drawing
creation, result inspection and undo are excluded; normal transaction/index/
history work and Python call overhead are included. Both produced 9900 new
IDs and 10000 entities, matched checked first/last-cell endpoints and undid
in one step to the 100 originals. This compares equivalent workflows; it
does not isolate geometry arithmetic from API/transaction overhead.

| Mode | Base median | Candidate median | After / before |
|---|---|---|---|
| Rotate items | 50.053234 ms | 19.694723 ms | 0.393476 |
| Keep orientation, explicit base | 28.168532 ms | 19.834554 ms | 0.704139 |

The temporary harness SHA-256 was
`2d42627a569c6424c926b8337703c8912206302d85eb1d1df17b1fc47c739685`.
Measurements waited until the other agent's tests ended. Process monitoring
at 10 ms intervals throughout both runs found no concurrent compiler, test,
lint or build process. Synthetic fixtures, scripts and raw samples stayed
outside the repository; no private drawing was used.

These same-machine ratios are diagnostic, not reference-floor acceptance.
The measurement excludes selection, command dispatch, redraw and cursor
interaction. Creating thousands of entities adds redraw work; measure actual
interaction on the i7-2600/GT 730 with `tools/reference_machine_report.sh`
and `uv run python tools/gpu_viewport_check.py`. No fixed cross-machine
multiplier or independent ProgeCAD validation is claimed.

### Path ARRAY diagnostic comparison (2026-11-20)

Maintainer-approved scenario: 100 synthetic lines, 100 new groups along
an open mixed straight/curved polyline, alignment on/off, two warmups and
ten samples per case. Base `c953a782f8abe1dede9a59ad46794dbfa5574888`
was extracted with `git archive` outside the repository and built with
`uv sync`. Candidate `1b8e283fa3fb059e5a23b8ebd62b83e920c63f9d` uses
the new atomic operation. Same lockfile and repository-default release PyO3
profile, x86-64-v2 baseline, Python 3.13.5, LMDE 7, Linux
6.12.117+deb13-amd64, Ryzen 9 5900X. No compiler CPU feature or system/
desktop setting changed.

Sources i=0..99 run from (100+i%10, 100+i//10) to
(100.5+i%10, 100+i//10), with base (100,100). Let L=5*pi; the guide's
vertices/bulges are ((0,0),0), ((L,0),1), ((L+10,0),0), open. The line
and the counterclockwise lower semicircle each have length L. Stations
j=0..99 are at distance 2*L*(j/99), including both endpoints. On the line,
point=(distance,0), rotation=0. On the arc, fraction=(distance-L)/L,
theta=pi+pi*fraction, point=(L+5+5*cos(theta),5*sin(theta)), rotation=theta+pi/2.
The final position is the stored endpoint (L+10,0).

The base precomputes these analytic placements outside timing, then uses
`doc.group("ARRAYPATH")`: COPY by point-base, followed by ROTATE of those
new IDs about point when alignment is enabled and rotation is nonzero.
The candidate calls `path_array(ids, guide, (100,100), 100, align)` once,
including native path analysis. Both retain sources and guide, produce
10000 new IDs and 10101 entities, match checked first/middle/final-cell
endpoints and undo in one step to 101 entities.

`perf_counter_ns`; GC collected before and disabled during timing. Drawing
creation, baseline analytic placement preparation, result inspection and
undo excluded. Normal transaction/index/history work and Python call
overhead included. This compares equivalent workflows, not isolated
geometry arithmetic or whole command/viewport latency.

| Mode | Base median | Candidate median | After / before |
|---|---|---|---|
| Align items | 40.513941 ms | 18.280985 ms | 0.451227 |
| Keep orientation | 26.347081 ms | 18.245295 ms | 0.692498 |

Temporary harness SHA-256:
`87a18eb67211f21d85a2cf4a14f31b248c31e9c9ab70c060b097e8f2b7aee8ed`.
Measurements waited for the other agent's tests/checks to finish. Process
monitoring at 10 ms intervals throughout both runs found no concurrent
compiler, test, lint or build process. Synthetic fixtures, scripts and raw
samples stayed outside the repository; no private drawing or real QSettings
was used.

Same-machine diagnostic ratios do not establish reference-floor acceptance.
Selection, dispatch, redraw and real cursor interaction remain excluded.
Check the actual i7-2600/GT 730 with `tools/reference_machine_report.sh` and
`uv run python tools/gpu_viewport_check.py` before accepting minimum
interactive performance. No cross-machine timing multiplier or independent
ProgeCAD validation is claimed.
