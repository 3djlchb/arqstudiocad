# Blocks palette — design brief

Status: proposal from the maintainer's UI mockups (2026-10-22), to be turned
into an ADR before any code. This brief describes *what* the palette does
and how it looks; the ADR decides *how* it is built within the technical
contract. Where this brief and an accepted ADR disagree, the ADR wins and
the conflict is raised with the maintainer.

Related decisions: ADR 0006 (i18n), ADR 0013 (application shell),
ADR 0015 (workspaces), ADR 0016 (editing workflow), ADR 0021 (spatial
index), ADR 0023 (blocks, in particular D2 INSERT and D3a dynamic blocks).

## 1. Purpose

A new dockable palette, **Blocks** (Spanish *Bloques*), where the user sees
the block definitions available to them, picks one, and inserts it into the
drawing by a button or by dragging it onto model space. It complements the
INSERT command (ADR 0023 D2); it never replaces it.

## 2. Sources

The palette shows one source at a time, chosen by a three-way segmented
control at its top:

| Source | Spanish | Shows |
|---|---|---|
| Template | Plantilla | Block definitions in a template or drawing file (`.dwt`, `.dwg`) the user loads. A combo lists recently used templates; a folder button opens a file dialog ("Load template…"). |
| Recent | Recientes | Blocks the user created (BLOCK) or inserted (INSERT, palette) in this session and the previous one, newest first, with when and how ("created here", "from the template", "created in Planta_alta.dwg"). Persisted in the settings, like `recent_files.py`. |
| Drawing | Dibujo | Block definitions in the active drawing. |

Anonymous (`*`) and layout blocks are never listed, as INSERT does not
offer them (ADR 0023 D2).

## 3. Layout (top to bottom)

1. **Panel title**: "Blocks", with only the pin and close buttons, like the
   other panels (ADR 0015, "Look"). Auto-hide works as for every panel.
2. **Source control**: Template · Recent · Drawing. The active segment uses
   the theme's `Button` color with a 2 px bottom line in `Highlight`.
3. **Source row**: depends on the source: the template combo and its folder
   button; a muted caption for Recent; "Definitions in <file name>" for
   Drawing.
4. **Search and view row**: a search field (filters by name and
   description, case-insensitive, as the user types) and a two-button toggle
   for **grid** and **list** view. The chosen view is remembered.
5. **Blocks area** (scrolls, `Base` background):
   - *Grid view*: three columns of cards. Each card holds a preview of the
     block (line drawing on `Base`, light grey strokes; accent strokes
     when selected) and its name below, elided with an ellipsis.
   - *List view*: the shared compact table style (`panel_style.py`): a
     header row, then rows with a small preview, the name, and a right
     column: "In drawing" (number of references) for Template and Drawing,
     "Used" (relative time) for Recent.
   - Selection uses the shared panel selection style (translucent
     background, accent top and bottom lines).
   - *No match*: "No block matches the search."
6. **Footer line** (muted, small): the number of blocks ("13 blocks"), and
   a legend for the dynamic-block tag followed by "drag onto the drawing to
   insert".
7. **Selected block** (a foldable section band, property-grid style):
   Name, Type ("Static" or "Dynamic block · <its parameters>" when they can
   be read), Units, Attributes ("None", "1 attribute", "6 attributes"),
   In drawing ("Not inserted", "1 reference", "6 references"), Source
   (file).
8. **Insert options** (a foldable section band): Scale (default 1.000),
   Rotation (default 0°), "Specify on screen" (checked by default) and
   "Explode". They feed INSERT's existing options.
9. **Actions**: **Insert** (primary), **Edit block** (BEDIT), **New…**
   (BLOCK).

Default docking: on the right, tabified with Properties and Layers; it
moves, floats and docks like every panel, and its state is saved per
workspace (ADR 0015).

### Empty template state

When the Template source has no file loaded: an outlined placeholder icon,
"No template is loaded", a muted line "Load a .dwt or .dwg file to see its
blocks here. You can also drop the file on this panel.", a primary
**Load template…** button, and below it the recently used templates as
buttons.

### Context menu (right-click on a block)

Insert (Enter) · Insert with options… · Copy to drawing — separator —
Edit block · Redefine from template · Rename… — separator — Select
references in drawing · Purge (shown only when the block is unused).

Items that need new core operations (rename, purge, redefine, copy a
definition between documents, select references) are listed here for the
whole design but may land in later steps; until then they are hidden, not
shown disabled without explanation.

## 4. Dynamic blocks

Dynamic blocks must be told apart from static ones at a glance.

> **Changed (maintainer, 2026-10-23):** the tag is a five-pointed star in
> `#004b4e` on `#ffa64d`, the same in every language, instead of the DB/BD
> letters below. See [ADR 0025 D4](../architecture/adr/0025-blocks-palette.md).

- **Tag**: a small label in the **top-left corner of the preview**, in
  both grid and list view.
- **Text**: **DB** in English; the Spanish catalog translates it as **BD**
  (*bloque dinámico*). The text goes through `tr()` like every user-facing
  string, so the language pack decides it; it is never hard-coded per
  language. Its tooltip and accessible name are "Dynamic block".
- **Colors**: orange background, turquoise text, as the maintainer asked.
  The mockup uses background `#ffa64d` and text `#004b4e` (bold, about 9 px
  in grid view, 8 px in list view, 2 px corner radius); this pair keeps the
  small text readable (contrast about 5:1). Brighter turquoises on orange
  fall far below readable contrast at this size. The tag keeps these colors
  in both themes.
