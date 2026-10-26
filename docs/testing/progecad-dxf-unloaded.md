# ProgeCAD DXF unloaded-reference investigation

Date: 2026-10-25 (America/Bogota). Application branch: `fix/dxf-xref-unloaded`,
started from `a50038c` (PR #65 merged), then synchronized with `main` at
`8c6ea9a` for the integration retest below. This investigates the unloaded-DXF
failure recorded in [the earlier results](progecad-xrefs-results.md).

> **Final validated revision:** `59aaf4c` replaces the experimental map-retention
> candidate. Both final DWG and DXF reopen in ProgeCAD with margins 1.5 and the
> unloaded Xref state intact. See the final acceptance section below; earlier
> sections retain the evidence for the preceding revisions.

## Independent native exports

The installed executable identifies itself as progeCAD 2026 (Spanish), product
version `13.1.508.P.VC17.x64.26.0.12.5`, file version `13,1,508,33299`.
The application's `ACADVER` is `19.0i`; that compatibility string is not its build.

Existing retained copies of `attachment.dwg` and `unloaded.dwg` were opened in
ProgeCAD with the unchanged `child.dwg` available beside them. DXFOUT exported
new ASCII files with Version A2013 and 16 decimal places. The header of both
outputs is `AC1027`. Neither the authored originals nor earlier outputs were
overwritten. Both new DXFs were independently reopened in ProgeCAD.

The export procedure follows the vendor's [DXFOUT reference](https://www.progesoft.com/products/progecad-professional/manual?mp=command-reference%2Falphabetical-list-of-commands%2Fdxf-out).
The [unload reference](https://www.progesoft.com/products/progecad-professional/manual?mp=working-with-other-files-in-your-drawings%2Fworking-with-external-references%2Fremoving-external-references)
distinguishes unloading, which retains the reference, from detaching it.

| Observation | Loaded control | Unloaded reference |
|---|---|---|
| Raw BLOCK group 70 | 100 | 4 |
| Raw BLOCK group 71 | absent | 1 |
| Stored path | child.dwg | child.dwg |
| Reopened `tblsearch` group 70 | 100 | 4 |
| Reopened `tblsearch` group 71 | absent | 1 |
| Reopened view | child line and circle displayed | only the host's local circle displayed |

BLOCK_RECORD units (group 70) remain zero in both files; they must not be
confused with BLOCK flags. The optional BLOCK 71=1 marker is present in the raw
DXF itself, not merely an AutoLISP representation. This is direct evidence
for the tested ProgeCAD build and AC1027 exports; it is not a claim that every
CAD application or DXF version documents or interprets that extension alike.
Missing references with BLOCK 70=4 and no group 71 must stay distinct from
unloaded references. ProgeCAD displayed a generic missing-reference notification
when opening the unloaded source and native unloaded DXF, despite the matching
child being present; the loaded control resolves that same child. The unloaded
state above is established by the explicit marker and reopened data, not by
interpreting that notification.

| Native export | SHA-256 |
|---|---|
| progecad-native-loaded-2013.dxf | `34c0a534d3cad2b15891e8572762497d18c1c77554efb0a9aa795bfc7f9f1e9e` |
| progecad-native-unloaded-2013.dxf | `a0045c9eb06c3d2971c0ecca29ffadfb706b747f501b17e01ec14d207219b301` |

Raw files remain in the older validation worktree's retained `unloaded-dxf`
package alongside their dependencies. The new task worktree records their
BLOCK/BLOCK_RECORD pairs and hashes in `target/native-dxf-comparison.json`.
`native-loaded-before.txt`, `native-unloaded-before.txt`,
`native-loaded-reopened.txt` and `native-unloaded-reopened.txt` record native
AutoLISP observations. `target/native-dxf-integrity.json` verifies all six
originals, frozen inputs and retained sources (18 hashes) against the authoring
manifest. All still match.

## Codec correction and verification

The original fork pin `955bdd8` neither reads nor writes the unload marker in
DXF. A hand-written regression reproduces both failures: reading native-style
71=1 produces a loaded block, and writing an unloaded block loses its state.
Two of the three focused tests fail before the patch; all three pass after it.

The patch is limited to `src/io/dxf/reader/section_reader.rs::read_block` and
`src/io/dxf/writer/section_writer.rs::write_block_definition` in our AcadRust
fork. It reads the explicit marker for external references, writes 71=1 when
unloaded, and does not infer unload state from unresolved/resolved flag bits.
Local blocks do not acquire reference state from a stray group 71.

`tests/xref_unloaded_dxf.rs` is independently authored, not a copy of a private
DXF. It covers explicit/absent/zero markers and 68 writer-reader combinations:
nine DXF versions, Attachment/Overlay and loaded/unloaded, in ASCII and binary
where supported (binary R12 is intentionally unsupported by the codec).

The product's reference-level safe-save comparison remains enabled. Its tests
now require unloaded DWG and DXF to save faithfully, while fault injection
still requires a corrupted unload marker to refuse replacement of the target.

The fork commit is `1ca0ae22341e243db140fd499b031c6116782c18`, retained by the
immutable tag [`arqstudiocad/on-c5555a5-xref-unloaded`](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-xref-unloaded).
The fork's mirror `main` and existing tags were not changed. Cargo.toml and
Cargo.lock pin that exact commit; no other lockfile dependency changed.
Provenance and MPL-2.0 obligations remain recorded in `THIRD_PARTY.md`.

### Codec suite and baseline comparison

The complete fork suite passes: 2,282 tests, zero failures and 81 ignored tests,
across 163 test executables/doc-test groups. The new three tests account for the
increase from the prior 2,279 passing tests. Full output is retained in the
codec worktree's `target/xref-unloaded-full-tests.log` and its JSON summary.

The existing `tools/dwg-bench` was copied into ignored task output and configured
to compile the baseline (`955bdd8`) and patched codec under separate aliases.
The same operation implementations measured both revisions on 25 fetched public
samples and the six frozen ProgeCAD DWGs, read in place. This is a 31-drawing
follow-up, not a rerun of the earlier 217-drawing upgrade comparison.

- 422 read/write/reopen jobs form 211 paired comparisons.
- Success status and measured summaries agree in every pair: 208 successful jobs
  per revision, plus the same three forced-AC1032 DWG write refusals per revision.
- The refusals concern ACDBASSOCEDGEACTIONPARAM in the 2004 Surface sample and
  DATATABLE in example_2000/example_r14. They already occur with the baseline;
  this benchmark's forced version conversion differs from product same-version saving.
- A warm-cache follow-up used one warm-up and four measured rounds with alternating
  revision order. Sums of per-file median read times are 497.368 ms (baseline)
  and 503.514 ms (patched); DXF write times are 151.833 and 150.804 ms. These small
  differences are measurements on this machine, not a performance guarantee.
- Peak resident memory is not measured by this Linux-oriented harness on Windows.
  Its summaries also do not encode unload state; the specific marker regressions
  and native reopen checks establish that semantic correction separately.

Evidence: `target/xref-bench-public/results.jsonl`,
`target/xref-bench-private/results.jsonl`, `target/xref-codec-benchmark-summary.json`
and `target/xref-bench-timing/` in the product task worktree.

### Product checks completed

| Check | Result |
|---|---|
| Full Rust default-member suite | Pass; 171 document tests passed, two optional tests ignored |
| Read-back verification tests | 12 passed, including corrupted-marker refusal |
| Native unloaded DXF import test | Explicitly run with the native export; passed |
| Public codec corpus / legacy MULTILEADERSTYLE regression | Four corpus tests and the legacy regression passed |
| Frozen local DWG corpus | All six passed, including edit/save/reopen/undo |
| Strict compatibility matrix | Six clean reads; six DWG and six DXF saves verified |
| Python suite | 449 passed |
| Rust formatting / workspace Clippy | Passed; warnings denied |
| Ruff check / format check / mypy | Passed; 172 formatted files and 102 typed source files |
| PyO3 stubtest | Passed |
| cargo deny | Passed; existing duplicate-dependency warnings remain |

The optional `compatibility_tests::native_progecad_unloaded_dxf_imports_the_explicit_marker`
was run with `ARQ_NATIVE_XREF_DXF` set to the independently exported unloaded
DXF. It asserts a clean read, an external `child` block, its unloaded flag and
stored `child.dwg` path. It remains ignored by default because the private
native export is not a CI fixture. The independently authored codec regressions
run without that file.

### Independent reopening of product outputs

All four verified outputs below were independently reopened in the same
ProgeCAD build, with `child.dwg` available. AutoLISP observations and visual
inspection agree on the loaded/unloaded distinction. No repair prompt was
observed; output hashes still match those captured at creation.

| Product output | Reopened BLOCK 70 / 71 | Observed view |
|---|---|---|
| arq-1ca0ae2-loaded.dxf | 100 / absent | Child line and circle displayed |
| arq-1ca0ae2-unloaded.dxf | 4 / 1 | Host local circle only |
| arq-1ca0ae2-native-unloaded.dxf | 4 / 1 | Host local circle only |
| arq-1ca0ae2-unloaded-roundtrip.dwg | 4 / 1 | Host local circle only |

The first two outputs start from the frozen loaded and unloaded DWGs. The third
starts from the full native unloaded DXF; the fourth converts the product's
unloaded DXF back to DWG. Every successful save used the strict product
verification path without accepting losses.

Evidence in this task worktree: `target/dxf-unloaded-matrix.json`,
`target/dxf-unloaded-retained.json` and the four `target/patched-*-reopened.txt`
observations. The retained JSON includes source/output hashes, native reference
flags and individual external verdicts. The native flags can differ from raw
output flags because ProgeCAD resolves the loaded child when reopening it.

### Separate limitation and remaining scope

Converting the **full native ProgeCAD unloaded DXF to DWG** is still refused:
`object DATAOBJECT: 1 -> 0`. No destination DWG was created and the source hash
is unchanged. This is an object-preservation limitation, separate from the
corrected unload marker. The same native file saves back to DXF successfully,
retaining the unloaded reference and passing the existing inventory checks.
No loss-acceptance exception or weakened verifier was introduced.

These results establish reference-state preservation for the tested files and
ProgeCAD build, not complete numerical geometry equivalence, all proprietary
object semantics, or cross-application compatibility. A full numerical external
geometry/reload acceptance test remains open. Linux, macOS and floor-CPU release
checks were not run in this Windows follow-up. Loading external drawings inside
the product remains future work under proposed ADR 0026; this patch preserves
metadata and does not implement that loader.

## Retest after recent main updates

The task branch was advanced to `origin/main` at
`8c6ea9aef34a3e856ef0eb96e00dca67333848ff` (PRs #66, #67 and #68), then the
Xref changes were restored without conflicts. The main working folder and
other agents' worktrees were not changed. `uv sync` rebuilt the extension
against this combined tree before Python and integrated-core measurements.
The earlier checks above describe the pre-integration run; the results below
supersede their product-suite counts for the updated branch.

| Check | Updated result |
|---|---|
| Full Rust default-member suite | 428 passed, zero failures; eight optional tests ignored by default |
| Document tests within that suite | 188 passed, three optional tests ignored |
| Explicit external corpus run | Five passed: codec samples, constraints, solids, fork fixes and all six local Xref drawings |
| Explicit optional document run | Three passed: native unloaded DXF import, dynamic-block parameter preservation during copying, legacy MULTILEADERSTYLE refusal |
| Full Python suite | 469 passed; eight Qt drag-event constructor deprecation warnings |
| Six-DWG Xref matrix | Six clean reads and 12 strict verified saves (six DWG, six DXF) |
| Full native unloaded DXF matrix | DXF verified; DWG refused for DATAOBJECT loss, as before |
| Formatting, Clippy, Ruff, mypy, stubtest, cargo deny | Passed; mypy checked 104 source files and Ruff formatting checked 174 files |
| Source integrity | All 18 original/frozen/retained input hashes still match |

The five corpus tests and three optional document tests were explicitly run
with the existing samples and `ARQ_CORPUS_DIR` / `ARQ_NATIVE_XREF_DXF` inputs;
they are additional to the default Rust suite. No empty or skipped corpus was
counted as a successful validation. Cargo deny retains its existing duplicate
dependency warnings. Python warnings concern deprecated Qt constructors in
`tests/python/test_blocks_palette.py`, not failed assertions.

Evidence: `target/main-retest-summary.json`, `target/main-retest-rust.log`,
`target/main-retest-pytest.log`, `target/main-retest-clippy.log`,
`target/main-retest-corpus.log`, `target/main-retest-optional-document.log`,
`target/main-retest-xref-matrix.json` and `target/main-retest-native-matrix.json`.
The native matrix command exits with status 1 because its DWG row is refused;
its DXF row is verified. This expected refusal is recorded as an outstanding
limitation, not reported as a fully passing conversion corpus.

The codec revision did not change in this integration retest. Its earlier full
suite, benchmark and four ProgeCAD reopen observations remain historical
evidence for that revision; they were not repeated here. This run adds automated
validation of the updated product tree, including the recent block-copy,
template-source and drag-and-drop features. It does not add a manual UI review
or Linux/macOS/floor-CPU validation.

## CELLSTYLEMAP preservation candidate and margin investigation

The candidate fork commit `a59b723fbd3a423b329cf6d40d3f02011b2b5ad9`
removes the CELLSTYLEMAP deletion arm from
`src/io/dwg/dwg_writer.rs::prepare_roundtrip_records`. Its annotated fork tag is
`arqstudiocad/on-c5555a5-xref-cellstylemap`; this is an experimental candidate,
not a completed interoperability acceptance. The main mirror is unchanged.

The raw native DXF contains CELLSTYLEMAP handle 32C, owned by dictionary 32B
under TABLESTYLE FA. The entry key is
`ACAD_ROUNDTRIP_2008_TABLESTYLE_CELLSTYLEMAP`. Removing the writer's deletion
preserves the complete typed object and its dictionary link. The synthetic
regression covers DWG 2007/2010/2013/2018 and repeated saves; an explicitly run
optional native regression compares the complete map and ownership links.
The complete fork suite passed 2,283 tests, zero failures, with 82 ignored.
Eight focused table tests, including the optional native case, passed.

Product checks against main 8c6ea9a and the candidate pin passed: 428 default
Rust tests, 469 Python tests, five external-corpus tests, three optional document
tests, formatting, Clippy, Ruff, mypy, stubtest and cargo deny. The six-source
Xref matrix verifies 12 saves; the native unloaded DXF verifies both DWG and DXF
saves. None accepted losses. The 31-drawing benchmark against 1ca0ae2 has 422
jobs / 211 pairs with no status or measured-summary differences and the same
three known forced-version DWG refusals per revision. Benchmark summaries and
the current verifier do not establish full table-style interoperability.

### Independent margin comparison

ProgeCAD 2026 was used to reopen an identical copy of its own saved AC1027 DWG,
with the copy's SHA-256 checked against that saved file. AutoLISP `dictsearch`
queries record the Standard TABLESTYLE, including legacy groups 40 and 41.
The native DXF and product output were queried with the same lookup. Handles,
reactors and extension-dictionary linkage are excluded only from the flat
property comparison; all 90 remaining property pairs were compared.

| Opened drawing | Group 40 | Group 41 |
|---|---|---|
| Native ProgeCAD DXF | 1.5 | 1.5 |
| Native ProgeCAD DWG, independently reopened | 1.5 | 1.5 |
| Product DWG using candidate a59b723 | 0.06 | 0.06 |
| Native DWG rewritten by the same fork | 1.5 | 1.5 |
| Product DWG with only its base cell style replaced by the native base | 0.06 | 0.06 |
| Product DWG with only the companion map and its dictionary entry removed | 1.5 | 1.5 |

The other 88 queried flat properties match between the native DXF and product
DWG. Debug inspection of the typed records shows identical modern row overrides
in the native and product DWGs. The product's stored base/row values retain 1.5
in our reader, so an internal reader-writer round trip alone misses the external
interpretation difference. Rewriting the native DWG preserves 1.5 even though
the existing writer clears flag bit 8; this rules out that flag alone as the
cause in this test. Replacing only the base style does not correct the symptom.

Removing only the companion map and its dictionary entry from a diagnostic copy
restores both queried margins to 1.5 while retaining the dictionary. This
isolates the retained legacy companion as the trigger for the observed
ProgeCAD behavior. The diagnostic copy was written directly by the codec and
is **not** an accepted-loss product save or a proposed workaround. The current
product verifier still refuses a removed DATAOBJECT without a proven equivalent
representation; no generic exception was introduced.

These observations do not prove the internal mechanism used by ProgeCAD, nor
measure rendered cell spacing: the exercise contains no table entity. They do
prove that the candidate's passing object-count and round-trip tests are
insufficient grounds to claim faithful external table-style behavior. Do not
merge the map-retention candidate as a completed fix. A replacement must prove
both complete table-style semantic equivalence and the native margin values,
without accepting arbitrary DATAOBJECT loss or hiding records under new keys.

Evidence: `target/cellstylemap-margin-investigation.json`, its six referenced
native observation files, `target/cellstylemap-retained.json`,
`target/cellstylemap-xref-matrix.json`, `target/cellstylemap-native-matrix.json`
and `target/cellstylemap-benchmark-summary.json`. The fork worktree retains the
full-suite log and temporary typed-record probes. Diagnostic drawings have
separate `margin-diagnostic-*` names; original DXF and product-output hashes
still match their recorded values. No source code changed during this margin
investigation.

## Final conversion and acceptance: 59aaf4c

The application was synchronized with `origin/main` at `05acd82` before the
final run. The exact engine pin is
`59aaf4cddee5a1a7c86a640c1246225be1bfe2bc`, with annotated immutable fork tag
`arqstudiocad/on-c5555a5-xref-table-conversion`. The upstream mirror is unchanged.

`TableStyle::represents_cell_style_map` in the fork's
`src/objects/table_style.rs` requires a nonempty map, unique cell ids, and exact
matching ids and complete named-cell records in the modern overrides.
`prepare_roundtrip_records` in `src/io/dwg/dwg_writer.rs` retires the legacy map
only after that proof and checks its owner, reactors and extension dictionary.
A differing or unknown companion is retained. This replaces the rejected
retention candidate: retaining the redundant companion caused ProgeCAD's
legacy-margin fallback from 1.5 to 0.06.

In the product, `crates/arq-document/src/table_style_verify.rs::converted`
independently checks the full cells, table properties and ownership links in
the source and reopened candidate before treating the removed map as a
representation change. `Document::losses_in` in `inventory.rs` offsets only
that proven map count; unrelated DATAOBJECT losses remain losses.
`Document::compare_table_style_maps` also rejects modified retained companions
or missing owners, independently of the accepted-loss callback. Row style
name/handle references must resolve to the same text-style record. Only
inactive legacy fill colors and single-border double-line spacing are
normalized; complete modern cell records are still compared exactly. A
synthesized modern base must match its canonical values and explicit margins.

Six document regressions cover strict DWG/DXF conversion, eleven cell/style
corruptions, broken ownership, unrelated DATAOBJECT loss, injected write
corruption, and legacy TABLESTYLE loss with an unchanged destination.

### Native reopening

The final strict product outputs are `arq-59aaf4c-native-unloaded.dwg` and
`arq-59aaf4c-native-unloaded.dxf` in the retained `unloaded-dxf` review folder.
Both were independently opened in ProgeCAD 2026. The queried TABLESTYLE group
40/41 values are **1.5 / 1.5** in both; all 90 flat style-property pairs match
the native source after excluding handles and dictionary/reactor metadata.
The external block remains `child.dwg` with unloaded marker 71=1. Output and
source hashes match the recorded values after reopening. No original was
saved over. These measurements concern style properties, not the spacing or
appearance of an actual rendered table entity.

Evidence is retained under `target/table-conversion-{retained.json,
dwg-reopened.txt,dxf-reopened.txt,xref-matrix.json,native-matrix.json}`.
The two matrices contain six clean local DWG reads and twelve strict saves,
plus both strict saves from the full native unloaded DXF.

### Legacy refusals and validation scope

Two R2000 corpus fixtures, `example_2000.dwg` and `2000/Constraints.dwg`, carry
a TABLESTYLE newer than their file format. The existing writer drops that
style when writing R2000 DWG, while retaining its companion map. The new
semantic guard correctly refuses the broken link. No legacy-format bypass is
used. The external-corpus tests explicitly require these refusals, inspect the
retained candidate for the actual TABLESTYLE loss, and assert that a
preexisting destination is unchanged. A successful save or a different failure
fails those expectations. DXF saves, editing, drawing coverage and undo remain
checked for the example fixture. Supporting lossless down-version conversion
of this data remains separate work.

The fork's full suite passed **2,284 tests**, zero failures, 82 ignored; nine
focused table tests included the optional native fixture. The reproducible
comparison against `1ca0ae2` covers 25 public and six local drawings: 422 jobs,
211 paired comparisons, zero new status/summary differences. Both revisions
have the same three known forced-2018 DWG encoding failures (DATATABLE and
ACDBASSOCEDGEACTIONPARAM). Summary equality is not a substitute for the full
cell comparisons or native reopening above. Concurrent build load makes these
runs unsuitable for a performance claim; peak memory was not measured here.

Final checks against main `05acd82`: **434 default Rust tests passed**, eight
optional tests omitted from that default invocation; all **five external-core
corpus groups** and all **three optional document tests** were then explicitly
run and passed. The two R2000 fixture DWG cases are verified refusals as
explained above. **529 Python tests passed** (nine existing warnings).
Workspace Clippy with warnings denied, formatting, Ruff (183 files), mypy
(113 source files), stubtest (one module), and dependency license/advisory checks
passed. Eighteen original/frozen/retained input hashes and both final output
hashes were checked again and remain unchanged. Full run logs and matrices
are retained under `target/table-conversion-*`.
No external-file resolver, reload control, BIM implementation, Linux/macOS
validation, floor-CPU release validation or broad milestone closure is claimed.
ADR 0026 remains Proposed.
