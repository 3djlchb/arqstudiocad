# 0036. Texture, Nodes and Materials: three horizontal phases with cores of their own

Date: 2026-11-10 · Status: Proposed

## Context

[ADR 0035](0035-phase-structure.md) made phases 05 on horizontal: modules
on the core, added at the end as the project needs them, never changing
the core on their own. On 2026-11-10 the maintainer asked for three:

- **Texture:** texturing the volumes the application will make, milestone
  by milestone as the need comes. References: the textures of
  [IngeTrazo](../../references/ingetrazo.md) and of ArmorPaint. Its own
  texture core in Rust, which can serve later a separate texturing
  application of our own, node-based, in the manner of ArmorPaint and
  Substance Designer.
- **Nodes:** a node system of our own after Blender's Geometry Nodes and
  the node graph Open CAD Studio is building, with our own geometry kernel
  as its core, the base of the future horizontal phases that need nodes.
- **Materials:** physically based (PBR) materials for what the
  application models.

None exists in the code: no crate, no materials or texture coordinates in
the document, no node graph (checked 2026-11-10).

## Options

1. **Add the phases only,** deciding their architecture with their first
   milestone.
2. **Add the phases and decide now that each has a core of its own,** a
   Rust crate on top of the core, so that the boundary is set before any
   code is written.

## Decision

Option 2 *(maintainer, 2026-11-10)*.

### D1 — Three horizontal phases, three modules

| Phase | Module |
|---|---|
| 13 Texture | M12 Texture |
| 14 Nodes | M13 Nodes |
| 15 Materials | M14 Materials |

Their numbers tell when they were added (ADR 0035 D2): Texture will use
Materials and, for its node-based authoring, Nodes, whatever their order.
No milestones are set yet; each phase has a starting order, and its
milestones 13.x, 14.x and 15.x are defined with the maintainer as the need
comes.

### D2 — A core of their own: `arq-texture`, `arq-nodes`, `arq-material`

- Each phase's core is its own Rust crate, owned by the project, as the
  geometry kernel is (ADR 0020): `arq-material`, `arq-texture` and
  `arq-nodes`.
- They depend on the core crates they need (`arq-geometry`, `arq-kernel`,
  `arq-document`); the core never depends on them.
- They follow the technical contract: no Qt, no `pyo3`. Python reaches
  them only through `arq-python`, and the interface only through Python.
- A crate is created with its phase's first milestone, not before.

### D3 — What they need of the core is a vertical milestone

By ADR 0035 D2.1 these phases never change the core on their own. What
they need of it, such as texture coordinates on faces and meshes, a
material assigned to an object, a layer or a face and kept in the DWG
(AutoCAD's materials), or a node graph kept with the drawing, is asked
for as a milestone of a vertical phase, or decided by an ADR when how the
core changes is open.

### D4 — Materials: physical appearance, textures apart

Materials are physically based: base color, roughness, metalness, normal,
opacity and emission, in a library, assigned to objects, layers and faces,
kept with the drawing, and carried to the viewport, the render (phase 09)
and IFC (phase 06). Physical properties for BIM (density, cost) join them
when BIM arrives. Textures are the image maps that feed a material's
channels; they belong to phase 13.

### D5 — Texture: maps painted, generated and mapped

Texture makes and maps the images that feed materials: projections and
texture coordinates, painting on the model, procedural maps, and, with
Nodes, authoring by node graphs. `arq-texture` is designed so that a
separate texturing application of our own, node-based in the manner of
ArmorPaint and Substance Designer, can be built on it later. Where that
application lives (this repository or another) and its name are decided
by its own ADR when it comes.

### D6 — Nodes: graphs evaluated on our own kernel

Nodes is a system of node graphs after Blender's Geometry Nodes and Open
CAD Studio's node graph for CAD (values flowing between objects'
properties, D7): typed
values flowing through nodes, evaluated and cached, whose geometry is our
own kernel's (`arq-kernel`), with no external geometry engine. What a
graph makes is ordinary geometry: measured, saved and combined like the
rest. It is the base of every later horizontal phase that needs nodes,
Texture's node-based authoring among them.

### D7 — References are ideas, not code

| Reference | License | What it is to us |
|---|---|---|
| [IngeTrazo](../../references/ingetrazo.md) | GPL-3.0-or-later | Textures and materials in a free modeler |
| [ArmorPaint](https://github.com/armory3d/armorpaint) | zlib/libpng (its source) | Painting textures on 3D models, node-based materials |
| Blender's Geometry Nodes | GNU GPL | Node-based geometry |
| [opencadgraph](https://github.com/HakanSeven12/opencadgraph), Open CAD Studio's node graph ([our analysis](../../references/opencadstudio.md)) | MPL-2.0 (the application GPL-3.0) | A node graph for CAD: dataflow over objects' property ports, with input, math, logic, list, text and geometry nodes |
| Substance Designer | Proprietary (Adobe) | Node-based texture authoring, as a design reference only |

Each phase starts by studying its references in a document of
`docs/references/`. No code is copied without recording it in
`THIRD_PARTY.md` with a license compatible with GPL-3.0-or-later; from a
proprietary product, only publicly described ideas are taken.

## Consequences

- The project gains three modules (M12–M14) and three phases (13–15) with
  no code yet: the vertical phases, ending with the first release (04.6),
  keep priority.
- The boundary is set before the code: three crates on top of the core,
  never inside it; their needs of the core become vertical milestones.
- Materials and Texture prepare what rendering (phase 09) and BIM (phase
  06) will use; Nodes prepares every later node-based phase.
- A separate texturing application stays possible without deciding it
  now.
