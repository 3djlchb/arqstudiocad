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
