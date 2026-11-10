# Reference analysis: IngeTrazo

Analyzed 2026-11-04 from its repository's `main` (version 0.5.8) through
GitHub's public API: README, roadmap, architecture, plugin and topology
documents, and the headers of its geometry core.
Repository: <https://github.com/ingelibre/ingetrazo> · License:
GPL-3.0-or-later.

## What it is

A free 3D modeler for architecture, engineering and 3D design, built in
Peru by Marco Sumari Tellez, of the same family as IngeCAD (the
[ingelibre](https://github.com/ingelibre) organization). A push/pull
modeler: the user draws faces as if by hand and pushes or pulls them into
volumes. Linux first, also Windows and macOS; about 1,400 stars and some
700 test files at the time of the analysis.

| Concern | How IngeTrazo does it |
|---|---|
| Language and UI | Python 3.11+ and PySide6 (Qt 6), a small Rust core (`rust/ingetrazo_core`) for heavy loops |
| Rendering | OpenGL 3.3 through Qt, GLSL shaders; shaded, hidden-line, X-ray styles, ambient occlusion, sun shadows |
| Geometry | Flat polygon faces (outer loop and holes) over `QVector3D` points (single precision); connectivity found by matching rounded positions, migrating to shared vertices and a non-manifold edge–face incidence; curves are facets with soft edges |
| File format | `.igz`, its own, self-contained (textures inside) |
| Exchange | `.skp` import through [OpenSKP](https://github.com/iamahsanmehmood/openskp) (MIT); STL, OBJ, COLLADA import; STL, OBJ, COLLADA, glTF export; IFC export of tagged objects; DWG planned with IngeCAD |
| Extensions | Python plugins dropped in a folder, one menu entry per tool; an AI assistant and an MCP bridge |

## Tools

- **Drawing**: line, rectangle and rotated rectangle, circle, polygon,
  arcs, freehand, 3D text, images; inference, axis locks and typed values.
- **Modeling**: push/pull with a guard that refuses a broken solid; offset
  (walls with thickness); follow me (sweeping a profile); fillet; bridge
  (side faces between two profiles: two rectangles make a box, a
  rectangle and a ridge line a hip roof); intersect faces.
- **Editing**: move, rotate (protractor), scale, flip; tape measure and
  guides every tool snaps to; eraser; groups and components.
- **Solids**: outer shell, union, subtract, trim, intersect, split;
  section planes with fill.
- **Presentation**: materials and textures, layers and scenes, dimensions
  and leader texts, a sheet composer with vector PDF and DXF export.
- **BIM**: tags on free geometry (a set of faces or a group becomes an
  IFC wall, slab, column…), quantities from them, IFC export toward
  [IngePresupuestos](https://ingepresupuestos.com).
- **Site**: georeferencing, map tiles, terrain from elevation models,
  survey points, photogrammetric meshes.

## What we take, and what we do not

| Take | Why |
|---|---|
| Its tools and their behaviour: push/pull, offset, bridge, follow me, inference, typed lengths, the guard against broken solids | They are what makes free modeling quick; a reference for [ADR 0031](../architecture/adr/0031-free-geometry.md) |
| BIM as tags on free geometry | It fits phase 06 (BIM and IFC) and our budgets (ADRs 0027, 0028) |
| `.skp` through OpenSKP | The same library, if `.skp` import comes |

| Do not take | Why |
|---|---|
| Its geometry and document | Single precision, connectivity by position, curves as facets, owned by Python; ours is an exact B-rep in `f64` owned by Rust (ADR 0024) |
| Its viewport and file format | Ours draws from Rust and keeps DWG as the working format (ADR 0018) |
| Its code wholesale | Two document models would live side by side; an algorithm taken from it is reimplemented in Rust and recorded in `THIRD_PARTY.md` |
