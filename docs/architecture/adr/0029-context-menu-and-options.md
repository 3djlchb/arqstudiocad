# 0029. The model-space context menu and the Options dialog

Date: 2026-10-29 (accepted 2026-10-31) · Status: Accepted · Builds on [0006](0006-i18n.md),
[0013](0013-application-shell.md), [0015](0015-workspaces.md),
[0016](0016-editing-workflow.md) and [0023](0023-blocks.md) (the block
editor's background). The maintainer's design briefs:
[model-space context menu](../../design/modelspace-context-menu.md) and
[Options dialog](../../design/options-dialog.md), with their images.

## Context

**What the briefs ask for:**

- A right-click on empty model space opens a short menu: **Repeat
  {command}** and **Options…**, as AutoCAD's default shortcut menu.
- **Options** (OPTIONS) is one dialog for the application's and the
  drawing's settings. It has nine sections in a sidebar, each subsection
  a titled group box, with OK, Apply, Cancel and Reset section.
- The briefs leave the open questions (their §11) to the maintainer. The
  maintainer decided them on 2026-10-29.

**What the code has (main `76950dc`):**

- **The right button is free.** The viewport handles the left button
  (pick, select, grips) and the middle one (pan); nothing uses the right.
- **The last command exists, but privately.** An empty Enter repeats the
  last command that repeats: `CommandRunner._last`, set in `start()` for
  commands with `repeat=True`. Nothing outside the runner reads it.
- **The cursor is the system's cross** (`Qt.CursorShape.CrossCursor`), not
  drawn by the program. Its size, type and color cannot be set.
- **The viewport repaints cheaply.** It draws a cached image of the
  drawing (rendered by the core) and paints selection rectangles, grips
  and snap markers over it.
- **Fixed values that a setting could replace:**
  - the wheel's zoom step (`WHEEL_STEP = 1.25`, 25 % per notch);
  - the block editor's background (`BLOCK_EDITOR_BACKGROUND`, a mid grey,
    meant to be set in Options: maintainer, 2026-10-25);
  - the snap marker colors.
- **Language and theme are set from the Tools menu.** They are kept with
  `application.open_settings()` (`QSettings`: the registry, or an INI file
  in portable mode). The saved language falls back to the system's
  (`choose_language`).
- **Missing, which most of the dialog's settings need:**
  - autosave (SAVETIME), `.bak` copies (ISAVEBAK), a default template;
  - SpaceMouse, plotting, polar tracking, a grid, layouts' paper colors;
  - the 3D display variables;
  - the dialogs behind the "…" buttons (plug-ins, shortcuts, aliases,
    drafting settings);
  - an API to change the drawing's header variables (the core reads
    ISOLINES only);
  - the GPU viewport: the viewport draws with the CPU, and the wgpu probe
    (`gpu.py`) is only a check (milestone 02.7, on hold).

## Decision

### D1 — The context menu *(maintainer)*

**When it opens:**

- A right-click in model space with **no command running and nothing
  selected** opens the menu at the cursor, kept inside the viewport.
- **Over an object too.** A right-click on an object with nothing
  selected opens this menu, as AutoCAD does, without selecting it.
- **With a selection:** no menu yet. The object menu (Cut, Copy, Erase,
  Properties…) is later work.
- **From the keyboard:** the Menu key and Shift+F10, with the viewport
  focused, open it at the cursor, or at the viewport's center when the
  cursor is outside.

**What it holds:**

1. **Repeat {command}**:
   - `{command}` is the last command that repeats, named in the active
     language (Repeat LINE / Repetir LINEA);
   - **Enter** is its shortcut hint, and it is the default item;
   - it starts exactly what an empty Enter starts.
   - **Before any command in the session**, the item reads "Repeat last
     command" and is **disabled**: the menu keeps its shape.
2. A separator.
3. **Options…**, which runs OPTIONS.

**What it needs from the runner:** a read-only `CommandRunner.last`. The
last command is the session's, not the drawing's: it does not change when
another drawing becomes active.

**Later:** AutoCAD's Recent Input list of the last commands.

### D2 — Right-click during a command acts as Enter *(maintainer)*

**What it does:** while a command runs, a right-click in the viewport acts
as Enter, as AutoCAD's default:

- it ends LINE or PLINE;
- it accepts a prompt's default;
- it closes a selection.

**What it does not do:** it never opens this menu during a command. A
menu of the command's own options is later work.

### D3 — The Options dialog *(maintainer)*

**Opening it:**

