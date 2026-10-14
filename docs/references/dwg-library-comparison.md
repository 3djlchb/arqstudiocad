# DWG/DXF library comparison (spike, 2026-10-13)

Which library should carry ArqStudioCAD's DWG/DXF interoperability:
**AcadRust**, **opencadcodec** (the codec we use since ADR 0008) or
**GNU LibreDWG**, alone, combined or unified. The decision itself is taken
in an ADR; this page is the evidence.

## The candidates

| | AcadRust 0.6.4 | opencadcodec rev 8189f83 | LibreDWG 0.14 |
|---|---|---|---|
| Language | Rust | Rust | C |
| License | MPL-2.0 | MPL-2.0 | GPL-3.0-or-later |
| Source | crates.io, 33 releases Feb–Oct 2026, 113 stars | git only, 13 stars | GNU, releases 2024 and 2026-08-31 |
| Relation | the original project (hakanaktt) | **a fork of AcadRust** (HakanSeven12, same declared author); 97 % of the code identical, about 5,500 differing lines of 200,000; it lacks AcadRust's DWG R12 reader and DXF code pages | independent |
| Used by | Open CAD Studio, CADCraft | Open CAD Studio, ArqStudioCAD | IngeCAD (as an external converter), QCAD plugins, others |

ADR 0008 believed the codec was not on crates.io: AcadRust, its upstream,
is.

## How it was measured

`tools/dwg-bench` (a Cargo workspace of its own, never part of the product)
links all three libraries; LibreDWG through a small C adapter
(`shim/shim.c`, the calls of its own `dwg2dxf`/`dwgwrite`). AcadRust and
opencadcodec run the very same Rust code (one macro instantiated per crate).
Every operation runs in a process of its own with a 120 s limit, so a crash
or a hang is recorded, not fatal; peak memory is the process's `VmHWM`.
Everything ran on Linux (WSL Debian 13), the only platform where LibreDWG
was built.

For each drawing and library: read; write DWG (codecs: 2018; LibreDWG:
2000, its documented reliable target, and 2018 as an extra attempt) and
DXF; read every output again **with all three libraries**. A re-read is
*faithful* when its entity types and counts (normalized: LibreDWG lists
BLOCK, ENDBLK, SEQEND, VERTEX and ATTRIB records as entities and names a few
types differently) and its LINE, CIRCLE and INSERT coordinate fingerprints
match the writer's original read. Dynamic-block, associative, solid, XDATA
and unknown-object counts are compared the same way.

Corpus, 220 files, built by `prepare_corpus.py` outside the repository:
LibreDWG's 196 test drawings (R1.4 to 2018: every entity kind, surfaces,
constraints, dynamic blocks, AutoCAD Architecture and TCH custom objects),
the 11 samples shipped with AcadRust (dynamic blocks), a generated
100,000-entity DXF (written as plain text, by none of the libraries), 9
damaged copies (cut at 50 % and 90 %, bytes flipped) and the maintainer's 3
drawings (read in place, never copied).

Reproduce: build LibreDWG into a prefix, then
`LIBREDWG_DIR=… LIBREDWG_SRC=… cargo build --release --features libredwg`,
`python3 prepare_corpus.py …`, `dwg-bench run <corpus> <work> 120`,
`python3 report.py <work> <corpus>`.

## Findings beyond the tables

1. **Custom objects and version conversion.** Saved in the drawing's own
   version, both codecs keep every custom object byte for byte (279
   AutoCAD Architecture objects in the maintainer's house plan, 26 TCH
   walls, 300 AEC objects). Converting to DWG 2018 they cannot re-encode
   them: **AcadRust refuses to save** (11 of 218 drawings, the house plan
   among them) while **opencadcodec saves and silently drops them** (25 TCH
   walls disappear: 360 → 335 entities). LibreDWG drops them too. ArqStudioCAD
   saves DWG in the drawing's own version (only R12 is upgraded), so today's
   saves keep them; a future "save as 2018" must not lose them silently.
   Section 8's "unknown kept" column measures the 2018 conversion.
