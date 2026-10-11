# 0001. Technology stack

Date: 2026-10-11 · Status: Accepted

## Context

A CAD application needs precise, fast geometry and large-document handling, a
flexible layer for commands, plugins and scripting, and a mature cross-platform
desktop UI. Mixing those concerns is the usual way CAD codebases decay.

## Decision

- **Rust** owns the document and everything persistent or performance-critical.
- **Python** owns commands, workflows, plugins, scripting and i18n.
- **PySide6** (Qt 6) owns the interface.
- Python calls Rust in-process through **PyO3**, packaged with **maturin**, as
  an `abi3` extension (`arqstudiocad._core`).
- **uv** manages Python environments and dev dependencies.

## Consequences

- Crates other than `arq-python` must build without Python; enforced by
  `default-members` and a CI check.
- Shipping requires bundling a Python runtime, PySide6 and our extension per
  platform; the freezing tool is chosen in phase 1.
- Contributors need both toolchains; `uv sync` hides most of it.
