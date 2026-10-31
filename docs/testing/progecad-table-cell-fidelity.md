# Native table cell fidelity: derived DXF cache

Date: 2026-10-31. Status: correction published; Git-pinned application acceptance passed.
Application base: main `b22b66e`. Codec base: `f1b0980d181fa64d36d5740dd8140f656f16b9fb` (AcadRust 0.6.8).
Both isolated working copies use `fix/table-cell-fidelity`. Original drawings and the shared main working copy remain untouched.

## Reproduced losses and correction

The previous [DWG-to-DXF correction](progecad-table-dxf-format.md), merged in PR #101, left sixty complete cell-record differences across 249 native measurements. Investigation distinguishes actual losses from representation differences: the flat cache omitted inherited cell heights, confused modern content-color flags with the legacy text-height bit (inventing 0.18), and failed to retain five shared-border records correctly.

The modern TABLECONTENT companion retains the complete original formatting. The writer now prepares a derived legacy cell cache in its private document copy: only asserted height overrides are resolved; conflicting row and column heights are retained in the modern companion rather than guessed. Shared borders retain independent color, weight and visibility properties, with a real owner and a virtual neighboring edge. Conflicting edge overrides remain independent. Duplicate global margin/height overrides are omitted only from this generated cache because native reopening otherwise reapplies them over the per-cell values. Existing vendor round-trip records are untouched. Cell overrides precede CELL_VALUE and the value-present flag is emitted. Visibility-only legacy overrides also survive DXF-to-DWG without inventing a lineweight.

The implementation is in the AcadRust fork:

- `src/io/dxf/writer/section_writer/table_cells.rs::project`: derived heights and shared-edge cache.
- `src/io/dxf/writer/section_writer/table_format.rs::with_legacy_cell`: independent modern-to-legacy property bits.
- `src/io/dxf/writer/section_writer.rs::write_table_cell`: native order, value flag and virtual-edge payload.
- `src/io/dxf/writer/table_roundtrip.rs::prepare`: original owned companion plus private cache projection.
- `src/entities/table.rs::cell_override_bit` and `TableCell::binary_layout`: independent visibility through both formats.
- `tests/table_cell_projection.rs`: fifteen focused regressions; `tests/table_dxf_format.rs`: updated modern-companion versus legacy-only expectations.

No product API, dependency, comparator tolerance or safe-save protection changes.

## Independent native controls

The unchanged `tools/progecad_table_corpus.lsp` captures four tables and 249 properties. `tools/compare_progecad_tables.py` still compares all properties strictly, validates exact drawing names and rejects incomplete captures. Each output was independently reopened read-only in ProgeCAD 2026; observations use fresh files.

| Native reopening | Differences against original DWG control | Interpretation |
|---|---:|---|
| Previous published DWG-to-DXF | 60 cell records | Includes measured height and border losses |
| Corrected DWG-to-DXF | 52 cell records | All sixty cell heights, border colors/weights and 189 non-cell properties match |
| Original ProgeCAD-exported AC1024 DXF, no codec rewrite | 52 cell records | Same CELL_VALUE flag normalization also occurs natively |
| Corrected native DXF rewrite | 52 cell records | Exactly matches the freshly reopened native DXF control: zero differences between the two |
| Corrected native DXF-to-DWG | 0/249 | Retains previous native conversion fidelity |
| Corrected DWG 2007 rewrite | 0/249 | Retains previous native reader/writer fidelity |

The 52 native DXF differences are CELL_VALUE group 93 flags changing from 6 to 0 in text cells. Text payload, value type, unit and empty format string remain equal. Changing only the native DXF version header to AC1032 also reproduces those differences, but the fresh original AC1024 control proves that they must not be attributed solely to the version header. The modern DWG-to-DXF candidate additionally has explicit visible-edge presence bits in five border records; the measured color, weight and visibility are retained. Compared directly against the native DXF control, those are the only five differing records. These are measured representation differences in this corpus, not a claim of universal format equivalence. The raw comparator remains nonzero and its full reports are retained.

Retained artifacts in the isolated application working copy:

- `target/cell-format-cache-candidate.{dxf,tsv}` and `cell-format-cache-comparison.json`.
- `target/cell-final-native-{original-dxf,dxf-dxf,dxf-dwg,2007-dwg}.tsv` and strict comparison JSON reports.
- `target/cell-final-native-{rewrite,modern}-vs-native-dxf.json`.
- `target/cell-final-native.scr`: native read-only batch audit.
- `target/native-format-as-ac1032-diagnostic.dxf` and its observation/report.

The original DWG SHA-256 is `6f4050e24aaa4413e46e290cc4a3fdcd4f39f1c8783c566c8002a00fe5705678`.
Unsupported ProgeCAD cell-height/value/format API getters returned errors during diagnosis and are not counted as passed measurements. Diagnosis also ruled out cell-field order alone, missing TABLEGEOMETRY, edge ordering and the table-data flag as sufficient fixes; those experimental modifications are not shipped.

## Regression and corpus evidence

