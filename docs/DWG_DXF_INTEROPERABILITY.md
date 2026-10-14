# ArqStudioCAD — DWG/DXF interoperability

Module M02, phase 03. The decision is
[ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md): AcadRust
(crates.io, `=0.6.4`) as the base, behind `arq-document`; opencadcodec's
improvements brought in one by one; LibreDWG out of the product. The
evidence is the [library comparison](references/dwg-library-comparison.md)
(220 drawings, 6,600 operations); the tool is `tools/dwg-bench`.

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
0.6.4, measured on the comparison corpus. Levels: **read** (the engine reads
it), **keep** (it survives our save and reopening), **show** (drawn in the
viewport), **edit** (the platform changes it), **recompute** (regenerated
when its parameters change).

| Content | Read | Keep (DWG / DXF) | Show | Edit | Recompute | Notes |
|---|---|---|---|---|---|---|
| Lines, circles, arcs | ✅ | 100 % / 100 % | ✅ | ✅ geometry, layer, color | — | |
| Lightweight polylines | ✅ | 100 % / 100 % | ✅ | ◐ as a whole (vertices in 02.8) | — | |
| Blocks (INSERT) | ✅ | 100 % / 100 % | ✅ expanded | ◐ as a whole, attributes follow | — | MINSERT arrays not transformed |
| Dimensions | ✅ | 100 % / 100 % | ✅ as AutoCAD drew them (their block) | ○ | ○ | phase 04 |
| Hatches | ✅ | 100 % / 100 % | ◐ boundaries, no fill pattern | ○ | ○ | phase 04 |
| Texts, MTEXT, attributes | ✅ | 100 % / 100 % | ◐ outline box | ○ | — | fonts in phase 04 |
| Leaders, multileaders | ✅ | 100 % / 100 % | ◐ broken into lines | ○ | ○ | |
| Tables | ✅ | 100 % / 100 % | ◐ broken into lines | ○ | ○ | |
| Images, underlays, wipeouts | ✅ | 100 % / 100 % | ◐ frame or box | ○ | — | |
| 3D solids, regions, surfaces (ACIS) | ✅ geometry kept (761 kB measured) | 100 % / 100 % | ◐ bounding box in plan | ○ | ○ | 3D model in 02.8 |
| Meshes, 3D faces | ✅ | 100 % / 100 % | ◐ edges in plan | ○ | — | |
| Ellipses, splines, 2D solids | ✅ | ✅ (not measured separately) | ◐ broken into lines and arcs | ○ | — | |
| Dynamic blocks | ✅ definitions, parameters, actions, grips | 100 % / 100 % | ✅ current state (the evaluated block) | ○ | ○ visibility, stretch… | ADR 0016 D7, our parametric engine (ADR 0017 D6) |
| Geometric and dimensional constraints | ◐ AcadRust misses `ACDBASSOC2DCONSTRAINTGROUP` (opencadcodec reads it) | ✅ preserved as data | — | ○ | ○ | milestone 03.2 brings the reading in |
| Associative network (ASSOC*) | ✅ | 100 % / 100 % | — | ○ | ○ | |
| XDATA, XRECORD | ✅ | 100 % (measured on 14,169 entities) | — | ○ | — | where application data goes in DWG |
| Custom / proxy / AEC objects (BIM) | ✅ preserved | 100 % in the same DWG version; refused on conversion to 2018 | ○ proxy graphics | — | — | lost as DXF: asked first ([No silent loss](#no-silent-loss)) |
| External references (Xrefs) | ? | ? | ? | ○ | — | **not in the corpus yet** (03.5) |
| BIM drawings (AutoCAD Architecture, Civil 3D, Revit/ArchiCAD exports) | ◐ | ◐ | ◐ | ○ | — | few samples; the maintainer will produce more |
| DWG R11/R12 | ✅ (one R11 file rejected) | — | ✅ | ✅ | — | saved as DWG 2018 |
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

`tools/dwg-bench` (a workspace of its own) reads, writes and re-reads a
corpus with AcadRust, opencadcodec and, when built with it, LibreDWG; every
engine upgrade runs it and the round-trip corpus, and the results go into
ADR 0017's log.
