# Reference projects

Analyses of the open-source CAD projects ArqStudioCAD learns from. They are
read, never vendored: clones live outside this repository. Code actually
reused is recorded in [THIRD_PARTY.md](../../THIRD_PARTY.md).

| Project | Stack | License | Analysis |
|---|---|---|---|
| Open CAD Studio | Rust, iced, wgpu | GPL-3.0 (app) / MPL-2.0 (codec, kernel) | [opencadstudio.md](opencadstudio.md) |
| IngeCAD | Python, PySide6, ezdxf, OpenGL | GPL-3.0-or-later | [ingecad.md](ingecad.md) |

## Proposals arising from the analyses

| # | Proposal | Phase | Source | Status |
|---|---|---|---|---|
| R1 | Use **opencadcodec** (MPL-2.0) for DWG/DXF instead of writing our own | 3 / 8 | Open CAD Studio | Accepted, [ADR 0008](../architecture/adr/0008-opencadcodec.md) |
| R2 | `arq-document` wraps the codec document as the model (the document *is* the file database) | 3 | both | Accepted, ADR 0008 |
| R3 | **PyInstaller onedir** for freezing; IngeCAD's packaging as template | 1 | IngeCAD | Proposed |
| R4 | Adopt the **local-frame** precision invariant in `arq-geometry` | 2 | opencadkernel | Proposed |
| R5 | Undo as symmetric per-handle before/after deltas | 3 | Open CAD Studio | Proposed |
| R6 | Spatial index: grid for 2D, BVH for 3D, measured first | 3 / 7 | Open CAD Studio | Proposed |
| R7 | Renderer with GPU fallback / safe mode | 7 | both | Superseded by [ADR 0007](../architecture/adr/0007-graphics-api.md): wgpu with runtime backend probing |
| R8 | Bundle self-check (`--check`) on every package build | 1 | IngeCAD | Proposed |