- **Where the information comes from**: `Document.is_dynamic_block(name)`
  already exists in the Python API (`crates/arq-python/src/documents.rs`).
  Reading the parameter names for the Type row is optional; if the engine
  does not expose them, the row shows "Dynamic block" alone.
- **Scope limit (ADR 0023 D3a)**: dynamic blocks are shown, inserted and
  exploded in the state the file holds. **Edit block** is disabled for
  them with the existing message "{name} is a dynamic block: it cannot be
  edited yet." as its tooltip. No parameter editing, grips or recomputation
  are built here: that is milestone 04.1, the parametric engine.

## 5. Drag and drop onto model space

- Any block (card or row) can be dragged from the palette onto the
  viewport. The drag carries the block's name and source in an
  application-specific MIME type (for example
  `application/x-arqstudiocad-block`).
- While a block is dragged over model space, the viewport shows it is a
  drop target (a dashed outline and "Drop to insert <name>"). A preview of
  the block following the cursor, as INSERT's drag preview, is preferred if
  it reuses that preview.
- Dropping inserts one reference at the drop point, converted from screen
  to world coordinates, with object snaps applied when they are on, using
  the scale and rotation from Insert options. It goes through the INSERT
  command, so it is one undo step, is echoed in the command window
  (`_INSERT <name>` and the insertion point) and is recorded in Recent.
- A block from a template is first copied into the drawing (its
  definition, with any blocks it nests), in the same undo step as the
  insertion. A name clash with a different definition in the drawing is
  asked about, never resolved silently.
- Dropping a `.dwt` or `.dwg` file on the palette loads it as the
  template.

## 6. Access points

- **Command**: a new command opens and raises the palette, for example
  `BLOCKSPALETTE` with a Spanish name such as `PALETABLOQUES`. Commands
  stay English and get their Spanish name through `commands.json`
  (ADR 0006). The final names are the maintainer's choice.
- **Ribbon** (ADR 0015 data in `workspaces.py`): the Insert tab gains a
  **Palettes** group (Spanish *Paletas*) with Blocks, Layers and
  Properties; Blocks is shown pressed while the palette is visible.
- **Classic**: the Block toolbar holds Insert, Create, Edit and Blocks;
  View › Palettes lists the panels as checkable items, Blocks among them.
- **Shortcut**: the mockup proposes Ctrl+3 (with Ctrl+1 Properties,
  Ctrl+2 Layers). To be confirmed by the maintainer against the existing
  shortcuts.

## 7. Previews

Previews are drawn from each block's drawing primitives by `arq-render`
on the Rust side (as the viewport is, ADR 0013 D5.2–D5.3), fitted to a
square, and cached per definition; they are rebuilt from the
`CadChangeSet` when a definition changes (BSAVE, BLOCK). No Python loop
runs per entity. Previews of a template's blocks are built when the
template is loaded, off the UI thread like background open/save.

## 8. Internationalization

Every string above is an English `tr()` source with a Spanish entry in
`python/arqstudiocad/locales/es/ui.json` (keys in alphabetical order, as
the i18n tests enforce). The panel retranslates live when the language
changes, like Layers (`retranslate()`), and restyles when the theme
changes (`restyle()`).

## 9. Suggested order of work

Each step its own pull request with tests, following ADR 0023 D5's style:

1. **ADR** for the Blocks palette, from this brief: sources, previews,
   drag and drop, template loading and copying definitions, the dynamic
   tag, access points. Roadmap entry under milestone 02.8 or a new one.
2. **Palette with the Drawing source**: dock widget, registration and
   tabification in `main_window.py`, workspace data (ribbon group, toolbar,
   View menu), search, grid/list views, selection, the Selected block
   section, Insert / Edit block / New wired to INSERT, BEDIT and BLOCK.
3. **Previews** rendered and cached in Rust.
4. **Dynamic-block tag** and Edit block disabled for dynamic blocks.
5. **Drag and drop** onto the viewport through INSERT.
6. **Recent** source, persisted.
7. **Template** source: loading a file's definitions, copying a
   definition into the drawing on insert, the empty state, dropping a file
   on the palette.
8. **Context menu** operations that need new core operations, each when
   its core operation exists.

## 10. Acceptance criteria (for the whole feature)

- The palette lists the active drawing's blocks (no anonymous or layout
  blocks), filters by search, switches between grid and list, and keeps
  its dock state per workspace.
- Every dynamic block shows the tag (DB in English, BD in Spanish, through
  the catalogs), and BEDIT stays refused for them.
- Insert and drag-and-drop each create exactly one reference in one undo
  step; undo removes it and, for a template block, the copied definition.
- A template block inserted into a drawing that has a different
  definition with the same name never overwrites it silently.
- Previews update after BSAVE and BLOCK without reopening the drawing.
- Strings are translated, catalogs stay sorted, and both themes show the
  palette correctly.
- Rust tests cover any new core operation; Python tests cover the panel
  model, commands and drag-and-drop (as `test_panels.py` and
  `test_commands.py` do now).

## 11. Mockup reference

The visual reference is the maintainer's design canvas *ArqStudioCAD —
Panel Bloques* (three boards: the main window in the Ribbon layout, the
same in the Classic layout, and the palette's states: list view, context
menu, empty template). Screenshots of these boards live next to this
brief in `docs/design/blocks-palette/`:
[the Ribbon layout](blocks-palette/main-window-ribbon.png),
[the Classic layout](blocks-palette/main-window-classic.png) and
[the palette's states](blocks-palette/palette-states.png). The sample
block names in the mockups (Puerta_090, Mesa_4sillas…) are illustrative
only.

The decisions this brief left open are taken in
[ADR 0025](../architecture/adr/0025-blocks-palette.md).
