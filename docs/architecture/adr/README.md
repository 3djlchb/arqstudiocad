# Architecture Decision Records

One file per decision, numbered, never rewritten once accepted: a decision
that changes gets a new ADR that supersedes the old one.

| # | Decision | Status |
|---|---|---|
| [0001](0001-technology-stack.md) | Rust core + Python application + PySide6 UI, bound with PyO3/maturin | Accepted |
| [0002](0002-dwg-native-format.md) | DWG as native format, DXF as alternative | Accepted, amended by 0008 |
| [0003](0003-target-platforms.md) | Target platforms and macOS universal2 | Accepted |
| [0004](0004-naming-and-licensing.md) | Names, Python 3.12+, GPL-3.0-or-later | Accepted |
| [0005](0005-development-conventions.md) | Toolchain pinning, quality tools, git workflow | Accepted |
| [0006](0006-i18n.md) | English source, translation packs (IngeCAD model) | Accepted |
| [0007](0007-graphics-api.md) | One wgpu renderer, backend chosen at runtime; GT 730 as low-end reference | Accepted (spike in phase 7), amended by 0013 |
| [0008](0008-opencadcodec.md) | opencadcodec (MPL-2.0) as the DWG/DXF engine | Accepted |
| [0009](0009-packaging.md) | PyInstaller onedir, Inno Setup (+ MSI later), app ID, platform floor | Accepted |
| [0010](0010-geometry-conventions.md) | Geometry: own validated types, radians in core / degrees in UI, DWG arcs, tolerance, local frames | Accepted |
| [0011](0011-document-model-and-transactions.md) | Document over opencadcodec, 2D in plan, before/after-image undo, groups, change sets, round-trip corpus | Accepted |
| [0012](0012-python-api.md) | Python API: tuples for points (any 2-sequence in, NumPy-readable arrays for bulk), immutable value classes, document handles, exception hierarchy, groups as context managers, GIL released for open/save | Accepted |
| [0013](0013-application-shell.md) | Application shell: Classic and Ribbon workspaces over one action set, render representation and CPU rasterizer in Rust (arq-render), command window, events, panels, tabbed documents; guaranteed floor (reference machine) scaling up by runtime detection (amends 0007) | Accepted |

Template:

```markdown
# NNNN. Title

Date: YYYY-MM-DD · Status: Proposed | Accepted | Superseded by NNNN

## Context
## Decision
## Consequences
```
