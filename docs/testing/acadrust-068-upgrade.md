# AcadRust 0.6.8 upstream upgrade

Date: 2026-10-28. Status: immutable fork tag published; Git-pinned integration validated, ready for PR.

## Baseline and scope

Initial application base: main `5f5cb5750c51081a2a77dc5e8105e65fbe551be0`, including
PR #87 (Map resource proxy preservation) and PR #88 (Block Data dialog).
The isolated application branch subsequently fast-forwarded to
`2891a151b976ff099cd83cc1a609ff0c72934987`, including PRs #89-91
(Budget data section, Clear Block Data, and Block Data properties).
The main working folder remains untouched. The task uses
`feature/acadrust-068-upgrade` in separate application and codec working copies.

The previous production codec was `c05e7be3480959677ed79f5b8eed8263fe375454`,
AcadRust 0.6.7 plus our patches. This upgrade merges upstream
`ea66abee31ad1b5d16a7043e0272d55a17810d39` into that history. Upstream identifies
itself as 0.6.8; this includes the released `afffe46` and the subsequent MTEXT
attachment-point and clockwise-HATCH corrections. There are 89 upstream commits
(including merges) after our previous upstream base `c5555a5`.

No new dependency or technology is introduced: upstream Cargo.toml changes only
the package version. A temporary local path with `=0.6.8` was used during
candidate testing; the application manifest now uses the exact published Git
revision. ADR 0017 requires corpus and comparison validation before
an exact, reachable fork revision is adopted.

## Relevant upstream changes under evaluation

- Legacy TABLE cell serialization and full color records (`7df09ea`).
- DWG-to-DXF SPLINE closure and periodic NURBS clamping (`61daff3`).
- Current text/dimension style and linetype names (`93ccf20`, `b1f7ac1`,
  `29cd466`, `f30cfd9`).
- MTEXT columns, Unicode escape handling, paragraph end properties and embedded
  attachment points (`22ba561`, `968cc39`, `d07725c`, `3fe804e`, `f6f6f65`).
- Clockwise HATCH arc storage/export and explosion (`1dab834`).
- Symbol-name comparisons (`e3e90d4`), dynamic-block property-table records
  (`5433d5f`), same-version unknown-object preservation (`62e450a`) and image
  definition reactors (`3462993`).
- Fallible DWG buffer allocation and a configurable total decompression budget
  (`ad6950c`). The new total budget defaults to None; this upgrade does not
  silently introduce an application-wide memory-limit policy.

These are changes read from the upstream commits, not claims that the 88
previous native table-format differences are fixed.

## Merge resolution and patch preservation

The three-way merge had one textual conflict in
`src/io/dwg/dwg_stream_readers/object_reader/entities.rs::read_table_cell_data`.
The resolution keeps upstream's `read_cm_true_color()` and our
`BorderPropertyFlags::COLOR`, independent visibility flags and virtual-edge
payload handling. The matching writer already merged both changes.

Our existing regression files remain, including table-instance formatting,
legacy 2007 borders, unloaded-Xref markers and native Map proxies. No guard
or loss allowance is relaxed. Upstream's new AGENTS.md/CONTRIBUTING.md govern
submissions to upstream; this task follows ADR 0017's private-fork upgrade
workflow and does not submit issues or PRs to the upstream repository.

## Acceptance gates

1. Default, sequential and all-feature codec tests; native Map/raw-DWG and
   previous table regressions.
2. Product build and required checks, plus fetched and private corpus groups.
3. Same-input comparison against `c05e7be`, preserving source versions and
   forcing AC1032, with every changed status/summary reviewed.
4. Native ProgeCAD reopening of retained table outputs and proxy records,
   using the existing strict 249-property comparator.
5. Document actual improvements, remaining limits and platform coverage.
6. Only after validation: prepare a concrete fork revision for immutable-tag
   publication and replace the temporary application path with an exact Git pin.

The earlier table-format differences and legacy safe refusals remain open until
new evidence establishes otherwise. Phase 03.4 is not closed by the version bump.

## Local codec revision

The validated merge is `a1a9a56965d67230295777d3dc419edc9b6571b3`.
Its parents are the production fork `c05e7be` and upstream `ea66abee`.
Published immutable annotated tag: `arqstudiocad/on-ea66abe` in
`3djlchb/acadrust`. The remote tag object is
`4fe0cabb8c4c82c59f304671b1a1272cbabeaa77`, peeling to the full merge revision
above. Publication was explicitly authorized by the maintainer. The application
adoption remains on its isolated feature branch. The fork's mirror main must
contain upstream history only.

