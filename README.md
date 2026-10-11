# ArqStudioCAD

Open-source 2D/3D CAD for architecture and engineering, with DWG as its native
format and DXF as an open alternative.

> **Status:** pre-alpha, phase 0 (foundation). Nothing usable yet.

| Layer | Technology | Owns |
|---|---|---|
| CAD core | Rust | Geometry, document, entities, transactions, undo/redo, spatial queries |
| Application | Python | Commands, use cases, plugins, scripting, events, i18n |
| Interface | PySide6 (Qt 6) | Windows, viewport, panels, command line, dialogs |

Python talks to Rust through [PyO3](https://pyo3.rs) and
[maturin](https://www.maturin.rs). See
[docs/architecture/technical-contract.md](docs/architecture/technical-contract.md)
for the rules every change must respect, and
[docs/architecture/adr/](docs/architecture/adr/) for the decisions behind them.

## Target platforms

Windows (x86-64, ARM64) · macOS (universal2: Apple Silicon and Intel) ·
Linux (`.deb`, `.rpm`, Arch/AUR, Flatpak, AppImage).

## Languages

The interface is written in English and translated through language packs;
Spanish is the first maintained translation. Commands typed in the command
line are always accepted in English. See
[ADR 0006](docs/architecture/adr/0006-i18n.md).

## Building from source

Requirements:

- Rust (the exact version is pinned in `rust-toolchain.toml`; `rustup` installs it automatically)
- [uv](https://docs.astral.sh/uv/)
- Windows: Visual Studio Build Tools with the C++ workload

```sh
uv sync            # creates .venv, builds the Rust extension, installs dev tools
uv run pytest      # Python tests
cargo test         # Rust core tests (no Python needed)
```

After changing Rust code, `uv sync` (or `uv run`) rebuilds the extension.

Contributors also install the local checks once:

```sh
cargo install cargo-deny --locked
uv run pre-commit install
```

## License

[GPL-3.0-or-later](LICENSE). Third-party code and ideas are credited in
[THIRD_PARTY.md](THIRD_PARTY.md).
