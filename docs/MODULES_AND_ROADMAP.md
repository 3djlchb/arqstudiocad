# ArqStudioCAD — Modules and roadmap

The order of work of the [master plan](MASTER_PLAN.md) (version 2.3,
2026-11-10), and where the project stands. Phases are not dates: a phase
ends when its exit criterion is met and tested, not when its code exists.

Phases 00–04 are **vertical**: the core, in sequence, ending with the first
release (milestone 04.6). Phases 05 on are **horizontal**: modules on the
core, through its APIs, numbered by when they were added, not by a strict
dependency; new ones go at the end. A horizontal phase never changes the
core on its own: what it needs of the core is a milestone of a vertical
phase, or an ADR ([ADR 0035](architecture/adr/0035-phase-structure.md)).

Legend: ✅ done · ◐ in progress · ○ to do.

## Where we are (2026-11-10)

- **Phases reorganized (2026-11-10, [ADR 0035](architecture/adr/0035-phase-structure.md)):**
  vertical phases 00–04 end with the first release (04.6), after
  dimensions, hatches, layouts and plotting to PDF (04.2–04.5); milestone
  02.14 is now the basic 2D commands; Free Geometry, milestone 02.14 until
  then, is phase 05, and the phases after it moved one number up.
- **New horizontal phases (2026-11-10, [ADR 0036](architecture/adr/0036-texture-nodes-materials.md)):**
  13 Texture, 14 Nodes and 15 Materials, each with a Rust core of its own
  on top of the core; their milestones come as the project needs them.
- **Technical MVP: 8 of 8** (master plan §15): milestone 02.6 saves and
  recovers the working document (DWG) from the own internal model.
- **Working format:** DWG, as BricsCAD does, with an own internal model; the
  native format `.ascad` is designed and reserved (ADR 0018).
- **Data protections (03.3):** done (ADR 0017 D5).
- **Milestone 02.7 complete (2026-11-10):** the spatial index and the GPU
  viewport ([ADR 0034](architecture/adr/0034-gpu-viewport-direct-path.md)).
  The reference machine (GT 730 on LMDE 7) passed its release gate
  (ADR 0007): the viewport draws with OpenGL ES on the GT 730 at 16.8 ms a
  frame against the CPU's 44–61 ms, the overlays the same as the CPU's to
  the pixel, and the application chooses the card by itself, with a crash
  sentinel, `--safe-mode` and the CPU as fallback. Windows is validated
  too; macOS waits for a Mac. Shading solids can now build on it.
- **Current:** milestone 02.8: texts (ADR 0022), blocks (ADR 0023: EXPLODE
  with attributes, BLOCK and INSERT, the block editor) and the PLINE
  command are done; dynamic blocks wait for the parametric engine,
  milestone 04.1 (D3a). The 3D document model is decided (ADR 0024) and the
  owned kernel `arq-kernel` reads DWG solids into its B-rep (step 1,
  analytic and NURBS geometry: all 75 of LibreDWG's bodies) and draws
  them by their edges and isolines (step 2, see); step 3, measure, is
  done: the kernel measures them (volume, area, centroid, bounds), the
  document and the Python API a selection, the solids inside block
  references included, and MASSPROP (Spanish PROPFIS) reports it in the
  ribbon's new 3D tab (maintainer, 2026-10-31); moments of inertia and
  the report file come later. Step 4, create, is planned (maintainer,
  2026-11-03): wireframe views from AutoCAD's preset directions first
  (ADR 0030), then the ACIS writer with BOX, the other primitives,
  EXTRUDE and REVOLVE, each in its own pull request. The views are
  done: the model is seen from AutoCAD's ten preset views (-VIEW,
  PLAN), selected in them and picked on their XY plane, and the drawing
  keeps its view, here and in AutoCAD (ADR 0030, accepted). BOX
  (PRISMARECT) makes the first solid, written as ACIS by our own writer
  (4a); WEDGE (CUÑA), CYLINDER (CILINDRO), CONE (CONO), SPHERE (ESFERA)
  and TORUS (TOROIDE) complete AutoCAD's primitives (4b); EXTRUDE
  (EXTRUSION) makes solids of closed polylines and circles (4c); REVOLVE
  (REVOLUCION) turns them about an axis, by any angle (4d). Step 4 is
  complete. Editing solids (step 5) began: MOVE, COPY, ROTATE, SCALE
  and MIRROR take solids as a whole (5a). Booleans (5b) are owned for
  planar solids ([ADR 0032](architecture/adr/0032-owned-planar-booleans.md)):
  Truck, [reviewed](references/truck.md), fails with coplanar faces. The
  kernel unions, intersects and subtracts them, on `arq-geometry`'s own
  2D polygon booleans, coplanar and touching faces included, merging the
  faces left in one plane, and UNION, SUBTRACT and INTERSECT (UNION,
  DIFERENCIA, INTERSEC) combine them in the 3D tab's Solid Editing group,
  checked in ProgeCAD.
- **Milestone 02.14, the basic 2D commands, began (2026-11-10):** RECTANG
  (RECTANG) draws rectangles by two corners, Area, Dimensions and Rotation,
  as AutoCAD; Chamfer and Fillet come next, then Elevation, Thickness and
  Width.
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
  on Windows with Intel UHD 630 / DX12 and, on 2026-11-08, on the reference
  machine (ADR 0007), which it now steers to OpenGL ES on the GT 730 instead
  of software Vulkan. The GPU viewport (ADR 0034) presents straight into the
  viewport's window since 2026-11-10. Milestone 02.5 is complete and manually
  validated by the maintainer, including property editors, shared panel/table
  styling, and CAD color selection; 03.2 is done and 03.4 compatibility work
  is in progress.
