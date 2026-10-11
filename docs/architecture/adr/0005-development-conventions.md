# 0005. Development conventions

Date: 2026-10-11 · Status: Accepted

## Decision

- **Rust toolchain pinned exactly** in `rust-toolchain.toml` (1.99.0 at the
  time of writing), edition 2024. Bumped deliberately, together with
  `rust-version` in `Cargo.toml`.
- **Quality tools:** rustfmt, clippy (warnings are errors in CI), ruff (lint +
  format), mypy (strict), cargo-deny; all wired into pre-commit and CI.
- **Language:** English for code, comments, documentation and UI source
  strings (see ADR 0006 for translations).
- **Git:** one branch per phase (`fase-N/<topic>`), small commits per step,
  pull request to `main` when the phase closes with green CI.
- Line endings are LF in the repository (`.gitattributes`).

## Consequences

- Every contributor, Codex and CI build with the same compiler.
- Toolchain bumps are explicit commits, easy to bisect.
