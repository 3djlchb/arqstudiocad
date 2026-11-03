# Existing MULTILEADERSTYLE preservation in legacy DWG

Date: 2026-11-03. Initial application base: `c302c07`; final integration base:
main `226a2fb`, including merged measurement PR #112. Codec baseline:
`bd369d9060751f3da6824a0d93529d05335563f1`, AcadRust 0.6.8.
Correction: `c8f4b05ec9bd0006fb5f40c45113f40e1a74ecaa`, published as immutable
fork tag `arqstudiocad/on-ea66abe-legacy-mleader`.
Status: codec, independent native and published Git-pin acceptance pass.
Product adoption awaits PR review and merge.

## Reproduction and correction

The two legacy Constraints inputs decode styles `0xD8` and `0xE5` owned by
`0xD7`. `prepare_legacy_document` in `src/io/dwg/dwg_writer.rs` removed their
ACAD_MLEADERSTYLE dictionary solely because the target was older than AC1018,
even when it was the original AC1014/AC1015 version and its class table already
declared MLEADERSTYLE. The preceding [triage](compatibility-loss-triage.md)
measured two DWG save refusals with two styles becoming zero.

Tests-only codec commit `9816b10` reproduces three dictionary-removal failures
(synthetic R2000 plus both native versions), with the genuine-downgrade
control passing (`target/mleader-red.log`). The fix permits retention only
when the target is AC1014 or AC1015 and matches the recorded DWG source
version; the MLEADERSTYLE class check remains. Existing R2004+ behavior,
new legacy documents and genuine downgrades keep their prior rules.
No production verifier, default loss allowance, consent rule, public API or
renderer changes; there is no new dependency or upstream submission.

Five focused codec tests pass, including both native inputs. The native tests
compare the complete decoded styles, exact handles, owner dictionary and
current-style name, then repeat after editing text heights and arrow sizes
and checking caller immutability. The synthetic fixture models legacy
records: name identity belongs to dictionary keys and the later vertical
attachment field is absent. Initial modern defaults were not representable
in this layout; the first-candidate log retains that diagnostic, while full
native equality checks remain unchanged. Tests:
`tests/legacy_mleader_preservation.rs`; `target/mleader-focused.log`.
The complete default fork suite passes **2,441 tests**, zero failures, 91
optional tests ignored.

The product's `compatibility_tests.rs::existing_legacy_multileader_styles_survive_verified_saving`
now requires normal saving and reopening, no recovery, no semantic/save loss
and complete style equality for both real files. Removing the two styles in
a diagnostic copy still produces the same inventory refusal, exceeding the
unchanged one-object empty-drawing allowance. The core constraint regression
also requires normal saving, without its former MULTILEADERSTYLE acceptance.

## Independent ProgeCAD 2026 validation

All original, candidate and previous-codec control DWGs are opened read-only.
The observer verifies the exact active full path before collecting data and
refuses to overwrite captures. No source or user document is saved. The two
candidate outputs are byte-identical to normal strict product saves, without
explicit loss acceptance (`target/mleader-product-artifacts.json`).

| Version | Source styles | Previous codec reopened natively | Corrected output | Compared observation keys | Differences |
|---|---:|---:|---:|---:|---:|
| R2000 / AC1015 | 2 | 1 replacement default | 2 original | 14 | 0 |
| R14 / AC1014 | 2 | 1 replacement default | 2 original | 14 | 0 |

Each key for style properties contains the complete ordered non-transport
DXF pair list. The comparison includes dictionary entries, style handles
`D8`/`E5`, owner `D7`, dictionary owner `C`, current style `Standard` and both
extension-dictionary absence flags. Native keys `Annotative` and `Standard`
are distinct even though both internal style descriptions are `Standard`.
Transient entity pointers are normalized to referenced type/name, with exact
style and ownership handles checked separately. Native numeric comparison
uses the existing comparator's tolerance of 1e-8; raw entget is retained.
This is style preservation evidence, not a complete drawing visual audit.

Accepted captures: `target/mleader-native-v2-{source,candidate}-{2000,r14}.tsv`;
previous-codec controls: `target/mleader-native-baseline-{2000,r14}.tsv`.
`target/compare_mleader_native_v2.py` rejects duplicates, incomplete key sets,
wrong drawing paths and any property difference. The first v1 observation
incorrectly keyed entries by the child object's group 3 rather than the
parent dictionary key; its duplicate-key refusal is retained and excluded
from acceptance. V2 traverses the dictionary's own key/reference pairs and
is collected afresh; no comparison requirement was relaxed.
The [portable native observations and capture hashes](legacy-mleader-native.json)
retain the accepted measurements independently of ignored target files.

The previous R14 rewrite raises both `Cannot set CTABLESTYLE to that value`
and `Cannot set CMLEADERSTYLE to that value` in native history. The corrected
rewrite retains only the CTABLESTYLE warning; the original R14 input does
not show it. The R14 table-style removal policy is unchanged, as confirmed
by the paired object summaries. This pre-existing limitation is **not** fixed
or certified by the multileader result. R2000 table-style preservation from
`bd369d9` remains covered separately. All source and candidate hashes were
checked again after native inspection and are unchanged.

