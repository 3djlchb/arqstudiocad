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
  be inserted, and its tooltip says why. *(maintainer, 2026-10-25)*:
  - The list keeps at most 30 blocks.
  - A block whose file is open in a tab is taken from that drawing, unsaved
    changes included, not from the file on disk.
  - A block of a drawing never saved leaves the list when that drawing is
    closed: no file holds it. Saved later, it comes from its new file.

Anonymous (`*`) and layout blocks are never listed, as INSERT does not
offer them (ADR 0023 D2). Blocks AutoCAD makes by itself under ordinary
names, such as dimension arrowheads (`_ArchTick`, `_Open90`…), are listed
like any other, as AutoCAD lists them *(maintainer, 2026-10-22)*. By default it docks on the right, tabbed with
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
both views: a **five-pointed star** filled in `#004b4e` on a `#ffa64d`
square with a 2 px corner radius. A symbol reads the same in every language,
so it replaces the letters the brief proposed (DB in English, BD in Spanish)
*(maintainer, 2026-10-23)*. Its tooltip and accessible name, "Dynamic
block", are translated. Its colors are fixed. They stay the same in both themes, an exception to ADR 0015's
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

**Dynamic blocks in a clash** *(maintainer, 2026-10-25)*: until the
parametric engine (milestone 04.1), a dynamic block that clashes, on
either side, can only be used as the drawing has it. Redefine and Copy
with another name are offered disabled, with the reason. A dynamic block
that does not clash is copied whole, its parameters and actions included.

**The dialog shows both blocks** *(maintainer, 2026-10-25)*: each clashing
block has two 96 px previews, the template's block on the left and the
drawing's on the right, so the user sees the difference before choosing.

**BLOCK checks the template too** *(maintainer, 2026-10-25)*: creating a
block, with BLOCK or the palette's New…, checks the name in two places.
- A name the drawing uses is refused, and the message says it is in the
  drawing.
- A name only the loaded template uses gets a warning, and BLOCK goes on:
  they are two files with no clash yet, and inserting that block from the
  template will ask.
- With the interface, both are dialogs as well as command-line messages
  *(maintainer, 2026-10-25)*: a warning with OK, after which BLOCK asks for
  the name again; and a note with **Continue** and **Another name**.

### D7 — Reading templates

A `.dwt` is a DWG file with another extension: the core opens it as DWG,
and saves one so when asked. Loading a template uses the background work
of ADR 0013 D5.7, so while it loads other commands get "Please wait", as
with opening a drawing. The palette never writes a template it loaded.

**A sample template of our own** *(maintainer, 2026-10-24)*: ArqStudioCAD
ships `resources/templates/arqstudiocad-sample.dwt`, ten simple
architectural blocks (door, window, table with chairs, bed, toilet,
washbasin, column, tree, north arrow, level mark) drawn in metres on
coloured `A-` layers. It is generated by `tools/make_sample_template.py`,
never copied from another application's templates (AutoCAD's are
Autodesk's), and dedicated to the public domain (**CC0 1.0**), so anyone
may use its blocks in any drawing. The Template source offers it when no
template is loaded. A full template system (title blocks, sheets, scales,
plot styles, NEW starting from a template) belongs to phase 04, and
discipline layer standards to phases 05 and 07; neither is built here.

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
8. The context menu's operations, as their core operations land. The
   maintainer split it on 2026-10-25:
   - 8a: editing a block by a double click on it in the drawing, through
     the Edit Block Definition dialog (ADR 0023 D3);
   - 8b: the context menu with the operations that exist (Insert, Insert
     with options, Copy to drawing, Edit block, Redefine from template);
   - 8c, 8d, 8e: Rename, Select references and Purge, each with its new
     core operation.

## Implementation notes

