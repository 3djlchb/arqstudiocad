# Specification for a faithful interactive UI prototype

Status: analysis deliverable only, 2026-10-22. No HTML, CSS, JavaScript,
PySide6 prototype or new asset was created. This specification does not
authorize one. The implementation baseline is documented in
[architecture](architecture.md), [component-inventory](component-inventory.md),
[ui-interactions](ui-interactions.md) and [design-system](design-system.md).

## Purpose and fidelity contract

A future prototype should demonstrate the existing desktop shell and its
state transitions with fixture data. Keep a separate proposal board for
future Blocks-palette scope. Do not present browser geometry, filesystem
dialogs or placeholder metadata as a working CAD kernel or a completed feature.
Authority: [technical contract](../architecture/technical-contract.md) §§1,3,8;
[ADR 0025](../architecture/adr/0025-blocks-palette.md), Proposed and D9.

Use the same logical action IDs as `actions.SHORTCUTS`, workspace declarations
from `workspaces.CAD`, and the current menu order from `MainWindow.__init__`.
Keep the menu bar visible in Ribbon and Classic. Include only 2D drafting
as an available discipline. Sources:
[actions.py](../../python/arqstudiocad/ui/actions.py):20,
[workspaces.py](../../python/arqstudiocad/workspaces.py), `CAD`,
[main_window.py](../../python/arqstudiocad/ui/main_window.py), `__init__`:76.

## Layout and reusable components

```text
Native title bar (browser representation is illustrative)
Menu bar: File Edit View Tools Insert Draw Modify Help
Ribbon tabs + groups OR Classic toolbars
Document tabs across the full content width
┌──────────────────────────────┬─────────────────────────┐
│                              │ Layers / Properties /   │
│ Active CAD plan view         │ Blocks dock tab group   │
│ or centered welcome          ├─────────────────────────┤
│                              │ History                 │
├──────────────────────────────┤                         │
│ Command history + input      │                         │
└──────────────────────────────┴─────────────────────────┘
Status: work message | busy | x,y | OSNAP | ORTHO | workspace
```

Layout source: `MainWindow.__init__` dock corners, split/tabify calls,
`tabs_strip`, `_stack`, command dock and status-bar setup. Initial window size
1024 × 700 and command-dock requested height 140 come from that method.
Other panel widths and Ribbon height are layout/font driven, so adjustable
browser fixture widths must be labeled approximations rather than extracted
pixel values. All measured constants and palette values are in
[design-system](design-system.md).

Reusable prototype components should mirror existing boundaries:

| Component | Fixture responsibilities | Source counterpart |
|---|---|---|
| Action button/menu item | Command ID, translated label, optional existing icon, shortcut, checked/enabled state | `CommandActions` in `actions.py` |
| Ribbon tab/group | Data-driven order, three-row columns, shared actions, fold/context state | `Ribbon`, `RibbonGroupWidget` in `ribbon.py` |
| Dock shell | Title, pin/close, docked/floating/auto-hidden state; one expanded overlay | `PanelTitleBar`, `AutoHide` in `auto_hide.py` |
| Document strip | Active tab, close, reorder, modified marker, full-path tooltip | `DocumentTabs` in `document_tabs.py`; MainWindow tab methods |
| Data table/list | Shared selection focus, alternating rows, empty/disabled state | `panel_style.apply_data_view_style`; `LayersPanel`, `BlocksPanel`, `HistoryPanel` |
| Property grid | Caption, foldable sections, read-only or editable values, mixed state | `Property`, `Section`, `PropertyGrid` in `property_grid.py` |
| Command console | Prompt, bounded history/recall, completion and staged answers | `CommandWindow` and `CommandRunner` |
| Color/text dialogs | ACI/RGB fixture selection, recent colors; plain multiline text | `ColorPickerDialog`, `TextDialog` |
| Plan-view surface | Fixture geometry, pan/zoom/selection/grip overlays; logical/device-coordinate distinction | `Viewport`, `CadView`, Rust `Camera` |

