# ArqStudioCAD — Data and formats

The document model, the working format and every exchange format: what each
one is for, its adapter and its state. Master plan §5–§6; decisions in
[ADR 0011](architecture/adr/0011-document-model-and-transactions.md),
[ADR 0017](architecture/adr/0017-dwg-dxf-engine-strategy.md) and
[ADR 0018](architecture/adr/0018-native-format.md).

## The document model

- A **document** is one drawing: entities, layers, the current layer, and
  whatever the platform does not own yet, preserved as it came.
- **Identity:** entities and layers have stable ids (`EntityId`, `LayerId`,
  today the DWG handle) that survive saves, reopenings, undo and redo.
- **Changes** are transactions: one undo step each, all or nothing, reported
  as a `ChangeSet` (contract §6).
- **Ownership** is staged (ADR 0018 D2): the own model is authoritative for
  the kinds it owns (stage 1: layers, LINE, CIRCLE, ARC); everything else
  lives in the DWG engine's database and is preserved.
- **Coordinates** are `f64` model units; angles are radians in the core and
  degrees in commands and the interface (ADR 0010).

## Formats

| Format | Purpose | Adapter | State |
|---|---|---|---|
| **DWG (R13–2018)** | **Working format** (as BricsCAD) and exchange with CAD applications | AcadRust in `arq-document` (ADR 0017), into and out of the own model (ADR 0018) | read and write; protections in 03.3 |
| `.ascad` (reserved) | Native format for when DWG plus a project database are not enough | — | designed, not implemented (ADR 0018 D5) |
| Project database | Project-wide data shared by several drawings (as BricsCAD's `.bsyslib`) | — | decided by the multidisciplinary test (ADR 0018 D4) |
| DXF (R12–2018) | Open exchange | AcadRust | read and write |
| IFC | BIM interoperability | IfcOpenShell, decoupled | phase 05 |
| 3DM / openNURBS | Exchange with Rhino | to be evaluated | later |
| STEP and other 3D | Per use case | to be evaluated | later |
| SVG / PDF | Documentation, printing, delivery | — | phase 04 |
| OBJ / glTF, meshes | Visualization, geometry exchange, walkthroughs | — | phases 08–09 |
| Blender / Cycles | High-quality external rendering | external process | phase 08 |
| FFmpeg | Sequences and video | external process | phase 08 |
| PNG | Viewport images, previews | tiny-skia (`arq-render`) | in use |

Each format has an independent adapter; none of them is the project's only
source of truth.

## Conversion rule

Every import and export says which kind it is:

| Kind | Meaning | What the user sees |
|---|---|---|
| Faithful import | all relevant data kept in the model | nothing special |
| Partial import | some entities converted, others preserved or not readable | a diagnostic listing what was preserved or skipped |
| Lossy export | the target format cannot hold some data | the incompatibilities, **before** the file is written, with the option to cancel |
| Semantic compatibility | relations, properties and behaviors the format admits are kept, not only geometry | — |

A damaged or partial file is reported as such and never taken for a valid
empty drawing (ADR 0017 D5).

## Unknown data

Data the platform does not understand is **kept** when technically
possible: in DWG, by saving in the drawing's own version (the engine writes
untouched records back) and, if `.ascad` is ever used, in its preserved DWG. When a
conversion cannot keep it (a DWG version change, another format), the loss
is announced and can be cancelled; it is never silent.