- **Step 2, first part (2026-10-22, PR #52):**
  - the palette lists the drawing's blocks, with a search;
  - its View-tab button toggles it and carries the selection focus while it is open.
- **Step 2b (2026-10-22):** the palette takes the mockup's layout:
  - the sources, with only Drawing enabled; Template and Recent say "Not available yet." until their steps;
  - the caption "Definitions in <drawing>";
  - the search, with the grid and list views (the grid waits for previews, step 3);
  - the list's Name and In drawing columns;
  - the Selected block section, in the Properties panel's property grid.
- The core answers with two new queries:
  - `block_info`: the name, description, insertion units (INSUNITS code), attribute definitions and whether the block is dynamic;
  - `block_references`: the references of every block, counted in one pass over the drawing.
- References count anywhere in the drawing: model and paper space, and inside other blocks.
  - A dynamic block's references through anonymous copies (`*U…`) count for it when the file links the copy to the block, either from the reference or from the copy. AcadRust reads both kinds of link.
  - A copy the file does not link to any block counts for no listed block. In the maintainer's drawings such copies hold plain geometry: no link to read, no count to give.
- The palette reads the drawing only while it is open. Closed, it waits and catches up when shown again, since counting references reads the whole drawing.
- **Step 2c (2026-10-22):** Insert options and actions.
  - Insert options: a foldable band with Scale, Rotation (in degrees) and Specify on screen.
  - The buttons Insert (the primary action), Edit block and New…
  - Enter or a double click on a block inserts it.
  - Insert and Edit block start INSERT and BEDIT with the block already named (`insert_named`, `edit_named`), so they behave as typed: one undo step, the same refusals.
  - On screen, INSERT follows the cursor and still offers Scale and Rotate. Otherwise the block goes at the origin at once.
  - The command line shows "Inserting block <name>.", since a button types nothing.
  - New… starts BLOCK.
  - Explode on insert stays in step 5 with drag and drop.
- **Step 2d (2026-10-22):** PROPERTIES (alias `PR`, Spanish `PROPIEDADES`) and Ctrl+1 open the Properties panel, with or without a drawing.
  - The View tab's Palettes group holds Blocks, Layers and Properties. Each button toggles its panel and carries the focus while the panel is open.
  - LAYER keeps needing a drawing. Its button, which the Home tab shares, does not stay pressed when LAYER refuses.
- **Step 3 (2026-10-22):** previews and the grid view.
  - `CadChangeSet` names the block definitions a change created, modified (its contents) and deleted (contract §6, amended). The fields are found at commit by comparing the block tables, inverted by undo and merged across steps.
  - `Document::block_drawing` draws a block as a reference at the origin would, every layer shown. `arq_render::preview` fits that drawing to a square, in one ink on transparency.
  - Each preview keeps the colors of its layers, as the viewport draws them over the theme's Base color (ACI 7 white on dark, black on light); `preview` can still ink a drawing in one color. A selected card shows its selection by the card's frame, not by tinting the drawing *(maintainer, 2026-10-23, instead of the mockup's grey strokes with accent when selected)*.
  - Previews are cached in the palette per drawing, size and block. A change that names any block forgets that drawing's previews, since one block may hold another; so does a theme change.
  - The grid is the default view, names elided. Its previews are a fixed 64 px and the list's 24 px, so a wider palette holds more columns rather than bigger cards, and resizing it never draws a preview again *(maintainer, 2026-10-23, instead of the mockup's three cards to a row)*. The list shows a small preview by each name. One selection drives both views, and the chosen view is remembered in the settings.
- **Step 4 (2026-10-23):** the dynamic-block tag.
  - A dynamic block's previews, plain and selected, carry the tag in their top-left corner: a five-pointed star in `#004b4e` on a `#ffa64d` square with a 2 px radius, 13 px on cards and 10 px in rows, in both themes. It is painted, not written, so it needs no font and no translation.
  - The card's tooltip and the accessible description say "Dynamic block", translated. The footer shows the tag itself and "Dynamic block" while a dynamic block is shown.
  - Edit block is disabled for a dynamic block, with BEDIT's refusal as its tooltip.
  - The Type row lists the parameters the block shows (`BlockInfo::parameters`, from AcadRust's dynamic properties), or says "Dynamic block" alone when none can be read.
  - The legend's "drag onto the drawing to insert" comes with drag and drop in step 5.
- **Step 5 (2026-10-25):** drag and drop, and Explode.
  - A card or a row drags `application/x-arqstudiocad-block` carrying its drawing and name. Only the viewport of that drawing accepts it; a template's or a recent block waits for the copy of step 7.
  - Over the drawing, the palette starts INSERT on screen with its insert options, so the block follows the cursor as INSERT's own preview, snapped and ORTHO as at a click, under a dashed frame and "Drop to insert <name>".
  - Dropping picks the insertion point where the block lands: one undo step, echoed as INSERT. Dragging away cancels that INSERT and nothing is left.
  - **Explode**, a new insert option, replaces the reference by its contents in the same undo step (ADR 0023 D1 for attributes), from the Insert button and from a drop.
  - The footer says "Drag a block onto the drawing to insert it."
- **Step 7a (2026-10-25):** copying a definition between documents (D6), in the core.
  - `Document::copy_block` copies a block from another document, with:
    - the blocks it nests;
    - the layers, linetypes, text styles and application names they use that the drawing lacks.
  - `block_clashes` lists the blocks whose names the drawing uses for other contents; `copy_block` takes a choice for each and refuses to run while one is missing.
  - A block with the same contents (base point and entities, ignoring handles) is used as the drawing has it. The blocks it nests are not asked about.
  - Use the drawing's, Redefine (every reference is redrawn) and Copy with another name work as D6 says. References inside the copy follow a renamed block.
  - A dynamic block keeps its parameters and actions: AcadRust moves the definition's object graph (`export_block_graph` / `import_block_graph`).
  - A copied layer takes the drawing's material and plot style of the same name. A standard one the drawing does not store (Global, ByLayer, ByBlock; Normal) becomes the drawing's implicit default, so the copy loses nothing.
  - What a copy cannot carry whole is refused with its reason, and the drawing is left unchanged:
    - entities other than plain geometry, texts, hatches, attribute definitions and block references (dimensions, multilines, leaders, images…);
    - links to materials, plot styles, visual styles or color books;
    - anonymous or external nested blocks;
    - a material or plot style the drawing does not have;
    - a dynamic block whose graph AcadRust cannot move.
  - In the maintainer's drawings, 45 of 51 blocks copy, and every copy saves and reads back. The other six are dynamic blocks whose graph cannot be moved yet.
  - One undo step, or part of INSERT's group: `CadApplication::copy_block` copies from a snapshot of the source document. The Python API is `CadDocument.copy_block(name, source, keep, redefine, rename)` and `block_clashes(name, source)`.
- **Step 7b (2026-10-25):** the Template source.
  - The Template segment works; Recent still says "Not available yet." (until step 6).
  - Its source row has:
    - a combo of the templates used last, newest first, with the sample template always offered;
    - a folder button, "Load template…".
  - The templates used are kept in the settings (at most eight).
  - With no template loaded, the palette shows the mockup's empty state:
    - "No template is loaded" and how to load one;
    - a primary **Load template…** button;
    - the templates used last as buttons.
  - A template loads in the background as OPEN does, so other commands get "Please wait". The file dialog is a new interface service (`choose_template_path`).
  - A loaded template:
    - is never shown in a tab nor written;
    - replaces and closes the previous one;
    - stays loaded when the active drawing changes.
  - A file that cannot be read leaves the loaded template as it was, and says why.
  - Dropping one `.dwt`, `.dwg` or `.dxf` file on the palette loads it.
  - The template's blocks show as the drawing's do, with previews and the dynamic-block tag. "In drawing" counts the active drawing's references to a block of that name, ignoring case. The Selected block section names the template file as its Source.
  - Edit block is disabled for a template's blocks, with the reason as its tooltip.
  - Insert, Enter, a double click or a drop onto the drawing copies the block (D6) and inserts it, in one INSERT undo step.
  - A block whose name clashes with a different block of the drawing is not inserted yet: INSERT stops and names the clashing blocks. The clash dialog comes with step 7c.
- **Step 6 (2026-10-25):** the Recent source, built on step 7's copy.
  - BLOCK and INSERT (typed or from the palette) publish a new event, `BlockUsed`: the block, the document its definition came from, that document's file, and how it was used (created, inserted, or inserted from the template). Contract §7 lists it.
  - The palette keeps the list (`RecentBlocks`, no Qt) in the settings as JSON, with a session number counted at each start: blocks of the session before the previous one are dropped on reading.
  - Each row shows its preview, its name and when it was used ("2 min ago"); the list's second column is Used instead of In drawing. The tooltip and the Selected block section say how, as the brief: "Created here", "Created in Planta_alta.dwg", "From the template …", "Inserted from …".
  - A block is found in the open drawing it came from, else in the open drawing or template of its file, else in its file read for the palette. Those files are read in the background when Recent is shown, as a template is, and only while no command is at work. They are never in a tab nor written, and are read again when they change on disk.
  - A file not read yet is read by INSERT itself, in the same command; a drop waits for the drag to end, as a clash does.
  - A block that cannot be had is greyed, without preview, and cannot be inserted nor dragged. The tooltip, the Insert button and the Selected block section say why: the file was not found, cannot be read, or no longer has the block. A file that cannot be read is tried again when Recent is chosen again.
  - Inserting a recent block from another file copies it, with the clash dialog of step 7c; one of the active drawing is inserted as it is.
  - Recent may list two blocks of one name from different files: the grid and the list follow each other by row, and drags carry the row.
- **Step 7c (2026-10-25):** the clash dialog.
  - INSERT from a template asks about every clashing block, nested ones included, before copying. Cancel changes nothing.
  - Each clash shows, as the maintainer asked:
    - the template's block on the left and the drawing's on the right, 96 px each, framed as the palette's cards;
    - the three choices: **Use the drawing's** (the default), **Redefine from the template**, and **Copy with another name**, which proposes `Name_2`.
  - A new name must be valid, free in the drawing, and not chosen twice. Until it is, Insert is disabled and the dialog says why.
  - A dynamic block, on either side, can only be used as the drawing has it. The other two choices are disabled, with the reason.
  - Without a dialog, the command line asks the same questions.
  - A template's block that clashes cannot be asked about during a drag. INSERT starts when the block is dropped, and places it at the drop point after the dialog, found as a click finds a point (object snaps, ORTHO).
  - BLOCK checks the name against the drawing and the loaded template, as decided in D6. The session knows the palette's template (`Session.template`).
  - The UI service `resolve_block_clashes` takes the `BlockClash` records and returns a `ClashChoice` for each.
- **Step 8a (2026-10-25):** a double click on a block reference opens the Edit Block Definition dialog with that block chosen, then the block editor on its grey background (ADR 0023 D3). The new UI service `choose_block_to_edit` replaces BEDIT's plain list; INSERT keeps its own.
- **Step 8b (2026-10-25):** the context menu, opened by a right click on a block, the Menu key or Shift+F10, in the grid and the list; off the blocks nothing opens. It shows what the source allows *(maintainer, 2026-10-25)*:
  - **Insert** and **Insert with options…** always. The latter opens a small dialog with Scale, Rotation, Specify on screen and Explode, filled with the palette's options, and inserts once with its own, leaving the palette's as they are.
  - **Copy to drawing** for a block of another file (the template, or a recent block not of the active drawing). It copies the definition without inserting it, through a new command, **COPYBLOCK** (Spanish `COPIARBLOQUE`): one undo step, clashes asked about as for INSERT, and nothing copied when the drawing already has the same block. Typed, COPYBLOCK copies a block of the palette's template.
  - **Edit block** for the drawing's blocks, as the button.
  - **Redefine from template** when the drawing's block of that name differs, neither of them dynamic. A dialog shows both blocks, the template's on the left, and how many references will change; Redefine runs COPYBLOCK with that choice, one undo step.
  - Without a drawing, or for a recent block that cannot be had, the items that need one are disabled.
- **Step 8c (2026-10-26):** renaming a block *(maintainer, 2026-10-26)*:
  - The core operation `Document::rename_block` renames the block record and every reference to it: in model and paper space, and inside other blocks. Changing only the case is allowed. One undo step, named RENAME. The change set names the old block deleted and the new one created, as D3 says. Python: `CadDocument.rename_block(name, new_name)`.
  - It refuses:
    - a name in use or invalid;
    - a block INSERT cannot place;
    - a dynamic block, whose anonymous references wait for milestone 04.1.
  - The command is **RENAME** (alias `REN`; Spanish `CAMBIANOM`), as AutoCAD's. For now it renames blocks only; other named objects can join it later.
  - RENAME opens a small Rename dialog through the new UI service `rename_block`: the block and its new name. OK stays disabled, with the reason, while the name cannot be taken. A block open in a block editor is refused until that editor is closed.
  - **-RENAME** (alias `-REN`; Spanish `-CAMBIANOM`) asks at the command line: the object type (Block), the old name, then the new name.
  - The palette's context menu gains **Rename…** for the drawing's blocks. It is disabled for a dynamic block, as Edit block.
  - After a rename, the session publishes `BlockRenamed`. The palette keeps the block selected under its new name, and the Recent source renames that drawing's entries.
- **Step 8d (2026-10-26):** selecting a block's references *(maintainer, 2026-10-26)*:
  - The core query `Document::find_block_references` finds a block's references in model space that can be selected, in the order drawn, as a click finds them: those that draw something. A reference on an off layer still shows what its block holds on other layers, so it counts as selectable.
  - It also counts the references that cannot be selected:
    - those that draw nothing (frozen or off layers);
    - those inside other blocks, which are selected as a whole;
    - those in layouts.
  - A dynamic block's anonymous references count for it, as in the palette's count. Python: `CadDocument.find_block_references(name)`.
  - **Copies that lost their link** *(maintainer, 2026-10-26, from a drawing saved by progeCAD)*:
    - An application that drops dynamic-block data when it saves keeps the anonymous copies (`*U…`) its references draw. The link to their block can be lost, as in the maintainer's drawing, where 18 columns referenced `*U52` and not "Columna general".
    - A copy the codec cannot link counts for a block INSERT can place when either:
      - its true name (`AcDbDynamicBlockTrueName`) names that block; or
      - exactly one such block has the same edit tag (`AcDbBlockRepETag`) and draws the same.
    - A named block is never taken for another's copy. The file is not changed.
    - Both the selection and the palette's count use this, and the budget tables of milestone 02.11 will too. Editing dynamic blocks still waits for milestone 04.1.
  - **SELECTBLOCK** (Spanish `SELECBLOQUE`) asks for the block's name. It replaces the selection with the references, without moving the view, and says how many were selected and how many could not be.
  - The palette's context menu gains **Select in drawing** for the drawing's blocks, dynamic ones included. It runs SELECTBLOCK with the block chosen.
  - The maintainer intends these selections to feed budget tables; the block data they need is milestone 02.11.
- **Step 8e (2026-10-26):** purging unused blocks *(maintainer, 2026-10-26)*:
  - The core operation `Document::purge_blocks(names, nested)` removes each block's contents, its BLOCK and ENDBLK, every object owned by any of them or by its record (extension dictionaries and what they hold), and the record itself. One undo step, named PURGE.
  - `Document::unused_blocks` lists the blocks INSERT can place that nothing references, anywhere, including through anonymous copies (step 8d). Dynamic blocks are listed, but `purge_blocks` refuses them until milestone 04.1. It also refuses a block in use. Python: `CadDocument.unused_blocks` and `purge_blocks(names, nested=True)`.
  - With **Purge nested items**, as AutoCAD's, the blocks a purged block held also go once nothing else uses them, and so on down.
  - A purged drawing passes the codec's audit with no new issue in memory on the test drawings. Saved and read back, the maintainer's real drawings keep exactly the issues they had. The codec still keeps index entries for removed records and entities, so an audit in memory flags them after purging a block from a real drawing. Clearing those entries needs an API in the codec fork.
  - **PURGE** (alias `PU`; Spanish `LIMPIA`) opens a Purge dialog through the new UI service `purge_blocks`. It lists the unused blocks with a box and a preview, and holds Purge nested items (checked by default), **Purge**, **Purge All** and **Close**. A dynamic block is listed without a box, saying why. Only blocks are purged for now; layers, styles and linetypes can join later.
  - **-PURGE** (alias `-PU`; Spanish `-LIMPIA`) asks at the command line for the object type (Blocks), the names (wildcards, `*` for all) and whether to purge nested items.
  - A block open in a block editor is not purged, and the command says so.
  - The palette's context menu gains **Purge…** for the drawing's blocks. It is enabled only for an unused block that is not dynamic, and asks first through `ask_purge_block`.
  - With this step, every step of D9 is done.

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
