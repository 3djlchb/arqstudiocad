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
| World XYZ overlay, 800 x 600 CPU viewport, 100 lines | Top 0.011100 → 0.121881 ms (10.980x); SW isometric 0.010960 → 0.105006 ms (9.581x) | LMDE 7 development host, Ryzen 9 5900X; release extension, Python 3.13.5, 5 warm-ups + 100 samples per view (2026-11-17); overlay-only diagnostic; reference acceptance pending |

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
