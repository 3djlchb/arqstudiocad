# ArqStudioCAD

**English** | [Español](README.es.md)

Open-source 2D/3D CAD for architecture and engineering, with DWG as its native
format and DXF as an open alternative.

> **Status:** pre-alpha. The application starts, switches between English and
> Spanish and is packaged for every target platform (phase 1). The geometry
> core is done — lines, circles, arcs, transformations, intersections and snap
> points (phase 2) — and so is the document core: it opens and saves DWG and
> DXF, edits lines, circles, arcs and layers with undo and redo, and keeps
> everything else in the drawing intact (phase 3). The interface cannot draw
> yet.

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
Linux (`.deb`, `.rpm`, AppImage).

## Languages

The interface is written in English and translated through language packs;
Spanish is the first maintained translation. Commands typed in the command
line are always accepted in English. See
[ADR 0006](docs/architecture/adr/0006-i18n.md).

## Building from source

You need four things on every platform: **Git**, a **C/C++ linker**,
**rustup** (installs Rust) and **[uv](https://docs.astral.sh/uv/)** (installs
Python and the Python tools). You do **not** need to install Python or a
specific Rust version yourself: uv downloads Python 3.12+ if missing, and
rustup installs the exact Rust version pinned in `rust-toolchain.toml` the
first time you build.

### Windows 10/11 (x86-64 or ARM64)

1. Install the tools from PowerShell:

   ```powershell
   winget install --source winget --id Git.Git
   winget install --source winget --id Rustlang.Rustup
   winget install --source winget --id astral-sh.uv
   winget install --source winget --id Microsoft.VisualStudio.BuildTools `
     --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

   The last command installs Visual Studio Build Tools with the C++ workload
   (MSVC linker and Windows SDK). If you already have Visual Studio with
   "Desktop development with C++", skip it.

2. **Close and reopen the terminal** so the new `PATH` is loaded.

3. Get the code and build:

   ```powershell
   git clone https://github.com/3djlchb/arqstudiocad.git
   cd arqstudiocad
   uv sync
   ```

### Linux

1. Install a compiler, Git and curl with your distribution's package manager:

   | Distribution | Command |
   |---|---|
   | Debian, Ubuntu, Linux Mint, LMDE | `sudo apt install build-essential git curl` |
   | Fedora | `sudo dnf install gcc gcc-c++ make git curl` |
   | openSUSE | `sudo zypper install -t pattern devel_basis && sudo zypper install git curl` |
   | Arch, Manjaro, EndeavourOS | `sudo pacman -S --needed base-devel git curl` |

   Desktop distributions already have the libraries Qt needs to open windows,
   except sometimes the X11 cursor library: `libxcb-cursor0` (Debian, Ubuntu),
   `xcb-util-cursor` (Fedora, openSUSE, Arch).

2. Install rustup and uv (both install into your home directory, no `sudo`):

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
   curl -LsSf https://astral.sh/uv/install.sh | sh
   ```

3. **Open a new terminal** (or run `source ~/.cargo/env`) so `cargo` and `uv`
   are on the `PATH`.

4. Get the code and build:

   ```sh
   git clone https://github.com/3djlchb/arqstudiocad.git
   cd arqstudiocad
   uv sync
   ```

### macOS (Apple Silicon or Intel)

Run `xcode-select --install` for the compiler, then follow the Linux steps 2–4.

### Run it (all platforms)

```sh
uv run arqstudiocad              # opens the application
uv run arqstudiocad --lang es    # in Spanish (also: Tools > Language)
uv run arqstudiocad --check      # verifies the installation, prints a report
```

And the test suites:

```sh
cargo test         # Rust core tests (no Python involved)
uv run pytest      # Python tests
```

`uv sync` creates a `.venv` folder, compiles the Rust extension in debug mode
and installs the development tools. After changing Rust code, `uv sync` (or
any `uv run ...`) rebuilds the extension automatically.

To build an optimized, installable package (a *wheel*) in `target/wheels/`:

```sh
uv run maturin build --release
```

Installers and portable packages (Windows installer and portable ZIP, macOS
`.dmg`, Linux AppImage/`.deb`/`.rpm`) are built with the scripts described in
[packaging/README.md](packaging/README.md).

### Contributors

Install the local checks once; they then run on every commit:

```sh
cargo install cargo-deny --locked
uv run pre-commit install
```

The full set of checks is listed in [AGENTS.md](AGENTS.md).

### Troubleshooting

| Symptom | Fix |
|---|---|
| `cargo`, `rustup` or `uv` not found | Open a new terminal (Windows: close all terminals, or restart VS Code) |
| `link.exe not found` (Windows) | Install the Build Tools C++ workload (step 1) |
| `linker 'cc' not found` (Linux) | Install the compiler package from step 1 |
| `Could not load the Qt platform plugin "xcb"` (Linux) | Install the X11 cursor library named in step 1 |
| `invalid peer certificate: UnknownIssuer` from uv | A proxy or antivirus intercepts HTTPS. Set `UV_SYSTEM_CERTS=1` so uv trusts the system certificate store |

## Contributing

Contributions are welcome, in English or Spanish. Read
[CONTRIBUTING.md](CONTRIBUTING.md) for the workflow and house rules, and the
[Code of Conduct](CODE_OF_CONDUCT.md).

## License

[GPL-3.0-or-later](LICENSE). Third-party code and ideas are credited in
[THIRD_PARTY.md](THIRD_PARTY.md).
