# ProgeCAD Map resource proxy preservation

Date: 2026-10-28. Status: published fork and exact product Git pin validated; product PR awaiting review.
This follows the [native table reader correction](progecad-table-2007-reader.md)
and addresses the remaining AC1032 DWG-to-DXF resource-manager loss in milestone
03.4. It does not establish complete table-format fidelity.

## Reproduction and native evidence

The private source `tables-native-ac1032.dwg` has SHA-256
`6f4050e24aaa4413e46e290cc4a3fdcd4f39f1c8783c566c8002a00fe5705678`.
Its original is untouched. A byte-identical review copy was opened in ProgeCAD
2026, its object and dictionary inspected with `entget`/`dictsearch`, and a
separate native DXF export retained locally.

The source registers `MAPRESOURCEMANAGEROBJECT`, C++ class
`AcMapResourceManager`, application `AcMapSharedComponents`, as a zombie
non-entity class. ProgeCAD reports the object as `ACAD_PROXY_OBJECT`:

| Property | Native source / candidate reopened in ProgeCAD |
|---|---|
| Object handle | `2A4` |
| Owner / reactor | `2A3` / `2A3` |
| Owner dictionary entry | `AcMapResourceManager` points to `2A4` |
| Original class identity | `AcMapResourceManager` |
| DWG / maintenance version | 33 / 55 |
| From-DXF flag | false |
| Application payload | 12 bits, hex `4060` |

At production fork revision `179aaeb`, `DwgDocumentBuilder` kept the whole
14-byte merged record as `ObjectType::Unknown`. The DXF writer excluded the
object from valid handles and omitted both its body and its dictionary entry.
The product correctly refused this loss. The initial native regression failed
with one manager before export and zero afterward.

## Candidate correction

The isolated AcadRust branch `fix/map-resource-preservation`, commit
`c05e7be3480959677ed79f5b8eed8263fe375454` on top of `179aaeb`, contains
the following related changes:

- `DwgDocumentBuilder`, `ClassNames::from_document` and
  `process_pass2_record`: recognize this specific zombie Map registration in
  R2007+ sources and retain its opaque application data, text stream, typed
  references, common ownership, reactors and extension dictionary as an
  existing `ProxyObject`. No application-specific payload schema is invented.
- `DwgObjectWriter::write_object`: reuse the original record for an untouched
  same-version DWG save, with the existing original-object snapshot check.
- DXF `SectionReader::read_classes`: preserve source class positions and
  duplicate names instead of merging registrations with default classes.
  DWG `prepare_legacy_classes` retains the class table when a proxy addresses
  a registered class, avoiding reassignment of its opaque data to another class.

The product inventory and safe-save protections have not been relaxed.
Live/non-zombie Map registrations retain their previous opaque treatment;
this is preservation, not Map application evaluation or a general class decoder.

## Completed focused validation

Four focused integration tests pass, including the private native source.
Synthetic R2007, R2010, R2013 and R2018 drawings exercise a 28-bit data stream,
a nonempty text stream, a typed object reference, reactors and an extension
dictionary through DWG-to-DXF and back. Other tests retain a live Map record
as unknown and verify positional DXF registrations with duplicate names.
A separate native unit regression passed and confirms byte-identical
same-version DWG record reuse. The earlier six instance-format and two native
2007-table regressions also passed with this correction present.

ProgeCAD independently reopened `map-resource-candidate.dxf`; the proxy and
its dictionary link match the native observations above. No original input
was overwritten. Native evidence stays under the private corpus or ignored
`target/`; user-specific paths in source metadata are not reproduced here.

## Table-format limit exposed by this route

The strict comparator checked all 249 table properties after native reopening
of the candidate. It found **88 differences** from the native DWG control:
60 cell-record measurements, eight margins, twelve row-type text heights and
eight texts in merged title cells. These differences remain unresolved.

A control DXF was produced with the previously validated comparison executable
using fork `179aaeb`. A complete text diff against the candidate changes only
(1) the recovered `AcMapResourceManager` dictionary entry and (2) the new proxy
record. All table records are identical. Therefore the measured table-format
limits predate this proxy correction; object preservation must not be reported
as a 249/249 table-fidelity result.

