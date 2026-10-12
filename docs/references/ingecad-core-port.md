# IngeCAD core: what to port, and where

Analyzed 2026-10-11 from IngeCAD's `core/` (Python, GPL-3.0-or-later). Its
algorithms may be ported with attribution in `THIRD_PARTY.md`. Destination
follows our contract: algorithms and data in **Rust**, user interaction and
settings in **Python**.

## Port to the Rust core

| IngeCAD module | What it brings | Destination | Phase |
|---|---|---|---|
| `ocs.py` | Plan view of OCS entities, including normal `(0,0,-1)` (mirrored arcs written by other CADs) | `arq-document` | **3** |
| `georef.py` | Georeference (UTM zone, hemisphere, datum, or EPSG code) stored in an XRECORD every CAD preserves | `arq-document` | 3 |
| `draworder.py` | Draw order (SORTENTSTABLE) | `arq-document` | 3–7 |
| `editmath.py` | TRIM, EXTEND, OFFSET, FILLET, CHAMFER, common tangents, arc between points | `arq-geometry` | 6 |
| `modify.py` | STRETCH (decided vertex by vertex), BREAK, JOIN geometry | `arq-geometry` / `arq-core` | 6 |
| `select.py` | Window/crossing rules, grips, point in ring, segment–rectangle tests | `arq-core` | 6 |
| `snap.py` | Snap engine sized for 1.35 M segments (a real cadastre) | `arq-core` + spatial index | 6–7 |
| `polyoffset.py` | Offset of polylines with bulges, self-crossing cleanup | `arq-geometry` | polylines |
| `hatch_trace.py`, `hatch_boundary.py` | HATCH "pick internal point": planar-graph region tracing | `arq-geometry` | hatches |
| `cleanup.py` | OVERKILL (merge duplicate lines/arcs), PURGE candidates | `arq-core` | later |
| `dimassoc.py` | Associative dimensions following their objects | `arq-core` | dimensions |
| `units.py` | Length/angle formats as AutoCAD writes them (decimal, engineering, architectural, fractional, DMS, surveyor) | Rust pure functions called from Python (to decide) | 5 |

## Port to the Python application layer

| IngeCAD module | What it brings | Phase |
|---|---|---|
| `coords.py` | Coordinate input grammar `10,5`, `@10,5`, `10<45`, `@10<45`, direct distance (degrees, at the UI boundary) | 5–6 |
| `actions.py`, `aliases.py` | Command dispatcher and `acad.pgp` aliases | 5 |
| `osnap.py` | Running snaps as AutoCAD's `OSMODE` bits | 6 |
| `drafting.py` | Polar tracking, grid and snap spacing (saved in the drawing) | 6 |
| `autosave.py`, `recent.py`, `templates.py`, `plugins.py`, `scripting.py` | Autosave and recovery, recent files, templates, plugins, SCRIPT | 5+ |
| `qselect.py`, `similar.py`, `isolate.py`, `pickfilter.py` | Selection filters over core queries | 6+ |

## Not applicable

`ezdxf_patches.py`, `encoding.py` (ezdxf / LibreDWG specific), `gl_fallback.py`
(we probe wgpu backends, ADR 0007), `gc_guard.py`, `appimage.py`.

## Design lessons for phase 3

- **A command pins its space** (model or a layout) at its first run; asking
  again at undo time put an erased paper-space entity back in model space.
- **Composite steps**: several operations as one undo step.
- **Autosave waits** for any running mutation before writing.
- IngeCAD implements undo with commands that write their own `undo`; we use
  before/after images instead (ADR 0011), so a command cannot get undo wrong.