2. **Truncated files open empty.** Both codecs read a DWG cut at 50 % as an
   empty drawing with a few notifications and no error; LibreDWG refuses it.
   An application must not offer to save over the original in that case.
3. **LibreDWG writes DWG 2018** in 0.14 (headers AC1032, re-read faithful
   80 % by itself), beyond what its documentation promises; its **DXF
   writer is weak** (23 % faithful) and its DXF reader times out on the
   100,000-entity drawing (the codecs read it in 0.4–0.7 s).
4. **3D solids from 2013 on.** In `example_2018.dwg` LibreDWG reads the
   three solids without their ACIS geometry (it is stored in a separate
   section that LibreDWG does not attach); the codecs read 12.7 kB of it.
5. **Executing dynamic blocks and constraints.** No library evaluates them.
   The codecs model every parameter, action, grip and associative object
   and resolve an insert's dynamic definition; **Open CAD Studio**
   (GPL-3.0, on this codec) switches visibility states, solves geometric
   and dimensional constraints and writes them as AutoCAD's own
   `AcDbAssoc2dConstraintGroup` objects. LibreDWG marks most of these
   classes unstable or debugging (dynamic blocks 15 stable of 54,
   associative 2 of 45, constraints and evaluation graph none).
6. **No complementary coverage.** LibreDWG reads no drawing that
   opencadcodec cannot; the only drawing opencadcodec reads and AcadRust
   does not is one R11 DWG (AcadRust's new R12 reader rejects it).

7. **3D solids and their geometry** survive saving in both codecs (100 %
   of 126 solids, 761 kB of ACIS, in DWG and DXF); LibreDWG keeps them in
   DWG but only 32 % in DXF, and reads nine without geometry.

## Annex: opencadcodec and AcadRust are one code base

The two codecs must not be read as independent implementations, so the
comparison between them focuses on where they part ways.

- **Lineage.** opencadcodec is closest to AcadRust **0.6.1** (2026-10-06):
  about 2,400 differing lines, against 17,000 from 0.6.0 and 5,500 from
  0.6.4. It is AcadRust 0.6.1 plus the fork's own changes, while AcadRust
  has moved on to 0.6.4.
- **Only upstream (0.6.2–0.6.4):** a DWG R12 reader and writer and DXF code
  pages (it reads the R11 test drawings in full, which the fork opens empty;
  one R11 drawing it rejects); a guard that **refuses** a DWG save that would
  lose custom objects; fixes for the active viewport's elevation and
  color-book colors.
- **Only in the fork:** fidelity work in DXF input/output, the document
  model, fields, sheet sets, ACIS, dynamic blocks and associative objects:
  block markers kept consistent with their records, FIELD hyperlinks and
  block units carried across versions, spline flags. Measured: it returns
  the **geometric-constraint groups** (`ACDBASSOC2DCONSTRAINTGROUP`) of all
  five `Constraints.dwg` (R14 to 2010) that AcadRust rejects, solid-history
  and surface-action objects, and has fewer errors (5 against 19 over every
  operation).
- **Correction (2026-10-13, milestone 03.2):** those constraint groups are
  **not read correctly** by the fork. R14 to 2010 store them in an older
  layout (`AcDbAssocAction` class version 1); the fork decodes it with the
  new layout and, lacking AcadRust's bounds checks, returns garbage without
  an error (node ids such as -801,111,547; 100,134 lines of output against
  249 for the same drawing saved as 2013). AcadRust was right to reject
  the records. Counting objects, as this comparison did, cannot tell a
  correct read from a lenient one: a "reads more" finding is checked
  against the same drawing in a version both libraries read well.
- **Section 12** lists every drawing where the two behave differently.

Repositories: [AcadRust](https://github.com/hakanaktt/acadrust),
[opencadcodec](https://github.com/HakanSeven12/opencadcodec),
[Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio),
[LibreDWG](https://www.gnu.org/software/libredwg/).

Not covered yet: drawings with **external references (Xrefs)**, of which the
corpus has none, and BIM drawings from AutoCAD Architecture, Civil 3D,
Revit or ArchiCAD beyond the few with AEC objects.

Interoperability with other CAD applications: a folder of outputs (the
house plan, `example_2018`, a dynamic block, constraints, a surface, from
each library) was handed to the maintainer to open in their CAD program;
the result is recorded with the decision.

## Measurements

220 drawings, 6607 operations, each in a process of its own (120 s limit).

### 1. Reading the original drawings

| Group | Files | acadrust | opencadcodec | libredwg |
|---|---|---|---|---|
| codec | 11 | 100 % (11/11) | 100 % (11/11) | 100 % (11/11) [recovered 4] |
| damaged | 9 | 100 % (9/9) [recovered 7] | 100 % (9/9) [recovered 6] | 11 % (1/9) [error 8, recovered 1] |
| generated | 1 | 100 % (1/1) | 100 % (1/1) | 0 % (0/1) [timeout 1] |
| libredwg | 196 | 99 % (194/196) [error 2, recovered 21] | 99 % (195/196) [error 1, recovered 12] | 95 % (187/196) [error 8, recovered 41, timeout 1] |
| local | 3 | 100 % (3/3) | 100 % (3/3) | 100 % (3/3) [recovered 1] |
| all | 220 | 99 % (218/220) [error 2, recovered 28] | 100 % (219/220) [error 1, recovered 18] | 92 % (202/220) [error 16, recovered 47, timeout 2] |

### 2. Speed and memory (files every library read)

201 files.

| Library | Read median ms | Read p95 ms | Read total s | Peak memory median MiB | Peak memory max MiB |
|---|---|---|---|---|---|
| acadrust | 12.6 | 38.5 | 2.7 | 11 | 64 |
| opencadcodec | 11.9 | 32.8 | 2.6 | 11 | 74 |
| libredwg | 3.1 | 127.4 | 12.4 | 8 | 59 |

Large drawing `grande-100k.dxf`: acadrust ok 370 ms 84 MiB; opencadcodec ok 685 ms 107 MiB; libredwg timeout 0 ms 0 MiB

### 3. Agreement between libraries on the originals

Normalized entity types and counts, LINE/CIRCLE/INSERT fingerprints.

- acadrust vs opencadcodec: 98 % (214/218)
- acadrust vs libredwg: 76 % (153/201)
- opencadcodec vs libredwg: 76 % (153/202)

### 4. Writing

| Library | Output | Written | Failed |
|---|---|---|---|
| acadrust | DWG 2018 | 95 % (207/218) | 11 |
| acadrust | DXF | 100 % (218/218) | 0 |
| opencadcodec | DWG 2018 | 100 % (219/219) | 0 |
| opencadcodec | DXF | 100 % (219/219) | 0 |
| libredwg | DWG 2000 | 100 % (202/202) | 0 |
| libredwg | DWG 2018 | 100 % (202/202) | 0 |
| libredwg | DXF | 100 % (202/202) | 0 |

### 5. Round trip: read → write → read again

Faithful = the re-read drawing has the same normalized types, counts and fingerprints as the original read by the writer.

| Writer | Output | Re-read by | Readable | Faithful |
|---|---|---|---|---|
| acadrust | DWG | acadrust | 100 % (207/207) | 100 % (206/207) |
| acadrust | DWG | opencadcodec | 100 % (207/207) | 100 % (206/207) |
| acadrust | DWG | libredwg | 100 % (207/207) | 84 % (174/207) |
| acadrust | DXF | acadrust | 100 % (218/218) | 93 % (202/218) |
| acadrust | DXF | opencadcodec | 100 % (218/218) | 93 % (202/218) |
| acadrust | DXF | libredwg | 82 % (179/218) | 73 % (159/218) |
| opencadcodec | DWG | acadrust | 100 % (219/219) | 98 % (215/219) |
| opencadcodec | DWG | opencadcodec | 100 % (219/219) | 99 % (216/219) |
| opencadcodec | DWG | libredwg | 100 % (219/219) | 83 % (181/219) |
| opencadcodec | DXF | acadrust | 100 % (219/219) | 93 % (204/219) |
| opencadcodec | DXF | opencadcodec | 100 % (219/219) | 93 % (204/219) |
| opencadcodec | DXF | libredwg | 82 % (180/219) | 74 % (162/219) |
| libredwg | DWG | acadrust | 100 % (202/202) | 74 % (149/202) |
| libredwg | DWG | opencadcodec | 100 % (202/202) | 76 % (154/202) |
| libredwg | DWG | libredwg | 97 % (196/202) | 81 % (164/202) |
| libredwg | DXF | acadrust | 99 % (200/202) | 20 % (40/202) |
| libredwg | DXF | opencadcodec | 99 % (200/202) | 20 % (40/202) |
| libredwg | DXF | libredwg | 97 % (195/202) | 23 % (46/202) |
| libredwg | R2018.DWG | acadrust | 100 % (202/202) | 69 % (140/202) |
| libredwg | R2018.DWG | opencadcodec | 100 % (202/202) | 76 % (153/202) |
| libredwg | R2018.DWG | libredwg | 97 % (196/202) | 80 % (161/202) |

### 6. Dynamic blocks and parametric (associative) objects


### dynamic-block objects: 24 drawings contain them

| Library | Read in N drawings | Objects read | Kept after own DWG round trip | Kept after own DXF round trip |
|---|---|---|---|---|
| acadrust | 24/24 | 561 | 100 % (303/303) | 100 % (561/561) |
| opencadcodec | 24/24 | 561 | 100 % (561/561) | 100 % (561/561) |
| libredwg | 22/24 | 472 | 93 % (437/472) | 17 % (81/472) |

### associative/constraint objects: 30 drawings contain them

| Library | Read in N drawings | Objects read | Kept after own DWG round trip | Kept after own DXF round trip |
|---|---|---|---|---|
| acadrust | 30/30 | 452 | 100 % (418/420) | 100 % (452/452) |
| opencadcodec | 30/30 | 476 | 100 % (476/476) | 100 % (476/476) |
| libredwg | 27/30 | 306 | 96 % (294/306) | 28 % (85/306) |

### 7. 3D solids (ACIS geometry)

24 drawings have 3DSOLID, REGION, BODY or surfaces.

| Library | Solids read | With ACIS geometry | ACIS kB | Geometry kept, own DWG round trip | Own DXF round trip |
|---|---|---|---|---|---|
| acadrust | 125 | 125 | 751 | 100 % (108/108) | 100 % (125/125) |
| opencadcodec | 126 | 126 | 761 | 100 % (126/126) | 100 % (126/126) |
| libredwg | 113 | 104 | 657 | 100 % (104/104) | 29 % (30/104) |

### 8. BIM-relevant data: extended data and objects a library does not know

| Library | Entities with XDATA | XRECORD objects | Unknown/proxy objects | XDATA kept, own DWG round trip | Unknown kept, own DWG round trip |
|---|---|---|---|---|---|
| acadrust | 14169 | 7686 | 890 | 100 % (11167/11167) | 100 % (273/273) |
| opencadcodec | 14170 | 7687 | 890 | 100 % (14170/14170) | 32 % (287/890) |
| libredwg | 9367 | 7710 | 1186 | 100 % (9367/9367) | 25 % (300/1186) |

### 9. Damaged drawings

| File | acadrust | opencadcodec | libredwg |
|---|---|---|---|
| example_2000-cut50.dwg | ok | ok | error |
| example_2000-cut90.dwg | ok | ok | error |
| example_2000-flipped.dwg | recovered | ok | recovered |
| example_2018-cut50.dwg | recovered | recovered | error |
| example_2018-cut90.dwg | recovered | recovered | error |
| example_2018-flipped.dwg | recovered | recovered | error |
| lookup-cut50.dwg | recovered | recovered | error |
| lookup-cut90.dwg | recovered | recovered | error |
| lookup-flipped.dwg | recovered | recovered | error |

### 10. Stability over every operation

| Library | Operations | Crashes | Timeouts | Panics | Errors |
|---|---|---|---|---|---|
| acadrust | 2129 | 0 | 0 | 0 | 19 |
| opencadcodec | 2129 | 0 | 0 | 0 | 5 |
| libredwg | 2349 | 0 | 15 | 0 | 154 |

### 11. Kinds of data kept after saving and reopening

Entities of each kind in the originals each library read, and the share still there when it re-reads its own DWG and DXF.

| Kind | acadrust read | DWG kept | DXF kept | opencadcodec read | DWG kept | DXF kept | libredwg read | DWG kept | DXF kept |
|---|---|---|---|---|---|---|---|---|---|
| Dimensions | 382 | 100 % (231/231) | 100 % (382/382) | 380 | 100 % (380/380) | 100 % (380/380) | 339 | 99 % (335/339) | 48 % (164/339) |
| Hatches | 212 | 100 % (100/100) | 100 % (212/212) | 212 | 100 % (212/212) | 100 % (212/212) | 197 | 100 % (197/197) | 47 % (93/197) |
| Block references | 973 | 100 % (557/557) | 100 % (973/973) | 969 | 100 % (969/969) | 100 % (969/969) | 719 | 99 % (709/719) | 28 % (200/719) |
| Texts | 21599 | 100 % (20604/20604) | 100 % (21599/21599) | 21593 | 100 % (21593/21593) | 100 % (21593/21593) | 1565 | 99 % (1548/1565) | 31 % (480/1565) |
| Leaders | 53 | 100 % (41/41) | 100 % (53/53) | 53 | 100 % (53/53) | 100 % (53/53) | 51 | 100 % (51/51) | 67 % (34/51) |
| Polylines | 22110 | 100 % (21531/21531) | 100 % (22110/22110) | 22108 | 100 % (22108/22108) | 100 % (22108/22108) | 1513 | 97 % (1475/1513) | 23 % (342/1513) |
| 3D solids, regions, surfaces | 125 | 100 % (108/108) | 100 % (125/125) | 126 | 100 % (126/126) | 100 % (126/126) | 113 | 100 % (113/113) | 32 % (36/113) |
| Meshes and 3D faces | 314 | 100 % (171/171) | 100 % (314/314) | 313 | 100 % (313/313) | 100 % (313/313) | 179 | 98 % (175/179) | 14 % (25/179) |
| Tables | 18 | 100 % (12/12) | 100 % (18/18) | 18 | 100 % (18/18) | 100 % (18/18) | 6 | 0 % (0/6) | 0 % (0/6) |
| Images, underlays, wipeouts | 113 | 100 % (100/100) | 100 % (113/113) | 113 | 100 % (113/113) | 100 % (113/113) | 113 | 100 % (113/113) | 65 % (73/113) |

### 12. AcadRust and opencadcodec, drawing by drawing

opencadcodec is a fork of AcadRust; every drawing where the two behave differently: reading, what they read, or writing.

| Drawing | Difference |
|---|---|
| damaged/example_2000-flipped.dwg | read: AcadRust recovered, opencadcodec ok; LINE: 67 vs 68; ACDBASSOCGEOMDEPENDENCY: 0 vs 1; VISUALSTYLE: 23 vs 24; XRECORD: 263 vs 264; write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DATATABLE at handle 0xB2C) |
| libredwg/2000/Constraints.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOC2DCONSTRAINTGROUP: 0 vs 1; ACDBASSOCGEOMDEPENDENCY: 0 vs 2 |
| libredwg/2000/PolyLine2D.dxf | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object LAYOUTPRINTCONFIG at hand) |
| libredwg/2000/TS1.dwg | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DATATABLE at handle 0x2A7) |
| libredwg/2004/Constraints.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOC2DCONSTRAINTGROUP: 0 vs 1; ACDBASSOCGEOMDEPENDENCY: 0 vs 2 |
| libredwg/2004/Surface.dwg | read: AcadRust recovered, opencadcodec ok; SURFACE: 4 vs 5; ACDBASSOCEDGEACTIONPARAM: 5 vs 7; ACDBASSOCPLANESURFACEACTIONBODY: 0 vs 1 |
| libredwg/2007/ATMOS-DC22S.dwg | read: AcadRust recovered, opencadcodec ok; ACSH_EXTRUSION_CLASS: 0 vs 5; ACSH_SWEEP_CLASS: 0 vs 4 |
| libredwg/2007/Constraints.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOC2DCONSTRAINTGROUP: 0 vs 1; ACDBASSOCGEOMDEPENDENCY: 0 vs 2 |
| libredwg/2010/Constraints.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOC2DCONSTRAINTGROUP: 0 vs 1; ACDBASSOCGEOMDEPENDENCY: 0 vs 2 |
| libredwg/2010/gh209_1.dwg | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DMSTYLECATEGORY at handle) |
| libredwg/2013/gh109_1.dwg | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom entity TCH_WALL at handle 0x28A ) |
| libredwg/2013/gh44-error.dwg | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object AEC_DISP_PROPS_ENT at han) |
| libredwg/example_2000.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOCGEOMDEPENDENCY: 0 vs 1; write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DATATABLE at handle 0xB2C) |
| libredwg/example_2004.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOCGEOMDEPENDENCY: 0 vs 1; write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DATATABLE at handle 0xB2F) |
| libredwg/example_2007.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOCGEOMDEPENDENCY: 0 vs 1 |
| libredwg/example_2010.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOCGEOMDEPENDENCY: 0 vs 1 |
| libredwg/example_r13.dwg | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DATATABLE at handle 0x105) |
| libredwg/example_r14.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOCGEOMDEPENDENCY: 0 vs 1; write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object DATATABLE at handle 0xD62) |
| libredwg/r11/ACEB10.dwg | read: AcadRust error, opencadcodec recovered; write DWG: AcadRust refused, opencadcodec ok (read: Invalid file format: R12 handle exceeds 8 bytes (recovering: Inv); write DXF: AcadRust refused, opencadcodec ok (read: Invalid file format: R12 handle exceeds 8 bytes (recovering: Inv) |
| libredwg/r11/entities-2d.dwg | read: AcadRust ok, opencadcodec recovered; ARC: 1 vs 0; ATTDEF: 2 vs 0; CIRCLE: 1 vs 0; DIMENSION_ALIGNED: 1 vs 0; INSERT: 2 vs 0 … |
| libredwg/r11/entities-3d.dwg | read: AcadRust ok, opencadcodec recovered; 3DFACE: 1 vs 0; ARC: 1 vs 0; ATTDEF: 2 vs 0; CIRCLE: 1 vs 0; DIMENSION_ALIGNED: 1 vs 0 … |
| libredwg/r14/Constraints.dwg | read: AcadRust recovered, opencadcodec ok; ACDBASSOC2DCONSTRAINTGROUP: 0 vs 1; ACDBASSOCGEOMDEPENDENCY: 0 vs 2 |
| local/Prueba_casa familiar.dwg | write DWG: AcadRust refused, opencadcodec ok (Not implemented: Cannot encode custom object AEC_AECDBDISPREPCURTAINWA) |
