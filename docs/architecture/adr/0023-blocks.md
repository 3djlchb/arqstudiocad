# 0023. Blocks in milestone 02.8: create, insert, edit, explode

Date: 2026-10-19 · Status: Accepted · Builds on [0016](0016-editing-workflow.md),
[0018](0018-native-format.md), [0022](0022-text-entities.md)

## Context

Block references (INSERT) are drawn, selected and transformed as a whole
(ADR 0016 D1), and EXPLODE replaces one with its contents (PR #36), but
refuses blocks with attributes, which it would lose. Blocks cannot be
created, inserted or edited. The maintainer's drawings hold many blocks.
The roadmap puts "block contents and explode" in milestone 02.8.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on 2026-10-19.

### D1 — EXPLODE keeps attribute values as texts *(maintainer)*

Exploding a block reference with attributes turns each visible attribute
into a single-line TEXT with its current value, where and as it was drawn
(position, justification, height, rotation, width factor, oblique angle,
style, layer and color), as AutoCAD Express Tools' BURST. Invisible
attributes are dropped, as BURST drops them; constant attributes of the
definition become texts with their value. Attribute definitions are not
copied into the drawing. The whole explode is one undo step.

### D2 — BLOCK and INSERT *(maintainer)*

- **BLOCK** (alias `B` is AutoCAD's English one and stays English;
  Spanish `BLOQUE`): a name, a base point and objects selected (first or at
  "Select objects"); the objects are replaced by a reference to the new
  block at the base point, as AutoCAD's default ("Convert to block"). A
  name that exists, is empty or holds characters DWG forbids is refused.
- **INSERT** (alias `I`; Spanish `INSERTAR`): a block name, then the
  insertion point with a drag preview, a uniform scale (1) and a rotation
  (0), with the options Scale and Rotate before the point, as AutoCAD.
  Anonymous (`*`) and layout blocks are not offered.
- New attributes (ATTDEF) and Xrefs are phase 04 or later.

### D3 — The block editor *(maintainer)*

As AutoCAD's BEDIT, not REFEDIT: **BEDIT** (Spanish `EDITARBLOQUE`) opens
a block's definition alone in a tab of its own, at its base point, where
every drawing and editing tool works. Closing the tab asks whether to save
the changes; saving replaces the definition's contents, so every reference
shows them, as one undo step of the drawing. The editor never writes a
file. Attribute definitions in the block are kept as they are.

Saving and closing *(maintainer, 2026-10-20)*:

- While a block editor is the active document, the ribbon shows a
  contextual **Block Editor** tab (Spanish *Editor de bloques*) with
  **Save Block** (`BSAVE`, `GUARDARBLOQUE`) and **Close Block Editor**
  (`BCLOSE`, `CERRARBLOQUE`), as AutoCAD's; the Insert tab's Block group
  gains **Edit** (`BEDIT`, alias `BE`; `EDITARBLOQUE`).
- In a block editor, **Save** (Ctrl+S) saves the block, as BSAVE; **Save
  As** is refused, since the editor never writes a file. **Close**, or
  closing its tab, is BCLOSE. Closing a drawing closes its block editors
  first, asking about each.

### D3a — Dynamic blocks wait for the parametric engine *(maintainer, 2026-10-20)*

Dynamic blocks are kept and shown in their current state (ADR 0017 D6);
changing their parameters needs ArqStudioCAD's parametric engine, which
is milestone 04.1, the start of phase 04 *(maintainer, 2026-10-21)*.
Until then BEDIT refuses a dynamic block, so
its parameters are never broken, while INSERT and EXPLODE work on the
state the file holds.

### D4 — An Insert ribbon tab *(maintainer)*

The ribbon gains an **Insert** tab (Spanish *Insertar*) whose **Block**
group holds Insert, Create and Edit, as AutoCAD's Insert tab; EXPLODE stays
in Home › Modify.

### D5 — Order of work and ownership

Three steps, each its own pull request: D1 with this ADR, then D2, then D3.
Blocks stay in the engine's database, edited through the adapter as now;
moving block definitions and polylines into the own model (ADR 0018 D2,
stage 2) is decided when the block editor shows what it needs from it.

## Implementation notes

- The engine boxes a block reference (and a dimension, drawn through a
  block) at its insertion point only. Since the spatial index (ADR 0021)
  takes entity boxes as its broad phase, a block could be picked, crossed
  or zoomed to only at that point. A block reference's or a dimension's box
  is now the box of everything it draws, as if every layer were on, so the
  index stays right when layers are turned off or on (step 2, 2026-10-20).
- Change sets list model-space entities only: a new block's contents keep
  their handles for undo but are drawn through the references to it.
- The block editor is a document of the session, made from a copy of the
  drawing's database whose model space holds the block's contents at their
  own coordinates (step 3, 2026-10-20). Saving copies its model space into
  the definition, adds the layers and blocks made in the editor (changes to
  existing layers stay in the editor), refuses a block that would hold
  itself, and marks every reference to the block as changed, so it is
  drawn and indexed again. One editor per block; BEDIT inside an editor is
  refused.

## Consequences

- Blocks with attributes explode without losing what they show.
- Blocks can be made, placed and changed in ArqStudioCAD, the most common
  workflow in the maintainer's drawings.
- Editing in a separate tab reuses every tool without a second editing
  mode in the drawing.
