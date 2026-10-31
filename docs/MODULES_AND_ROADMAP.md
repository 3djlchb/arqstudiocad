# ArqStudioCAD — Modules and roadmap

The order of work of the [master plan](MASTER_PLAN.md) (version 2.1,
2026-10-13), and where the project stands. Phases are a sequence, not
dates: a phase ends when its exit criterion is met and tested, not when its
code exists. Phases may overlap once their contracts are stable.

Legend: ✅ done · ◐ in progress · ○ to do.

## Where we are (2026-10-31)

- **Technical MVP: 8 of 8** (master plan §15): milestone 02.6 saves and
  recovers the working document (DWG) from the own internal model.
- **Working format:** DWG, as BricsCAD does, with an own internal model; the
  native format `.ascad` is designed and reserved (ADR 0018).
- **Data protections (03.3):** done (ADR 0017 D5).
- **On hold:** milestone 02.7's GPU viewport waits for the reference
  machine (GT 730 on LMDE 7), its release gate (ADR 0007); the work
  resumes when the maintainer has it at hand (maintainer, 2026-10-18).
  Milestone 02.8 goes on without it; only shading solids waits for it.
- **Current:** milestone 02.8: texts (ADR 0022), blocks (ADR 0023: EXPLODE
  with attributes, BLOCK and INSERT, the block editor) and the PLINE
  command are done; dynamic blocks wait for the parametric engine,
  milestone 04.1 (D3a). The 3D document model is decided (ADR 0024) and the
  owned kernel `arq-kernel` reads DWG solids into its B-rep (step 1,
  analytic and NURBS geometry: all 75 of LibreDWG's bodies) and draws
  them by their edges and isolines (step 2, see); step 3, measure, is
  next. The 3D modeling commands get a ribbon tab of their own, 3D
  (maintainer, 2026-10-31).
- **Done since 02.8 began:** the Blocks palette (02.9, ADR 0025), the
  Block Definition dialog (02.10), block data for budgets (02.11, ADR
  0027), the budget table (02.12, ADR 0028), and the model-space context
  menu with the Options dialog (02.13, ADR 0029).
- **Engine:** AcadRust 0.6.8 plus upstream post-release fixes, patched from our
  fork at `f1b0980` (ADR 0017's log, 2026-10-30;
  [upgrade validation](testing/acadrust-068-upgrade.md),
  [table DXF formatting](testing/progecad-table-dxf-format.md)).
- **Before:** milestone 02.7 uses the spatial index for bounding-box
  queries and as the broad phase for picking, window/crossing selection, and
  object snaps; exact geometry checks remain in place. A crash-isolated wgpu
  probe checks adapter, pipeline, offscreen draw and readback; it passed locally
  on Windows with Intel UHD 630 / DX12. GPU viewport rendering and reference
  Linux validation remain open. Milestone 02.5 is complete and manually
  validated by the maintainer, including property editors, shared panel/table
  styling, and CAD color selection; 03.2 is done and 03.4 compatibility work
  is in progress.
- **Milestone 02.8** goes on while 02.7 is on hold: the 3D mathematical
  foundation, polyline vertex editing and PLINE, texts, blocks, the 3D
  document decision, reading solids (analytic and NURBS) and drawing their
  edges and isolines in plan are done. What remains, in order (checked
  against the code, 2026-10-31): measuring solids; writing ACIS from the
  B-rep, which saving created or edited solids needs; creating primitives,
  extrusions and revolutions; transforming solids as a whole; booleans,
  through Truck once reviewed; stage 2 of the own model (ADR 0018 D2:
  polylines, block references and definitions; the model owns stage 1
  today); and 3D wireframe entities with Z and user coordinate systems.
  Shading waits for the GPU viewport (02.7).
- Before the master plan the phases were numbered 0–10 (ADR 0014) and
  milestones 2.1–2.8; the table at the end maps old names to new ones.

## Phase 00 — Specification and contracts ✅

Technical contract, ADRs, licenses, conventions, acceptance criteria;
charter, master plan and governing documents (2026-10-13).
Exit: decisions documented and a compilable skeleton.

## Phase 01 — Technological foundation ✅

Cargo workspace, uv, PyO3 + maturin, PySide6 window with i18n, local checks,
CI on every pull request and push to `main`, packages for every target
(ADR 0001–0006, 0009). PRs #1–#4.
Exit: a minimal reproducible application on Windows showing a result
computed in Rust.

## Phase 02 — CAD core ◐

Exit: create, edit, save and recover a basic drawing.

| Milestone | Content | Status |
|---|---|---|
| 02.1 Geometry | `arq-geometry`: points, vectors, lines, circles, arcs, transformations, intersections, snaps (ADR 0010) | ✅ PR #5 |
| 02.2 Document | `arq-document`: entities, layers, ids, change recording; `arq-core`: transactions, groups, undo/redo, queries, document store; round-trip corpus (ADR 0011) | ✅ PR #6 |
| 02.3 Python API | `arq-python`: the whole core from Python, typed (ADR 0012) | ✅ PR #7 |
| 02.4 Application shell | Drawing primitives, CPU rasterizer, viewport, events, commands and command window, tabs, background open/save, panels, Ribbon and Classic workspaces, themes (ADR 0013, 0015) | ✅ PR #8 |
| 02.5 Editing workflow | Selection, multifunction grips, object snaps and ORTHO, AutoCAD point syntax, LINE…ERASE, blocks and polylines transformed as a whole, property editors, Spanish command names (ADR 0016) | ✅ steps 2.5.1–2.5.8 complete and manually validated by the maintainer |
| 02.6 Own internal model | Stage 1 of ADR 0018 D2: layers, current layer, LINE, CIRCLE, ARC owned by our model; the DWG engine reads into it and writes from it; save and recover the DWG from the own model; written through to the engine, checked after every transaction in debug builds | ✅ |
| 02.7 Spatial index and GPU viewport | Spatial index (ADR 0021); wgpu renderer with backend probing (ADR 0007); CPU rasterizer as fallback | ◐ R*-tree accelerates bounding-box queries and supplies candidates for picking, window/crossing selection, and object snaps; exact geometry checks remain; the offscreen backend probe passes locally on Windows/DX12; GPU viewport rendering and reference Linux validation remain open |
| 02.8 Wider entities and 3D | Polyline vertices, block contents and explode, texts; staged 3D geometry foundation and later document model (ADRs 0019 and 0014 question 4) | ◐ 3D mathematical foundation complete (PR #24); polyline vertex grips and transactional vertex edits complete (PR #35); block-reference EXPLODE complete (PR #36; attribute values kept as texts since ADR 0023 D1, PR #41); PLINE (PR #48); texts complete (ADR 0022: drawn with their letters in a built-in stroke font, transformed as a whole, one grip, content, height and rotation in Properties, TEXT and basic MTEXT commands with word wrapping; the drawing's fonts and formatted MTEXT in phase 04); blocks complete (ADR 0023); the 3D document model decided (ADR 0024: own B-rep, ACIS kept, Truck as first temporary backend); step 1, read, done: `arq-kernel` holds the B-rep with analytic and NURBS geometry, and all 75 of LibreDWG's bodies read into it and check; step 2, see, done: solids drawn in plan by their edges and isolines (ISOLINES), in blocks too; step 3, measure, next; the 3D modeling commands get a ribbon tab of their own, 3D (maintainer, 2026-10-31); 02.7's GPU viewport on hold, which only shading waits for |
| 02.9 Blocks palette | A dockable palette of the drawing's, a template's and recent blocks, with previews, the dynamic-block tag, insertion by button or drag and drop, and copying definitions between files (ADR 0025; [design brief](design/blocks-palette.md)) | ✅ PR #52 added the Drawing source as a searchable list, BLOCKSPALETTE and Ctrl+3, and synchronized palette toggles in the View tab and menu; step 2b took the mockup's layout (sources, list with each block's references, Selected block section) on new core queries `block_info` and `block_references`; step 2c added the insert options (scale, rotation, on screen) and the Insert, Edit block and New actions through INSERT, BEDIT and BLOCK; step 2d added PROPERTIES and Ctrl+1, and Layers and Properties in the Palettes group; step 3 added previews drawn in Rust, the grid of cards and block definitions in change sets (contract §6); step 4 tagged dynamic blocks with a star, listed their parameters and kept them out of the block editor; step 5 dropped blocks onto the drawing through INSERT and added Explode; step 7a added the core operation that copies a block from another document, with its nested blocks, layers, linetypes and styles, and the clash choices; step 7b added the Template source (load in the background, sample template, empty state, file drop, insert by copying); step 7c added the clash dialog (both blocks previewed, three choices) and BLOCK's check of the template's names; step 6 added the Recent source (this session's and the previous one's blocks, from the file each came from); step 8a made a double click on a block open the Edit Block Definition dialog and a grey block editor; step 8b added the blocks' context menu (Insert with options, Copy to drawing through COPYBLOCK, Redefine from template, confirmed); step 8c added RENAME (alias REN, Spanish CAMBIANOM) for blocks, with its dialog, -RENAME's prompts and the menu's Rename…; step 8d added SELECTBLOCK (Spanish SELECBLOQUE) and the menu's Select in drawing; step 8e added PURGE (alias PU, Spanish LIMPIA) for unused blocks, with its dialog, nested items, -PURGE's prompts and the menu's Purge…, completing ADR 0025 D9; the maintainer accepted the ADR (2026-10-27), independent of the 3D kernel steps |
| 02.10 Block definition dialog | BLOCK's dialog, as AutoCAD's *Block Definition* (maintainer, 2026-10-25): name with the drawing's blocks, description, base point and objects typed or picked on screen, Retain / Convert to block / Delete the objects, Scale uniformly, Allow exploding, units, and Open in block editor; Annotative shown disabled until annotation scales (phase 04). Order: brought forward, before Blocks palette steps 8c–8e and the model-space context menu and Options (maintainer, 2026-10-26). ADR 0023 D2a records the maintainer's decisions; then two pull requests: the core (Retain and Delete, writing the description and units, reading, writing and honoring the two flags in INSERT and EXPLODE), then the dialog | ✅ ADR 0023 D2a (PR #76); the core: define_block with Retain, Convert and Delete, redefining, the record's description, units and flags, EXPLODE honoring Allow exploding (PR #77); the dialog: BLOCK opens it, -BLOCK keeps the prompts, a name in use is redefined once confirmed |
| 02.11 Block data | Each block definition carries its own data for budget tables (maintainer, 2026-10-26): category, type, material, cost and supplier; the cost has a currency from the drawing's register (maintainer, 2026-10-27). Decisions ([ADR 0027](architecture/adr/0027-block-data.md)): the data is stored inside the DWG, on the block definition (an extension dictionary with an XRECORD), so it travels with the drawing and with a copied block; it belongs to the definition, not to each reference, and a budget's quantity is the number of references (SELECTBLOCK's count); category and type are free text, suggesting the values the drawing already uses; the cost is an exact amount with its currency's decimals; the drawing's register, kept in the DWG, lists its currencies (US dollars by default), several active and one principal, picked from an ISO 4217 catalog or the user's own, so that a budget can later be broken down by currency. The Block Definition dialog shows them in an optional section, folded by default; they are edited later from the Blocks palette (Block data…) and the Properties panel, and the currencies from a Currencies dialog (CURRENCIES), without the block editor, which keeps them when it saves. Attributes stay for what differs between references (a door's code, a room's number). A library shared between drawings, and the budget table itself (quantities × cost), come later; after it, measured quantities for budget line items (perimeter, area, height, volume, weight, each in its unit, the measure each block is priced by chosen and changeable), taken from each reference, so a dynamic block's quantities follow its parameters with the parametric engine (04.1) (maintainer, 2026-10-28; ADR 0027 D7). Order: after Blocks palette step 8e; an ADR first, then the core, the Currencies dialog, the Budget data section with BLOCKDATA and the palette, and the Properties panel | ✅ ADR 0027, accepted 2026-10-29; step 2, the core: block data and the currency register kept in the DWG and checked on save, BLOCKDATA and CURRENCIES as undo steps, copied, renamed and purged with their block, and the Python API; step 3: CURRENCIES (Spanish MONEDAS) and its Currencies dialog, with the ISO 4217 catalog and the number format (thousands and decimal separators, D3a), general for the drawing and, if wanted, its own for each currency; step 4a: BLOCKDATA (Spanish DATOSBLOQUE) and -BLOCKDATA with the Block Data dialog, and the Blocks palette's Block data… and Budget data section; step 4b: the folded Budget data section of the Block Definition dialog, written with the block in one undo step; clearing a block's data at once: Clear data in the Block Data dialog and Clear block data in the palette's menu, confirmed first; step 5: the Properties panel's Block data group, edited in place for references of one block, completing ADR 0027 D8 |
| 02.12 Budget table | The budget from the drawing (maintainer, 2026-10-29; [ADR 0028](architecture/adr/0028-budget-table.md)): a dockable Budget panel and the BUDGET command (Spanish PRESUPUESTO), grouped by category with subtotals; each block's quantity is its references in model space, nested ones multiplied by the block holding them, layouts excluded; blocks without a cost listed apart, never summed; totals per currency, never mixed; the whole drawing or the selection only; export to CSV and to Excel (.xlsx, through openpyxl, MIT). Order: the ADR, the core's quantities, the panel, the export | ✅ ADR 0028, accepted 2026-10-29 |
| 02.13 Context menu and Options | The maintainer's design briefs (2026-10-22; decisions 2026-10-29, [ADR 0029](architecture/adr/0029-context-menu-and-options.md)): a right-click on model space with nothing selected opens Repeat {command} and Options…, and acts as Enter during a command; the Options dialog (OPTIONS, OP, Spanish OPCIONES), modal, showing only settings whose feature exists: language and theme, a crosshair drawn by the viewport with a live preview, the block editor's background, wheel zoom, the right-click setting. Order: the ADR, the menu, the dialog, the crosshair, zoom | ✅ ADR 0029, accepted 2026-10-31: the menu, the dialog with General, User preferences (Right-click, Zoom), `OptionsChanged`, Ctrl+Tab, the crosshair drawn by the viewport and Options › Display |

## Phase 03 — DWG/DXF ◐

Exit: a documented compatibility matrix and no known silent losses in the
cases covered. Module M02. Decision: [ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md);
detail: [DWG/DXF interoperability](DWG_DXF_INTEROPERABILITY.md).

| Milestone | Content | Status |
|---|---|---|
| 03.1 Engine on AcadRust | AcadRust 0.6.4 from crates.io behind `arq-document`; corpus and tests green | ✅ (since 2026-10-28 AcadRust 0.6.8 plus post-release fixes, patched from our fork: ADR 0017's log) |
| 03.2 opencadcodec's improvements | One by one with regression drawings: geometric constraints first, then fidelity fixes and diagnostics; kept in our AcadRust fork | ✅ every measured difference checked and the right ones brought in (R2004 lofted surfaces); the fork's wrong readings replaced by keeping objects verbatim; R11 linetypes, TOLERANCE in DXF and our own RAY and ACIS checks fixed; LibreDWG regression reads and round trips covered; two R2000 TABLESTYLE-owner losses are now explicitly checked as safe refusals ([evidence](testing/progecad-dxf-unloaded.md)) |
| 03.3 Data protections | Damaged files reported, safe save with read-back validation, no silent loss on version conversion, regression drawings | ✅ damaged files refused or recovered and marked; saves read back and checked; losses asked before saving; regression drawings made by the tests |
| 03.4 Compatibility matrix | By version, entity and operation; dynamic blocks, constraints, ACIS, Xrefs, tables, dimensions, hatches, custom BIM objects | ◐ reproducible integrated-core read/safe-save report (`tools/compatibility_matrix.py`); initial 30-file run: DWG 27 verified / 3 refused, DXF 16 verified / 14 refused; two legacy MULTILEADERSTYLE refusals explained and regression-tested; two R2000 TABLESTYLE-owner losses remain safely refused; synthetic Xref corpus checks path, flags and INSERT transformations through DWG/DXF; filename markers represent Xref INSERTs; fork 59aaf4c preserves the explicit DXF unloaded marker verified in native ProgeCAD exports and reopening; all six local Xref drawings and 12 strict DWG/DXF saves pass, with the reference-level save guard retained ([investigation](testing/progecad-dxf-unloaded.md), [baseline results](testing/progecad-xrefs-results.md)); native table corpus measures three ProgeCAD inputs and 249 properties per reopening; fork 0d438b6 corrects the measured DXF-to-DWG margins, row text heights and right-border loss, with 0/249 differences against native DWG reopening ([correction](testing/progecad-table-instance-formatting.md)); the subsequent fork 179aaeb corrects AC1021 native border-table reading and strict rewriting, also matching 249/249 native measurements ([reader correction](testing/progecad-table-2007-reader.md)); fork f1b0980 corrects eight DWG-to-DXF margins, twelve row-type text heights and eight merged-title values, reducing native differences from 88 to 60; the subsequent fork 5c09a39 corrects inherited heights and shared borders; remaining raw differences are measured against a fresh native DXF control ([cell fidelity](testing/progecad-table-cell-fidelity.md)); other format limits remain open ([baseline](testing/progecad-tables-results.md)); reference resolution, broader version/operation coverage, visual/semantic fidelity and external validation remain open ([measurements](DWG_DXF_INTEROPERABILITY.md#integrated-product-matrix-034)) |
| 03.5 External validation | Outputs opened in independent CAD applications; Xref and BIM drawings in the corpus | ◐ [ProgeCAD exercise](testing/progecad-xrefs.md): six real source DWGs, ten retained host outputs reopened per engine run (0.6.5 and 0.6.7); unloaded-DXF state preservation is corrected and independently checked; the full native DXF-to-DWG companion conversion preserves margins 1.5 and unloaded state (fork 59aaf4c); native table measurements validate the AC1032 DWG round trip and the corrected DXF-to-DWG instance format in fork 0d438b6 ([correction](testing/progecad-table-instance-formatting.md)); the AC1021 border-table reader is corrected in fork 179aaeb and verified by native reopening ([reader correction](testing/progecad-table-2007-reader.md)); native reopening of fork f1b0980 confirms all 189 non-cell-record properties after DWG-to-DXF export, with sixty cell-record differences still reported ([DXF correction](testing/progecad-table-dxf-format.md)); the cell-cache correction independently matches all 249 native DXF-to-DWG and DWG 2007 rewrite properties, while its same-format DXF rewrite matches the reopened native DXF control exactly ([cell fidelity](testing/progecad-table-cell-fidelity.md)); reload and broader coverage remain open ([results](testing/progecad-xrefs-results.md)). Loading design is [proposed in ADR 0026](architecture/adr/0026-external-reference-loading.md), for phase 04 or later per ADR 0023 |
| 03.6 Fidelity across versions | DWG as the default save format once the corpus allows it (contract §10); linetypes, lineweights, layouts | ○ |

The [Map resource proxy correction](testing/progecad-map-resource-preservation.md)
is locally validated on main `76d2cc0`: all six native table safe saves and
all twelve Xref safe saves verify, with no other integrated status change.
Fork commit `c05e7be` is published under its immutable tag and the task branch
pins that exact Git revision; its repeated matrix and Python checks pass.
The Map correction and upstream 0.6.8 upgrade are merged. The subsequent
[table DXF formatting correction](testing/progecad-table-dxf-format.md) is
merged in PR #101. It corrects the measured margins, row-type text heights
and merged-title values. The subsequent [cell-cache investigation](testing/progecad-table-cell-fidelity.md)
corrects demonstrated per-cell height and shared-border losses. Fifty-two
CELL_VALUE flag differences also occur when reopening
ProgeCAD's own DXF export; they remain visible in the strict comparator.
Fork `5c09a39` is published under its immutable tag and pinned on the integration
branch: 488 Rust and 795 Python tests pass, with the same 34-drawing matrix.
Product adoption awaits PR review and merge.
Milestones 03.4 and 03.5 remain in progress.

## Phase 04 — Associative technical documentation ○

Dimensions, texts and fonts, annotations, hatches, views, scales, sheets,
title blocks, drawing templates (title blocks, sheets, scales and styles;
NEW starting from a template), plotting to PDF; projections, sections and elevations as
**associative views** that know their source and regenerate keeping manual
annotations (ADR 0014); the **project container** (models, drawings, views,
sheets, resources; an ADR first). Module M09.

It opens with the engine its associativity stands on *(maintainer,
2026-10-21)*:

| Milestone | Content | Status |
|---|---|---|
| 04.1 Parametric engine | ArqStudioCAD's own engine (ADR 0017 D6; an ADR first): a dependency graph that regenerates what depends on what changed; 2D geometric and dimensional constraints, solved and kept through DWG; dynamic blocks recomputed when a parameter changes (visibility, stretch, flip, lookup…), so the block editor accepts them (ADR 0023 D3a). Associative dimensions and views are built on it; BIM objects (phases 05 and 07) reuse it | ○ |

## Phase 05 — BIM and IFC ○

IfcOpenShell through a decoupled integration: semantic elements, properties,
spatial relations, IFC import/export, exchange validation; GlobalId,
properties and relations kept through every operation; never a collection of
anonymous meshes. Model ownership decided by ADR first (ADR 0014, open
question 1). The BIM workspace takes ArchiCAD's interface as its reference
(ADR 0015). The **multidisciplinary test** of ADR 0018 D4 runs here: it
decides whether DWG plus a project database suffice or the reserved `.ascad`
format is implemented. Module M03.

## Phase 06 — Topography and surfaces ○

Survey points, triangulation (TIN), contour lines, profiles, sections,
slopes, cut and fill volumes; QGIS bridge for maps. Module M04.

## Phase 07 — Architecture, structures and MEP ○

Specialized modeling, links between disciplines, loads, analysis through
appropriate engines (structural FEM verified against reference cases);
results independently verifiable. Modules M05 and M06; environmental
studies (M07) and transport and infrastructure (M08) follow the same pattern
as independent modules on public APIs.

## Phase 08 — Rendering and animation ○

Blender + Cycles headless, model transfer, PBR materials, lighting, cameras,
image sequences, FFmpeg for video, a render queue with progress and
cancellation. Transversal capability.

## Phase 09 — Coordination and walkthroughs ○

Clash detection, revision comparison, multidisciplinary review,
traceability; interactive real-time navigation on the GPU renderer. Module
M10.

## Phase 10 — Extensions and advanced optimization ○

Plugin API, automation, performance profiles, large models and terrains,
additional libraries, LuxCoreRender evaluated against Cycles on shared
scenes. Module M11.

## Phase 11 — Cross-platform distribution ○

Installers, bundled dependencies, signing and notarization where they apply,
updates, installation docs, tests on Windows (x86-64, ARM64), Linux (x86-64,
ARM64; native packages, AppImage, Flatpak) and macOS (Apple Silicon, Intel);
public releases only when the release criteria of the master plan (§14) are
met.

### Milestone 02.8 sequence

Milestone 02.8 proceeds in independently reviewable steps:

1. **3D mathematical foundation (ADR 0019):** `Point3`, `Vector3`, `Bounds3`
   and `Transform3` in `arq-geometry`, with unit/property tests and precision
   cases near 1e6 model units. This does not add 3D document entities.
2. **Wider 2D entities:** polyline vertex editing, block contents and explode,
   and text support under the existing document and editing contracts. The
   polyline vertex grips and transactional vertex editing (PR #35) and the
   transactional EXPLODE of block references (PR #36) are complete; their
   attribute values become texts (ADR 0023 D1). Texts are complete (ADR 0022: drawn with their letters
   in a built-in stroke font, transformed as a whole, one grip, content,
   height and rotation in Properties, the TEXT command; the drawing's own
   fonts and formatted MTEXT stay in phase 04). Blocks are complete (ADR
   0023). PLINE draws new polylines: straight segments and arcs tangent to
   the segment before, Arc and Line, Undo and Close, one undo step; widths
   and the other arc options come later (maintainer, 2026-10-21). Stage 2
   of the own model (ADR 0018 D2) follows.
3. **3D document model (ADR 0014 question 4):** decided by ADR 0024: a
   solid is our own B-rep; DWG ACIS is read into it and kept byte for byte
   until the solid is edited; Truck is the first temporary backend, after
   its review; the capabilities go read, see, measure, create, edit.
4. **Owned kernel, progressively:** implement and test each geometry capability
   in Rust in `arq-kernel`. A reviewed temporary backend (Truck first; OCCT a
   reference for differential tests) computes, behind an adapter, only what
   the kernel cannot do yet; a capability moves to the kernel after its
   acceptance suite passes (ADR 0020, amended; ADR 0024 D3).
5. **Curves, surfaces and solids, in this order** (ADR 0024 D4; the
   [geometry representations](references/geometry-representations.md)
   reference explains the choices):
   1. read: analytic geometry, then NURBS (both done);
   2. see: tessellation; edges in plan, shading with the GPU viewport;
   3. measure: exact bounds, area, volume, centroid;
   4. write ACIS from our B-rep, to save edited solids;
   5. create: primitives, extrusion and revolution of 2D profiles;
   6. transform solids as a whole;
   7. booleans: Truck first, then the kernel (planar, analytic, NURBS);
   8. fillets, chamfers, shelling, offsets, sweeps and lofts: Truck, then
      the kernel in the long run;
   9. the temporary backend retired when nothing uses it.

   Steps 1 to 6 need no backend.

**Where the 3D commands go:** the ribbon gets a tab of its own, **3D**,
for measuring, creating and editing solids (maintainer, 2026-10-31). It
joins the 2D drafting workspace's tabs (Home, Insert, Annotate, View) as
the first 3D command arrives.

## Modules and phases

| Module | Main phase |
|---|---|
| M01 CAD 2D/3D | 02 |
| M02 DWG/DXF interoperability | 03 |
| M03 BIM/IFC | 05 |
| M04 Topography and civil works | 06 |
| M05 Architecture and structures | 07 |
| M06 Building services (MEP) | 07 |
| M07 Environmental studies | 07 (pattern) |
| M08 Transport and infrastructure | 07 (pattern) |
| M09 Associative technical documentation | 04 |
| M10 Coordination | 09 |
| M11 Automation and extensions | 10 (the Python API exists since 02.3) |

## Old names (ADR 0014 roadmap)

| Before 2026-10-13 | Now |
|---|---|
| Phases 0, 1 | 00, 01 |
| Milestones 2.1–2.5 | 02.1–02.5 |
| 2.6 Spatial index and GPU viewport | 02.7 |
| 2.7 Wider CAD entities | 02.8 |
| 2.8 Projects | Phase 04 (project container) |
| Phase 3 CAD exchange and 2D documentation | 03 (exchange) and 04 (documentation) |
| Phases 4–8 | 05–09 |
| Phase 9 LuxCoreRender, 10 Optimization and distribution | 10, 11 |

## Branches

One branch per feature or fix: `feature/<topic>`, `fix/<topic>` (master
plan §13), a pull request to `main` each. Branches started before
2026-10-13 keep their names (`fase-2.5/edicion`).