## Completed validation

Windows x64, Rust tests with `CARGO_BUILD_JOBS=2`,
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`, matching upstream
CI's build settings. Counts include doctests; ignored optional tests are listed
separately rather than counted as passing.

| Check | Result |
|---|---|
| Codec `cargo test` | 2,409 passed, 0 failed, 86 ignored |
| Codec `cargo test --no-default-features` | 2,409 passed, 0 failed, 86 ignored |
| Codec `cargo test --all-features` | 2,498 passed, 0 failed, 93 ignored |
| Native Map integration regressions, including ignored | 4 passed |
| Untouched native Map DWG record regression | 1 passed |
| Native table-instance formatting regressions | 6 passed |
| Native table-2007 regressions | 2 passed |
| Initial application Rust suite, base 5f5cb57 | 482 passed, 0 failed, 11 ignored |
| Initial complete Python suite, base 5f5cb57 | 662 passed, 0 skipped; 9 existing warnings |
| External core corpus groups | 4 public codec groups and 1 six-file local Xref group passed |
| Optional document corpus tests | 6 passed |
| Initial format, Clippy, Ruff, mypy, stubtest, cargo-deny | Passed |

The first Python invocation began before external corpus extraction and skipped
17 fixture-dependent tests; the complete 662-test invocation after extraction
supersedes that run. Samples were fetched using `tools/fetch_codec_samples.py`;
25 public samples plus six native Xref DWGs and three native table inputs form
the 34-drawing comparison set. Tests against the subsequently updated main are
recorded below; the initial counts are not presented as current-main acceptance.

## Paired codec comparison

The same `tools/dwg-bench` wrapper compares this merge directly with the exact
previous production fork `c05e7be`, in separate child processes. Only the codec
revision differs between the two libraries. The retained local benchmark project
is `target/acadrust-068-codec-bench`; reports are under
`target/upgrade-bench-{source,2018}-{public,xrefs,tables}`.

| Profile | Jobs | Paired operations | Changed status / summary |
|---|---:|---:|---:|
| Preserve source version (`DWGBENCH_KEEP_VERSION=1`) | 476 | 238 | 0 / 0 |
| Force DWG AC1032 (variable absent) | 464 | 232 | 0 / 0 |

All source-version jobs succeed. Forced AC1032 has the same three write refusals
in each codec: `Surface.dwg` with `ACDBASSOCEDGEACTIONPARAM`, and
`example_2000.dwg` / `example_r14.dwg` with `DATATABLE`. These require compatible
raw DWG data and are not newly introduced failures. Paired summaries compare
entities, types, objects, layers, blocks, geometry aggregates, XDATA and recovery
information. They do not establish visual equivalence or a performance claim.
The machine was running concurrent validation, so timings are not used.

The integrated application matrix independently reads all 34 inputs cleanly:
58 verified saves, 8 blocked losses, and 2 errors. The latter are the known
R2000 TABLESTYLE ownership refusals. All six native table saves and all twelve
Xref saves verify. The production baseline matrix has identical statuses and
source hashes. A verified inventory remains distinct from native table fidelity.

## Native ProgeCAD 2026 measurements

The existing `tools/progecad_table_corpus.lsp` and unchanged
`tools/compare_progecad_tables.py` capture and strictly compare four tables and
249 properties. ProgeCAD opens the actual candidate files from the comparison
bench, without resaving them. The DXF-to-DWG and DWG-to-DXF outputs were opened
read-only; comparator arguments use ProgeCAD's exact ` (Sola-lectura)` document
suffix rather than bypassing document identity checks.

| Input and output | Native reference | Differences |
|---|---|---:|
| `tables-native-ac1021.dwg` to source-version DWG | `control-ac1021.tsv` | 0 / 249 |
| `tables-native-ac1024.dxf` to DWG | Native DWG `control-ac1032.tsv` | 0 / 249 |
| `tables-native-ac1032.dwg` to DXF | `control-ac1032.tsv` | 88 / 249 |

The last comparison's complete difference list is **identical** to the previous
Map-resource candidate's native capture: 60 cell records, 8 margins, 12 row text
heights and 8 merged-title text values. Upgrading does not fix or worsen that
measured limitation. The native DWG reference for DXF-to-DWG is the same control
used by the earlier table-format correction. Comparing that DWG against the
native DXF control instead yields 52 cell-record differences (group 93 value
encoding); that diagnostic is retained separately, not relabeled as a regression
or used to weaken the comparator.

Native `entget` and `dictsearch` of the candidate DXF still return an
`ACAD_PROXY_OBJECT` named `AcMapResourceManager` at handle `2A4`, linked through
dictionary `2A3`, with 12 payload bits, bytes `4060`, source DWG version 33 and
maintenance version 55. The private capture is kept under ignored `target/`;
no user drawing or embedded private resource path is committed.

The original AC1032 source SHA-256 is
`6f4050e24aaa4413e46e290cc4a3fdcd4f39f1c8783c566c8002a00fe5705678`.
Native captures, comparison JSON and output hashes are listed in
`target/acadrust-068-evidence-manifest.json`. The source set remains unchanged.

## Limits

This validation is local Windows coverage. Linux, macOS, the release CPU floor,
large private-corpus performance and complete visual/semantic compatibility
have not been established by these runs. Xref resolution remains a later
feature; the existing marker and metadata preservation are retained. Milestones
03.4 and 03.5 remain open.

The exact merge is reachable through the published immutable tag. The application
uses this Git revision instead of the temporary local path, with its lockfile,
THIRD_PARTY.md, roadmap and ADR 0017 log updated. Final checks against the
Git-fetched built extension are recorded below.

## Updated-main acceptance

The isolated branch was fast-forwarded to main `2891a15` before rebuilding the
extension. On this base, with the identical local codec merge:

- `uv sync`: successful native extension rebuild.
- `cargo test`: 483 passed, zero failures, 11 optional tests ignored.
- Full Python suite with `QT_QPA_PLATFORM=offscreen`: 688 passed, zero skips,
  nine existing warnings.
- Ruff check and formatting check, mypy (132 source files), stubtest (one
  extension module), Cargo formatting, workspace/all-target Clippy with warnings
  denied, and cargo-deny advisory/license/source checks: passed.
- Repeated integrated matrix: 34 clean reads, 58 verified saves, 8 blocked losses
  and the same 2 legacy TABLESTYLE-owner errors.

The optional external Rust corpus rerun also passed: four public codec groups,
one six-file local Xref group and six document tests. No product source or
protection rule was changed to make the upgraded codec pass.

The compatibility-matrix command exits with status 1 because its report contains
the two documented safe errors; its 34 records and 68 outcomes were inspected.
This is not counted as an all-green conversion corpus.

## Published Git-pin acceptance

After the maintainer authorized publication, the remote annotated tag was
published and its peeled commit independently verified. Cargo now resolves
AcadRust `=0.6.8` from Git revision
`a1a9a56965d67230295777d3dc419edc9b6571b3`. An automated comparison of Cargo.lock
confirms that every other package entry is unchanged from main.

- `uv sync` fetched the published revision and successfully rebuilt and installed
  the native extension (15 minutes 53 seconds on this run).
- `cargo test --locked`: 483 passed, zero failures, 11 optional tests ignored.
- Locked workspace/all-target Clippy with warnings denied, Cargo formatting and
  cargo-deny advisory/license/source checks passed.
- The complete Python suite on the rebuilt extension: 688 passed, zero skips,
  nine existing warnings (120.70 seconds).
- Stubtest checked the rebuilt extension against its API stub successfully.
- The repeated 34-drawing matrix has 34 clean reads, 58 verified saves,
  8 blocked losses and the same 2 TABLESTYLE-owner errors. Each full source path,
  source hash and outcome was compared with the local-candidate report; error
  diagnostics differ only in randomized temporary-directory names. Full source
  paths are necessary because the public corpus contains several files named
  `Constraints.dwg` in different version directories.

Git-pin logs and reports use the `target/acadrust-068-git-*` prefix. Their hashes
are retained in `target/acadrust-068-evidence-manifest.json`. No new paired bench
or native ProgeCAD run is claimed after publication: those runs tested this
identical committed codec source before the immutable tag was published.

The application branch contains only the manifest/lockfile adoption and its
provenance/evidence documents. The main working folder was not changed by this
task; the maintainer creates the application PR manually.
