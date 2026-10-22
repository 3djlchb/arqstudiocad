# 0025. The Blocks palette

Date: 2026-10-22 · Status: Proposed · Builds on [0006](0006-i18n.md),
[0013](0013-application-shell.md), [0015](0015-workspaces.md),
[0016](0016-editing-workflow.md), [0023](0023-blocks.md)

## Context

Blocks can be created, inserted, edited and exploded (ADR 0023), but only
from the command line: INSERT asks for a name the user must know. The
maintainer's drawings are mostly blocks, and AutoCAD users expect a palette
that shows them. The maintainer designed one with mockups; the
[design brief](../../design/blocks-palette.md) describes what it does and
how it looks, with
[screenshots](../../design/blocks-palette/main-window-ribbon.png). This
ADR decides how it is built within the technical contract and settles the
brief's open questions. Where the brief and this ADR disagree, this ADR
wins.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on
2026-10-22, in the brief or when asked.

### D1 — A Blocks palette, three sources *(maintainer)*

A new dockable panel, **Blocks** (Spanish *Bloques*), lays out as the
brief's section 3: a source control, a source row, search with grid and
list views, the blocks, a footer, the Selected block and Insert options
sections, and the Insert, Edit block and New… actions. It complements
INSERT and never replaces it. It shows one source at a time:

- **Drawing**: the active drawing's block definitions.
- **Template**: the definitions in a `.dwt`, `.dwg` or `.dxf` file the user
  loads, which is opened read-only, never in a tab.
- **Recent**: blocks created or inserted in this session and the previous
  one, newest first, persisted in the settings as recent files are. Each
  entry keeps the path of the file its definition came from. If that file
  is missing or no longer holds the block, the entry is shown but cannot
  be inserted, and its tooltip says why.

Anonymous (`*`) and layout blocks are never listed, as INSERT does not
offer them (ADR 0023 D2). By default it docks on the right, tabbed with
Properties and Layers. It moves, floats, auto-hides and saves its state
per workspace like every panel (ADR 0015).

### D2 — Command, access points and shortcuts *(maintainer)*

- **BLOCKSPALETTE** (Spanish `PALETABLOQUES`), as AutoCAD 2020's command,
  opens and raises the palette. It has no alias.
- **PROPERTIES** (alias `PR`, as AutoCAD; Spanish `PROPIEDADES`) does the
  same for Properties, so that panel has a command the ribbon can show
  (ADR 0015 lists command names only).
- **Ctrl+1** opens Properties, as in AutoCAD. **Ctrl+3** opens Blocks, in
  the place of AutoCAD's tool palettes, the closest thing it has. Ctrl+2 is
  left free: in AutoCAD it is DesignCenter.