## Minimum fixture model

Proposed prototype-only data, derived from `Session`, `CadDocument` public
views, `Drafting`, `PropertyGrid` descriptors and workspace data:

- Window: `layout`, `theme`, `language`, dock positions/visibility/pins,
  active document, busy message, contextual block-editor state.
- Document: synthetic ID, display name, synthetic path, modified flag,
  layer rows, fixture entities, named block list, selection IDs and history
  cursor. Paths are illustrative, never read from a user's filesystem.
- Command: current ID, prompt kind/options/base point, entered answers,
  repeatable previous command, message history.
- View: center, scale, size, pointer, hover ID, selection rectangle,
  snap kind/target, grips, hot grip and transform preview.
- Drafting: OSNAP enabled, selected snap keys, ORTHO and one-shot override.

This data model is a design proposal for a simulation, not a serialization
format or a new API. Actual persistent state stays in Rust and browser state
must not be introduced into production as an alternative document owner.
Evidence: [session.py](../../python/arqstudiocad/session.py), `Session.__init__`;
[drafting.py](../../python/arqstudiocad/drafting.py), `Drafting`;
[viewport.py](../../python/arqstudiocad/ui/viewport.py), `Viewport.__init__`;
technical contract §§1–3.

## Required current-state scenarios

| Scenario | What the future prototype should show | Implementation evidence |
|---|---|---|
| No document | Welcome, no document strip, disabled/empty document panels, command entry | `MainWindow._on_active_document_changed`, `_show_tabs_strip`; `LayersPanel.refresh`, `PropertiesPanel.refresh`, `HistoryPanel.refresh`, `BlocksPanel.refresh` |
| Drawing open | One viewport per tab, active document synchronization, path tooltip and modified `*` | `MainWindow._on_document_opened`, `_update_tab_titles`; `DocumentTabs` |
| Multiple documents | Switching updates panels and per-document selection; reorder tabs and close with save question | `Session.activate`, `selection`, `MainWindow._on_tab_chosen`, `_on_tab_close_requested`; `DocumentTabs._on_tab_moved` |
| Ribbon / Classic | Same actions; Home, Insert, Annotate, View; conditional Block Editor tab; double-click fold | `workspaces.CAD`; `Ribbon.set_context`, `set_folded`; `WorkspaceSwitcher.show` |
| Dock state | Close/reopen, resize, tabify/float representation, hover overlay and pin; distinguish hidden from auto-hidden | `MainWindow.show_panel`, `panel_open`; `AutoHide.unpin`, `expand`, `_check_leave` |
| Layers | Select/rename/new/delete/current, visibility/frozen/lock/plot toggles, color menu; refused-operation message | `LayersPanel` methods `new_layer`, `_on_click`, `_on_item_changed`, `_edit` |
| Properties | Drawing vs selected entity; mixed layer/color; editable geometry/text, invalid-value feedback, folded section | `selection_properties`, `PropertiesPanel._edit_property`, `_edit_geometry`, `_edit_text`; `PropertyGrid._remember` |
| Selection / navigation | Wheel about pointer, middle pan/extents, blue window/green crossing, Shift removal, hover, grips and Esc | `Viewport` handlers:292–410 and paint helpers:436–502 |
| Command flow | Demonstrate LINE and MOVE end to end; preserve real prompts/options, Space/text distinction and cancellation behavior | `commands.draw.line`:49; `commands.modify.move`:90; `CommandWindow.eventFilter`:130; `CommandRunner` |
| History | Select past/future row; muted redo entries; matching modified/selection fixture updates | `HistoryPanel.refresh`:51, `go_to`:72 |
| Blocks baseline | Searchable name list, empty/no-match states, footer, View toggle/Ctrl+3; no in-panel insertion control | `BlocksPanel.names`:64, `refresh`:72; `MainWindow._palette_button`; `actions.SHORTCUTS` |
| Static block editor | Separate editor document and contextual tab, save/close actions; dynamic edit refusal | `commands.blocks.bedit`:102, `bsave`:127, `bclose`:133; `Session.open_block_editor` |
| Work / failure | Indeterminate work status, disabled side panels, command refusal; failed-open and failed-save messages, save-loss questions | `MainWindow._on_work_started`, `_on_work_finished`; `CommandRunner._refuse_while_busy`; `QtUiServices` recovery/loss methods |
| Theme / language | Dark/light shell with dark model space; English/Spanish labels, preserved English command keys | `theme.palette`, `MainWindow.change_theme`, `change_language`, `retranslate`; `i18n.tr` |

