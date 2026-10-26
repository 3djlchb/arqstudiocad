# ProgeCAD Xref validation: AcadRust 0.6.5 and 0.6.7

Baseline date: 2026-10-24 (America/Bogota); safeguard follow-up: 2026-10-25.
Milestones 03.4 / 03.5 remain open.

The baseline and safeguard observations below are historical. For current DXF
unloaded-state support, see the [native DXF investigation](progecad-dxf-unloaded.md).
The [safeguard follow-up](#safeguard-follow-up-2026-10-25) records the preceding
filename-marker and refusal-only stage.

## Scope and provenance

This rerun starts at `0e6fd1693de9f466d7f745fb56aa47c377a642bb`: PR #61
(AcadRust upgrade) and PR #62 (Blocks palette step 4) are both merged.
Worktree branch: `feature/progecad-xrefs-067`. The main folder was not edited.
No product code, tests, dependencies or rendering policy were changed by this validation.

| Run | Application revision | AcadRust fork |
|---|---|---|
| Initial source/safe-save/external run | `101beb8` | 0.6.5, `5f473ccba816c7b6ff937e32de0df4174bde28c4` |
| Rerun after both merges | `0e6fd16` | 0.6.7, `955bdd8e68d554be1db681541bd1e53140a79ebc` |

The six original exercise drawings were authored in ProgeCAD 2026 Professional
(Spanish). All have DWG 2013 (`AC1027`) headers. Units were set to INSUNITS 0,
world UCS and base point (0,0,0). The exact ProgeCAD build remains unrecorded;
its `ACADVER = 19.0i` compatibility string is not an application build number.
See [the authoring procedure](progecad-xrefs.md) for the intended geometry.

The baseline consists of saved on-disk bytes captured during the initial run,
when some originals were still open. The rerun reads those frozen copies directly
from the earlier task worktree; it does not duplicate the corpus. Original files,
raw reports and output drawings remain outside tracked files.

| Source | SHA-256 (identical inputs in both runs) |
|---|---|
| attachment.dwg | `709516386458aded540f5c21cd19a87706c04474bbd880bdbe40c10d8f025e01` |
| child.dwg | `74ee20a10c35ca1a9f45bd41cc153f8d79dc251f8d4430c79da1aa97abd0b0e7` |
| master.dwg | `08fc30854bb6694ba6c328ddb8f2dfeafa610001afe426f24f7d14d7e933ff13` |
| missing.dwg | `091f3d4898752abfbfcdf5b7394c94e27c9e14b2c10471b2316cb5486ec070d3` |
| overlay.dwg | `a25ae7429edc889bc4b4b96eacfd9f354ef3812aedbd9c86ad886ed86a7db653` |
| unloaded.dwg | `3a873b7e955a77c00ada3e52dd6273d84ff06e12a2e14b5e9f34b246aa533c13` |

The final shared-read hash check verified 32 files unchanged: six originals,
six frozen inputs, ten 0.6.5 outputs and ten new 0.6.7 outputs. The same source
hashes appear in both machine-readable matrix reports.

## Integrated-core read and safe-save comparison

Environment: Windows 11 build 26300, CPython 3.14.8, application/core 0.1.0.
The existing `tools/compatibility_matrix.py` was used without changes, through
Python and the Rust public API. No loss was accepted.

| Measure | 0.6.5 run | 0.6.7 rerun |
|---|---|---|
| Clean reads | 6/6 | 6/6 |
| DWG core-verified saves | 6/6 | 6/6 |
| DXF core-verified saves | 6/6 | 6/6 |
| Output versions | AC1027 | AC1027 |
| Recovery / crash / timeout | 0 / 0 / 0 | 0 / 0 / 0 |

Each direct host has CIRCLE + INSERT in model space, the master has two INSERTs,
and the child has LINE + CIRCLE. No undecoded-object diagnostic was reported.
Core-verified means the current safe-save checks passed; it does not establish
Xref-state fidelity, as the independently observed unloaded-DXF failure shows.

Reproduce from the current task worktree:

```powershell
uv run python tools/compatibility_matrix.py ../arqstudiocad-progecad-xrefs-validation/target/progecad-xrefs-baseline --format json --output target/progecad-067-matrix.json
```

## External reopens of the 0.6.7 outputs

New `<host>-067-output.dwg` and `.dxf` files were written beside the existing
unchanged DWG dependencies in the earlier task worktree's ignored review folders.
Neither the six source DWGs nor the 0.6.5 outputs were overwritten. A local
diagnostic driver calls the existing `compatibility_matrix.roundtrip` function;
no second production exporter or resolver was introduced. Only each host was
converted: its dependencies remain the original DWGs.

All ten outputs were opened in ProgeCAD, with Zoom Extents and native
`tblnext`/`tblsearch`/`entget` queries. No repair prompt was observed. Paths were
not repaired, references were not bound or exploded, and product outputs were
not saved back through ProgeCAD. View-only changes left some tabs dirty.
Screenshots were inspected during the run; no durable screenshot archive or
complete Xref-manager tree capture was retained.

The observed reopen checks match in nine rows; one differs. The full-exercise
verdict remains NOT RUN for the matching rows because independent numerical
measurements and reload comparisons are still incomplete. This is not a claim
of nine complete external PASS results.

| Host / format | Reopen checks | Full exercise verdict | Observation |
|---|---|---|---|
| attachment / DWG | match | NOT RUN | Reopened; child.dwg attachment flags70=100, direct transform retained; line, child circle and marker visible; no repair prompt. |
| attachment / DXF | match | NOT RUN | Reopened; child.dwg attachment flags70=100, direct transform retained; child line/circle and local marker visible; no repair prompt. |
| overlay / DWG | match | NOT RUN | Reopened; child.dwg overlay flags70=108 and direct transform retained; child geometry and local marker visible; no repair prompt. |
| overlay / DXF | match | NOT RUN | Reopened; child.dwg overlay flags70=108, direct transform retained; child line/circle and local marker visible; no repair prompt. |
| master / DWG | match | NOT RUN | Reopened; attachment.dwg and overlay.dwg retained, child .\child.dwg resolved; attachment branch shows child line/circle and marker, overlay branch only marker. Final INSERT overlay at(100,0,0), unit scales, rotation0. No repair prompt. |
| master / DXF | match | NOT RUN | Reopened; attachment.dwg and overlay.dwg retained, child .\child.dwg resolved; attachment branch displays child line/circle plus marker, overlay branch only marker. Final INSERT overlay at(100,0,0), unit scales and rotation0. No repair prompt. |
| missing / DWG | match | NOT RUN | Reopened; absent-child.dwg, BLOCK70=4, transform retained; placeholder and local marker, no child geometry or repair prompt. |
| missing / DXF | match | NOT RUN | Reopened; absent-child.dwg, BLOCK70=4 and INSERT transform retained; missing placeholder and local marker visible; no repair prompt. |
| unloaded / DWG | match | NOT RUN | Reopened; child.dwg, BLOCK70=4, BLOCK71=1, direct INSERT transform retained; only local marker visible; no repair prompt. |
| unloaded / DXF | mismatch | FAIL | Reopened; child.dwg retained but BLOCK71 absent and flags70=100. Child line and circle now visible. INSERT transform retained; no repair prompt. Same defect as 0.6.5. |

Direct INSERT queries retain position (10,20,0), scales (2,2,1), rotation
0.523599 radians and handle 2D7. Loaded attachment flags are 100; overlay flags
are 108. The master query confirms its final overlay INSERT at (100,0,0), unit
scales, zero rotation and handle 305; full numerical comparison of both master
instances is pending. The attachment branch displays the child line/circle;
only the local marker is propagated through the overlay branch.

### Unloaded DXF fidelity failure persists

Both engine versions write a DXF that ProgeCAD loads as an active reference.
The source and product DWG retain BLOCK group 71 = 1 and show only the local
marker. The new DXF has no BLOCK group 71; ProgeCAD reports flags 100 and draws
child geometry. The stored path and INSERT transform survive. INSERT group 71
is a separate field and is not evidence of the reference's unloaded state.

In fork `955bdd8`, `src/io/dxf/writer/section_writer.rs::write_block_definition`
(line 2109) does not serialize `BlockFlags::is_xref_unloaded` and
`src/io/dxf/reader/section_reader.rs::read_block` (line 3454) does not read it.
The DWG reader/writer preserves the field. The product validator
`crates/arq-document/src/verify.rs::Document::validate_roundtrip` compares
entities/layers, and `inventory.rs::Document::inventory` counts block definitions;
neither verifies this state. The synthetic
`compatibility_tests.rs::xref_fixture_metadata_roundtrips` tests path,
attachment/overlay and INSERT transforms, but does not cover unloading.

This defect was not fixed or hidden by this measurement. A focused follow-up
should preserve the verified DXF unload representation and add both a regression
and a safe-save check that rejects its loss. It is independent of implementing
an external-file loader.

## Integration and regression checks

| Check on merged main | Result |
|---|---|
| `uv sync` | rebuilt successfully after fast-forward |
| `cargo test --locked` | passed; ordinary core/geometry/kernel/render tests and doctests |
| `uv run pytest -q` | 445 passed |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `uv run ruff check` / `uv run ruff format --check` | passed / 167 files formatted |
| `uv run mypy` | passed, 100 source files |
| `uv run stubtest arqstudiocad._core` | passed |
| `cargo deny check` | advisories, bans, licenses and sources passed; duplicate-version warnings for hashbrown and syn |
| `cargo test --locked -p arq-core --test corpus -- --ignored codec` | 4 passed, with fetched inputs present |
| `cargo test --locked -p arq-document legacy_multileaderstyle_refusals -- --ignored --nocapture` | 1 passed, covering R14 and 2000 |
| Local six-DWG Xref corpus | failed: five hosts draw no external INSERT geometry; child passes |

The full Python run includes the 27 tests in `tests/python/test_blocks_palette.py`,
including the dynamic star, plain/selected icons, tooltips and edit refusal, fixed
64 px cards, layer-colored previews and redraw-cache behavior. Rust tests also
exercise dynamic properties and colored previews. The maintainer had separately
confirmed the step-4 manual test before merging PR #62; no new manual palette
acceptance is inferred from these automated tests.

The MULTILEADERSTYLE check still demonstrates two objects becoming zero while
an empty-drawing allowance permits only one; strict saving refuses that loss.

### Local corpus failure and pending rendering decision

With ARQ_CORPUS_DIR pointing to the frozen six-DWG baseline,
`cargo test --locked -p arq-core --test corpus local_drawings_round_trip -- --ignored --nocapture`
fails for attachment, overlay, missing, unloaded and master; child passes.
The five hosts have six affected INSERTs: decimal handle 727 in each direct host
and handles 725 / 773 in the master. Every reported problem is `INSERT draws nothing`.
The corpus also runs its edit/save/reopen/undo checks; no additional difference
is listed in this run. This corpus accepts some conversions by design; its
result must not be confused with the strict no-accepted-loss matrix.

The assertion is in `crates/arq-core/tests/corpus.rs::drawing_coverage` (line 370):
a visible entity with bounds and no drawing primitives is an error. That
function is unchanged between `101beb8` and `0e6fd16`. The reference loader
remains unimplemented under proposed ADR 0026 and ADR 0023 D2. No placeholder
was added and no assertion was relaxed. Whether to show a filename marker or
admit an intentionally empty Xref drawing in this test remains a maintainer
decision. The current local-corpus gate is explicitly not green.

## Remaining limits and local evidence

- Record the exact ProgeCAD build and durable reference-tree images.
- Independently measure source/output endpoints, line length and circle radii.
- Complete numerical checks of both master instances and a disposable-child reload.
- Resolve the unloaded-DXF fidelity defect and the separate rendering-test policy.
- Broader versions, BIM semantics, clipping, cycles, styling, foreign-platform paths
  and reference Linux/GT 730 release checks are outside this run.

Current worktree ignored evidence: `target/progecad-067-matrix.json` / `.md`,
`progecad-067-retained.json`, `progecad-067-external.json`,
`progecad-067-integrity.json`, `progecad-067-corpus.log`, `codec-067-corpus.log`
and `legacy-multileaderstyle-067.log`. Retained drawings stay in the earlier
worktree's `target/progecad-xrefs-review/<host>-<format>/` with their dependencies.

## Retained output hashes (0.6.7)

| Host / output | SHA-256 |
|---|---|
| attachment / DWG | `e948a8fe2245493dc9d2c4705fc18581b7276e57d1c749468fcf6b8318ad9bfe` |
| attachment / DXF | `fcb2abb62c8c138af1e46643d15bb5e91a7c47177eb4259fdf3d17b4e2125d24` |
| overlay / DWG | `a64d1005d1e57f95d2e9d8ef2fc7d417982afd909bd3e6e8220d3343241e2e45` |
| overlay / DXF | `048dfbde4fbec100cc5b61aeff6c221e5c77a10561560f79324d9f49a905e4c8` |
| master / DWG | `73b4b5124bd750d94c4e95fe3abca86f463a1dca9f23d6a5c9555bfcd2cc65a8` |
| master / DXF | `0355fec6a6c3a166c67e6c89d74f020ec8f8c4e85facef822a573e482665a675` |
| missing / DWG | `9805c3f778e5bc06887cb52ffc9c31d7faddcedbbe1cbbf24e4f4b2ac28963b4` |
| missing / DXF | `79c6d94c018962795c138fbb884e1ce9b71d555c6463e50825dfc1a203757b6b` |
| unloaded / DWG | `8bf51eed1f793b06f179033c205c17d7425c78e2646f44b1257dfd72cefc5778` |
| unloaded / DXF | `6a1239103e383e3f7782d20e9927582bafd71411041abbeef924a9a2e125cf23` |


## Safeguard follow-up (2026-10-25)

Branch `fix/xref-preservation-marker` started from `fae7de6` (PR #63), then
integrated `bd66cd1` (PR #64, sample templates) before publication. It keeps
the same AcadRust 0.6.7 pin. The maintainer authorized a derived filename
marker and reference-level read-back validation; no external-file loader was
implemented. The historical outputs and observations above are unchanged.

`drawing.rs::Painter::expand_insert` renders a filename with the built-in stroke
font at each reference insertion, respecting its transform, array placements,
visibility and resolved color. The text bounds participate in selection. Empty
paths fall back to the block name; Windows and POSIX separators are recognized.
The marker does not load the child or write text entities into the host.

`verify.rs::Document::compare_xrefs` now checks stored paths, reference and overlay
flags, unloaded state, and all stored reference INSERT placements and ownership.
A same-count metadata change fails safe-save verification even when inventory
losses have been accepted. Regression tests verify that the previous destination
remains byte-for-byte intact and that a diagnostic copy is retained on refusal.

### Integrated measurements

The existing frozen six-DWG corpus was read in place, with no new corpus copy.
The strict matrix (`tools/compatibility_matrix.py`) reports:

| Source | Clean read | DWG safe save | DXF safe save |
|---|---|---|---|
| attachment.dwg | yes | verified | verified |
| child.dwg | yes | verified | verified |
| master.dwg | yes | verified | verified |
| missing.dwg | yes | verified | verified |
| overlay.dwg | yes | verified | verified |
| unloaded.dwg | yes | verified | refused: unloaded state changed |

This is six verified DWG saves and five verified DXF saves. The DXF refusal is
`NotVerified`, reported as `error` by the unchanged matrix tool. AcadRust still
loses the unloaded flag in DXF; the safeguard prevents that output from replacing
the destination. It does not add DXF support for the flag or repair an already
exported DXF that has lost it. Native DXF encoding and independent-application
interoperability still need investigation; do not infer a raw DXF group 71 from
an AutoLISP `tblsearch` result.

The unchanged local-corpus test passes attachment, child, master, missing and
overlay. Its only remaining failed drawing is unloaded, whose DXF saves before
and after editing are explicitly refused. No `INSERT draws nothing` or INSERT
placeholder-box failure remains. The complete local-corpus gate therefore still
fails; no assertion, accepted-loss policy or corpus input was weakened.

The five passing drawings produce 84, 2, 241, 162 and 84 shapes respectively
(in the order attachment, child, master, missing, overlay), with zero placeholder
boxes. These counts establish representation, not external geometry fidelity.

SHA-256 checks of all six authored originals and all six frozen inputs still
match the original authoring manifest. Local ignored evidence is in
`target/xref-marker-matrix.md` / `.json` and `target/xref-marker-integrity.json`
in this task's worktree. New independent ProgeCAD reopening, numerical geometry
measurements and reload checks were not performed for this safeguard.


### Follow-up checks

| Check | Result |
|---|---|
| `cargo test --quiet` | passed; document crate: 170 passed, one corpus-dependent test ignored |
| `cargo fmt --all --check` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `cargo test -p arq-core --test corpus -- --ignored codec` | 4 passed |
| `cargo test -p arq-document legacy_multileaderstyle_refusals -- --ignored` | 1 passed |
| `uv run pytest -q` | 449 passed after integrating PR #64 |
| `uv run ruff check` / `uv run ruff format --check` | passed |
| `uv run mypy` | passed, 102 source files |
| `uv run stubtest arqstudiocad._core` | passed |
| `cargo deny check` | passed; existing duplicate hashbrown and syn warnings |
| Local six-DWG corpus | 5 drawings pass; unloaded DXF is refused; the overall test fails |
| Strict six-DWG matrix | 6 clean reads, 6 DWG verified, 5 DXF verified, 1 DXF refused |

Eight new document-layer regressions cover actual filename strokes, path
separators, layer visibility, transformed and mirrored/nonuniform strokes,
unloaded/pathless references, array placements, changed reference metadata,
out-of-plane placements, and safe-save refusal with an intact destination.

The five hosts were also rendered through `CadView` at 1000 by 650 pixels into
`target/xref-marker-<host>.png`; `master` and `unloaded` images were visually
inspected. The unloaded host renders 84 shapes, including its filename marker.
`target/xref-marker-render.json` records all five shape counts and
`target/xref-marker-corpus.log` retains the failing local-gate result. This CPU
viewport check is separate from external ProgeCAD validation and proves no
child geometry was loaded. No new release or cross-platform acceptance is claimed.


## Unloaded-state correction follow-up (2026-10-25)

Native ProgeCAD exports establish the explicit BLOCK 71=1 unload marker in the
DXF file itself. Fork `1ca0ae2` preserves it in DXF reading and writing, with the
existing reference-level safe-save comparison retained. The six-DWG local
corpus now passes, including its edit/save/reopen/undo checks; the strict matrix
verifies six DWG and six DXF saves. No corpus assertion or accepted-loss policy
was weakened. The loaded control and unloaded output also retain their distinct
states when reopened in ProgeCAD.

The complete native unloaded DXF initially safe-saved to DXF but refused DWG
because the DATAOBJECT inventory could not prove companion-map conversion.
The final fork `59aaf4c` and product semantic proof resolve this specific modern
conversion. Both strict outputs reopen in ProgeCAD with unloaded state and all
90 compared table-style properties intact, including margins 1.5. The original
unloaded-state correction remains enabled. Two R2000 fixtures that actually
lose the TABLESTYLE owner remain safely refused; their corpus cases assert an
unchanged destination. See the [investigation](progecad-dxf-unloaded.md) for
historical candidates, exact build, raw records, hashes, tests and limitations.
Broad milestone 03.4/03.5 exit criteria remain open.
