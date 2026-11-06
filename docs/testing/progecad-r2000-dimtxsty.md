# A read default dimension style naming a stale text style (R2000 AUDIT)

Date: 2026-11-06. Application base: main `e8725c6` with the documentation
branch `fix/codec-pin-docs`. Codec baseline: `b6e2a2ac662b37dc7229eea633947838d63315a7`
(tag `arqstudiocad/on-ea66abe-mleader-version-eed-v2`). Correction:
`6e03fb16f4481bfca6a00af8f18e31858d1b4bc8`, published as immutable fork tag
`arqstudiocad/on-ea66abe-r2000-dimtxsty` (red test `5ab73a0`), stacked on the
baseline so the DATATABLE/XRECORD and MLEADERSTYLE corrections are kept.
Status: codec tests, application test and independent ProgeCAD 2026 AUDIT
pass. Product adoption awaits PR review and merge.

## How it was found

The automated ProgeCAD 2026 run over the 3D solid samples (2026-11-05, see
[DWG 2004 multileader-style version](progecad-dwg2004-mleader-version.md))
reported, for the DWG 2000 samples built on LibreDWG's `example_2000.dwg`:

```
Total errors found during audit 2, fixed 0
AcDbDimStyleTableRecord(1B), Dimtxsty, Validation: <Object of class AcDbLinetypeTableRecord can't be cast to AcDbTextStyleTableRecord.>, Default value is: < Standard>.
AcDbDimStyleTableRecord(1B), Audit Failed, Validation: <Object of class AcDbLinetypeTableRecord can't be cast to AcDbTextStyleTableRecord.>, Default value is: < Removed>.
```

The original drawing audits clean, as does a new drawing saved as DWG 2000.

## Reproduction

`example_2000.dwg` rewritten by the codec alone, unchanged, reproduces the two
errors. LibreDWG's reading of the original lists one dimension style, ISO-25
(0x27), whose DIMTXSTY names the text style Standard (0x11); the DIMSTYLE
control holds only it. The codec's reading also has "Standard" (0x1B): the
default a new document holds, kept because the drawing has none. Its DIMTXSTY
is 0x14, the handle the default text style has in a new document; reading
replaced that text style by the drawing's own (0x11), and in this drawing 0x14
is the ByBlock linetype.

## Correction

After reading, a dimension style the drawing does not hold whose DIMTXSTY
names no text style gets the text style named by the name it keeps
(`dimtxsty`), else the drawing's current text style. The drawing's own styles
keep what they were read with. Whether a "Standard" style the drawing lacks is
added at all is unchanged. Codec commit `5ab73a0` holds only the test,
failing for that reason; `6e03fb1` the fix.

## Validation

- Codec: the new test (a drawing whose text style Standard moved and whose
  only dimension style is ISO-25; every dimension style read names Standard)
  and the whole suite (200 groups) pass.
- Application: `every_dimension_style_of_example_2000_names_a_text_style`
  (needs the codec samples) checks every dimension style of `example_2000.dwg`
  read, and saved and reopened, names a text style; with the previous pin it
  fails.
- ProgeCAD 2026, independent AUDIT of `example_2000.dwg` rewritten by the
  codec: two errors with the previous pin, none with the correction.

## Entities added to `example_2000.dwg`: not a codec fault

A line or a solid added to `example_2000.dwg` and saved as DWG 2000 reads back
here in model space, chained correctly (the former last entity 0x894 links to
it, the model-space record names it last; LibreDWG agrees), yet ProgeCAD does
not list it. ProgeCAD lists 45 model-space entities of the *original*
drawing too, stopping at 0x398, whose successor 0x399 is an ARC_DIMENSION, a
later entity in this DWG 2000; the 23 entities after it, up to 0x894, are not
listed either. An entity appended to the chain therefore falls in the part
ProgeCAD does not show. In the native DWG 2000 `2000/Constraints.dwg`, a line
added by the codec is listed (three entities, the new one last), as is a box
in a new DWG 2000 drawing (measured 24). The DWG 2000 samples of ADR 0024
steps 4a to 4c were built on `example_2000.dwg`: their solids are hidden by
the same cause, not by the writer.

## Still open

ATMOS (LibreDWG's `2007/ATMOS-DC22S.dwg`) saved again in DWG 2007/2018 is
refused by ProgeCAD; three AutoCAD solids whose construction history holds a
sweep node (`ACSH_SWEEP_CLASS`) are enough to cause it.
