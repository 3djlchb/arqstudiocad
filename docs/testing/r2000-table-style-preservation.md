# R2000 table-style preservation

Investigation: 2026-10-31. Published-build validation: 2026-11-01.
Status: codec correction published; product acceptance recorded below, awaiting PR review and merge.
Application base: main `6962ae5` (PR #105 cell fidelity and PR #106 documentation merged).
Codec base: `5c09a390bbe41c85f53f7982da72c437ec4bc9f4`, AcadRust 0.6.8.
Both isolated working copies use `fix/r2000-tablestyle`.

## Reproduction and cause

Two fetched public LibreDWG fixtures, `libredwg/example_2000.dwg` and `libredwg/2000/Constraints.dwg`, already contain TABLESTYLE objects and their companion graph in R2000 (AC1015). The product's semantic verifier correctly rejected their previous DWG rewrites: the writer removed the ACAD_TABLESTYLE dictionary and its style while retaining the owned companion map. A loss-acceptance callback could not override that broken graph.

`src/io/dwg/dwg_writer.rs::prepare_legacy_document` removed the dictionary solely because TABLESTYLE was introduced after R2000. The same-version class-table preservation earlier in `DwgWriter::write_to_writer` did not protect the objects from this later removal. Three regressions reproduce missing styles before the fix, including both actual fixtures (`target/r2000-tablestyle-red.log` in the codec working copy).

The correction retains the dictionary when the target is AC1015, the recorded DWG source version equals that target and the prepared class table declares TABLESTYLE. Existing encoding and round-trip companion machinery write the style and its ownership graph. Real down-version conversion retains its previous policy. No product guard, loss allowance, public API or rendering rule changes; no new dependency is introduced.

## Regression coverage

`tests/r2000_table_style.rs` covers same-version style/dictionary retention, caller immutability, explicit margin edits, unchanged genuine-downgrade behavior and both optional public fixtures. The fixture tests check style identity, owner and margins, extension-dictionary identity, all dictionary entries and the complete cell-map object. XRecords compare every modeled field and payload, excluding their encoded DWG transport bytes/version/bit-count; those bytes can change when handle streams are rewritten and are not a semantic equality claim.

The complete default fork suite passes **2,438 tests**, zero failures, 89 optional tests ignored. The focused/native run passes **44 tests**, including the five new cases and existing native table, DWG 2007, per-cell and Map-resource regressions. Scoped rustfmt and diff checks pass; unrelated upstream formatting is untouched.

The application adds `table_style_verify_tests.rs::existing_r2000_style_graph_survives_verified_saving`, which saves both real fixtures through the normal verifier, reopens them and checks recovery, round-trip differences and absence of TABLESTYLE inventory loss. The six existing semantic-guard tests still pass, including broken ownership, corrupted cells, arbitrary DATAOBJECT loss, injected corruption and real legacy-style loss.

The core corpus no longer special-cases these files as expected TABLESTYLE refusals: they follow the complete save/edit/reopen/undo checks. The R2000 constraint test still permits only its declared MULTILEADERSTYLE loss. The focused document test requires MULTILEADERSTYLE loss prompting and an unchanged destination in both R14 and R2000, with no TABLESTYLE loss in the corrected R2000 case and no semantic mismatch. The separate R14 table-style policy is unchanged. All four public core corpus groups pass on the candidate.

## Paired corpus benchmark

The existing comparison wrapper runs the candidate and previous published codec in separate processes on 25 public samples, six native Xref files and three native table sources. Reports use new directories and do not resume earlier jobs.

| Protocol | Jobs | Paired operations | Changed success states | Changed summaries |
|---|---:|---:|---:|---:|
| Preserve source version | 476 | 238 | 0 | 4 |
| Force DWG AC1032 | 464 | 232 | 0 | 0 |

The four changed summaries are exactly the self/cross reads of the two corrected R2000 DWG outputs: each retains one additional TABLESTYLE and its dictionary. All entity/type counts, geometry aggregates, other object counts, recovery and diagnostics remain equal. Source-version jobs all succeed. Forced AC1032 retains the same three write refusals per engine: ACDBASSOCEDGEACTIONPARAM in Surface.dwg and DATATABLE in example_2000.dwg / example_r14.dwg. No performance claim is made from these concurrent unoptimized runs.

Evidence: `target/r2000-bench-{source,2018}-{public,xrefs,tables}` and `target/r2000-benchmark-summary.json`.

## Independent ProgeCAD 2026 observations

Both original fixtures and candidate source-version DWG outputs are opened read-only in ProgeCAD. The private audit records native dictionary/style handles and owners, all non-transport style DXF properties and available extension dictionaries/companions. Exact drawing-name checks prevent observing an already-open document under another file's label. Explicit native document activation is required: opening an already-open file alone does not activate it in this application.

The accepted captures are `target/r2000-native-v4-{source,candidate}-{example,constraints}.tsv`. Each source/output pair has **90 identical style DXF pairs**, plus matching observed dictionary/style handles and owner properties (nine compared observation keys). Native loaded observations have no remaining style extension dictionary in both source and candidate; complete companion retention is proved separately by the codec and product graph checks. Raw entget data is retained, with runtime entity references excluded from comparison; numeric comparison uses the existing 1e-8 tolerance. The strict report is `target/r2000-native-comparison.json`.

Earlier diagnostic captures are not acceptance evidence: the initial helper failed before completing observations, and v3 captures inspected the wrong active drawing. Those attempts remain under target and are explicitly rejected rather than counted as passes. No original drawing was overwritten. ProgeCAD also reports that CMLEADERSTYLE cannot be set when reopening the rewritten Constraints drawing: unsupported legacy multileader styles remain a separate, declared loss; these TABLESTYLE measurements do not establish full fidelity of every header reference or other object.

## Publication and integration

Published codec commit: `bd369d9060751f3da6824a0d93529d05335563f1`.
Immutable fork tag: `arqstudiocad/on-ea66abe-r2000-tablestyle`.
Annotated tag object: `fdcc45f1dd08bd91c2827219136a3375a3f59e60`, remotely verified to peel to that exact commit.
The application pins that Git revision and retains `=0.6.8`; the preliminary local path is removed. Published-build acceptance is recorded below.

Milestones 03.4 and 03.5 remain open. This corrects a same-version preservation defect, not general down-version TABLESTYLE support or MULTILEADERSTYLE encoding. The default write refusals and loss-consent rules remain active. Private generated CAD files, native captures and logs are not committed. The maintainer creates the pull request manually.

## Published Git-pin product acceptance

The product is rebuilt with `uv sync` against the published Git revision, not a local codec path. On application base `6962ae5`, the final Rust suite passes **488 tests**, zero failures, with 13 optional cases excluded from the default run. The four public core corpus groups, the native table corpus group, the native Xref corpus group and all **eight optional document tests** pass separately. Clippy (`--workspace --all-targets -- -D warnings`), cargo fmt and cargo deny pass. The offscreen Python suite passes **808 tests**, zero failures (11 Qt deprecation warnings from drag-event test constructors). Stubtest passes for the built module; Ruff check, Ruff format (231 files) and mypy (147 source files) pass.

| Integrated matrix on 34 unchanged inputs | Previous pin | Published correction |
|---|---:|---:|
| Clean reads | 34 | 34 |
| Verified DWG/DXF saves | 58 | 59 |
| Saves blocked pending loss consent | 8 | 9 |
| Semantic save errors | 2 | 0 |

Only two DWG rows change: `example_2000.dwg` now verifies, and `2000/Constraints.dwg` now reaches the existing MULTILEADERSTYLE consent refusal instead of a TABLESTYLE graph error. Both DXF rows and every other drawing's full result remain identical after normalizing only random temporary-directory suffixes. The matrix command intentionally exits 1 while any loss remains blocked; this is not an all-verified corpus claim. The strict baseline comparison passes (`target/r2000-git-matrix-comparison.json`).

Both product-generated DWGs are byte-identical to the corresponding independently observed ProgeCAD artifacts. The example saves without explicit loss acceptance. The Constraints artifact is produced only after the private test explicitly accepts `object MULTILEADERSTYLE`; the strict matrix leaves that loss blocked. Their SHA-256 values are respectively `d1ce4469ed8cc9ff194ad1b3845c7d71d7e14eef5d2ad0622cb27e5555de0dd5` and `d54bac36b910e437b934129d3295088e86dcf101acd2bffdfa5cdd3e5deeda4a`. Original input hashes remain unchanged. See `target/r2000-git-product-artifacts.json`.

Acceptance logs use the `target/r2000-git-` prefix. The evidence manifest records hashes of accepted native captures, codec regression logs, benchmark outputs and published-build reports; unsuccessful native diagnostic captures are excluded. Windows local validation does not establish Linux/macOS or release CPU-floor acceptance; those checks remain for the corresponding CI/release gates.
