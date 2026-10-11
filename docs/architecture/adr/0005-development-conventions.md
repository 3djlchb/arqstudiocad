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
- **Testing is local.** Every check runs on the developer machine (pre-commit
  runs them on each commit); Linux is tested locally through WSL2 from phase 1.
  GitHub Actions CI runs **only on manual dispatch**: before a version, and to
  build/test platforms not available locally (macOS, Windows ARM64).
  CI runner images are pinned (`ubuntu-24.04`, `windows-2025`, `macos-15`),
  never `*-latest`, and bumped deliberately like the Rust toolchain.

## Consequences

- Every contributor, Codex and CI build with the same compiler.
- Toolchain bumps are explicit commits, easy to bisect.
- macOS and Windows ARM64 regressions are only caught when CI is run by hand,
  so it must be run before merging anything platform-sensitive (packaging,
  rendering, file paths, native dependencies).
