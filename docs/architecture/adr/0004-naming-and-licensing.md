# 0004. Names, Python version and license

Date: 2026-10-11 · Status: Accepted

## Decision

- Product name: **ArqStudioCAD**.
- Python package `arqstudiocad`; native module `arqstudiocad._core`.
- Rust crates `arq-geometry`, `arq-document`, `arq-core`, `arq-render`, `arq-python`.
- Minimum Python **3.12**; the extension is built as `abi3-py312`, so one wheel
  per platform serves every later Python.
- License **GPL-3.0-or-later**, compatible with the reference projects
  (Open CAD Studio, IngeCAD) and with GNU LibreDWG.
- Dependency licenses are checked by `cargo-deny` (`deny.toml`).

## Consequences

- Code from GPL-3.0 projects may be reused with attribution in `THIRD_PARTY.md`.
- Dependencies under GPL-incompatible licenses are rejected by CI.
