# DATATABLE DWG-to-DXF preservation investigation

Status: **in progress; candidate not adopted or published**. Updated 2026-11-05.
Milestones 03.4 and 03.5 remain open. The product still uses fork `c8f4b05`.
The isolated product worktree was fast-forwarded from `7a334e2` to `2a74106`;
the shared main worktree is not modified by this task.

## Scope and actual cause

The public LibreDWG `example_2000.dwg` and `example_r14.dwg` contain one
DATATABLE each, at handles `B2C` and `D62`, respectively. Both tables have
flags 2, four columns, 180 rows, column types `[2, 1, 1, 6]`, and 180 hard-owned
XRECORD targets in the final column. Type 6 is HardOwnerId. Its native DWG
reference kind is HardOwnership and its DXF group code is 360.

The published codec's `DataTableCellType::has_native_codec`
(`src/objects/class_object.rs`) accepts only types 1 through 5. Consequently,
the DWG class-object reader retains each entire table as opaque bytes.
The DXF unknown-object writer has no DXF body to emit, omits the table, and
the product inventory guard correctly blocks the missing object.

## Candidate changes and native findings

Executable changes are saved locally in isolated AcadRust commit `be58a0b`,
on branch `fix/datatable-dxf-preservation`, after the retained red reproductions.
They are not published or a product dependency update yet.

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

## Remaining native blocker

The complete R14 ASCII DXF still fails to open in ProgeCAD with
`Invalid ResBuf`. The published `c8f4b05` output reproduces the same rejection,
so this is a pre-existing additional defect rather than a regression caused by
the new DATATABLE marker. Neither changing the table marker nor restoring
XRECORD cloning rule 1 removes it. Removing XDATA-range codes from XRECORDs
in a diagnostic copy also does not remove it. Removing Boolean-range codes
also fails; removing all XRECORD objects allows the diagnostic drawing to open.
An individual-record scan logs 133 openings, nine wrong-path results and one
missing result out of 143 legacy records. All nine wrong-path records contain
an EXTNAMES payload attached to a dictionary; this is a lead, not yet a proven
root cause. Neither wrong-path nor missing results count as accepted openings.
Such intentionally incomplete diagnostic outputs are never accepted as
preservation evidence.

Do not declare the two product DXF rows fixed or publish/adopt the candidate
until this rejection is explained and the complete generated outputs are
independently reopened and compared. Internal read-back alone is insufficient.

## Verification recorded so far

- Red tests are retained in codec commits `eab8c57` (hard ownership),
  `868b46b` (R14 implicit cloning), and `18effbe` (native uppercase subclass).
  The interim `0ecf4ec` refusal hypothesis is superseded by the measured marker.
- The first type-6 candidate passed 2,442 default codec tests. This count
  predates the R14 default and marker fixes and is not final acceptance.
- The revised focused tests pass: three hard-ownership cases (including both
  public inputs), six existing DATATABLE cases, two legacy-subclass cases and
  one R14 default case. Repeated ASCII conversion is checked in source versions.
- The revised complete codec suite passes **2,445 active tests**; 93 tests
  remain ignored by the suite's normal selection. The two optional native
  DATATABLE cases are run separately with the corpus environment configured.
- The rebuilt paired benchmark completes **944 jobs on the same 34 drawings**
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