Further tools follow the current command matrix in
[ui-interactions](ui-interactions.md), not invented Ribbon groups. Fixture
undo must model the illustrated command's actual grouping; Esc does not
universally roll back an entire command (LINE keeps completed segments,
INSERT rolls back the temporary reference). Sources: `draw.line` finally
block and `blocks.insert` finally block.

## Browser representation versus native validation

| Can be represented in HTML/CSS/JavaScript with fixtures | Requires the production PySide6/Rust path to validate |
|---|---|
| Palette colors, spacing, button states, menu/Ribbon composition, property rows, text overflow | Fusion/native metrics, real SVG QIcon selection, high-DPI rounding, platform font and window-decoration differences |
| Dock placement, tab groups, floating-panel appearance and overlay state transitions | QMainWindow docking, native floating windows, drag capture across monitors, `saveState`/`restoreState` and Qt focus behavior |
| Command prompts, fixture action dispatch, completion/recall, synthetic messages | Real CommandRunner generators, core validation, rollback/undo and event ordering under worker completion |
| Sample line/circle plan view, selection rectangles, fake snap/grip previews | Exact Rust hit tests, spatial queries, geometric precision, large coordinates, ACIS/B-rep and real render performance |
| Simulated dialogs, synthetic paths and recent lists | QFileDialog integration, permissions, recovery/loss protection and read-back-validated save |
| English/Spanish fixture labels and keyboard focus previews | QTranslator, QSettings platform backends, Qt StandardKey resolution, accessibility tree and assistive technologies |

The native column follows concrete dependencies in `Viewport`, `CadView`,
`QtUiServices`, `QtEventBridge`, `WorkspaceSwitcher` and `application.open_settings`;
these mappings are architectural constraints, not a reason to embed a web UI
in the application. No new frontend framework or product dependency is proposed.

## Separate future-design board

If later authorized, add a visibly labeled ADR 0025 proposal view: Drawing /
Template / Recent, grid/list previews, dynamic-block star badge, Selected block,
Insert options, Insert/Edit/New and cross-file clash dialogue. Follow the ADR's
View tab, Ctrl+3, proposed Ctrl+1 and free Ctrl+2, rather than the older brief's
Insert-tab shortcut suggestions. Template work is read-only; insertion goes
through INSERT and copies definitions within one undo group. These are targets,
not baseline functionality. Source: [ADR 0025](../architecture/adr/0025-blocks-palette.md), D1–D9.

## Review checklist for an authorized prototype

- Compare all visible command IDs/groups against `workspaces.CAD` and
  `actions.SHORTCUTS`; mark missing, partial and future items explicitly.
- Exercise the scenario table in both layouts and languages; check long labels
  at 1024 × 700 and a smaller fixture viewport. Smaller sizes are review cases,
  not newly asserted support guarantees.
- Match extracted constants and keep unmeasured dimensions/font choices
  identified as approximations; keep Light palette validation pending Qt capture.
- Validate keyboard/focus and contrast separately; a visually convincing
  browser mockup cannot certify accessibility or native desktop behavior.
- Before translating a prototype into code, agree acceptance criteria against
  the master plan and current repository scan. Preserve the Rust/Python/Qt
  boundaries, common actions, translation rules and existing undo semantics.

These checks are proposed review criteria for future authorized work, not
tests executed during this audit. Evidence remains the linked current symbols
and the architectural contracts; no UI improvement has been implemented.
