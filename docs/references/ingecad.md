# Reference analysis: IngeCAD

Analyzed 2026-10-11 from a shallow clone of `main` (v0.6.6).
Repository: <https://github.com/ingelibre/ingecad> · License: GPL-3.0-or-later.

## What it is

A deliberately small 2D CAD for civil engineers and architects migrating from
AutoCAD: classic menus + toolbars + a real command line, `acad.pgp`-style
aliases, faithful DWG/DXF round-trips. "Linux/Wayland first". Topography and
terrain plugins (survey points, TIN, contours, georeferencing via PROJ).
Part of the "Inge" ecosystem (IngeTrazo for 3D/BIM).

## Architecture

**Pure Python + PySide6** — the same UI stack as ArqStudioCAD, without a
compiled core. ~71,000 lines of Python.

| Concern | How IngeCAD does it |
|---|---|
| Dependencies | PySide6 ≥ 6.7, **ezdxf**, numpy, Pillow, pyproj |
| Document model | **The ezdxf DXF database is the model.** Unknown data is preserved on save |
| DWG | Never parsed in-app: bundled **GNU LibreDWG** command-line converters run as a satellite process (`formats/dwg_bridge.py`). Reads up to R2018, **writes only R2000**; ODA File Converter optional |
| Rendering | `QOpenGLWidget` + shaders + vertex buffers. ezdxf's drawing frontend resolves CAD semantics (blocks, MTEXT, linetypes, hatches, dimensions); IngeCAD's backend packs primitives into numpy arrays bucketed by (layer, color). Curves flattened at a world tolerance ("regen") |
| Layout | `core/` (document, commands, osnap, selection, layers...), `views/` (Qt widgets), `tools/` (interactive commands), `render/`, `formats/`, `plugins/` |
| i18n | `core/i18n/` + `i18n/<code>/{meta,ui,commands}.json` — adopted by us in ADR 0006 |
| Plugins | Loaded by path from `plugins/`, contract in `docs/plugins.md` |
| Tests | pytest, ~770 tests including i18n invariants and real-window smoke tests |

## Packaging (directly reusable as a template for phase 1)

| Target | Tool | File |
|---|---|---|
| Freezing | **PyInstaller, onedir** | `packaging/ingecad.spec` |
| AppImage | script around the PyInstaller output | `packaging/build-appimage.sh` |
| Flatpak | manifest + launcher + AppStream metainfo | `packaging/flatpak/` |
| Snap | snapcraft | `packaging/snap/` |
| Windows | **Inno Setup**, x64 only | `packaging/windows/ingecad.iss` |
| RPM | spec file | (referenced by the Flatpak/AppImage tooling) |
| macOS | — | not supported |

Lessons recorded in the spec: **onedir, not onefile** — onefile unpacks
~250 MB to `/tmp` on every launch. Language packs and plugins travel as data
files at the same relative paths, so the app resolves them identically frozen
or not. `main.py --check` fails a bundle that ships no languages.

## What we adopt

1. **i18n model** — already accepted (ADR 0006).
2. **PyInstaller onedir** as the freezing tool (proposal for phase 1): proven
   with PySide6 on Linux and Windows. We must add macOS universal2 and
   Windows ARM64, which IngeCAD does not cover.
3. **Packaging layout and AppStream/desktop files** as templates.
4. **A `--check` self-test** of the frozen bundle (languages, native module,
   Qt plugins present) run on every package build.
5. **QOpenGLWidget is proven inside Qt for CAD-scale drawing**; a strong
   candidate for phase 7, with vertex buffers produced by `arq-render` in Rust
   instead of numpy.
6. **Interaction conventions:** command line, aliases, window/crossing
   selection, osnap markers, "deliberately small" scope discipline.

## What we do differently, and why

| IngeCAD | ArqStudioCAD | Reason |
|---|---|---|
| ezdxf (Python) as model | Rust core | Performance on large drawings; types and invariants enforced in one place |
| DWG through external LibreDWG, writes R2000 only | In-process DWG through opencadcodec (proposed), R2018 write | DWG is our native format (ADR 0002); a satellite converter writing R2000 is not enough |
| Linux first, no macOS | All platforms from phase 1 (ADR 0003) | Project goal |
| Windows x64 only | x64 + ARM64 | Project goal |

## Risks / notes

- Its rendering leans on `ezdxf.addons.drawing` to resolve CAD semantics. We
  will have to do that resolution ourselves in Rust (or rely on what
  opencadcodec exposes) — a real chunk of work in phases 7–8.