- **Milestone 02.8** went on while 02.7 was on hold: the 3D mathematical
  foundation, polyline vertex editing and PLINE, texts, blocks, the 3D
  document decision, reading solids (analytic and NURBS) and drawing their
  edges and isolines in plan are done. What remains, in order (checked
  against the code, 2026-10-31): measuring solids; writing ACIS from the
  B-rep, which saving created or edited solids needs; creating primitives,
  extrusions and revolutions; transforming solids as a whole; booleans,
  through Truck once reviewed; stage 2 of the own model (ADR 0018 D2:
  polylines, block references and definitions; the model owns stage 1
  today); and 3D wireframe entities with Z and user coordinate systems.
  Shading can build on the GPU viewport (02.7, complete since 2026-11-10).
- Before the master plan the phases were numbered 0–10 (ADR 0014) and
  milestones 2.1–2.8, and before ADR 0035 Free Geometry was 02.14 and the
  later phases one number lower; the tables at the end map old names to
  new ones.
´
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
´
| Milestone | Content | Status |
|---|---|---|
| 02.1 Geometry | `arq-geometry`: points, vectors, lines, circles, arcs, transformations, intersections, snaps (ADR 0010) | ✅ PR #5 |
| 02.2 Document | `arq-document`: entities, layers, ids, change recording; `arq-core`: transactions, groups, undo/redo, queries, document store; round-trip corpus (ADR 0011) | ✅ PR #6 |
| 02.3 Python API | `arq-python`: the whole core from Python, typed (ADR 0012) | ✅ PR #7 |
| 02.4 Application shell | Drawing primitives, CPU rasterizer, viewport, events, commands and command window, tabs, background open/save, panels, Ribbon and Classic workspaces, themes (ADR 0013, 0015) | ✅ PR #8 |
| 02.5 Editing workflow | Selection, multifunction grips, object snaps and ORTHO, AutoCAD point syntax, LINE…ERASE, blocks and polylines transformed as a whole, property editors, Spanish command names (ADR 0016) | ✅ steps 2.5.1–2.5.8 complete and manually validated by the maintainer |
| 02.6 Own internal model | Stage 1 of ADR 0018 D2: layers, current layer, LINE, CIRCLE, ARC owned by our model; the DWG engine reads into it and writes from it; save and recover the DWG from the own model; written through to the engine, checked after every transaction in debug builds | ✅ |
| 02.7 Spatial index and GPU viewport | Spatial index (ADR 0021); wgpu renderer with backend probing (ADR 0007); CPU rasterizer as fallback | ✅ Complete (2026-11-10): the reference machine passed its release gate (ADR 0007): OpenGL ES on the GT 730 at 16.8 ms panning and 16.9 ms zooming against the CPU's 44.1 and 60.6, the overlays 0 pixels apart, the GPU tests passing there with `ARQ_GPU_TESTS=require`, the card chosen by itself, `--safe-mode` and the crash sentinel working; X11 and Windows validated, Wayland through XWayland, macOS to try when a Mac is at hand (best effort, not blocking). The way there: R*-tree accelerates bounding-box queries and supplies candidates for picking, window/crossing selection, and object snaps; exact geometry checks remain; rstar 0.14.0 fixes a panic on editing large opened drawings (ADR 0021, 2026-11-08); the offscreen backend probe passes locally on Windows/DX12 and on the reference machine, where it selects OpenGL ES on the GT 730 rather than software Vulkan (2026-11-08); the Qt integration spike works on X11 there, both drawing straight into the widget and offscreen into a `QImage` ([results](testing/wgpu-qt-spike.md), 2026-11-09); Wayland, Windows and macOS untried; an offscreen `GpuRenderer` draws the CPU rasterizer's very segments and is checked against it, but reading the image back costs about 20 ms a frame there, so the viewport takes the direct path, overlays drawn by wgpu (maintainer, 2026-11-09); how, in four pull requests, in [ADR 0034](architecture/adr/0034-gpu-viewport-direct-path.md): the drawing kept on the GPU, the overlays as geometry; D6.1 done (2026-11-10): the viewport presents with the GPU on X11, opt-in with `ARQ_GPU_VIEWPORT`, panning and zooming at 17 ms a frame on the GT 730 against the CPU's 45–63 ms, checked against the CPU rasterizer, which got a fix for lines entering at the top edge; D6.2 done (2026-11-10): the overlays described once and drawn by both paths, the same on screen to the pixel, 20 ms a frame on the GPU against the CPU's 54–71 ms; D6.3 done (2026-11-10): the GPU on by default where a hardware adapter passes the probe, chosen in the background without delaying the start, a crash sentinel, Renderer and antialiasing in Options, `--safe-mode`, the command window told; D6.4 written (2026-11-10): Windows and macOS present through `HWND` and `NSView`, Wayland through XWayland, Automatic keeping the CPU on each until tried on real hardware with `tools/gpu_viewport_check.py`; Wayland tried in a nested weston (the GPU through XWayland, the same on screen as the CPU); Windows tried on the development laptop (2026-11-09): DirectX 12 on the UHD 630 draws as the CPU does at 16.6 ms a frame (60 Hz) against the CPU's 36–45 ms, so Automatic uses the GPU there too; OpenGL on the UHD 630 draws black and Vulkan crashes in its probe, both recorded in ADR 0034; OpenGL on an Intel adapter under Windows passed over since 2026-11-10, no surface setting making it draw, the CPU drawing when nothing else does and the command window and Options saying why; after the milestone, every card of every backend listed and probed, the discrete one tried first and the viewport drawing with the card chosen (2026-11-10, ADR 0034 "After 02.7"): Automatic takes DirectX 12 on the GTX 1050 Ti on the development laptop, and Vulkan on Intel under Windows, which closes the application in a window, is passed over too; Options › Graphics then lists the cards (Automatic, the CPU, each card with its kind) and each card's APIs, surveyed at start in the background and on demand, what cannot be chosen disabled with why (2026-11-10); `tools/gpu_viewport_check.py` now reads the composed screen on Windows and reports blank captures; macOS to try |
| 02.8 Wider entities and 3D | Polyline vertices, block contents and explode, texts; staged 3D geometry foundation and later document model (ADRs 0019 and 0014 question 4) | ◐ 3D mathematical foundation complete (PR #24); polyline vertex grips and transactional vertex edits complete (PR #35); block-reference EXPLODE complete (PR #36; attribute values kept as texts since ADR 0023 D1, PR #41); PLINE (PR #48); texts complete (ADR 0022: drawn with their letters in a built-in stroke font, transformed as a whole, one grip, content, height and rotation in Properties, TEXT and basic MTEXT commands with word wrapping; the drawing's fonts and formatted MTEXT in phase 04); blocks complete (ADR 0023); the 3D document model decided (ADR 0024: own B-rep, ACIS kept, Truck as first temporary backend); step 1, read, done: `arq-kernel` holds the B-rep with analytic and NURBS geometry, and all 75 of LibreDWG's bodies read into it and check; step 2, see, done: solids drawn in plan by their edges and isolines (ISOLINES), in blocks too; step 3, measure, done: the kernel measures volume, area, centroid and bounds by integrating over each face's boundary (maintainer, 2026-10-31), every 3DSOLID of LibreDWG's drawings measured; the document and the Python API measure a selection together, the solids inside block references included (maintainer, 2026-11-01), mirrored ones too; MASSPROP (PROPFIS) reports them with every digit that holds, in the ribbon's own 3D tab (maintainer, 2026-10-31), its Analysis group; moments of inertia and the report file later; step 4, create, planned (maintainer, 2026-11-03): wireframe views first (ADR 0030, accepted 2026-11-03: the ten preset views with -VIEW and PLAN, selection and picked points in them, and the view kept in the drawing; thickness and kept zooms later), then 4a the ACIS writer with BOX (PRISMARECT, done: saved and reread in every DWG version and DXF; ProgeCAD's check next), then 4b the other primitives in three pull requests (maintainer, 2026-11-04: WEDGE, CYLINDER, CONE, SPHERE and TORUS done), 4c EXTRUDE of closed polylines and circles (done), 4d REVOLVE by any angle (done); step 5, editing solids: 5a solids moved, turned, scaled and mirrored as a whole by their ACIS placement (done), 5b booleans owned for planar solids (maintainer, 2026-11-07; [ADR 0032](architecture/adr/0032-owned-planar-booleans.md), proposed): Truck, reviewed, fails with coplanar faces ([review and amendment](references/truck.md)), so the kernel unions, subtracts and intersects planar solids itself, in four pull requests, curved booleans later: the kernel done (`Body::union`, `intersection`, `difference`, on `arq-geometry`'s `Region` booleans), coplanar and touching faces and merged faces included, checked against Truck where it works ([comparison](testing/truck-boolean-comparison.md)); UNION, SUBTRACT and INTERSECT (UNION, DIFERENCIA, INTERSEC) done, in the 3D tab's Solid Editing group and Modify > Solid Editing, the result in the first solid chosen, nothing changed when a solid is curved or nothing would be left (maintainer, 2026-11-07), reopened, measured and audited in ProgeCAD ([evidence](testing/progecad-solid-booleans.md)); step 5b complete; shading can build on 02.7's GPU viewport, complete since 2026-11-10 |
| 02.9 Blocks palette | A dockable palette of the drawing's, a template's and recent blocks, with previews, the dynamic-block tag, insertion by button or drag and drop, and copying definitions between files (ADR 0025; [design brief](design/blocks-palette.md)) | ✅ PR #52 added the Drawing source as a searchable list, BLOCKSPALETTE and Ctrl+3, and synchronized palette toggles in the View tab and menu; step 2b took the mockup's layout (sources, list with each block's references, Selected block section) on new core queries `block_info` and `block_references`; step 2c added the insert options (scale, rotation, on screen) and the Insert, Edit block and New actions through INSERT, BEDIT and BLOCK; step 2d added PROPERTIES and Ctrl+1, and Layers and Properties in the Palettes group; step 3 added previews drawn in Rust, the grid of cards and block definitions in change sets (contract §6); step 4 tagged dynamic blocks with a star, listed their parameters and kept them out of the block editor; step 5 dropped blocks onto the drawing through INSERT and added Explode; step 7a added the core operation that copies a block from another document, with its nested blocks, layers, linetypes and styles, and the clash choices; step 7b added the Template source (load in the background, sample template, empty state, file drop, insert by copying); step 7c added the clash dialog (both blocks previewed, three choices) and BLOCK's check of the template's names; step 6 added the Recent source (this session's and the previous one's blocks, from the file each came from); step 8a made a double click on a block open the Edit Block Definition dialog and a grey block editor; step 8b added the blocks' context menu (Insert with options, Copy to drawing through COPYBLOCK, Redefine from template, confirmed); step 8c added RENAME (alias REN, Spanish CAMBIANOM) for blocks, with its dialog, -RENAME's prompts and the menu's Rename…; step 8d added SELECTBLOCK (Spanish SELECBLOQUE) and the menu's Select in drawing; step 8e added PURGE (alias PU, Spanish LIMPIA) for unused blocks, with its dialog, nested items, -PURGE's prompts and the menu's Purge…, completing ADR 0025 D9; the maintainer accepted the ADR (2026-10-27), independent of the 3D kernel steps |
| 02.10 Block definition dialog | BLOCK's dialog, as AutoCAD's *Block Definition* (main´tainer, 2026-10-25): name with the drawing's blocks, description, base point and objects typed or picked on screen, Retain / Convert to block / Delete the objects, Scale uniformly, Allow exploding, units, and Open in block editor; Annotative shown disabled until annotation scales (phase 04). Order: brought forward, before Blocks palette steps 8c–8e and the model-space context menu and Options (maintainer, 2026-10-26). ADR 0023 D2a records the maintainer's decisions; then two pull requests: the core (Retain and Delete, writing the description and units, reading, writing and honoring the two flags in INSERT and EXPLODE), then the dialog | ✅ ADR 0023 D2a (PR #76); the core: define_block with Retain, Convert and Delete, redefining, the record's description, units and flags, EXPLODE honoring Allow exploding (PR #77); the dialog: BLOCK opens it, -BLOCK keeps the prompts, a name in use is redefined once confirmed |
| 02.11 Block data | Each block definition carries its own data for budget tables (maintainer, 2026-10-26): category, typ´e, material, cost and supplier; the cost has a currency from the drawing's register (maintainer, 2026-10-27). Decisions ([ADR 0027](architecture/adr/0027-block-data.md)): the data is stored inside the DWG, on the block definition (an extension dictionary with an XRECORD), so it travels with the drawing and with a copied block; it belongs to the definition, not to each reference, and a budget's quantity is the number of references (SELECTBLOCK's count); category and type are free text, suggesting the values the drawing already uses; the cost is an exact amount with its currency's decimals; the drawing's register, kept in the DWG, lists its currencies (US dollars by default), several active and one principal, picked from an ISO 4217 catalog or the user's own, so that a budget can later be broken down by currency. The Block Definition dialog shows them in an optional section, folded by default; they are edited later from the Blocks palette (Block data…) and the Properties panel, and the currencies from a Currencies dialog (CURRENCIES), without the block editor, which keeps them when it saves. Attributes stay for what differs between references (a door's code, a room's number). A library shared between drawings, and the budget table itself (quantities × cost), come later; after it, measured quantities for budget line items (perimeter, area, height, volume, weight, each in its unit, the measure each block is priced by chosen and changeable), taken from each reference, so a dynamic block's quantities follow its parameters with the parametric engine (04.1) (maintainer, 2026-10-28; ADR 0027 D7). Order: after Blocks palette step 8e; an ADR first, then the core, the Currencies dialog, the Budget data section with BLOCKDATA and the palette, and the Properties panel | ✅ ADR 0027, accepted 2026-10-29; step 2, the core: block data and the currency register kept in the DWG and checked on save, BLOCKDATA and CURRENCIES as undo steps, copied, renamed and purged with their block, and the Python API; step 3: CURRENCIES (Spanish MONEDAS) and its Currencies dialog, with the ISO 4217 catalog and the number format (thousands and decimal separators, D3a), general for the drawing and, if wanted, its own for each currency; step 4a: BLOCKDATA (Spanish DATOSBLOQUE) and -BLOCKDATA with the Block Data dialog, and the Blocks palette's Block data… and Budget data section; step 4b: the folded Budget data section of the Block Definition dialog, written with the block in one undo step; clearing a block's data at once: Clear data in the Block Data dialog and Clear block data in the palette's menu, confirmed first; step 5: the Properties panel's Block data group, edited in place for references of one block, completing ADR 0027 D8 |
| 02.12 Budget table | The budget from the drawing (maintainer, 2026-10-29; [ADR 0028](architecture/adr/0028-budget-table.md)): a dockable Budget panel and the BUDGET command (Spanish PRESUPUESTO), grouped by category with subtotals; each block's quantity is its references in model space, nested ones multiplied by the block holding them, layouts excluded; blocks without a cost listed apart, never summed; totals per currency, never mixed; the whole drawing or the selection only; export to CSV and to Excel (.xlsx, through openpyxl, MIT). Order: the ADR, the core's quantities, the panel, the export | ✅ ADR 0028, accepted 2026-10-29 |
| 02.13 Context menu and Options | The maintainer's design briefs (2026-10-22; decisions 2026-10-29, [ADR 0029](architecture/adr/0029-context-menu-and-options.md)): a right-click on model space with nothing selected opens Repeat {command} and Options…, and acts as Enter during a command; the Options dialog (OPTIONS, OP, Spanish OPCIONES), modal, showing only settings whose feature exists: language and theme, a crosshair drawn by the viewport with a live preview, the block editor's background, wheel zoom, the right-click setting. Order: the ADR, the menu, the dialog, the crosshair, zoom | ✅ ADR 0029, accepted 2026-10-31: the menu, the dialog with General, User preferences (Right-click, Zoom), `OptionsChanged`, Ctrl+Tab, the crosshair drawn by the viewport and Options › Display |
| 02.14 Basic 2D commands | So that phase 02 meets its exit criterion (create, edit, save) ([ADR 0035](architecture/adr/0035-phase-structure.md) D4, maintainer, 2026-11-10): the 2D commands a drawing needs that the code lacks (checked 2026-11-10), each with its Spanish name (ADR 0016), in the order architecture uses them: 1. RECTANG, OFFSET, TRIM, EXTEND (walls and openings); 2. FILLET, CHAMFER (corners); 3. STRETCH as a command (today only a grip mode), ARRAY, BREAK, JOIN (editing in bulk); 4. MATCHPROP; 5. ELLIPSE, SPLINE (new curves, new entities in the own model). Until 2026-11-10, 02.14 was Free Geometry, now phase 05 | ◐ RECTANG (RECTANG) done (2026-11-10): by two corners, Area (with the length or the width), Dimensions (the point picks the quarter) and Rotation (typed or by two points), previewed under the cursor, one closed polyline, the rotation, sizes and area remembered for the session as AutoCAD; its first prompt's options in two later pull requests (maintainer, 2026-11-10): Chamfer and Fillet next, then Elevation, Thickness and Width with the polyline's own elevation, thickness and width |
| 02.15 Owned booleans | UNION, SUBTRACT and INTERSECT of every kind of solid by the owned kernel, no external kernel (maintainer, 2026-11-07; [ADR 0033](architecture/adr/0033-owned-booleans-milestone.md), proposed): in the order architecture needs, planar solids, planes with cylinders and cones, spheres and tori, curved with curved, NURBS; intersection curves exact where conics, fitted NURBS between curved surfaces; each delivery with exact architecture cases, volume identities, Truck where it works and ProgeCAD | ◐ delivery 1, planar solids, done (ADR 0032: #133, #137, #139, the codec's DWG 2000 voids #138); delivery 2, planes with cylinders and cones, in four pull requests: the 2D booleans with curved edges done (`CurvedRegion`: segments, ellipse arcs and the graphs a plane leaves on a cylinder or cone, kept exact), planes with cylinders in general position done (round holes, columns through slabs, ellipses where boxes cut columns askew), 2.3 in three (maintainer, 2026-11-07): flush columns and holes, columns standing on slabs and merged faces done (2.3a), cones done (2.3b, 2026-11-10: with flat-faced solids, pointed or cut short, flush too; ellipses exact, parabolas and hyperbolas as rational quadratic NURBS edges written to ACIS), cylinders on parallel axes (2.3c) next |

## Phase 03 — DWG/DXF ◐

Exit: a documented compatibility matrix and no known silent losses in the
cases covered. Module M02. Decision: [ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md);
detail: [DWG/DXF interoperability](DWG_DXF_INTEROPERABILITY.md).

| Milestone | Content | Status |
|---|---|---|
| 03.1 Engine on AcadRust | AcadRust 0.6.4 from crates.io behind `arq-document`; corpus and tests green | ✅ (since 2026-10-28 AcadRust 0.6.8 plus post-release fixes, patched from our fork: ADR 0017's log) |
| 03.2 opencadcodec's improvements | One by one with regression drawings: geometric constraints first, then fidelity fixes and diagnostics; kept in our AcadRust fork | ✅ every measured difference checked and the right ones brought in (R2004 lofted surfaces); the fork's wrong readings replaced by keeping objects verbatim; R11 linetypes, TOLERANCE in DXF and our own RAY and ACIS checks fixed; LibreDWG regression reads and round trips covered; same-version R2000 TABLESTYLE-owner preservation is corrected in fork bd369d9, with genuine-downgrade and other loss guards retained ([evidence](testing/r2000-table-style-preservation.md)) |
| 03.3 Data protections | Damaged files reported, safe save with read-back validation, no silent loss on version conversion, regression drawings | ✅ damaged files refused or recovered and marked; saves read back and checked; losses asked before saving; regression drawings made by the tests |
| 03.4 Compatibility matrix | By version, entity and operation; dynamic blocks, constraints, ACIS, Xrefs, tables, dimensions, hatches, custom BIM objects | ◐ reproducible integrated-core read/safe-save report (`tools/compatibility_matrix.py`); initial 30-file run: DWG 27 verified / 3 refused, DXF 16 verified / 14 refused; same-version R14/R2000 MULTILEADERSTYLE retention is corrected in fork c8f4b05 and verified independently in ProgeCAD ([evidence](testing/legacy-mleader-preservation.md)); new DWG 2004 drawings, refused by ProgeCAD 2026 for lack of the MLEADERSTYLE version EED AutoCAD writes before R2010, are corrected in fork b6e2a2a (on top of the DATATABLE fix bab8ac0) and reopen with AUDIT clean ([evidence](testing/progecad-dwg2004-mleader-version.md)); the default dimension style a read drawing lacks now names that drawing's text style (fork 6e03fb1), so saves built on `example_2000.dwg` audit clean ([evidence](testing/progecad-r2000-dimtxsty.md)); SAT 400 lumps and shells keep their next pointers (fork f10cc74), so a solid's void and a body's second lump survive DWG 2000, measured in ProgeCAD ([evidence](testing/progecad-acis400-topology.md)); same-version R2000 TABLESTYLE-owner loss is corrected in fork bd369d9, with strict style/companion verification retained ([evidence](testing/r2000-table-style-preservation.md)); synthetic Xref corpus checks path, flags and INSERT transformations through DWG/DXF; filename markers represent Xref INSERTs; fork 59aaf4c preserves the explicit DXF unloaded marker verified in native ProgeCAD exports and reopening; all six local Xref drawings and 12 strict DWG/DXF saves pass, with the reference-level save guard retained ([investigation](testing/progecad-dxf-unloaded.md), [baseline results](testing/progecad-xrefs-results.md)); native table corpus measures three ProgeCAD inputs and 249 properties per reopening; fork 0d438b6 corrects the measured DXF-to-DWG margins, row text heights and right-border loss, with 0/249 differences against native DWG reopening ([correction](testing/progecad-table-instance-formatting.md)); the subsequent fork 179aaeb corrects AC1021 native border-table reading and strict rewriting, also matching 249/249 native measurements ([reader correction](testing/progecad-table-2007-reader.md)); fork f1b0980 corrects eight DWG-to-DXF margins, twelve row-type text heights and eight merged-title values, reducing native differences from 88 to 60; the subsequent fork 5c09a39 corrects inherited heights and shared borders; remaining raw differences are measured against a fresh native DXF control ([cell fidelity](testing/progecad-table-cell-fidelity.md)); other format limits remain open ([baseline](testing/progecad-tables-results.md)); reference resolution, broader version/operation coverage, visual/semantic fidelity and external validation remain open ([measurements](DWG_DXF_INTEROPERABILITY.md#integrated-product-matrix-034)) |
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
The cell-fidelity correction is merged in PR #105. The subsequent
[R2000 style-preservation correction](testing/r2000-table-style-preservation.md)
is published in fork `bd369d9`; its full default suite, 44 focused/native tests,
940-job comparison and native ProgeCAD style observations pass. Git-pinned
product acceptance passes on main base `6962ae5`: 488 Rust and 808 Python tests, six external
core corpus groups and eight optional document tests. The 34-drawing matrix now
has 59 verified saves, nine declared-loss blocks and no semantic save errors;
only the two corrected R2000 DWG rows change. Product adoption is merged in
PR #108. The subsequent [loss triage](testing/compatibility-loss-triage.md) on
main `fbf5b58` reproduces all 68 normal safe-save outcomes and classifies the
nine remaining blocks: four dynamic-block DXF omissions, two opaque DATATABLE
DXF losses, two legacy MULTILEADERSTYLE DWG losses and one Surface associative
DXF loss. No intrinsic destination-format impossibility is established; guards
remain unchanged. The subsequent [MULTILEADERSTYLE correction](testing/legacy-mleader-preservation.md)
in fork `c8f4b05` preserves existing class-bearing R14/R2000 dictionaries and
styles on same-version saves. ProgeCAD confirms both complete style property
lists and their identities in each version; the previous codec recreates
only one default style. The published Git-pin matrix improves to 61 verified
saves and seven declared-loss blocks, with zero semantic save errors. Acceptance
on updated main `226a2fb` passes 524 Rust and 812 Python tests, seven public
core groups, both native corpus groups and eight optional document tests.
Adoption awaits PR review and merge; production loss guards and
genuine-downgrade policy are unchanged.
The next [DATATABLE investigation](testing/datatable-dxf-preservation.md)
reproduces unsupported hard-owned cells and the legacy DXF subclass mismatch;
its isolated codec candidate is not adopted. A separate native R14 `Invalid
ResBuf` rejection, reproduced with the published codec, is traced to an
incorrect R2000 XRECORD cloning prefix in R14 DXF. Fork `bab8ac0`
corrects versioned reader/writer handling after red tests; ProgeCAD reopens
the actual corrected R14 DXF and matches 2,724 table/child properties. Fresh
R2000 DWG/DXF and R14 DWG controls also match their native references. The
revised codec suite passes 2,448 active tests and the three focused ownership
cases, and a repeated 944-job paired corpus has no status or summary
regressions. The authorized immutable tag
`arqstudiocad/on-ea66abe-datatable-dxf` is published and remotely verified;
published-pin product acceptance on base `3007dd8` passes 616 Rust and 864
Python tests, nine public core groups, both native corpus groups and nine
optional document tests. The same 34-input integrated matrix has 63 verified
saves, five loss blocks and no semantic errors; only the two intended DXF rows
change. All four strict product outputs match the independently observed
artifacts byte for byte. Product adoption awaits PR review and merge.
Milestones 03.4 and 03.5 remain in progress.

## Phase 04 — Associative technical documentation ○

Dimensions, texts and fonts, annotations, hatches, views, scales, sheets,
title blocks, drawing templates (title blocks, sheets, scales and styles;
NEW starting from a template), plotting to PDF; projections, sections and elevations as
**associative views** that know their source and regenerate keeping manual
annotations (ADR 0014); the **project container** (models, drawings, views,
sheets, resources; an ADR first). Module M09.

It opens with the engine its associativity stands on *(maintainer,
2026-10-21)*, and ends with the first release, which closes the vertical
phases *(maintainer, 2026-11-10)*:

| Milestone | Content | Status |
|---|---|---|
| 04.1 Parametric engine | ArqStudioCAD's own engine (ADR 0017 D6; an ADR first): a dependency graph that regenerates what depends on what changed; 2D geometric and dimensional constraints, solved and kept through DWG; dynamic blocks recomputed when a parameter changes (visibility, stretch, flip, lookup…), so the block editor accepts them (ADR 0023 D3a). Associative dimensions and views are built on it; BIM objects (phases 06 and 08) reuse it | ○ |
| 04.2 Dimensions | Dimensions created and edited, associative on the parametric engine; dimension styles kept through DWG | ○ |
| 04.3 Hatches | Hatches created and edited, with their patterns drawn and kept through DWG | ○ |
| 04.4 Layouts | Paper space: layouts, viewports, title blocks and scales | ○ |
| 04.5 Plotting to PDF | Layouts and model space plotted to PDF | ○ |
| 04.6 First release | The end of the vertical phases ([ADR 0035](architecture/adr/0035-phase-structure.md) D3, maintainer, 2026-11-10). Exit, beyond master plan §14: OpenGL drawing black on the Intel UHD 630 fixed, or excluded with the CPU as fallback (ADR 0034 D6.4; met 2026-11-10: passed over, the CPU as fallback); the curved booleans of 02.15 done, or refused with a clear message; `docs/KNOWN_LIMITATIONS.md` (or its equivalent), a CHANGELOG and a short guide to installing and using it; the installer tried on a clean machine, the reference floor machine included, with `--check`; a trademark note in both READMEs; a legal review of the name before it is publicized. The official release stays subject to the maintainer's decision that the application is mature (ADR 0014 D6) | ○ |

## Phase 05 — Free Geometry ○

The first horizontal phase ([ADR 0035](architecture/adr/0035-phase-structure.md)):
a workspace of its own for organic forms that CAD and BIM's strict volumes
make hard (maintainer, 2026-11-04; [ADR 0031](architecture/adr/0031-free-geometry.md),
proposed). Three ways of modeling, mixed by object in one drawing, with a
mode filter for who wants one (maintainer, 2026-11-05): flat faces pushed
and pulled after SketchUp and [IngeTrazo](references/ingetrazo.md), exact
solids at once; subdivision meshes (Catmull-Clark, with creases) saved as
AutoCAD's MESH and made exact solids on request; NURBS surfaces as in
Rhino, saved as ACIS surfaces and solids. Meshes and NURBS are edited in an
environment entered on the object; its tabs show only in its workspace,
its commands are typed everywhere. Module M01. Until 2026-11-10 it was
milestone 02.14.

Exit: organic forms modeled in the three ways and mixed in one drawing,
saved in DWG (MESH, ACIS surfaces and solids) and reopened, measured and
combined in this application, in AutoCAD and in ProgeCAD; what a free
form makes a solid is measured, saved and combined as any solid.

Milestones 05.1… are defined with the maintainer. ADR 0031 D13 gives the
order to start from: shading on the CPU; picking in 3D and a working plane;
the workspace with its mode filter and flat faces; MESH and the subdivision
environment; NURBS in the ACIS writer and a mesh made a solid; NURBS
modeling. By ADR 0035 D2, what of this the core needs (shading, picking in
3D, the working plane, the MESH entity, NURBS surfaces in the writer) is
asked for as milestones of the vertical phases, not built here.

## Phase 06 — BIM and IFC ○

IfcOpenShell through a decoupled integration: semantic elements, properties,
spatial relations, IFC import/export, exchange validation; GlobalId,
properties and relations kept through every operation; never a collection of
anonymous meshes. Model ownership decided by ADR first (ADR 0014, open
question 1). The BIM workspace takes ArchiCAD's interface as its reference
(ADR 0015). The **multidisciplinary test** of ADR 0018 D4 runs here: it
decides whether DWG plus a project database suffice or the reserved `.ascad`
format is implemented. Module M03.

## Phase 07 — Topography and surfaces ○

Survey points, triangulation (TIN), contour lines, profiles, sections,
slopes, cut and fill volumes; QGIS bridge for maps. Module M04.

## Phase 08 — Architecture, structures and MEP ○

Specialized modeling, links between disciplines, loads, analysis through
appropriate engines (structural FEM verified against reference cases);
results independently verifiable. Modules M05 and M06; environmental
studies (M07) and transport and infrastructure (M08) follow the same pattern
as independent modules on public APIs.

## Phase 09 — Rendering and animation ○

Blender + Cycles headless, model transfer, PBR materials, lighting, cameras,
image sequences, FFmpeg for video, a render queue with progress and
cancellation. Transversal capability.

## Phase 10 — Coordination and walkthroughs ○

Clash detection, revision comparison, multidisciplinary review,
traceability; interactive real-time navigation on the GPU renderer. Module
M10.

## Phase 11 — Extensions and advanced optimization ○

Plugin API, automation, performance profiles, large models and terrains,
additional libraries, LuxCoreRender evaluated against Cycles on shared
scenes. Module M11.

Automation-language direction recorded by the maintainer on 2026-11-01:
Python remains the primary public automation API; ArqLISP is the first
planned compatibility language, followed by evaluation of ArqBasic.
See [master plan section 9.1](MASTER_PLAN.md#91-planned-automation-languages-m11-phase-11).
The milestones below are planned, with no implementation started. They do
not bring phase 11 ahead of the current work.

| Milestone | Content and acceptance | Status |
|---|---|---|
| 11.1 Shared automation and plugin contract | Build on the existing typed Python core API (02.3) and application command/session/event machinery. Define supported extension entry points, loading and lifecycle, API versioning and compatibility tests, errors, permissions/trust, execution limits and cancellation. An ADR precedes interpreter/runtime selection; all adapters preserve Rust document ownership, transactions, undo/redo, event delivery and safe-save guards. | ○ planned; the current core API exists, but a complete public plugin/scripting environment is not implemented |
| 11.2 ArqLISP initial source compatibility | Independently implement a documented subset of Lisp and AutoLISP `.lsp` routines through 11.1: language basics, command registration/execution, selection and supported entity queries/edits. Publish a supported/unsupported function matrix and representative routine tests, including invalid input, failed-operation rollback, cancellation and GUI event synchronization. Select a runtime only through the master plan section 16 review and the ADR. | ○ planned, after 11.1; no interpreter selected or implemented |
| 11.3 ArqLISP compatibility expansion | Expand only against identified routine needs and regression tests. Evaluate DCL, Visual LISP/ActiveX and compiled `.fas`/`.vlx` support separately, with platform and licensing review; initial `.lsp` support does not imply those capabilities. | ○ planned, after 11.2; advanced compatibility scope undecided |
| 11.4 ArqBasic feasibility | After the ArqLISP foundation, evaluate an independent BASIC dialect and a documented VBA-compatible subset using representative source macros. Assess language semantics, the CAD host-object model, forms, COM and `.dvb` projects separately. Record a go/no-go decision, licensing, platform limits and acceptance tests in an ADR before any implementation commitment. | ○ evaluation planned; neither Microsoft VBA integration nor a complete VBA clone is approved |

**Portability gate for 11.1-11.4:** ArqLISP and any approved ArqBasic
implementation must keep their language core and common CAD API portable
across Windows x86-64/ARM64, macOS Intel/Apple Silicon (universal2), and
Linux x86-64; Linux ARM64 retains ADR 0003's feasibility condition. Select
runtimes and dependencies accordingly. Run common source-routine tests on
each supported OS/architecture, with equivalent results within documented
numeric tolerances, and verify the packaged runtime/native dependencies.
CI must gain missing target coverage before support is claimed; existing
application builds are not language acceptance evidence.

Keep COM/ActiveX, OCX, Windows API and platform-specific DLL integrations as
optional extensions. Provide capability queries, document platform limits,
and reject unsupported calls clearly before their operation mutates the
document. Test both capability-present and capability-absent paths. ArqBasic
feasibility includes this gate rather than assuming VBA dependencies are
portable. See master plan section 9.1 for the binding design criterion.

Future language implementation code is GPL-3.0-or-later, with third-party
provenance and compatible licenses checked before reuse. The existing
Python API and these future adapters share one application/core contract;
they do not create separate document models or mutation paths. The plan
approves future work and evaluation, not proprietary runtime redistribution
or a claim of full AutoLISP/VBA compatibility.

## Phase 12 — Cross-platform distribution ○

Installers, bundled dependencies, signing and notarization where they apply,
updates, installation docs, tests on Windows (x86-64, ARM64), Linux (x86-64,
ARM64; native packages, AppImage, Flatpak) and macOS (Apple Silicon, Intel);
public releases only when the release criteria of the master plan (§14) are
met.

## Phase 13 — Texture ○

Texturing the volumes the application models, milestone by milestone as
the need comes (maintainer, 2026-11-10; [ADR 0036](architecture/adr/0036-texture-nodes-materials.md),
proposed): the image maps that feed materials' channels, mapped by
projections and texture coordinates, painted on the model, generated
procedurally and, with phase 14, authored by node graphs. Its own core,
the `arq-texture` crate, on top of the core and never inside it, designed
so that a separate node-based texturing application of our own, in the
manner of ArmorPaint and Substance Designer, can be built on it later
(where and under what name, decided by its own ADR). References: the
textures of [IngeTrazo](references/ingetrazo.md) and ArmorPaint. Module
M12.

Exit: the volumes the application makes, textured: maps painted, generated
or loaded, mapped onto them and feeding their materials, kept with the
drawing, reopened, and seen in the viewport and in the render.

Milestones 13.1… are defined with the maintainer. Order to start from: a
study of the references; texture coordinates on faces and meshes (a
vertical milestone, ADR 0035 D2); image maps through materials (phase 15);
projections and unwrapping; painting on the model; procedural maps;
node-based authoring with phase 14.

## Phase 14 — Nodes ○

A node system of our own after Blender's Geometry Nodes and the node graph
[Open CAD Studio](references/opencadstudio.md) is building (opencadgraph,
dataflow over objects' properties), its geometry our own kernel's
(maintainer, 2026-11-10; [ADR 0036](architecture/adr/0036-texture-nodes-materials.md),
proposed): typed values flowing through nodes, evaluated and cached, with
no external geometry engine; what a graph makes is ordinary geometry,
measured, saved and combined like the rest. Its own core, the `arq-nodes`
crate, on top of the core. The base of every later horizontal phase that
needs nodes, Texture's node-based authoring among them. Module M13.

Exit: geometry built and changed by node graphs evaluated on the owned
kernel, the graphs kept with the drawing and evaluated again when it is
reopened, their results usable as any solid or mesh, and the system used
by at least one other phase.

Milestones 14.1… are defined with the maintainer. Order to start from: a
study of Geometry Nodes and opencadgraph; the graph (nodes, typed sockets,
fields) and its evaluation and caching; basic geometry nodes (primitives, transforms,
booleans, instances); graphs kept with the drawing (a vertical milestone if
the document must change, ADR 0035 D2); the node editor; then its use by
other phases.

## Phase 15 — Materials ○

Physically based (PBR) materials for what the application models
(maintainer, 2026-11-10; [ADR 0036](architecture/adr/0036-texture-nodes-materials.md),
proposed): base color, roughness, metalness, normal, opacity and emission,
in a library, assigned to objects, layers and faces, kept with the drawing
and carried to the viewport, the render (phase 09) and IFC (phase 06);
physical properties for BIM (density, cost) join them when BIM arrives.
Textures, the image maps that feed their channels, are phase 13. Its own
core, the `arq-material` crate, on top of the core. Module M14.

Exit: a library of PBR materials, assigned to objects, layers and faces,
kept in the DWG (AutoCAD's materials where they apply) and reopened, seen
in the viewport, and carried to the render and to IFC.

Milestones 15.1… are defined with the maintainer. Order to start from: a
study of the references (IngeTrazo's materials, glTF 2.0's metallic-
roughness model, AutoCAD's materials in the DWG); the material model and
its library; assigning materials in the document and keeping them in the
DWG (vertical milestones, ADR 0035 D2); shading with materials in the
viewport; then the render and IFC.

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
   5. create: primitives, extrusion and revolution of 2D profiles, seen
      first in wireframe views from AutoCAD's preset directions (ADR 0030);
   6. transform solids as a whole;
   7. booleans, owned: planar done (ADR 0032), then curved solids in
      milestone 02.15 (ADR 0033), no backend;
   8. fillets, chamfers, shelling, offsets, sweeps and lofts: Truck, then
      the kernel in the long run;
   9. the temporary backend retired when nothing uses it.

   Steps 1 to 6 need no backend.

**Where the 3D commands go:** the ribbon gets a tab of its own, **3D**,
for measuring, creating and editing solids (maintainer, 2026-10-31). It
joined the 2D drafting workspace's tabs (Home, Insert, Annotate, 3D, View)
with MASSPROP, the first 3D command, in its Analysis group.

## Modules and phases

| Module | Main phase |
|---|---|
| M01 CAD 2D/3D | 02 (core); 05 Free Geometry |
| M02 DWG/DXF interoperability | 03 |
| M03 BIM/IFC | 06 |
| M04 Topography and civil works | 07 |
| M05 Architecture and structures | 08 |
| M06 Building services (MEP) | 08 |
| M07 Environmental studies | 08 (pattern) |
| M08 Transport and infrastructure | 08 (pattern) |
| M09 Associative technical documentation | 04 |
| M10 Coordination | 10 |
| M11 Automation and extensions | 11 (the Python API exists since 02.3) |
| M12 Texture | 13 |
| M13 Nodes | 14 |
| M14 Materials | 15 |

## Old names

Before master plan 2.0, the ADR 0014 roadmap; the right-hand numbers are
those of 2026-10-13, which the next table updates:

| Before 2026-10-13 | Then |
|---|---|
| Phases 0, 1 | 00, 01 |
| Milestones 2.1–2.5 | 02.1–02.5 |
| 2.6 Spatial index and GPU viewport | 02.7 |
| 2.7 Wider CAD entities | 02.8 |
| 2.8 Projects | Phase 04 (project container) |
| Phase 3 CAD exchange and 2D documentation | 03 (exchange) and 04 (documentation) |
| Phases 4–8 | 05–09 |
| Phase 9 LuxCoreRender, 10 Optimization and distribution | 10, 11 |

Before [ADR 0035](architecture/adr/0035-phase-structure.md) (2026-11-10):

| Before 2026-11-10 | Now |
|---|---|
| 02.14 Free Geometry | Phase 05 Free Geometry (02.14 is now the basic 2D commands) |
| Phase 05 BIM and IFC | 06 |
| Phase 06 Topography and surfaces | 07 |
| Phase 07 Architecture, structures and MEP | 08 |
| Phase 08 Rendering and animation | 09 |
| Phase 09 Coordination and walkthroughs | 10 |
| Phase 10 Extensions and advanced optimization; milestones 10.1–10.4 (ArqLISP, ArqBasic) | 11; 11.1–11.4 |
| Phase 11 Cross-platform distribution | 12 |

## Branches

One branch per feature or fix: `feature/<topic>`, `fix/<topic>` (master
plan §13), a pull request to `main` each. Branches started before
2026-10-13 keep their names (`fase-2.5/edicion`).
