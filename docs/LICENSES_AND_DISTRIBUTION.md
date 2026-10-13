# ArqStudioCAD — Licenses and distribution

Master plan §11, §12, §14, §16; decisions in
[ADR 0004](architecture/adr/0004-naming-and-licensing.md),
[ADR 0009](architecture/adr/0009-packaging.md),
[ADR 0014](architecture/adr/0014-platform-direction.md) and
[ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md).

## Licenses

- **Own code:** GPL-3.0-or-later (`LICENSE`).
- **Documentation:** CC BY-SA 4.0 (`docs/LICENSE`).
- **Dependencies:** each reviewed for license, provenance and obligations
  before it enters; preference MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0,
  then other compatible licenses after review. `cargo deny check` enforces
  the Rust side; `THIRD_PARTY.md` is the inventory of dependencies, code and
  ideas taken from other projects, and modifications.
- **No incompatible dependency** without an explicit legal and technical
  evaluation, recorded in an ADR.
- **Being open source does not remove obligations.** They depend on the
  license, how the library is linked, which files are modified and how the
  product is distributed. Example: AcadRust is MPL-2.0; its files and our
  changes to them stay MPL-2.0 and their source is offered with the product,
  while the program as a whole is GPL-3.0-or-later.
- **Commercial libraries** are assessed separately and only as optional
  adapters whose absence never prevents using the fundamental capabilities.

## Rule for new technologies

Before a library, engine or technology is integrated (master plan §16):
does it improve a concrete objective; respect the Rust core's independence;
keep the integration contract stable; have a license compatible with the
intended distribution; have enough upkeep, documentation and tests; improve
performance, fidelity or a needed capability; can it be replaced? If it
fails the essentials it does not enter the core; it may be an optional
extension or be discarded.

## Platforms

Development starts on Windows x86-64. Targets: Windows x86-64 and ARM64;
Linux x86-64 and ARM64 (native packages where feasible, AppImage, Flatpak);
macOS Apple Silicon and Intel, subject to toolchains and dependencies. A
platform is supported only once the application and its critical
dependencies have been built, installed and tested on it. Packaging today:
PyInstaller onedir, Inno Setup, `.deb`, `.rpm`, AppImage, Flatpak manifest
(ADR 0009). The extension `.ascad` is reserved for the native format
(ADR 0018 D5); installers register it if it is ever implemented.

## When a release is ready

Not because it compiles. A release completes the flows it declares; passes
its automatic and regression tests; documents its known limitations; keeps
licenses and attributions right; installs and starts on the platforms it
announces; destroys no data when an operation fails; warns clearly before
lossy conversions; has reproducible build and packaging procedures; and has
enough documentation to install, use and extend it. No release is published
until the application is mature (maintainer's decision).