## Local integrated product check

The product branch was fast-forwarded to main `76d2cc0` and rebuilt against the
local candidate through a temporary dependency path. The public compatibility
matrix checked 34 drawings: 25 fetched samples, six Xrefs and three native
ProgeCAD table sources. All 34 read cleanly. Across 68 saves, 58 verify, eight
are refused for inventory loss and two are refused by table-style verification.
The three table sources now pass **all six strict saves**, including the
previously blocked AC1032 DWG-to-DXF conversion. All six Xref sources pass their
12 strict saves. These results use the unchanged normal safe-save API.

The remaining public-corpus refusals cover the dynamic-block purge preventer,
associative edge parameters, legacy DATATABLE/MULTILEADERSTYLE and the two
R2000 table-style-owner cases. The same-input baseline run confirms exactly one status change: AC1032
DWG-to-DXF moves from an inventory-loss refusal to verified. Every other
drawing/format keeps its baseline status, and all 34 source hashes match. A successful count/read-back check does
not override the native table-format limit described above.

## Completed local checks

- Fork: **2,295 default tests passed**, with 86 optional/documentation tests
  ignored. Runtime tests passed in the full invocation; 13 documentation
  examples initially could not locate a library being replaced by the other
  build, then all passed in a separate sequential `cargo test --doc` run.
- Native fork checks: byte-identical DWG record reuse, four Map/class tests,
  six previous table-format tests and two previous 2007-reader tests passed.
- Product at main `76d2cc0`: **480 default Rust tests passed**, 11 optional
  tests ignored; four fetched-corpus groups and the local six-Xref group
  passed; all six optional document checks passed with the native DXF path set.
- Python: **636 passed**, nine existing warnings, using the normal `offscreen`
  backend. An initial startup wait was investigated separately; the diagnostic
  `minimal` backend had one resize assertion failure and is not the acceptance
  result. No UI code or test assertion was changed.
- Workspace Clippy with warnings denied, Rust formatting, Ruff (203 files),
  mypy (127 source files), stubtest (one module) and cargo-deny passed. The
  temporary path override leaves the allowed Git source unused in cargo-deny;
  advisories, bans, licenses and source checks all pass.
- Comparison bench: **476 jobs / 238 paired operations** keeping source
  versions, plus **464 jobs / 232 pairs** forcing AC1032, across the same
  34 drawings. Neither protocol changes a success/failure state. Public and
  Xref summaries match exactly. Each protocol has five changed table-fixture
  summaries, all limited to recognizing the original opaque Map object as a
  proxy or retaining it in DXF instead of losing it. Geometry/count summaries
  otherwise match. The forced profile retains the same three write refusals
  per library. These are status/inventory comparisons, not performance or
  comprehensive semantic-fidelity claims.

Raw logs, paired comparison JSON, artifact hashes and native captures are
retained under ignored `target/` in the task working copy. The local validation used a temporary dependency path. After maintainer authorization,
the annotated immutable tag
[arqstudiocad/on-c5555a5-map-resource-proxy](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5-map-resource-proxy)
was published and its peeled remote target verified as
`c05e7be3480959677ed79f5b8eed8263fe375454`. Cargo.toml now pins that exact Git
revision. Cargo.lock records the same source commit and no transitive dependency
version changes. The downloaded Git source matches all five locally tested
changed files after line-ending normalization.

The final Git-pinned extension was rebuilt and installed with `uv sync`.
Its repeated 34-drawing matrix matches the local candidate in source hashes,
read state, entity/model counts, recovery, undecoded-object counts and every
save status: 58 verified, eight inventory refusals and two verification refusals.
All six table saves and twelve Xref saves verify. The pinned extension also
passed all 636 Python tests (nine existing warnings) and stubtest. Formatting
and cargo-deny pass with the final Git source. Full Rust/static checks and the
comparison bench above tested this identical fork revision before publication.

## Remaining work

Merge the product PR after CI and review. Phase 03.4 remains in progress:
the separate table-format differences, the two R2000 TABLESTYLE-owner refusals
and broader version/visual coverage remain open.
