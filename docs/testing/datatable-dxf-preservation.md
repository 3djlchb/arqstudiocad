# DATATABLE DWG-to-DXF preservation investigation

Status: **codec published; local published-pin product acceptance passed**. Updated 2026-11-06.
Milestones 03.4 and 03.5 remain open. This branch pins fork `bab8ac0`;
main still pins `c8f4b05` at inspection.
The isolated product branch is based on main `3007dd8` after
rebasing its own progress commit. The shared main worktree is not modified
by this task; earlier product baseline measurements retain their recorded base.

## Scope and actual cause

The public LibreDWG `example_2000.dwg` and `example_r14.dwg` contain one
DATATABLE each, at handles `B2C` and `D62`, respectively. Both tables have
flags 2, four columns, 180 rows, column types `[2, 1, 1, 6]`, and 180 hard-owned
XRECORD targets in the final column. Type 6 is HardOwnerId. Its native DWG
reference kind is HardOwnership and its DXF group code is 360.

The previous `c8f4b05` codec's `DataTableCellType::has_native_codec`
(`src/objects/class_object.rs`) accepts only types 1 through 5. Consequently,
the DWG class-object reader retains each entire table as opaque bytes.
The DXF unknown-object writer has no DXF body to emit, omits the table, and
the product inventory guard correctly blocks the missing object.

## Candidate changes and native findings

Executable changes are saved locally in isolated AcadRust commit `be58a0b`,
on branch `fix/datatable-dxf-preservation`, after the retained red reproductions.
The subsequent DXF XRECORD correction is saved in commit `bab8ac0`, after red-test
commit `9faadfd`. The revised complete suite passes 2,448 active tests with
93 ignored by normal selection; all three focused hard-ownership cases,
including the two optional external inputs, also pass on that commit.

- Add native type-6 DWG/DXF read and write paths, preserving reference kind,
  handles, table owners and all modeled fields of the 180 owned XRECORDs.
- Use group-100 subclass `ACDBDATATABLE` for R14/R2000 DXF and
  `AcDbDataTable` from R2004 onward. Read either form. The CLASS record's C++
  name remains `AcDbDataTable`.
- Decode the implicit R14 XRECORD cloning rule as KeepExisting (1). The field
  is absent in R14 DWG; using 0 loses the native rule when promoting formats.

Independent ProgeCAD 2026 controls establish the subclass distinction.
A minimal native fixture and a hard-owned-cell variant reopen with two columns
and one row in current DXF. Under old headers, changing only the group-100
subclass to uppercase preserves both columns and the cell values. Changing
only the CLASS C++ name does not. A fresh ProgeCAD Save As operation selecting
**AutoCAD 2000 ASCII** produces actual header AC1015 and that uppercase
subclass. A version-limit refusal was an investigative hypothesis and has
been removed; these observations do not establish a format impossibility.

The full R2000 same-version controlled DXF reopening matches all 2,890
non-metadata observation keys, including 180 children. Same-version candidate
DWG controls also match both source observations. Source DATATABLE owner
references cannot be resolved by native entget after opening the legacy TABLE
round-trip graph; codec tests check those owners independently. This is not a
claim that the native application exposes the complete original ownership graph.

## Native R14 blocker diagnosis and correction

Before the final correction, the complete R14 ASCII DXF failed to open in ProgeCAD with
`Invalid ResBuf`. The previous `c8f4b05` output reproduces the same rejection,
so this is a pre-existing additional defect rather than a regression caused by
the new DATATABLE marker. Neither changing the table marker nor restoring
XRECORD cloning rule 1 removes it. Removing XDATA-range codes from XRECORDs
in a diagnostic copy also does not remove it. Removing Boolean-range codes
also fails; removing all XRECORD objects allows the diagnostic drawing to open.
An individual-record scan logs 133 openings, nine wrong-path results and one
missing result out of 143 legacy records. All nine wrong-path records contain
an EXTNAMES payload attached to a dictionary; this is a lead, not yet a proven
root cause. Neither wrong-path nor missing results count as accepted openings.
A subsequent direct GUI opening confirms `Invalid ResBuf` for isolated
record `CF6`, independently of the batch helper's wrong-path result. Its
XRECORD belongs to extension dictionary `CF5`, which belongs to dictionary
`17` (MLINESTYLE). The payload is `(102, EXTNAMES)`, `(1, STANDARD)`,
`(2, Standard)`, after the `AcDbXrecord` subclass and cloning flag 1. The
isolated input SHA-256 is
`27f3e735d673a504c1dbf69644c475f968efcaa44440618ecc7726c7cb16874b`.
This establishes a reproducible rejecting record, not the offending field
or a safe transformation of that record.

The resumed native controls distinguish the failure from EXTNAMES content:

| Diagnostic change | ProgeCAD result |
|---|---|
| Add braces around EXTNAMES | Invalid ResBuf |
| Remove only the EXTNAMES marker | Invalid ResBuf |
| Set the cloning flag to 0 | Invalid ResBuf |
| Change only the header to AC1015 | Opens; diagnostic only |
| Remove only the leading cloning field 280 in isolated CF6 | Opens |
| Remove only the leading cloning field in all 323 XRECORDs of the full R14 candidate | Opens; DATATABLE and all 180 children match the original |