- the command **OPTIONS**, alias **OP** (AutoCAD's), Spanish **OPCIONES**;
- Tools › Options…;
- the context menu's Options….

**Behavior:**

- **Modal:** the drawing cannot change while it is open.
- About 900 × 640 and resizable. It remembers its size and the last
  section shown.

**Layout:** as the brief shows.

- **Sidebar:** a list of tall entries, the current one filled with
  `#0b6fa6` and white text (5.5:1 contrast).
- **Content:** scrolls. Each subsection is a `QGroupBox` titled with its
  name, made of rows of a muted label and its control, with help text
  under a control in a smaller muted font. Some group boxes have
  **Restore defaults** at their top right.
- **Action row:**
  - **Reset section**: the section's defaults, pending until applied;
  - **OK**: the primary button, taking the free width; applies and
    closes;
  - **Apply**: applies and stays open; enabled only with pending changes;
  - **Cancel**, Esc or the close button: discards the pending changes.
- **The sidebar switches sections** without losing pending changes.

**Keyboard:**

- Ctrl+Tab and Ctrl+Shift+Tab, or Ctrl+PgDn and Ctrl+PgUp, move between
  sections.
- Enter is OK; Esc is Cancel.

**Labels:** they keep AutoCAD's variable names in parentheses, untranslated,
as the mockups show: "Reverse wheel zoom (ZOOMWHEEL)".

### D4 — Only working settings are shown *(maintainer)*

**What is shown:** a setting appears only when the feature behind it
exists. A section with no setting yet is not listed. Each one joins the
dialog in the step that builds its feature, so nothing in the dialog does
nothing.

**The first version shows:**

| Section | Group | Settings |
|---|---|---|
| General | Language | Interface language: System language, Español, English |
| General | Theme | Dark, Light (the Tools menu keeps its items too) |
| Display | Crosshair | Size 1–100 % (default 5), Type (Crosshair, Arrow, Small cross), Crosshair color, Object-snap marker color, Command-line text color, the live preview (D6) |
| Display | Block editor | Background color (default the current mid grey) |
| User preferences | Zoom | Reverse wheel zoom (ZOOMWHEEL), Zoom factor in percent per notch (default 25, today's step) |
| User preferences | Right-click in model space | With nothing selected: Show the context menu, or Repeat the last command; During a command: Acts as Enter |

**Notes on these settings:**

- "During a command" offers only Acts as Enter until a command's options
  menu exists. It is shown so the setting is visible where it will grow.
- **Hidden for now**, with the features they wait for:
  - Files: locations and the default template;
  - Open and save: the default save format (contract §10 keeps a
    drawing's own version);
  - Display: autosave, the drawing area's mode and paper colors (no paper
    view yet), the grid, lineweight display;
  - the Drawing section, which needs a header-variable API and polar
    tracking;
  - 3D modeling, which waits for the 3D viewport;
  - Selection: PICKADD, PICKDRAG, selection-area effects;
  - SpaceMouse and the navigation style;
  - Graphics: wgpu drawing (02.7, on hold);
  - the "…" dialogs.

### D5 — Where settings are kept

**The first version's settings belong to the user:**

- They are kept in `application.open_settings()`, the registry or the
  portable INI file, under an `options/` group.
- They are not undone; they apply at once when applied and last across
  restarts.
- **Language and theme keep their existing keys** (`language`, `theme`),
  shared with the Tools menu.

**Drawing settings, later:** a setting stored in the drawing (SNAPANG,
ISOLINES…) changes the drawing in one undo step through the core's API.
None is in the first version.

**Changes are announced:** one `OptionsChanged` event carries the keys
that changed, so the viewport, the command window and the panels follow.

### D6 — The crosshair drawn by the program *(maintainer)*

**Who draws it:** the viewport draws its own crosshair, so it takes its
size, type and colors from Options (maintainer, 2026-10-29). While the
cursor is over the drawing, the system cursor is hidden; panning keeps the
hand.

**Its shape:**

- **Crosshair** is two lines crossing at the cursor. Each arm is the
  size's percent of the viewport's half-width or half-height; at 100 %
  they cross the whole viewport (AutoCAD's CURSORSIZE).
- **The pick box** (`PICK_PIXELS`) is drawn at the center, the arms broken
  around it.
- **Arrow** is the system arrow; **Small cross** is a short fixed cross
  with the pick box.

**Its color:**

- The chosen `#RRGGBB`.
- Empty means automatic contrast: light on a dark background, dark on a
  light one, so the block editor's grey is covered too.

**How it paints:** it is an overlay on the cached image of the drawing. A
mouse move repaints only the old and new crosshair, never the drawing, as
the reference machine needs (ADR 0013 D5.8).

**The preview** in Options paints the same crosshair, live before Apply:

- on the viewport's background;
- at the chosen size, type and color.

### D7 — Steps

Each step is its own pull request with tests:

1. **This ADR and the roadmap row.** ADR 0028 is accepted, and milestone
   02.12 marked done.
2. **The context menu** (D1, D2):
   - `CommandRunner.last`;
   - the right button: the menu, or Enter during a command;
   - the Menu key and Shift+F10;
   - OPTIONS / OP / OPCIONES and Tools › Options… open a first dialog
     shell.
   - Done. A shell with no section would show nothing (D4), so the
     maintainer chose (2026-10-29) to bring forward the dialog's
     structure (D3) and its General section (language and theme).
   - **What remains for step 3:** User preferences, `OptionsChanged`,
     and Ctrl+Tab between sections, which needs a second section to do
     anything.
   - **Shift+F10:** not every platform reports it as a context-menu
     event, so the viewport handles the key itself. A keyboard
     context-menu event within half a second of it is not taken as
     another request.
   - **Repeat** sends an empty line through the command window, so its
     echo is an empty Enter's. Text typed on the command line is dropped
     first.
3. **The dialog** (D3, D4, D5):
   - the sidebar, the group boxes and the action row with its semantics;
   - General (language, theme);
   - User preferences › Right-click, which the menu then reads;
   - `OptionsChanged`.
   - Done, with Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PgDn and Ctrl+PgUp
     between sections, wrapping around.
   - **The right-click setting** is kept as `options/right_click`
     (`menu` or `repeat`); a value this version does not know reads as
     the default. "Repeat the last command" is exactly an empty Enter:
     with no last command it does nothing.
   - **The Menu key and Shift+F10** always open the menu, whatever the
     setting: they ask for a menu, and keep Options… reachable from the
     keyboard (maintainer, 2026-10-29).
   - **During a command** shows its one choice, Acts as Enter, in an
     enabled list (maintainer, 2026-10-29); the command's options menu
     adds the second.
   - **`OptionsChanged`** carries the changed settings by the dialog's
     field names (`language`, `theme`, `right_click`). OK or Apply
     publishes one for everything applied; the Tools menu's language and
     theme items publish one each. `LanguageChanged` is still published
     for a new language.
4. **The crosshair** (D6):
   - the viewport draws it;
   - Display › Crosshair with the live preview, the snap-marker and
     command-line colors;
   - Display › Block editor background.
   - **In two pull requests** (maintainer, 2026-10-30): 4a, the viewport
     draws the crosshair, and 4b, the Display section with its preview,
     the colors and the block editor's background.
   - **4a done:** `arqstudiocad.ui.crosshair` holds the shape and the
     automatic color, with no widget, so the preview paints the same
     crosshair.
     - The pick box is always drawn. A size of 100 % crosses the whole
       viewport wherever the cursor is.
     - Small cross: arms of 8 px beyond the pick box, whatever the size.
     - Automatic color: white or black by the background's relative
       luminance, so the block editor's grey gets white.
     - The crosshair is hidden while panning and when the cursor or a
       dragged block leaves the drawing. It follows a block dragged from
       the palette.
     - A mouse move repaints one thin strip per line, where the crosshair
       was and where it goes.
   - **For 4b** (maintainer, 2026-10-30): an empty object-snap marker
     color keeps AutoCAD's yellow, as the mockup shows; the swatches open
     Qt's color dialog, since these are interface colors, not CAD colors.
   - **4b done:** Display holds Crosshair (size with its slider, number
     and live preview; type; the crosshair's, snap markers' and command
     line's colors) and Block editor (its background color).
     - Settings are kept under `options/` (`crosshair_size`,
       `crosshair_type`, `crosshair_color`, `marker_color`,
       `command_line_color`, `block_editor_background`). A value this
       version does not take reads as its default.
     - A color is `#RRGGBB` or empty; the block editor's background is
       never empty. Text that is not a color marks the field and keeps
       its last valid value.
     - An empty command-line color is the theme's text color.
     - The window applies them at once to every viewport (a block
       editor's background too) and to the command line, and to every
       viewport opened later.
     - The preview's caption reuses "Preview" ("Vista preliminar"), as
       the block dialogs do.
     - Labels are 200 px wide and wrap, so every group's controls line up.
5. **Zoom:** User preferences › Zoom (ZOOMWHEEL and the zoom factor).
   - Done. The Zoom group comes before Right-click, as the brief shows.
   - **Reverse wheel zoom (ZOOMWHEEL):** a check box across the group;
     forward zooms out when it is on.
   - **Zoom factor:** a slider and a number kept in step, "% per notch",
     from 5 to 100 % (maintainer, 2026-10-31), default 25 %, the step it
     had. The number is the percent itself, not the mockup's 60 → 10 %
     scale, as D4 decided.
   - Kept as `options/zoom_wheel_reversed` and `options/zoom_factor`; the
     window applies them to every viewport, and to those opened later.

Later steps add each hidden setting with its feature (D4).

## Consequences

- **The right button gets AutoCAD's two habits:** a menu on empty model
  space, and Enter during a command.
- **Options has a fixed shape, ready to grow.** Every later setting knows
  where it goes and how it is kept and undone.
- **A drawn crosshair:** a little more painting per mouse move, kept to
  the overlay. In return it has a size, a type and a color, and it is the
  same on every platform.
- **Nothing in Options pretends.** Missing features stay out of sight
  instead of being shown disabled.
