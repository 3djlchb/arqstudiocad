# Third-party code and ideas

Every piece of code copied or adapted from another project is listed here,
with its origin and license. Design ideas taken from other projects are listed
too, so their authors get credit.

ArqStudioCAD is GPL-3.0-or-later; only code under a compatible license may be
copied.

## Code

| Path in this repo | Origin | License | Notes |
|---|---|---|---|
| `crates/arq-document/src/ocs.rs` | Approach of IngeCAD's `core/ocs.py` (plan view of OCS entities, normal `(0,0,-1)`), reimplemented in Rust | GPL-3.0-or-later | Idea and rules ported, no code copied |

## Documents

| Path in this repo | Origin | License | Notes |
|---|---|---|---|
| `CODE_OF_CONDUCT.md` | [Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) | CC BY 4.0 | Contact address changed |
| `CONTRIBUTING*.md`, `.github/ISSUE_TEMPLATE/`, `.github/PULL_REQUEST_TEMPLATE.md` | [IngeCAD](https://github.com/ingelibre/ingecad) | GPL-3.0-or-later | Adapted to this project's stack and rules |

## Design references

| Project | License | What we take from it |
|---|---|---|
| [IngeCAD](https://github.com/ingelibre/ingecad) | GPL-3.0-or-later | i18n model: English command line, language packs, prompt-key convention ([ADR 0006](docs/architecture/adr/0006-i18n.md)); packaging approach (PyInstaller onedir, AppImage, Flatpak); command-line conventions ([analysis](docs/references/ingecad.md)) |
| [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio) | GPL-3.0 | Undo as per-handle deltas, spatial indexing, GPU fallback ([analysis](docs/references/opencadstudio.md)); look of the panels: title with pin and close, property grid with foldable sections ([ADR 0015](docs/architecture/adr/0015-workspaces.md)); ribbon workspace |
| [opencadkernel](https://github.com/HakanSeven12/opencadkernel) | MPL-2.0 | Local-frame precision invariant, provenance for round-trips |
| [AcadRust](https://github.com/hakanaktt/acadrust) | MPL-2.0 | DWG/DXF engine — **dependency** of `arq-document`, from crates.io, pinned at `=0.6.4`, patched (`[patch.crates-io]`) from [our fork](https://github.com/3djlchb/acadrust/tree/arqstudiocad/preserve-undecoded) at `b2358ab` with our changes, offered upstream: undecodable class objects kept verbatim (milestone 03.2); its sample drawings are extracted from the package for the test corpus, never committed (`tools/fetch_codec_samples.py`) ([ADR 0017](docs/architecture/adr/0017-dwg-dxf-engine-strategy.md), [ADR 0011](docs/architecture/adr/0011-document-model-and-transactions.md)) |
| [opencadcodec](https://github.com/HakanSeven12/opencadcodec) | MPL-2.0 | Fork of AcadRust, our engine until ADR 0017; its improvements are brought in one by one with regression drawings, each recorded here |
| [LibreDWG](https://github.com/LibreDWG/libredwg) test data | GPL-3.0-or-later | `Constraints.dwg` in seven DWG versions, downloaded and checksummed as **test input** for the round-trip corpus (`tools/fetch_codec_samples.py`, tag 0.13.3), never committed nor shipped |
