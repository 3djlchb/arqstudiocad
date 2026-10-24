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
| `crates/arq-document/fonts/standard.lff` | [LibreCAD](https://github.com/LibreCAD/LibreCAD) `librecad/support/fonts/standard.lff` ("ISO 3098-2", by Rallaz, based on a public-domain Wikimedia drawing of ISO 3098) | GPL-2.0-or-later | Copied unchanged (master, 2026-10-18); the built-in stroke font texts are drawn in until phase 04 (ADR 0022). The LFF reader in `text_layout.rs` is our own |

## Dependencies

| Path in this repo | Origin | License | Notes |
|---|---|---|---|
| `crates/arq-core` | [rstar](https://github.com/georust/rstar), crates.io `=0.13.0` | MIT OR Apache-2.0 | Private R*-tree implementation for 2D model-space bounding-box queries; no types cross the core API (ADR 0021) |
| `crates/arq-render` | [wgpu](https://github.com/gfx-rs/wgpu), crates.io `=30.0.1` | MIT OR Apache-2.0 | Cross-platform GPU API for the 02.7 backend probe and renderer (ADR 0007) |
| `crates/arq-render` | [pollster](https://github.com/zesterer/pollster), crates.io `=0.4.0` | MIT OR Apache-2.0 | Runs wgpu initialization futures synchronously in the isolated probe process |

## Documents

| Path in this repo | Origin | License | Notes |
|---|---|---|---|
| `CODE_OF_CONDUCT.md` | [Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) | CC BY 4.0 | Contact address changed |
| `CONTRIBUTING*.md`, `.github/ISSUE_TEMPLATE/`, `.github/PULL_REQUEST_TEMPLATE.md` | [IngeCAD](https://github.com/ingelibre/ingecad) | GPL-3.0-or-later | Adapted to this project's stack and rules |

## Design references

| Project | License | What we take from it |
|---|---|---|
| [IngeCAD](https://github.com/ingelibre/ingecad) | GPL-3.0-or-later | i18n model: English command line, language packs, prompt-key convention ([ADR 0006](docs/architecture/adr/0006-i18n.md)); packaging approach (PyInstaller onedir, AppImage, Flatpak); command-line conventions ([analysis](docs/references/ingecad.md)) |
| [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio) | GPL-3.0 | Undo as per-handle deltas, spatial indexing, GPU fallback ([analysis](docs/references/opencadstudio.md)); look of the panels: title with pin and close, property grid with foldable sections ([ADR 0015](docs/architecture/adr/0015-workspaces.md)); ribbon workspace; CAD color selection by indexed palette, true color and recent colors, reimplemented without copied code |
| [Truck](https://github.com/ricosjp/truck) | Apache-2.0 | Geometry-kernel architecture and capability decomposition (ADR 0020); design reference only, not currently a dependency |
| [Open CASCADE Technology](https://github.com/Open-Cascade-SAS/OCCT) | LGPL-2.1 with the Open CASCADE exception | Candidate temporary 3D reference/backend by capability (ADR 0020); no integration or source reuse is approved by this entry |
| [openNURBS](https://github.com/mcneel/opennurbs) | Robert McNeel & Associates; no copyright or copyleft restrictions per its site (its `LICENSE` holds notices only); bundles zlib | NURBS mathematics, B-rep data structures and the 3DM format (ADR 0020 amendment); design reference only, no source copied |
| [opencadkernel](https://github.com/HakanSeven12/opencadkernel) | MPL-2.0 | Local-frame precision invariant, provenance for round-trips |
| [AcadRust](https://github.com/hakanaktt/acadrust) | MPL-2.0 | DWG/DXF engine — **dependency** of `arq-document`, from crates.io, pinned at `=0.6.7`, patched (`[patch.crates-io]`) from [our fork](https://github.com/3djlchb/acadrust/tree/arqstudiocad/on-c5555a5) at `955bdd8`: the original's commit `c5555a5` (0.6.7, which includes opencadcodec's improvements) with our changes, maintained in our fork (ADR 0017 D2): undecodable class objects kept verbatim (beyond the auxiliary classes the original preserves itself), inline surfaces' binary extra modeler block read, TOLERANCE text written with `^J` and a zero height boxed as the default (milestone 03.2), R11/R12 current layer, text style and linetype names read; our DXF hatch elliptic edge angle patch was replaced by the original's own rule, and our R11/R12 16-bit linetype index fix dropped, the original reading them so since its own R11/R12 support; its sample drawings are extracted from the package for the test corpus, never committed (`tools/fetch_codec_samples.py`) ([ADR 0017](docs/architecture/adr/0017-dwg-dxf-engine-strategy.md), [ADR 0011](docs/architecture/adr/0011-document-model-and-transactions.md)) |
| [opencadcodec](https://github.com/HakanSeven12/opencadcodec) | MPL-2.0 | Fork of AcadRust, our engine until ADR 0017; its improvements are brought in one by one with regression drawings, each recorded here |
| [LibreDWG](https://github.com/LibreDWG/libredwg) test data | GPL-3.0-or-later | `Constraints.dwg` in seven DWG versions, downloaded and checksummed as **test input** for the round-trip corpus (`tools/fetch_codec_samples.py`, tag 0.13.3), never committed nor shipped |
