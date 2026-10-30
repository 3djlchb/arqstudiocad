# Model-space context menu — design brief

Status: proposal from the maintainer's mockups (2026-10-22).
Related decisions: ADR 0006 (i18n), ADR 0013 (application shell: commands
and command window), ADR 0016 (editing workflow). Companion brief:
`options-dialog.md` (the dialog this menu opens and the setting that
controls this menu).

## 1. Purpose

Right-clicking empty model space gives quick access to repeating the last
command and to the Options dialog, as AutoCAD's default shortcut menu does.
It complements Enter on an empty command line (which already repeats the
last command) and the Tools menu; it replaces nothing.

## 2. Where it lives and how it opens

- **Kind**: context (shortcut) menu of the viewport.
- **Opens**: right-click on model space where no object is under the
  pickbox, with no command running and nothing selected. The viewport does
  not use the right button today (`viewport.py` handles left and middle),
  so nothing has to move.
- **Setting**: Options › User preferences › *Right-click in model space*
  can change "With nothing selected" to "Repeat the last command" (then
  right-click repeats directly, no menu) — see `options-dialog.md`.
- No command of its own.

## 3. Layout (top to bottom)

1. **Repeat {command}** — icon (circular arrow), text, and the shortcut
   hint **Enter** right-aligned. `{command}` is the last command that
   repeats, in the active language (LINE / LINEA). Highlighted on open as
   the default item.
2. Separator.
3. **Options…** — icon (gear).

The menu opens with its top-left corner at the cursor, kept inside the
viewport.

## 4. States

| State | What is shown |
|---|---|
| A command was run before | "Repeat {command}" enabled (`01-context-menu.png`, Repeat LINEA). |
| No command run yet in this session | "Repeat last command" disabled. |
| Last command does not repeat (`repeat=False`, e.g. a grip edit) | It is not offered; the item shows the last command that does repeat, or disabled as above. |
| A command is running | No menu: right-click acts as Enter by default (setting "During a command"); out of scope here except for not opening this menu. |
| Something is selected, or an object is under the cursor | Not this menu (object menu: later, §10). |
| Hover | Item gets the shared hover background. |
| Light theme | Same structure; Qt palette colors. |

## 5. Actions and events

| # | Trigger | Where | What happens | Command run | Undo step | Command-line echo | Updates |
|---|---|---|---|---|---|---|---|
| 1 | Right-click empty model space (no command, no selection) | Viewport | Opens the menu at the cursor | — | none | — | — |
| 2 | Click "Repeat {command}" or press Enter while the menu is open | Menu | Exactly as an empty Enter: starts the last repeating command | {command} | per command | `Command: {command}` and its first prompt (mockup: "Specify first point:") | Command window |
| 3 | Click "Options…" | Menu | Opens the Options dialog | OPTIONS | none | — | — |
| 4 | Esc, click elsewhere, or right-click elsewhere | Anywhere | Closes the menu (a right-click elsewhere reopens it there) | — | none | — | — |
| 5 | Right-click with the setting "Repeat the last command" | Viewport | No menu; repeats directly, as #2 | {command} | per command | as #2 | — |

Reacts to: **language changed** (item texts and command names
retranslate); **theme changed** (Qt palette); **drawing switched** (the
last command is the session's, not the drawing's).

## 6. Keyboard

- Menu key or Shift+F10 with the viewport focused (no command, no
  selection) opens the menu at the cursor position, or at the viewport's
  center when the cursor is outside.
- Up/Down move between items; Enter activates; Esc closes.
- The underlined mnemonics follow the catalog (`&Repeat`, `&Options…`).

## 7. Data shown and where it comes from

| Field | Example | Source (if known) | Exists today? |
|---|---|---|---|
| Last repeating command | LINE | `CommandRunner._last` (set in `start()` when `command.repeat`) — expose read-only | Yes (private) |
| Command display name in the active language | LINEA | Spanish names in `locales/es/commands.json` | Yes |
| Object under the cursor | — | Viewport picking (spatial index broad phase + exact check) | Yes |
| Selection empty | — | Session selection | Yes |

## 8. Strings

| English source | Spanish | Notes |
|---|---|---|
| Repeat {command} | Repetir {command} | `{command}` = display name in the active language |
| Repeat last command | Repetir el último comando | Disabled item when nothing ran yet |
| Enter | Intro | Shortcut hint |
| Options… | Opciones… | |
| Command: {command} | Comando: {command} | Command-window echo (existing pattern) |
| Specify first point: | Precise el primer punto: | Example prompt from LINE (existing) |

## 9. Colors and look

Qt Fusion menu with the application palette: background Window, text
Text, separator Mid, hover/selected item the shared translucent focus
background. Icons drawn in Text; the Repeat icon in Highlight. No fixed
hex colors.

## 10. Out of scope / later

- The object context menu (right-click on an object or with a selection:
  Cut, Copy, Erase, Move, Properties…).
- Right-click during a command showing a menu of the command's options.
- "Recent input" submenu (AutoCAD's list of recent commands and points).

## 11. Open decisions for the maintainer

1. Should the menu also list the last few commands (AutoCAD's "Recent
   Input"), or only Repeat?
2. With nothing run yet: show "Repeat last command" disabled (as proposed)
   or hide the item and the separator?
3. Right-click on an object with nothing selected: open this menu (as
   AutoCAD does) or select the object first?

## 12. Suggested order of work

1. ADR (may be shared with the Options dialog's ADR).
2. Expose the last repeating command; the menu with Repeat and Options…
   (Options… may open an empty dialog shell until the dialog lands).
3. The *Right-click in model space* setting (with the Options dialog's User
   preferences section).

## 13. Acceptance criteria

- Right-click on empty model space with no command and no selection opens
  the menu at the cursor; it never opens over an object, with a selection,
  or during a command.
- "Repeat {command}" starts exactly what an empty Enter would, shows the
  name in the active language, and is disabled before any command ran.
- "Options…" opens the Options dialog.
- Enter/Esc/arrows and Menu or Shift+F10 work as in §6.
- Correct in both themes and both languages; strings in the catalogs with
  keys in alphabetical order.

## 14. Mockup reference

Images in `docs/design/modelspace-context-menu/`:

- `01-context-menu.png` — right-click on empty model space (crosshair at
  the click point): Repeat LINEA (Enter), separator, Options…; the command
  window shows the previous commands.
