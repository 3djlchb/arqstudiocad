# Architecture Decision Records

One file per decision, numbered, never rewritten once accepted: a decision
that changes gets a new ADR that supersedes the old one.

| # | Decision | Status |
|---|---|---|
| [0001](0001-technology-stack.md) | Rust core + Python application + PySide6 UI, bound with PyO3/maturin | Accepted |
| [0002](0002-dwg-native-format.md) | DWG as native format, DXF as alternative | Accepted, amended by 0008; kept by 0018 (DWG is the working format) |
| [0003](0003-target-platforms.md) | Target platforms and macOS universal2 | Accepted |
| [0004](0004-naming-and-licensing.md) | Names, Python 3.12+, GPL-3.0-or-later | Accepted |
| [0005](0005-development-conventions.md) | Toolchain pinning, quality tools, git workflow | Accepted, CI trigger amended by 0014 |
| [0006](0006-i18n.md) | English source, translation packs (IngeCAD model) | Accepted |
| [0007](0007-graphics-api.md) | One wgpu renderer, backend chosen at runtime; GT 730 as low-end reference | Accepted (spike in phase 7), amended by 0013 |
| [0008](0008-opencadcodec.md) | opencadcodec (MPL-2.0) as the DWG/DXF engine | Accepted, engine amended by 0017 |
| [0009](0009-packaging.md) | PyInstaller onedir, Inno Setup (+ MSI later), app ID, platform floor | Accepted |
| [0010](0010-geometry-conventions.md) | Geometry: own validated types, radians in core / degrees in UI, DWG arcs, tolerance, local frames | Accepted |
| [0011](0011-document-model-and-transactions.md) | Document over opencadcodec, 2D in plan, before/after-image undo, groups, change sets, round-trip corpus | Accepted |
| [0012](0012-python-api.md) | Python API: tuples for points (any 2-sequence in, NumPy-readable arrays for bulk), immutable value classes, document handles, exception hierarchy, groups as context managers, GIL released for open/save | Accepted |
| [0013](0013-application-shell.md) | Application shell: Classic and Ribbon workspaces over one action set, render representation and CPU rasterizer in Rust (arq-render), command window, events, panels, tabbed documents; guaranteed floor (reference machine) scaling up by runtime detection (amends 0007) | Accepted |
| [0014](0014-platform-direction.md) | Platform direction: CAD/BIM multimodeler (charter, roadmap 0–10), no restart, names kept, docs CC BY-SA 4.0, external render engines, open questions | Accepted, question 3 settled by 0015, question 4 by 0024 |
| [0015](0015-workspaces.md) | Workspaces: a discipline (declared as data in Python) shown in a layout (Ribbon for all, foldable and the default; Classic only for 2D drafting); chosen separately, layout state saved per workspace | Accepted |
| [0016](0016-editing-workflow.md) | Editing workflow: AutoCAD selection (window/crossing), picking and snaps in Rust, running object snaps + ORTHO, AutoCAD point syntax, multifunction grips, first draw/edit commands, blocks and polylines transformed as a whole, property editors, Spanish command names | Accepted |
| [0017](0017-dwg-dxf-engine-strategy.md) | DWG/DXF engine: AcadRust (crates.io) as the base, opencadcodec improvements recovered one by one with regression drawings, LibreDWG out of the product, codec behind our read/write/capabilities/round-trip contract, four mandatory data protections | Accepted, interoperability validation pending |
| [0018](0018-native-format.md) | Working format: DWG with an own internal model (BricsCAD approach); IFC for BIM exchange; a multidisciplinary test decides whether a native format is needed; native `.ascad` (versioned ZIP, own model, preserved DWG) designed and reserved | Accepted |
| [0019](0019-geometry-3d-foundations.md) | Dependency-free 3D value types in `arq-geometry`, right-handed Z-up coordinates, local-frame affine transforms; document ownership and solid kernel remain open | Accepted |
| [0020](0020-progressive-own-geometry-kernel.md) | Build an owned Rust geometry kernel incrementally; reviewed references (OCCT, Truck…) as optional temporary backends behind an adapter, migrating by capability after parity tests; openNURBS a reference (amended 2026-10-21) | Accepted |
| [0021](0021-spatial-index.md) | Use an internal R*-tree for bounding-box candidates behind the existing core query API; synchronize it from committed changes and undo/redo | Accepted |
| [0022](0022-text-entities.md) | TEXT and MTEXT in 02.8: transform as a whole, one grip, content/height/rotation in Properties, TEXT command; mirrored texts stay readable; letters drawn in a built-in stroke font; basic MTEXT command with word wrapping; the drawing's own fonts and formatted MTEXT in phase 04 | Accepted |
| [0023](0023-blocks.md) | Blocks in 02.8: EXPLODE keeps attribute values as texts; BLOCK and INSERT; a BEDIT-like block editor in its own tab; an Insert ribbon tab | Accepted |
| [0024](0024-3d-document-model.md) | The 3D document model: a solid is our own B-rep; DWG ACIS read into it and kept byte for byte until edited; Truck the first temporary backend after review; read, see, measure, then create and edit | Accepted |
| [0025](0025-blocks-palette.md) | The Blocks palette: Drawing, Template and Recent sources; previews in Rust from new change-set block fields; dynamic blocks tagged; inserting by button or drop through INSERT; copying definitions with clashes asked about; BLOCKSPALETTE, PROPERTIES, Ctrl+1 and Ctrl+3 | Accepted |
| [0026](0026-external-reference-loading.md) | External reference inventory, bounded path resolution, read-only child snapshots, reference save guards and reload lifecycle; implementation remains phase 04 or later | Proposed |
| [0027](0027-block-data.md) | Block data for budgets: category, type, material, supplier and a cost on each block definition, in an XRECORD; the cost in a currency from the drawing's register (several active, one principal, ISO 4217 catalog or the user's own); BLOCKDATA and CURRENCIES | Accepted |
| [0028](0028-budget-table.md) | The budget table: a dockable Budget panel grouped by category, model-space quantities with nested references multiplied, blocks without cost apart, totals per currency, export to CSV and Excel (openpyxl) | Accepted |
| [0029](0029-context-menu-and-options.md) | The model-space context menu (Repeat, Options…; right-click acts as Enter during a command) and the Options dialog: only working settings shown, user settings in QSettings, a crosshair drawn by the viewport | Proposed |

Template:

```markdown
# NNNN. Title

Date: YYYY-MM-DD · Status: Proposed | Accepted | Superseded by NNNN

## Context
## Decision
## Consequences
```
