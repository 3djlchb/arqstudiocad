# Remaining compatibility-loss triage (03.4)

Date: 2026-11-01. Analysis base: main `fbf5b58` (PRs #108, #109 and #110 merged).
Codec: AcadRust 0.6.8, fork `bd369d9060751f3da6824a0d93529d05335563f1`.
Status: all nine remaining blocked saves reproduced and classified; no codec
fix or loss-policy change in this investigation. The separate ProgeCAD
mass-property references merged in PR #110 are unchanged.

## Scope and measured result

A private Rust diagnostic, compiled against this checkout's `arq-document`
and the published codec, repeats all 68 normal safe saves on the previous
34-drawing matrix's exact source paths. Its dependency lock was initialized
from the application lock: the only additional name/version is the diagnostic
package itself. All 34 inputs open without recovery, and every source SHA-256
still matches the previous report. All 68 format/status/loss tuples match.

| Operation | Verified | Blocked loss | Other failures |
|---|---:|---:|---:|
| DWG save | 32 | 2 | 0 |
| DXF save | 27 | 7 | 0 |
| Total | 59 | 9 | 0 |

Every save starts with a pre-existing sentinel destination. All nine rejected
saves leave it byte-for-byte unchanged. Rejected raw serializations are kept
separately only for diagnostic inspection: none is a successful product save,
and no loss-acceptance callback is used. Source reads and successful safe
saves exercise the current document layer; this is not a rebuilt Python matrix
run, external-CAD validation or a full regression-suite run.

## The nine blocked operations

Paths below are relative to the fetched public corpus, not basename-only keys.
Each loss shows full inventory counts, not the excess over a default allowance.

| Source | Source version | Output | Reported loss | Classification |
|---|---|---|---|---|
| `acadrust/fixtures/dynamic-blocks/lookup.dwg` | AC1032 | DXF | `ACDB_DYNAMICBLOCKPURGEPREVENTER_VERSION` 1 to 0 | Deliberate DXF omission; restoration/compatibility limitation |
| `acadrust/fixtures/dynamic-blocks/point.dwg` | AC1032 | DXF | Same, 1 to 0 | Same |
| `acadrust/fixtures/dynamic-blocks/polar.dwg` | AC1032 | DXF | Same, 1 to 0 | Same |
| `acadrust/fixtures/dynamic-blocks/xy.dwg` | AC1032 | DXF | Same, 1 to 0 | Same |
| `libredwg/2000/Constraints.dwg` | AC1015 | DWG | `MULTILEADERSTYLE` 2 to 0 | Legacy writer policy, on a same-version rewrite |
| `libredwg/r14/Constraints.dwg` | AC1014 | DWG | `MULTILEADERSTYLE` 2 to 0 | Same; R14 must be investigated independently |
| `libredwg/2004/Surface.dwg` | AC1018 | DXF | `ACDBASSOCEDGEACTIONPARAM` 7 to 5 | Two opaque DWG objects lack a DXF representation in the codec |
| `libredwg/example_2000.dwg` | AC1015 | DXF | `DATATABLE` 1 to 0 | Opaque DWG object lacks a DXF representation in the codec |
| `libredwg/example_r14.dwg` | AC1014 | DXF | `DATATABLE` 1 to 0 | Same |

These are four causal groups, not nine independent bugs. No group has been
proved to be an unavoidable limitation of its destination format. A writer
comment or a currently unsupported schema is not such proof. Existing valid
same-version DWG inputs are particularly important: an old version alone does
not justify dropping every class-bearing object. None of these observations
justifies relaxing the inventory guard or automatically accepting a loss.

## MULTILEADERSTYLE: first investigation priority

Both legacy Constraints inputs contain decoded styles `0xD8` and `0xE5`,
owned by `0xD7`. Both describe Standard but differ in `property_changed`;
they cannot be treated as interchangeable defaults on that basis. The input
class table declares MLEADERSTYLE, but the codec's
`src/io/dwg/dwg_writer.rs::prepare_legacy_document` (lines 1015-1050) keeps
that dictionary only from AC1018 onward. For these AC1014/AC1015 targets it
removes `ACAD_MLEADERSTYLE` and its children, producing the measured 2-to-0 loss.

The existing product regression
`crates/arq-document/src/compatibility_tests.rs::legacy_multileaderstyle_refusals_exceed_the_default_allowance`
checks both files. `inventory.rs::default_losses` measures one missing default
style in an empty legacy drawing; `save_losses` still blocks loss of two
styles. The new inspection reproduces the exact two missing handles.

Next: build a focused same-version reproduction with distinguishable style
properties and dictionary/header references, investigate R2000 first and R14
separately, and compare native source/output observations. A candidate must
preserve the style data, identity and references, keep genuine downgrade
behavior explicit, and pass both internal and independent ProgeCAD checks.
Do not extend the TABLESTYLE exception mechanically: MULTILEADERSTYLE has its
own record layout and reference rules. A same-version preservation fix is a
candidate, not a demonstrated solution yet. The earlier Constraints
CMLEADERSTYLE warning is recorded in the [R2000 report](r2000-table-style-preservation.md).

## Dynamic-block purge preventer: four DXF cases

The missing records are decoded `DynamicBlock` objects, not merely a naming
alias. The handles are respectively `0x67C`, `0x26C`, `0x3DF` and `0x2A5`;
each carries representation flags 1 and a block reference, owner and reactor.
The DXF result has no corresponding object at that handle.

In the codec, `src/io/dxf/writer/section_writer.rs::is_unrestorable_assoc_object`
(lines 64-73) explicitly recognizes this class. `SectionWriter::build_valid_handles`
(lines 150-184) excludes it from valid output references, and `write_objects`
(lines 6179-6184) skips it even though it is decoded. The DWG reader's
`object_reader/dynamic_block.rs` (lines 665-674) and the DXF reader's
`section_reader.rs` (around line 4732) already have representation readers.
That does not prove that their decoded subset can restore the native object.

Next: inspect a native ProgeCAD DXF control, compare the class and exact
owner/reactor/block graph, and check reload/audit behavior before changing the
skip rule. Historical audit-skip evidence is present in the codec's
`tests/issue51_rt_issues/rt1_recoverlog.txt`; it is not a new validation of these
four files. A fresh successful native control is needed. Preserve the guard
until a complete representation or verified preservation mechanism exists.
This is definition preservation; dynamic parameter recomputation remains
separate work under ADR 0017 D6 and phase 04.

## DATATABLE: two opaque legacy records

`example_2000.dwg` has opaque DATATABLE `0xB2C`, owner `0xADB`;
`example_r14.dwg` has `0xD62`, owner `0xCAB`. Each retains 754 bytes of raw
DWG data with its source version, and neither has raw DXF pairs. The DXF output
omits it. Other DATATABLE fixture inputs in the same corpus do verify: this
is not absence of all DATATABLE support.

`src/io/dxf/writer/section_writer.rs::write_unknown_object` (lines 9597-9613)
only emits objects with `raw_dxf_codes`; a DWG-only opaque payload is skipped.
The corresponding native class reader in
`src/io/dwg/dwg_stream_readers/object_reader/class_object.rs` (lines 880-930)
can return no decoded object when counts or cell types are unsupported. The
exact failing field of these two legacy records has not yet been isolated;
unsupported cell type is a hypothesis, not a finding. Existing
`tests/datatable_roundtrip.rs::unsupported_dwg_cell_types_preserve_the_complete_record`
proves opaque same-version preservation for synthetic unsupported schemas;
it does not establish a DXF conversion for these two inputs.

Next: isolate the legacy decode failure and obtain a native DXF/control
observation of these exact objects, including columns, values, handle types
and ownership. Prefer complete semantic decoding where verified. Consider an
opaque preservation design only after proving native compatibility and
round-trip identity; do not relabel arbitrary bytes as native DATATABLE data.

## Surface associative objects: one DXF case

Of seven ACDBASSOCEDGEACTIONPARAM objects, five have decoded representations
and survive. Only `0x2E3` (owner `0x2E2`) and `0x50F` (owner `0x50E`) are missing;
each is an Unknown object with 93 raw DWG bytes, AC1018 provenance and no DXF
pairs. The same `write_unknown_object` omission applies. The codec already
has `write_associative_object_dxf` support for decoded EdgeActionParam in
`src/io/dxf/writer/section_writer/associative.rs` (around line 834).

Next: isolate how those two record variants differ from the five decoded
ones and compare their edge/curve/dependency representation in native CAD.
This is a targeted decoding/preservation gap; the current evidence does not
prove DXF cannot represent the class or that those two records are redundant.

## Acceptance, evidence and limits

Evidence remains under ignored `target/`: `loss-probe/src/main.rs`, its
manifest/lock, `loss-probe-final.log`, `loss-probe-final/probe.json`, separate
raw candidate files, and `loss-triage-evidence.json` with source/output hashes.
The probe uses a 32 MiB worker stack. An initial standalone run on the default
Windows executable stack overflowed; its partial outputs are not acceptance
evidence. The final complete run uses the product lock's dependency versions.

The codec source reviewed is the exact published commit above, in the earlier
own codec working copy. The private Rust harness is diagnostic only and adds
no production dependency or API. No drawing or private raw report is committed.
No new ProgeCAD session, all-feature suite, Python build or platform CI run is
claimed for this classification-only change. Source links and documentation
consistency are checked; production code and Claude's references are untouched.

The next implementation should address one demonstrated preservation defect
at a time, starting with the MULTILEADERSTYLE feasibility/reproduction above,
with red regression evidence, a scoped codec change only if validated, native
reopening, then a pinned-product matrix comparison. Phase 03.4 and external
validation 03.5 remain open. Successful geometry validation is not proof that
all non-geometric objects survived: all nine raw diagnostic outputs have an
empty `validate_roundtrip` difference list but still fail the inventory guard.
