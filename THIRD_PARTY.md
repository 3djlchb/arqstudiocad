# Third-party code and ideas

Every piece of code copied or adapted from another project is listed here,
with its origin and license. Design ideas taken from other projects are listed
too, so their authors get credit.

ArqStudioCAD is GPL-3.0-or-later; only code under a compatible license may be
copied.

## Code

| Path in this repo | Origin | License | Notes |
|---|---|---|---|
| _(none yet)_ | | | |

## Design references

| Project | License | What we take from it |
|---|---|---|
| [IngeCAD](https://github.com/ingelibre/ingecad) | GPL-3.0 | i18n model: English command line, language packs, prompt-key convention ([ADR 0006](docs/architecture/adr/0006-i18n.md)) |
| [IngeCAD](https://github.com/ingelibre/ingecad) | GPL-3.0-or-later | Packaging approach (PyInstaller onedir, AppImage, Flatpak), command-line conventions ([analysis](docs/references/ingecad.md)) |
| [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio) | GPL-3.0 | Undo as per-handle deltas, spatial indexing, GPU fallback ([analysis](docs/references/opencadstudio.md)) |
| [opencadkernel](https://github.com/HakanSeven12/opencadkernel) | MPL-2.0 | Local-frame precision invariant, provenance for round-trips |