The final focused run passes 39 tests: fifteen new cache regressions, twelve previous DXF-format regressions, six instance-format regressions, two native DWG 2007 cases and four Map-resource cases. Retained red logs demonstrate the original height/color-bit/border/value-flag failures, ordering, derived inheritance, shared edges, ambiguous heights and visibility loss; no test exemptions were added. The first complete-suite attempt exposed three old synthetic conversion fixtures that used legacy masks without declaring the legacy encoding or coupled visibility to weight. Those fixtures now declare the legacy format and its independent visibility bits; all original value, fill, edge and immutability assertions remain, and all four conversion tests pass. The failed run is retained as `target/table-cell-acceptance-suite.log`, with the final repeat recorded separately.

The final paired benchmark compares the candidate against published `f1b0980`, in separate processes on 25 public samples, six native Xref drawings and three native table inputs. Output directories were new, so no completed jobs were reused.

| Profile | Jobs | Paired operations | Changed success states | Changed summaries |
|---|---:|---:|---:|---:|
| Preserve source version | 476 | 238 | 0 | 0 |
| Force DWG AC1032 | 464 | 232 | 0 | 0 |

All source-version jobs succeed. The forced profile retains the same three refusals per codec: ACDBASSOCEDGEACTIONPARAM in Surface.dwg and DATATABLE in example_2000.dwg / example_r14.dwg. These are existing conversion protections. Timings from concurrent unoptimized runs are not used as performance evidence. Reports: `target/table-cell-final-bench-{source,2018}-{public,xrefs,tables}` and `target/table-cell-final-benchmark-summary.json`.

The complete default codec suite, including doctests, passes **2,435 tests**, with zero failures and 87 optional tests ignored (`target/table-cell-acceptance-final-suite.log`). Scoped rustfmt checks pass for the modified leaf modules and tests; unrelated upstream formatting is not changed.

Published codec commit: `5c09a390bbe41c85f53f7982da72c437ec4bc9f4` (parent `f1b0980`). Immutable fork tag: `arqstudiocad/on-ea66abe-table-cell-cache`; remote annotated tag object `be3f49862f9afb3052cc54b91686f8b059294400` peels to that exact revision. The product now pins that Git revision, retaining `=0.6.8`. The temporary local path is removed. Published Git-pinned application acceptance passed as recorded below.

## Scope and next work

Milestones 03.4/03.5 remain open. This corrects the demonstrated cell-height/border losses without claiming complete fidelity for unmeasured properties, all table versions or table editing. Keep expanding operation/version and independent CAD coverage; Xref resolution and BIM-specific corpora remain separate work. The historical sixty raw differences are investigated, rather than silently exempted from acceptance.

## References

- [Autodesk TABLE DXF reference](https://help.autodesk.com/cloudhelp/2020/ENU/AutoCAD-DXF/files/GUID-D8CCD2F0-18A3-42BB-A64D-539114A07DA0.htm): cell codes and virtual shared edges.
- [ProgeCAD Table API members](https://www.progesoft.com/products/progecad-professional/manual?mp=developer-reference/api-reference/objects/table-object/members): supported independent observation API.

No external implementation or documentation text was copied.

## Published Git-pin application acceptance

`uv sync` fetched the published revision and rebuilt/installed the extension successfully (7 minutes 35 seconds). Comparing all 233 lockfile packages confirms that only AcadRust's Git source changed; the product keeps `=0.6.8`, with no local path.

| Check on application base b22b66e | Result |
|---|---|
| Locked Rust default suite | 488 passed, zero failures, 12 optional tests ignored |
| Locked workspace/all-target Clippy, warnings denied | Passed |
| External core corpus groups | Four public groups, native three-input table group and six-file Xref group passed |
| Optional document regressions | All seven passed, including native unloaded-reference DXF |
| Full Python suite, QT_QPA_PLATFORM=offscreen | 795 passed, no skips, 11 deprecation warnings, 87 seconds |
| Stubtest | One extension module passed |
| Ruff check / format | Passed; 229 files inspected by formatting |
| mypy | 146 source files passed |
| Cargo formatting / cargo-deny | Passed; advisories, bans, licenses and sources accepted |
| Repeated integrated matrix | 34 clean reads, 58 verified saves, 8 blocked losses, 2 known safe errors |

The matrix's exit status 1 remains expected for the two R2000 TABLESTYLE-owner refusals, not an all-green conversion result. Every full source path, source hash, inventory, diagnostic and all 68 save outcomes match the freshly measured baseline extension. Only random temporary-directory suffixes in error messages are normalized for that comparison. Evidence: `target/table-cell-{baseline,git}-matrix.json` and `target/table-cell-git-matrix-comparison.json`.

The actual published-build safe-save output `target/table-cell-git-product-candidate.dxf` has SHA-256 `c1bb8ee790906e773f66ed3e42a927016da8ca7887e8c8bf00d53565c529fe7c`, identical to the independently ProgeCAD-validated `cell-format-cache-candidate.dxf` and final codec benchmark export. The original source hash is unchanged. No additional native reopening is claimed after publication: the published build produces the identical verified artifact.

Private observations, generated CAD files and raw logs are not committed. Their hashes are retained in `target/table-cell-evidence-manifest.json` in the isolated working copy. These are local Windows checks, not Linux/macOS CI or release CPU-floor coverage. The default codec suite is the required acceptance for this non-feature-gated writer change; no new all-features run is claimed. The maintainer creates the PR manually.
