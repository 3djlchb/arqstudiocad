# Architecture Decision Records

One file per decision, numbered, never rewritten once accepted: a decision
that changes gets a new ADR that supersedes the old one.

| # | Decision | Status |
|---|---|---|
| [0001](0001-technology-stack.md) | Rust core + Python application + PySide6 UI, bound with PyO3/maturin | Accepted |
| [0002](0002-dwg-native-format.md) | DWG as native format, DXF as alternative | Accepted |
| [0003](0003-target-platforms.md) | Target platforms and macOS universal2 | Accepted |
| [0004](0004-naming-and-licensing.md) | Names, Python 3.12+, GPL-3.0-or-later | Accepted |
| [0005](0005-development-conventions.md) | Toolchain pinning, quality tools, git workflow | Accepted |
| [0006](0006-i18n.md) | English source, translation packs (IngeCAD model) | Accepted |

Template:

```markdown
# NNNN. Title

Date: YYYY-MM-DD · Status: Proposed | Accepted | Superseded by NNNN

## Context
## Decision
## Consequences
```