## Paired codec benchmark

The candidate and previous published codec run in separate processes on
34 drawings: 25 public samples, six native Xref DWGs and three native tables.

| Protocol | Jobs | Pairs | Changed success states | Changed summaries |
|---|---:|---:|---:|---:|
| Preserve source version | 476 | 238 | 0 | 8 |
| Force DWG AC1032 | 464 | 232 | 0 | 0 |

Exactly four legacy DWG outputs retain their MULTILEADERSTYLE object(s) and
one dictionary, observed in self/cross reads (eight summary differences).
All other object counts, geometry/entity aggregates and summaries match.
Forced AC1032 retains the same three refused inputs per engine: Surface
associative objects and two opaque DATATABLEs. No performance claim is made.
Evidence: `target/mleader-benchmark-summary.json` and
`target/mleader-bench-{source,2018}-{public,xrefs,tables}`.

## Integrated product acceptance

The original published pin was rebuilt on base `c302c07`, giving 34 clean
reads, 59 verified saves, nine loss blocks and no other save errors. The
local candidate changes only the two intended DWG rows: **61 verified,
seven blocked, zero other errors**, with all 34 source hashes unchanged.
All DXF outcomes and the other 32 drawings' results match after normalizing
only random temporary-directory suffixes. The matrix exits 1 intentionally
while blocked losses remain; this is not an all-verified corpus claim.

The earlier local candidate passes 513 Rust and 808 Python tests, Clippy,
formatting, Ruff, mypy, stubtest and cargo deny; six public corpus groups,
both local table/Xref groups and eight optional document tests pass. These
counts predate merged PR #112. Final published-Git acceptance on updated main `226a2fb` independently
passes **524 Rust and 812 Python tests** (11 Qt drag-event deprecation
warnings), **seven public core corpus groups**, both native table/Xref
groups and **eight optional document tests**. Clippy with all targets and
warnings denied, cargo fmt, cargo deny, Ruff check/format (234 files), mypy
(148 source files) and stubtest pass. The extension was rebuilt by `uv sync`
from the exact published Git pin; the lockfile changes only AcadRust
provenance. Acceptance logs use the `target/mleader-published-` prefix.

The final published matrix repeats **61 verified saves / seven blocked
losses / zero other errors**, with only the R14/R2000 Constraints DWG rows
changed from the original baseline; `target/mleader-published-matrix-comparison.json`
checks complete per-drawing outcomes and all 34 source hashes. Final strict
product DWGs again match the independently observed artifacts byte for byte;
[published artifact/source hashes](legacy-mleader-native.json) are retained
with the native observations. No explicit loss consent is used. The earlier
baseline is base `c302c07`; the final run includes merged PR #112 on `226a2fb`.
Its API, measurement implementation and mass-property references are unchanged
by this correction.

Remaining seven blocks: four dynamic-block purge-preventer DXF omissions,
two DATATABLE DXF omissions and one Surface associative-object DXF omission.
R14 table-style behavior, broader format/version coverage, Xref resolution
and external BIM validation remain separate work. Milestones 03.4/03.5 stay
open. Windows validation does not establish Linux/macOS or release CPU-floor
acceptance. Claude's merged mass-property references are unchanged.

## Exact Open CAD Studio reference review


A fresh dependency review resolves this chain, rather than assuming the
2026-10-11 historical snapshot is current:

| Component | Inspected revision |
|---|---|
| [OpenCADStudio](https://github.com/HakanSeven12/OpenCADStudio/tree/27fddaa55bd32f454c92d2566b27d1e27e78d49b) | `27fddaa55bd32f454c92d2566b27d1e27e78d49b` |
| Its `codec` dependency, [opencaddocument](https://github.com/HakanSeven12/opencaddocument/tree/598b1bb9adc89ab66c8a9c1ca0327e09f8f5af3e) | `598b1bb9adc89ab66c8a9c1ca0327e09f8f5af3e` |
| Underlying [HakanSeven12/acadrust](https://github.com/HakanSeven12/acadrust/tree/7e7940f6fb17419629958ceeb79dfbd2d954dabb) | `7e7940f6fb17419629958ceeb79dfbd2d954dabb` |
| [hakanaktt/acadrust upstream](https://github.com/hakanaktt/acadrust/tree/17a3125153335eac19efa6409ace130a75c0bf54) | `17a3125153335eac19efa6409ace130a75c0bf54` |

OpenCADStudio's Cargo.toml names opencaddocument; its Cargo.lock and that
package's manifest identify the underlying AcadRust fork. Both the inspected
reference writer and current upstream still use the same legacy removal rule;
no ready-made preservation fix was found there. The relevant files are
`src/io/dwg/dwg_writer.rs` and
`src/io/dwg/dwg_stream_writers/object_writer/objects.rs`. Their repository
manifest/LICENSE identify MPL-2.0; no source is copied for this candidate.
Primary files and resolved revisions are retained in `target/reference-review/`.
No reference engine upgrade or new product dependency is proposed.
