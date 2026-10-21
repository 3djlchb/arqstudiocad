# Reference projects

Analyses of the open-source CAD projects ArqStudioCAD learns from. They are
read, never vendored: clones live outside this repository. Code actually
reused is recorded in [THIRD_PARTY.md](../../THIRD_PARTY.md).

| Project | Stack | License | Analysis |
|---|---|---|---|
| Open CAD Studio | Rust, iced, wgpu | GPL-3.0 (app) / MPL-2.0 (codec, kernel) | [opencadstudio.md](opencadstudio.md) |
| IngeCAD | Python, PySide6, ezdxf, OpenGL | GPL-3.0-or-later | [ingecad.md](ingecad.md), [what to port from its core](ingecad-core-port.md) |
| opencadcodec | Rust (DWG/DXF engine, ADR 0008) | MPL-2.0 | [internals](opencadcodec-internals.md) |
| Rhino 3D (openNURBS, rhino3dm) | C++ / Python / JS, `.3dm` | open parts MIT; kernel proprietary | [rhino.md](rhino.md) — for the 3D phase and `.3dm` interop |

Background: [geometry representations](geometry-representations.md):
analytic, NURBS, meshes, SubD, CSG, implicit and others; what the owned
kernel needs, and its roadmap.

## Proposals arising from the analyses

| # | Proposal | Phase | Source | Status |
|---|---|---|---|---|
| R1 | Use **opencadcodec** (MPL-2.0) for DWG/DXF instead of writing our own | 3 / 8 | Open CAD Studio | Accepted, [ADR 0008](../architecture/adr/0008-opencadcodec.md) |
| R2 | `arq-document` wraps the codec document as the model (the document *is* the file database) | 3 | both | Accepted, ADR 0008 |
| R3 | **PyInstaller onedir** for freezing; IngeCAD's packaging as template | 1 | IngeCAD | Accepted, [ADR 0009](../architecture/adr/0009-packaging.md) |
| R4 | Adopt the **local-frame** precision invariant in `arq-geometry` | 2 | opencadkernel | Accepted, [ADR 0010](../architecture/adr/0010-geometry-conventions.md) |
| R5 | Undo as symmetric per-handle before/after deltas | 3 | Open CAD Studio | Proposed |
| R6 | Spatial index: grid for 2D, BVH for 3D, measured first | 3 / 7 | Open CAD Studio | Proposed |
| R7 | Renderer with GPU fallback / safe mode | 7 | both | Superseded by [ADR 0007](../architecture/adr/0007-graphics-api.md): wgpu with runtime backend probing |
| R8 | Bundle self-check (`--check`) on every package build | 1 | IngeCAD | Accepted, ADR 0009 (done in every build script) |
