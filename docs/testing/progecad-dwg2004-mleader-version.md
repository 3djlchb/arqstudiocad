# New DWG 2004 drawings refused by ProgeCAD: MLEADERSTYLE version EED

Date: 2026-11-06. Application base: main `a251fae` (after PR #128). Codec
baseline: `bab8ac01a48765648615a79089d77f3d594b6c0e` (tag
`arqstudiocad/on-ea66abe-datatable-dxf`), AcadRust 0.6.8 with our patches.
Correction: `b6e2a2ac662b37dc7229eea633947838d63315a7`, published as immutable
fork tag `arqstudiocad/on-ea66abe-mleader-version-eed-v2` (red tests
`26257d2`). It was first made on `c8f4b05` (red `f8132f7`, fix `f163b41`,
tag `arqstudiocad/on-ea66abe-mleader-version-eed`) and moved, unchanged, onto
the DATATABLE preservation merged meanwhile (PR #128).
Status: codec tests, application pin test and independent ProgeCAD 2026
reopening pass. Product adoption awaits PR review and merge.

## How it was found

The 3D solid samples of ADR 0024 steps 4a to 5a were opened in ProgeCAD 2026
Professional by an automated run (2026-11-05): each sample copy opened from
ProgeCAD's command line with a startup script that measures every ACIS body
(volume, centroid, box) and audits the drawing (`AUDITCTL` 1, report beside
the copy), the instance closed without saving; a dialog that blocks the run
is read and recorded, nothing pressed. Every DWG 2000 and 2018 and DXF sample
opened with AUDIT clean. Any DWG 2004 (AC1018) we wrote did not, down to an
empty new drawing or one LINE: *"Error: Object improperly read … No se puede
abrir este dibujo"*. DWG 2004 drawings written by AutoCAD, read and written
again by the codec, opened.

## Reproduction

Codec-only drawings (no ArqStudioCAD code) isolated it:

| AC1018 drawing written | ProgeCAD 2026 |
|---|---|
| New document, empty, or one LINE | refused |
| The same in AC1021 and AC1024 | opens |
| Native `Surface.dwg`, `Constraints.dwg` (2004) read and written | opens |
| New document without each default object kind in turn | refused, except without its MLEADERSTYLE: opens |

Declaring the class's DWG version (25, 27, 31) did not help. LibreDWG's
specification and a native AutoCAD AC1018 drawing show the cause: before
R2010 an MLEADERSTYLE does not hold its class version in its own data;
AutoCAD writes it as EED of the `ACAD_MLEADERVER` application, `1070 2`
(both styles of the native `Constraints.dwg` carry it, the APPID in its
table). Version 2 adds the text angle type and text-always-left fields the
codec writes; without the EED a reader takes version 1 and misreads the rest.

## Correction

Before R2010 the writer now gives every MLEADERSTYLE without it the
`ACAD_MLEADERVER` EED, `1070 2`, adding the APPID when the drawing lacks one.
A style that already carries the EED, read verbatim or set, keeps its own;
R2010 and later keep the version in the object, untouched. Codec commit
`26257d2` holds only the tests, failing for that reason; `b6e2a2a` the fix.

## Validation

- Codec: the three new tests (pre-R2010 EED written in AC1018 and AC1021, an
  existing EED kept once, none added from R2010) and the whole suite pass.
- Application: `a_new_drawing_saved_before_r2010_says_its_multileader_style_version`
  saves new drawings as DWG 2004 and 2007 and finds the EED reopened; with the
  previous pin it fails.
- ProgeCAD 2026, independent reopening: codec-written AC1018 drawings (empty,
  one LINE, native ones rewritten) and an application-written DWG 2004 and
  2007 with a line, a box and a cylinder open with AUDIT 0 errors. ProgeCAD
  measures the box at 24 exactly and the cylinder at 6.283178863 (ours 2π,
  ProgeCAD's volumes of curved solids agreeing to about 1e-6).

## Still open

Found by the same run, separately tracked:

- ATMOS (LibreDWG's `2007/ATMOS-DC22S.dwg`) saved by us is refused in 2007 and
  2018; three AutoCAD solids whose construction history holds a sweep node
  (`ACSH_SWEEP_CLASS`) are enough, alone, to cause it. Written with the
  codec's raw bisection aid it opens; the cause is being isolated.
- R2000 saves of drawings built on `example_2000.dwg` audit with two errors in
  ProgeCAD: the Standard dimension style's DIMTXSTY points at a linetype
  record. The original audits clean.
