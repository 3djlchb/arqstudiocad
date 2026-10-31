# ArqStudioCAD — DWG/DXF interoperability

Module M02, phase 03. The decision is
[ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md): AcadRust
(crates.io, `=0.6.8`, patched from our fork) as the base, behind `arq-document`; opencadcodec's
improvements brought in one by one; LibreDWG out of the product. The
evidence is the [library comparison](references/dwg-library-comparison.md)
(220 drawings, 6,600 operations); the tool is `tools/dwg-bench`.

The current fork merges upstream `ea66abee` with our preservation patches at
`a1a9a56`; [upgrade validation](testing/acadrust-068-upgrade.md) records the
paired corpus and native ProgeCAD checks, including the unchanged table-format
limitations.

## Protections (ADR 0017 D5)

| Protection | State |
|---|---|
| Damaged and partial files reported, not opened as valid empty drawings | ✅ see [Damaged files](#damaged-files) |
| Safe save: temporary file, read back, validate, then replace | ✅ see [Safe save](#safe-save) |
| No silent loss on version conversion; warn and allow cancelling | ✅ see [No silent loss](#no-silent-loss) |
| Unknown data preserved where possible | ✅ same-version DWG saves keep custom objects byte for byte (measured: 279 AutoCAD Architecture objects, 26 TCH walls, 300 AEC objects) |
| Written files validated by reading back and in independent applications | ◐ round-trip corpus; independent applications pending (03.5) |

## Capability matrix

What ArqStudioCAD can do with each kind of DWG/DXF content, on AcadRust
0.6.4, measured on the comparison corpus (re-measured on the 0.6.5 upgrade, ADR 0017's log). Levels: **read** (the engine reads
it), **keep** (it survives our save and reopening), **show** (drawn in the
viewport), **edit** (the platform changes it), **recompute** (regenerated
when its parameters change).

| Content | Read | Keep (DWG / DXF) | Show | Edit | Recompute | Notes |
|---|---|---|---|---|---|---|
| Lines, circles, arcs | ✅ | 100 % / 100 % | ✅ | ✅ geometry, layer, color | — | |
| Lightweight polylines | ✅ | 100 % / 100 % | ✅ | ✅ drawn (PLINE: segments and tangent arcs), as a whole and by vertex | — | |
| Blocks (INSERT) | ✅ | 100 % / 100 % | ✅ expanded | ◐ created (BLOCK), placed (INSERT), transformed as a whole with their attributes, definitions edited in the block editor (BEDIT; not dynamic ones); EXPLODE keeps attribute values as texts (ADR 0023) | — | MINSERT arrays not transformed |
| Dimensions | ✅ | 100 % / 100 % | ✅ as AutoCAD drew them (their block) | ○ | ○ | phase 04 |
| Hatches | ✅ | 100 % / 100 % | ◐ boundaries, no fill pattern | ○ | ○ | phase 04 |
| Texts, MTEXT, attributes | ✅ | 100 % / 100 % | ◐ letters in a built-in stroke font; MTEXT as plain lines, wrapped to its width | ◐ TEXT and MTEXT created and transformed as a whole; content, height, rotation (ADR 0022) | — | the drawing's fonts (SHX, TrueType) and formatted MTEXT in phase 04; attributes with their block |
| Leaders, multileaders | ✅ | 100 % / 100 % | ◐ broken into lines | ○ | ○ | |
| Tables | ✅ | 100 % / 100 % | ◐ broken into lines | ○ | ○ | |
| Images, underlays, wipeouts | ✅ | 100 % / 100 % | ◐ frame or box | ○ | — | |
| 3D solids, regions, surfaces (ACIS) | ✅ geometry kept (761 kB measured); R2004 lofted surfaces fixed in our fork | 100 % / 100 % (DXF keeps the body as SAT, without the DWG wireframe cache: boxed from its vertices) | ◐ wireframe in plan: edges and ISOLINES (ADR 0024 step 2); bounding box if not translated | ○ | ○ | 3D model in 02.8 |
| Meshes, 3D faces | ✅ | 100 % / 100 % | ◐ edges in plan | ○ | — | |
| Ellipses, splines, 2D solids | ✅ | ✅ (not measured separately) | ◐ broken into lines and arcs | ○ | — | |
| Dynamic blocks | ✅ definitions, parameters, actions, grips | 100 % / 100 % | ✅ current state (the evaluated block) | ○ | ○ visibility, stretch… | ADR 0016 D7, our parametric engine (ADR 0017 D6), milestone 04.1 |
| Geometric and dimensional constraints | ✅ every version, R14–2010's old layout included (since AcadRust 0.6.7) | ✅ preserved, DWG and DXF, no loss (LibreDWG's `Constraints.dwg`, R14–2018) | — | ○ | ○ | 03.2; the old layout decoded by the original's #135, from opencadcodec |
| Associative network (ASSOC*) | ✅ | 100 % / 100 % | — | ○ | ○ | |
| XDATA, XRECORD | ✅ | 100 % (measured on 14,169 entities) | — | ○ | — | where application data goes in DWG |
| Custom / proxy / AEC objects (BIM) | ✅ preserved | 100 % in the same DWG version; refused on conversion to 2018 | ○ proxy graphics | — | — | lost as DXF: asked first ([No silent loss](#no-silent-loss)) |
| External references (Xrefs) | ◐ synthetic DXF path, flags and INSERT | ◐ synthetic metadata round-trips through DWG and DXF | ? | ○ | — | [Controlled corpus](#xref-metadata-corpus); resolution and independent drawings pending (03.5) |
| BIM drawings (AutoCAD Architecture, Civil 3D, Revit/ArchiCAD exports) | ◐ | ◐ | ◐ | ○ | — | few samples; the maintainer will produce more |
| DWG R11/R12 | ✅ (R11 16-bit linetype indices fixed in our fork; ACEB10 reads in full) | — | ✅ | ✅ | — | saved as DWG 2018 |
| Damaged files | ✅ refused, or recovered and marked | — | — | — | — | [Damaged files](#damaged-files) |

## Damaged files

Milestone 03.3, step 1 (ADR 0017 D5.1). A file is read strictly first; a
damaged file is then read in AcadRust's recovery (failsafe) mode:

- **Nothing usable:** opening fails with `DamagedFileError`, listing what
  the reader found wrong.
- **Something usable:** the drawing opens **marked as recovered**: the
  command line and a dialog say it is damaged and part of it may be
  missing, with the reader's problems. Saving over the damaged file is
  refused until the user chooses *Replace* (or *Save as*, the default);
  saving elsewhere does not lift that guard.

Measured on DWG R13–2018 and DXF, cut, zeroed or with bytes changed (the
regression drawings are made by the tests, `reading_tests.rs`):

| Damage | Before 03.3 | Now |
|---|---|---|
| R13–2000 DWG cut short or zeroed | opened as an **empty drawing**, no error (the engine's statistics showed it) | refused as damaged |
| 2007 DWG with bytes changed | the engine **panicked** (the application crashed) | the panic is caught: refused or recovered |
| DWG/DXF the strict read refuses | an error | recovered and marked when something is usable |

Known limits: a few changed bytes the engine reads without noticing (in
2013 and 2018 DWG measured so far without losing entities) are not
detected: checking the file's CRCs would be the next guard. A DXF value
that is not a number (a coordinate written as text) is read without an
error; that is a reader fidelity issue (03.2).

## Safe save

Milestone 03.3, step 2 (ADR 0017 D5.2). Every save writes the whole file
next to the target under a temporary name, **reads it back** and compares
it with the drawing (`Document::validate_roundtrip`, the same rule the
round-trip corpus applies): every entity by id (kind, layer, color,
geometry within tolerance), every layer and the current layer. The read
must be clean, not a recovery. Only then is the file renamed over the
target.

Allowed as no loss: geometry within the contract's tolerance; a linetype
name in other capitals (an R12 `CONTINUOUS` is `Continuous` in DWG); RGB as
an ACI index before 2004 and every layer plotted before 2000, which those
versions cannot hold.

If the check fails, the target is untouched and the save fails with
`SaveNotVerifiedError`, naming the differences; what was written is kept
beside it as `<name>.unverified.<ext>`, so the work never stays only in
memory. Cost, measured on 200,000 lines: save and check 0.85 s as DWG,
1.6 s as DXF (on the worker thread; drawing goes on).

## No silent loss

Milestone 03.3, step 3 (ADR 0017 D5.3). The safe save's read-back also
compares an **inventory** of the drawing and of the file: every entity and
object type (unknown, proxy and AEC objects included), the XDATA on
entities and the table records. If the file holds less of anything, the
save stops **before replacing anything** with `LossyConversionError`,
listing what would be lost; the user chooses *Save anyway* or *Cancel*
(Cancel by default). An accepted loss is not asked again for that
document while it stays open; a new one is.

Not counted as losses: more of something (a writer adding its own
dictionaries); a 2D POLYLINE written as an LWPOLYLINE (as AutoCAD does with
PLINETYPE 2); and what an empty drawing loses the same way: the engine's
own default objects (a multileader style and a table style, with their
dictionaries) that DWG 2000 cannot hold.

Measured (2026-10-13) on the engine's samples and the maintainer's
drawings; what saving loses today, now always asked:

| Drawing | Saved as | Lost |
|---|---|---|
| AutoCAD Architecture house (2013) | DXF | AEC display objects (`AECS_DISP_*`), dynamic-block markers |
| Dynamic-block samples (2018) | DXF | `ACDB_DYNAMICBLOCKPURGEPREVENTER_VERSION` |
| `point_object_id.dxf` | DWG | `DATAOBJECT` |
| Every other drawing, both formats; every DWG in its own version | — | nothing |

## Version support

| | Read | Write |
|---|---|---|
| DWG | R11/R12, R13–2018 (AC1032) | R13–2018; R12 drawings are saved as 2018 |
| DXF | R12–2018, ASCII and binary | R12–2018 (R12 written as R13) |

## How to re-measure

### Integrated product matrix (03.4)

`tools/compatibility_matrix.py` measures the current public Python/core API,
including our pinned engine patch and the product's safe-save protections.
It complements the historical library comparison below; the comparison
spike does not establish the current application's compatibility.

```sh
uv run python tools/compatibility_matrix.py target/corpus/acadrust target/corpus/libredwg --output target/compatibility-matrix.md
uv run python tools/compatibility_matrix.py target/corpus/acadrust target/corpus/libredwg --format json --output target/compatibility-matrix.json
```

Pass additional files or directories to include local drawings. Inputs are
searched recursively and deduplicated; a missing input or empty corpus is
an error. Original drawings are never saved over. Each drawing is inspected
in a separate process (120 seconds by default, configurable with `--timeout`),
and DWG/DXF copies are written only in a temporary directory. A crash or
timeout is recorded rather than dropping the row.

The Markdown matrix groups whole-drawing results by **source format,
source version and model-space entity kind**. A drawing with several kinds
appears in several rows: the rows must not be added together. A successful
save verifies the whole drawing through the core, not each kind independently.
Per-file rows include the actual written version, including legacy-version
upgrades. JSON also records source SHA-256, entity counts, dynamic-definition
counts, undecoded-object names, recovery diagnostics and refused loss details.
Both formats identify the environment, tool hash and checked-out Cargo.lock
hash; these are provenance aids, not a compiled-binary fingerprint.

Statuses:

- Read `ok`: opened without recovery; `recovered`: usable recovery with
  diagnostics, never counted as a clean read or exported by this tool.
- Save `verified`: the core accepted the save and its read-back validation,
  and the output reopened without recovery.
- Save `blocked_loss`: the core refused possible losses; the tool **never
  accepts losses**. A refusal is evidence to investigate, not proof that
  each reported loss is semantically significant.
- `error`, `timeout` or `crash`: failed measurement; `not_run`: no save was
  attempted. A timeout/crash may occur during reading or saving; it does
  not identify the failing stage.

Exit 0 requires a nonempty corpus, every read clean and both formats verified;
exit 1 means at least one refusal, recovery or failed measurement; exit 2
means invalid input/options or a report I/O error. Reports are still written
on exit 1. Core version strings alone do not identify local source changes;
retain the checkout/revision and test results with reports when comparing runs.

This measures **read and safe-save behavior** only. It does not certify Xref
resolution, visual fidelity, BIM semantics, editing, recomputation or opening
outputs in independent applications. The existing capability matrix's claims
must not be promoted on these counts alone. Local reports may contain private
file paths and technical details: keep them under ignored `target/`, never
commit drawings or raw reports containing the maintainer's project data.

#### First product run (2026-10-22)

Measured on Windows with the current 0.1.0 development build: the 11 fetched
AcadRust samples, 14 fetched LibreDWG regressions and five maintainer-provided
exercise drawings. No input was modified. The machine-readable report is
generated locally, not committed. The checkout was based on `56e0fe5` and
included the pre-existing, uncommitted block-metadata API work; this is
working-tree evidence, not a release certification.

| Corpus | Clean reads | DWG verified | DWG blocked loss | DXF verified | DXF blocked loss |
|---|---|---|---|---|---|
| AcadRust + LibreDWG samples | 25/25 | 22/25 | 3/25 | 12/25 | 13/25 |
| Maintainer's exercises | 5/5 | 5/5 | 0/5 | 4/5 | 1/5 |
| Total | 30/30 | 27/30 | 3/30 | 16/30 | 14/30 |

No recovery, timeout, crash or other error occurred. Eight source versions
were represented: AC1009, AC1014, AC1015, AC1018, AC1021, AC1024, AC1027 and
AC1032; this does not cover every supported version or every conversion pair.

Three DWG saves were refused: the known DATAOBJECT in
`acadrust/datatable/point_object_id.dxf`, and MULTILEADERSTYLE counts in
LibreDWG's R14 and 2000 `Constraints.dwg`. The latter are explained by the
[focused investigation](#legacy-multileaderstyle-refusals); the guard remains enabled.
DXF refusals include dynamic-block markers, old constraint-group objects,
DATATABLE, procedural/associative surface objects and AEC content in one
exercise drawing. They are explicit refusals, not silently successful exports.

Xref coverage in this initial 30-file run remains unverified: this API does not inventory or resolve
external references, so the filenames and successful reads do not establish
that any of these inputs exercise Xrefs. AEC preservation evidence does not
close semantic BIM or external-application validation (03.5).

#### Legacy MULTILEADERSTYLE refusals

Follow-up investigation (2026-10-22), pinned AcadRust fork `5f473cc`:

- Both `r14/Constraints.dwg` and `2000/Constraints.dwg` decode two
  `MultiLeaderStyle` objects, handles 216 and 229 (decimal), owned by 215.
  Both have description `Standard`, but `property_changed` is respectively
  false and true. Their decoded names are empty. This does not establish
  that either is disposable or that either is used by visible geometry.
- The fork's `src/io/dwg/dwg_writer.rs::prepare_legacy_document` removes
  the `ACAD_MLEADERSTYLE` dictionary and its children for these old versions.
  Reading the raw output confirms `MULTILEADERSTYLE: 2 -> 0` in each case.
- `crates/arq-document/src/inventory.rs::default_losses` measures only
  one such loss in an empty drawing of the same version. `save_losses`
  rejects the two-object loss because it exceeds that allowance. The
  diagnostic reports full before/after counts, not the excess alone.
- `crates/arq-document/src/compatibility_tests.rs::legacy_multileaderstyle_refusals_exceed_the_default_allowance`
  reproduces the counts and checks that a normal save refuses and leaves an
  existing target untouched. Raw serialization is used only in a temporary
  diagnostic file to inspect the rejected output; production saves retain
  all guards. Run after fetching the existing corpus:

  ```sh
  cargo test -p arq-document legacy_multileaderstyle_refusals -- --ignored --nocapture
  ```

The existing `crates/arq-core/tests/corpus.rs::codec_regression_constraints_kept_undecoded`
explicitly accepts `object MULTILEADERSTYLE` to inspect constraint preservation.
Its success therefore does not contradict the strict matrix's refusal.
No broader default allowance or automatic loss acceptance is justified by
this evidence. Semantic use of these styles and external application behavior
remain unverified; the refusal is retained, not relabeled as a verified save.

#### Xref metadata corpus

`tests/fixtures/xrefs/` now supplies five original synthetic DXFs: four hosts
(relative attachment, overlay, missing target, nested attachment) and one
line-only child. See its README for the file graph and limits.

`crates/arq-document/src/compatibility_tests.rs::xref_fixture_metadata_roundtrips`
checks the source metadata and eight safe-save/reopen paths (four hosts times
DWG/DXF): reference path, attachment/overlay flags, INSERT name, position,
scale and rotation. The host keeps only its INSERT; the child geometry is
not merged. These tests run with ordinary `cargo test` and require no download.
The fixtures are AC1009 inputs; this is not coverage of all source versions.
These dedicated test assertions do not extend the production save validator:
`verify.rs::validate_roundtrip` compares entities and layers, while the
inventory counts block definitions rather than comparing Xref paths or flags.

```sh
cargo test -p arq-document xref_fixture_metadata_roundtrips
uv run python tools/compatibility_matrix.py tests/fixtures/xrefs --output target/xref-compatibility-matrix.md
```

Measured on 2026-10-22 with the rebuilt working tree based on `f55b382`
(including pre-existing block-preview changes): five clean reads, five DWG
saves verified as AC1032 and five DXF saves verified as AC1012, without
recovery or accepted losses. Source SHA-256 values stayed unchanged. Local
reports are `target/xref-compatibility-matrix.md` and `.json`. These five
synthetic inputs are reported separately from the initial 30-file corpus.

This closes the lack of a controlled metadata corpus, not Xref support as a
whole. The product still lacks a dedicated public reference inventory/resolver.
Loading external geometry, nested/overlay propagation, missing-file reporting,
cycles, cached geometry, authored DWG hosts and independent application checks
remain open. The generic matrix remains a read/safe-save report, unchanged.

The [ProgeCAD exercise](testing/progecad-xrefs.md) now has six independently
authored DWG 2013 sources. Both the initial 0.6.5 run and the 0.6.7 rerun after
PRs #61/#62 read all six cleanly and pass twelve core safe saves, without
accepted losses. Ten retained host outputs per run were reopened in ProgeCAD.
**Unloaded state is lost on DXF export in both runs**, despite a core-verified
save; DWG retains it. Paths, attachment/overlay behavior and INSERT parameters
match the stated checks in the other observed cases. Full numerical/reload
acceptance remains open. The separate local Rust corpus fails the five hosts
because their external INSERTs draw nothing; its rendering-policy decision is
still pending. See [hashes, observations, checks and limits](testing/progecad-xrefs-results.md).
The future inventory/resolver/display/reload design is
[ADR 0026](architecture/adr/0026-external-reference-loading.md)
(proposed, implementation in phase 04 or later).

### Native table corpus (2026-10-26)

[Exercise](testing/progecad-tables.md), [measured results](testing/progecad-tables-results.md)
and [hash/status manifest](testing/progecad-tables-results.json), at application
`3d4459c` with codec fork `59aaf4c`. Three independently authored inputs each
contain four 5-row / 3-column tables: native DWG 2007/2018 and DXF 2010.
Two read cleanly; the 2007 border-table record is reported as recovered, with
three tables instead of four. ProgeCAD independently reopens all four and
matches the native 2018 control's 249 measured properties.

Three strict product saves pass core validation. DWG 2018 -> DXF is refused
for MAPRESOURCEMANAGEROBJECT loss. Independent ProgeCAD reopening distinguishes
the three verified files: the DWG 2018 round trip matches all 249 measurements;
DXF -> DWG changes four margin values, twelve table-level text-height values
and cell records including right-border overrides; DXF -> DXF matches scalar
measurements/texts but differs in five border records, still under representation
review. The tools report these differences without accepting losses or
normalizing unproved equivalences. A verified core save is not full external
fidelity. These are the historical `59aaf4c` baseline measurements.

The [table-instance follow-up](testing/progecad-table-instance-formatting.md)
fixes the proven DXF -> DWG loss in fork `0d438b6`: owned TABLECONTENT supplies
missing instance formatting, and serialized right/inside grid indexes are
translated correctly. Independent ProgeCAD reopening now matches all 249
native DWG measurements, including the custom margins, text heights and red
0.50 mm right borders. The [DWG 2007 reader follow-up](testing/progecad-table-2007-reader.md)
in fork `179aaeb` also reads and rewrites all four native AC1021 tables strictly,
with 249/249 native measurements matching. The resource-manager refusal and
same-format DXF border investigation remain open. No full table-format
exemption was added to safe saving. This extends 03.4/03.5 evidence and does
not close either milestone.

The resource-manager refusal above was subsequently corrected by the
[Map proxy follow-up](testing/progecad-map-resource-preservation.md), retained
through the [0.6.8 upgrade](testing/acadrust-068-upgrade.md). The
[DWG-to-DXF formatting follow-up](testing/progecad-table-dxf-format.md), fork
`f1b0980`, corrects eight margin measurements, twelve row-type text heights and
eight merged-title values. Independent ProgeCAD reopening now matches all 189
non-cell-record properties, with sixty cell-record differences still reported.
The unchanged integrated 34-drawing matrix has 58 verified saves, eight blocked
losses and two known R2000 TABLESTYLE-owner errors. This does not close 03.4/03.5
or establish full cell-level fidelity.

### Historical library comparison

`tools/dwg-bench` (a workspace of its own) reads, writes and re-reads a
corpus with AcadRust, opencadcodec and, when built with it, LibreDWG; every
engine upgrade runs it and the round-trip corpus, and the results go into
ADR 0017's log.

### Per-cell table follow-up (2026-10-31)

The [derived cell-cache investigation](testing/progecad-table-cell-fidelity.md)
corrects demonstrated inherited-height and shared-border losses in fork
`5c09a39` after PR #101. All sixty cell heights and measured border
properties now match. Fifty-two raw CELL_VALUE flag differences also occur in
a fresh reopening of ProgeCAD's original DXF export, without a codec rewrite.
The correction's same-format DXF rewrite matches that native DXF control exactly;
DXF-to-DWG and native DWG 2007 rewriting retain 249/249 original DWG measurements.
The raw comparator is unchanged; explicit visible-edge presence differs in five
modern-export records, with measured values retained. The final paired corpus
has 940 jobs, with zero changed success states or summaries. Fork `5c09a39` is published;
Git-pinned acceptance passes 488 Rust and 795 Python tests, six external core
corpus groups and seven optional document tests. The repeated integrated matrix
retains 34 clean reads, 58 verified saves, eight blocked losses and the same two
R2000 TABLESTYLE-owner refusals, with zero changed rows. Product adoption awaits
PR review and merge; 03.4/03.5 remain open.