- The **View** tab gains a **Palettes** group (Spanish *Paletas*) with
  Blocks, which LAYER and PROPERTIES join later *(maintainer, 2026-10-22,
  changing the brief's Insert tab)*. The ribbon builds this group from
  data, like every other group.
- A palette's button **toggles** it: pressing it shows the palette, and
  pressing it again closes it. While the palette is open, the button
  carries the panels' selection focus, the translucent background and
  accent color of a selected layer in the Layers panel. When the palette
  is closed, the button has no focus. The View menu lists the palette as
  a checkable item, like every panel, and the item and the button always
  agree *(maintainer, 2026-10-22)*.
- The palette starts closed on first run, as AutoCAD's does.

### D3 — Previews in Rust, updated from change sets *(maintainer)*

Previews are drawn in Rust from each definition's drawing primitives
(ADR 0013 D5.2–D5.3), fitted to a square and cached per definition. Python
receives the pixels as it does the viewport's; no Python loop runs per
entity. A template's previews are built when it loads, off the UI thread.

To know which previews to rebuild, the **`CadChangeSet` gains
`blocks_created`, `blocks_modified` and `blocks_deleted`**, the names of
the block definitions a transaction created, changed or removed, as it
already does for layers. This amends contract §6 and is written into the
contract in the same step that implements it (step 3). A renamed block
shows up as one name deleted and another created. ADR 0023's note that
change sets list only model-space entities still holds: these fields
are additional to it.

### D4 — Dynamic blocks are tagged, not edited *(maintainer)*

A dynamic block carries a tag in the top-left corner of its preview, in
both views. The tag text is a `tr()` string: **DB** in English, **BD** in
the Spanish pack. Its tooltip and accessible name are "Dynamic block". Its
colors are fixed: text `#004b4e` on `#ffa64d`, bold, with a 2 px corner
radius. They stay the same in both themes, an exception to ADR 0015's
"follow the active theme" that the maintainer chose so the tag reads the
same everywhere; the contrast is about 5:1. As ADR 0023 D3a requires,
dynamic blocks are shown, inserted and exploded in the state the file
holds. Edit block is disabled for them, with the existing refusal message
as its tooltip. Their parameter names are shown in the Type row only if
the engine exposes them.

### D5 — Inserting: a button or a drop, always through INSERT *(maintainer)*

Insert, Enter, a double click and dropping a block on the viewport all run
the **INSERT command** with the block's name. Each insertion is therefore
one undo step, echoed in the command window and recorded in Recent.

- **Insert options** feed INSERT. If "Specify on screen" is checked,
  INSERT asks for the point with its drag preview; otherwise the block is
  inserted at the origin. Scale and Rotation are the defaults INSERT
  offers.
- **Explode** is a new INSERT option: it inserts the block exploded, one
  undo step in all, as EXPLODE does (attributes as texts, ADR 0023 D1).
  This extends ADR 0023 D2.
- **Drag and drop**: the drag carries `application/x-arqstudiocad-block`,
  holding the source and the block's name. While over model space the
  viewport shows that it accepts the drop. The drop point goes from screen
  to world coordinates, with running object snaps applied, and is given to
  INSERT as its point. If INSERT's preview can follow the drag, it is used.
- Dropping a `.dwt`, `.dwg` or `.dxf` file on the palette loads it as the
  template.

### D6 — Copying a definition from another file

Inserting a Template or Recent block first copies its definition into the
drawing, in the same undo step as the insertion; undo removes both. A new
core operation copies, all or nothing:

- the definition;
- the blocks it nests;
- the layers, linetypes and text styles it uses that the drawing lacks.

A layer or style the drawing already has is used as it is, as AutoCAD
does. Attribute definitions are copied as they are, and dynamic blocks
keep their data (ADR 0017 D6).

**Name clashes** *(maintainer)*: if the drawing has a block with the same
name and the same contents, the drawing's definition is used without
asking. If the contents differ, a dialog offers three choices:

- **Use the drawing's**, the default;
- **Redefine from the template**, which changes every reference to it;
- **Copy with another name**, proposing `Name_2`, which the user can edit.

The clash is never resolved silently. Nested blocks that clash are asked
about in the same dialog.

### D7 — Reading templates

A `.dwt` is a DWG file with another extension: the core opens it as DWG.
Loading a template uses the background work of ADR 0013 D5.7, so while it
loads other commands get "Please wait", as with opening a drawing.
Templates are never written.

### D8 — What waits

The context menu's Rename, Purge, Redefine from template, Copy to drawing
and Select references in drawing each need a new core operation. Each one
lands when its operation exists, and until then it is hidden, not
disabled. Editing dynamic-block parameters is milestone 04.1. Tool
palettes, DesignCenter and Xrefs are not part of this ADR.

### D9 — Order of work

Each step is its own pull request with tests, as ADR 0023 D5:

1. This ADR, the brief and the roadmap entry.
2. The palette with the Drawing source:
   - the panel, the PROPERTIES and BLOCKSPALETTE commands, Ctrl+1 and Ctrl+3;
   - the ribbon group, the toolbar and the menu;
   - search, grid and list views, the selection and the Selected block section;
   - Insert, Edit block and New wired to INSERT, BEDIT and BLOCK.
3. Previews in Rust and the change set's block fields (contract §6).
4. The dynamic-block tag; Edit block disabled for dynamic blocks.
5. Drag and drop onto the viewport, and INSERT's Explode option.
6. The Recent source.
7. The Template source: reading `.dwt`, copying definitions (D6), the
   clash dialog, the empty state, dropping a file.
8. The context menu's operations, as their core operations land.

## Consequences

- Blocks become usable without knowing their names, the most common work
  in the maintainer's drawings.
- The change set grows by three fields, so the contract, the Rust struct,
  the binding, the `.pyi` stub and the events change together in step 3.
- Copying definitions between documents is the first core operation that
  reads one document while it changes another. It needs Rust tests for
  nesting, dependencies, clashes and undo.
- Recent depends on files outside the drawing, so it must handle files
  that have moved or changed.
- PROPERTIES and Ctrl+1 benefit every user, not only users of the palette.
