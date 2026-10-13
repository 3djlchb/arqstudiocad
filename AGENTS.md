# Instructions for coding agents (Codex, Claude Code, ...)

Read [docs/architecture/technical-contract.md](docs/architecture/technical-contract.md)
before changing code. It is binding. Decisions are in [docs/architecture/adr/](docs/architecture/adr/).

## Non-negotiable rules

- Rust owns the document. Python never mutates core state except through the
  PyO3 API; PySide6 never talks to Rust directly, only through Python.
- Only `crates/arq-python` may depend on `pyo3`. No crate depends on Qt.
- The core never calls Python or Qt; it returns results and `CadChangeSet`s.
- Coordinates are `f64` model units; reject NaN/infinity. Angles are radians
  in Rust and in the Python API; commands and the UI read and show degrees,
  converting only at that boundary (ADR 0010).
- Operations are all-or-nothing: a failed operation leaves the document unchanged.
- English everywhere in code, comments, docs and UI source strings. User-facing
  text goes through `tr()`; commands and prompt keys stay English (ADR 0006).
  Never write the product name in user-facing text: use `{app}` in `tr()`
  strings and `arqstudiocad.product` constants elsewhere.
- `README.md` / `README.es.md` and `CONTRIBUTING.md` / `CONTRIBUTING.es.md`
  (English / Spanish) are kept in sync: a change to one is made to the other in
  the same commit. Everything else is English.
- Code or ideas copied from other projects are recorded in `THIRD_PARTY.md`
  and must be GPL-3.0-or-later compatible.

## Commands

```sh
uv sync                                        # build extension + dev env
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test                                     # core crates, no Python
uv run pytest                                  # Python tests
uv run ruff check && uv run ruff format --check && uv run mypy
uv run stubtest arqstudiocad._core            # the .pyi stub matches the module
cargo deny check                               # dependency licenses + advisories
```

One-time setup: `cargo install cargo-deny --locked` and `uv run pre-commit install`.

A change is done when all of the above pass locally and it has tests at the
right layer (contract §12). A check that inspects nothing is a failure, not a
pass. GitHub Actions CI is manual-only; it is not a substitute for running the
checks locally.

## Git

Branch per phase (`fase-N/<topic>`), small commits, PR to `main`.