ProgeCAD exported a hash-matched copy of the original R14 DWG using the GUI's
**AutoCAD Release 14 ASCII** selection. The resulting `source.dxf` has actual
header AC1014 and 580,692 bytes. Its XRECORDs do not carry a leading cloning
field; a later 280 occurs as payload after a named round-trip section. This
export consumes/rebuilds legacy data (45 XRECORDs versus 323 in the candidate),
so it is a structural control, not proof that all original objects survive.

The previous codec writer emitted the R2000 cloning prefix for R14 as well.
Its reader also consumed the first 280 unconditionally, potentially losing a
real R14 payload value. The native observations support version-gating both
paths at AC1015, retaining KeepExisting as the implicit R14 rule. Removing the
prefix does not remove a user payload entry. Red-test commit `9faadfd` records the two demonstrated failures. The
modern control passes before the fix. The local reader/writer correction now
passes all three new tests and the existing R14 cloning-default test.

The full diagnostic DXF capture matches **2,724 object-property keys**, including
all 180 XRECORD children, with zero differences. This excludes four capture
metadata keys. Fresh captures of actual codec-generated R2000 DWG and DXF each
match **2,889 object-property keys**; R14 DWG matches **2,724**. The same native
owner-resolution limitation described above remains. Strict path checks reject
captures from a different active document. A batch opened the three outputs but
wrote no captures because its Lisp context stayed on the initiating document;
only the later individually executed, exact-path captures are accepted.

Hashes, per-control outcomes and the native-export version are recorded in
`target/datatable-r14-extnames-controls/manifest.json`; property comparisons are
in `target/datatable-r14-no-cloning-comparison.json` and
`target/datatable-final-native-comparison.json`. A new source-version DXF was then generated using the exact library reported
by Cargo for the passing regression tests. ProgeCAD independently opens this
actual output (`fixed.dxf`, SHA-256
`e1c19cd021e5114cd1699bb01be5dccc6eb937e3c6444f949f1604b96fa5284f`)
and matches all 2,724 object-property keys and 180 children with zero
differences. See `target/datatable-r14-fixed-native-comparison.json`. Its lines
match the diagnostic copy exactly; only line endings differ. The newly
generated R2000 DXF is byte-identical to the already validated output (SHA-256
`fd1dbffa6cc6bf9b817183e551a631620cbe8367a0060eefe6daabdf8730b696`).
No production loss allowance changed. Original corpus inputs were not edited.
Product adoption is recorded separately below.

A separate read-only harness compares all XRECORDs through the corrected
codec, not only table children: 323 in R14 and 264 in R2000. Payload entries,
identity, owner/reactor/extension-dictionary fields, cloning rules and
completeness match. References encoded by the payload retain their handles
and kinds. DXF reconstructs redundant reference caches that are empty in 18
R14 and nine R2000 source records; this representation difference is recorded
separately, not hidden by claiming complete object equality. No product guard
was changed. The harness and results are retained in the codec worktree as
`target/datatable_xrecord_verify.rs` and
`target/datatable-all-xrecords-comparison.json`.

The final paired run on `bab8ac0` repeats all 944 jobs on the same 34
hash-checked inputs. Compared with the preceding candidate, all statuses and
summary fields are unchanged. Two error strings differ only in the first
rejected Surface object handle, in both candidate and baseline; both still
refuse the same unsupported ACDBASSOCEDGEACTIONPARAM promotion to AC1032.
The exact differences are retained in `target/datatable-bench-v7-vs-v6.json`.

The concurrent DWG 2004 writer task is isolated on `fix/dwg2004-writer`
(codec `f163b41`, product worktree based on `3007dd8` at inspection). Its
codec changes touch `src/io/dwg/dwg_writer.rs` and its own test; these do
not overlap this task's changed paths. Both codec branches descend from
`c8f4b05`; neither includes the other. Product dependency updates must
preserve both accepted changes when their PRs are integrated. No changes
were made in that task's worktrees or in the shared main folder.

The maintainer authorized publication on 2026-11-06. The annotated fork tag
`arqstudiocad/on-ea66abe-datatable-dxf` was published and its remote peeled
commit verified as `bab8ac01a48765648615a79089d77f3d594b6c0e`. The tag object
is `608c431be8d56560e26687f4d2799fe5a8eaa374`. No fork main or upstream branch
was changed. This branch updates only the AcadRust Git source in Cargo.lock.
Other locked dependencies are unchanged. Published-pin product acceptance is
recorded below. Earlier benchmark counts are retained for provenance; the final
v7 run is recorded above.

## Verification recorded so far

- Red tests are retained in codec commits `eab8c57` (hard ownership),
  `868b46b` (R14 implicit cloning), and `18effbe` (native uppercase subclass).
  The interim `0ecf4ec` refusal hypothesis is superseded by the measured marker.
- The first type-6 candidate passed 2,442 default codec tests. This count
  predates the R14 default and marker fixes and is not final acceptance.
- The revised focused tests pass: three hard-ownership cases (including both
  public inputs), six existing DATATABLE cases, two legacy-subclass cases and
  one R14 default case. Repeated ASCII conversion is checked in source versions.
- Before the final XRECORD correction, the complete codec suite passed 2,445
  active tests. On `bab8ac0`, **2,448 active tests pass**, with 93 ignored by
  normal selection (198 result summaries, zero failures). The three focused
  hard-ownership tests, including both external DATATABLE inputs, also pass
  with `ARQ_DATATABLE_CORPUS` configured. Logs:
  codec `target/datatable-codec-full-v6.log` and
  `target/datatable-native-focused-v7.log`.
- The earlier candidate benchmark completes **944 jobs on the same 34 drawings**
  across source-version and forced-2018 profiles. No write-status regression:
  the candidate additionally writes the two forced-2018 DWGs previously blocked
  by opaque DATATABLE payloads. Both codecs still refuse the separate Surface
  associative-object downgrade. Inventory/fingerprint analysis is separate from
  these status results; native reopening remains a required gate.
- Product baseline at `7a334e2`: 34 clean reads, 61 verified saves, seven declared
  loss blocks, zero semantic save errors. No updated product matrix is claimed.
- No new dependency, source-format promotion, production loss allowance or
  product verification relaxation has been introduced.

Private reproduction logs, generated drawings, scripts and per-pair native
observations are retained under the task worktree's ignored `target/datatable*`
paths. `datatable-native-controls-manifest.json` records control hashes.
The early v2 DXF R14 capture is invalid: its helper continued after an open
failure and mislabeled the previous active drawing. It is excluded. Later
helpers use actual full-path checks and conditional guards before creating a
capture; a missing or wrong drawing produces no accepted result.

## Published-pin product acceptance

The extension was rebuilt with `uv sync` from the exact published Git pin on
main base `3007dd8`. Local acceptance passes **616 Rust and 864 Python tests**
(11 existing Qt drag-event deprecation warnings), **nine public core corpus
groups**, both native Xref/table corpus groups and **nine optional document
tests**. Clippy with all targets and warnings denied, cargo fmt, cargo deny,
Ruff check/format (242 files), mypy (152 files) and stubtest pass. The initial
bare optional-document run lacked `ARQ_NATIVE_XREF_DXF`; its native import
case refused the missing setup. The configured rerun executes all nine cases
successfully, including the separately hash-verified native DXF. The lockfile changes only AcadRust's Git source. The
integrated matrix repeats the same 34 inputs and checks every original SHA-256
before and after execution: **34 clean reads, 63 verified saves, five declared
loss blocks and zero semantic save errors**, versus 61 / seven / zero in the
recorded baseline. Only `example_r14.dwg` and `example_2000.dwg` DXF saves
change from blocked to verified. All other per-drawing fields match after
normalizing only random temporary-directory names in refusal diagnostics.
The matrix exits 1 intentionally while blocked losses remain.

The four strict product outputs match the independently observed codec
artifacts byte for byte; no explicit loss acceptance is used:

| Input | Output | SHA-256 |
|---|---|---|
| R14 | DWG | `946e7c20aa729d37d415858fcde593adc7b65dfca426390323a8d00fe689b9b3` |
| R14 | DXF | `e1c19cd021e5114cd1699bb01be5dccc6eb937e3c6444f949f1604b96fa5284f` |
| R2000 | DWG | `361dc22afc946aa534f408a20def13fad620d8f3e83ccbf497e5e4dfca1f7e59` |
| R2000 | DXF | `fd1dbffa6cc6bf9b817183e551a631620cbe8367a0060eefe6daabdf8730b696` |

Evidence: `target/datatable-product-pin-matrix.json`,
`target/datatable-product-pin-matrix-comparison.json` (retains raw differences
and the narrowly normalized comparison), and
`target/datatable-product-native-artifacts-comparison.json`.
The remaining five blocks are four dynamic-block purge-preventer DXF omissions
and the independent Surface associative-object DXF omission. No loss allowance,
production verifier, public API or runtime dependency was added or relaxed.
Broader version/operation coverage, external BIM validation and Xref resolution
remain separate work. Windows checks do not establish Linux/macOS or release
CPU-floor acceptance. Milestones 03.4 and 03.5 remain open.

## Reference review

Per ADR 0017 D2, reviewed Open CAD Studio `76a7bb9`, its locked
opencaddocument `19363a7dc6aab867bdbc4ad562b300b522f63198`, actual codec
`HakanSeven12/acadrust` at `0907cc7847e5fa805d53ded992d4b886b7e5d600`,
and AcadRust upstream `17a3125153335eac19efa6409ace130a75c0bf54`.
The corresponding class-object model, DWG reader/writer and DXF reader/writer
still have the same unsupported type-6 gate. No ready-made fix was found or
copied. Source snapshots and MPL-2.0 license texts are retained in
`target/reference-review`; agreement between related codecs is not independent
CAD validation.
