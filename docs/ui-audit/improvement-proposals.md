# UI findings and improvement proposals

2026-10-22. These are analysis and proposals only. They do not authorize
implementation, refactoring, dependencies, configuration edits or changes to
architectural decisions. See [architecture](architecture.md) for the static
audit's scope. Priorities below are audit recommendations, not a replacement
for the project's roadmap.

## Findings with evidence

| ID / classification | Finding and evidence | Impact and proposed validation |
|---|---|---|
| F01 — confirmed synchronous path; latency risk | `Viewport.paintEvent` calls `_render`, which calls `CadView.render` synchronously; Rust detaches the GIL but returns only after rasterization. [viewport.py](../../python/arqstudiocad/ui/viewport.py):276,514; [view.rs](../../crates/arq-python/src/view.rs), `CadView::render`. | Large/high-DPI views may stall input. Measure GUI event latency during pan/zoom before changing scheduling. No FPS or stall duration was measured. |
| F02 — confirmed synchronous quit save; responsiveness risk | `MainWindow.closeEvent` calls `_save_on_quit`, then `Session.save`; normal SAVE instead yields background work. [main_window.py](../../python/arqstudiocad/ui/main_window.py), `closeEvent`, `_save_on_quit`; [session.py](../../python/arqstudiocad/session.py):112; [builtin.py](../../python/arqstudiocad/commands/builtin.py), `_save_to`:161. | Saving a large file on quit can hold the GUI thread. A future change should retain all recovery/loss/cancel protections while using a reviewed asynchronous close flow. Existing quit tests are in `test_documents_ui.py`:136 onward. |
| F03 — confirmed full initial scene and table rebuilds; scale risk | `Viewport.__init__` constructs `CadView` on GUI path; `Scene::update` rebuilds on layer/document changes; `PropertyGrid.show_sections` clears/rebuilds editors; `LayersPanel.refresh` refills rows and resizes columns. [view.rs](../../crates/arq-python/src/view.rs), `new`; [scene.rs](../../crates/arq-render/src/scene.rs):43; [property_grid.py](../../python/arqstudiocad/ui/property_grid.py):227; [layers_panel.py](../../python/arqstudiocad/ui/layers_panel.py):146. | Measure new-tab, mass-layer and large-selection costs. Optimize targeted refresh only after measurements, maintaining signal guards and change-set contracts. |
| F04 — confirmed Python overlay work; scale risk | `_draw_outlines` constructs QPolygonF/QPointF values in Python for polylines returned by Rust, and `_paint_grips` loops grips. [viewport.py](../../python/arqstudiocad/ui/viewport.py):470,502. | The optimized base renderer does not eliminate overlay allocation costs for huge selections. Measure selected/hovered complexity separately from raster time. |
| F05 — confirmed icon coverage gap | Only six file/history and three drawing commands have icons; other Ribbon controls are text-only. [command_icons.py](../../python/arqstudiocad/ui/command_icons.py), `_ACTION_ICONS`, `_DRAW_ICONS`, `command_icon`:21; [ribbon.py](../../python/arqstudiocad/ui/ribbon.py):36. | Mixed button footprints can complicate scanning and translated layouts. Compare a future consistent icon/text system without inventing current assets or adopting a new icon library. |
| F06 — confirmed duplicated style literals | Selection RGBA values occur in `panel_style`, the Layers color menu and ColorPickerDialog. [panel_style.py](../../python/arqstudiocad/ui/panel_style.py), constants; `LayersPanel._on_click`; [color_picker.py](../../python/arqstudiocad/ui/color_picker.py), `__init__`:135. | Future edits can drift. A later approved change can reuse existing token helpers; no new styling technology is needed. |
| F07 — confirmed partial Blocks palette | Current `BlocksPanel` only filters names and lists them; ADR 0025 D9 step 2 includes additional controls and steps 3–8 more behavior. [blocks_panel.py](../../python/arqstudiocad/ui/blocks_panel.py):26; [ADR 0025](../architecture/adr/0025-blocks-palette.md). | Complete in independent reviewed steps. Do not describe metadata bindings as a finished panel or silently promote the Proposed ADR to Accepted. |
| F08 — confirmed planned/actual shortcut difference | PROPERTIES/PR and Ctrl+1 are proposed but absent from current registries; Ctrl+3 exists. [actions.py](../../python/arqstudiocad/ui/actions.py), `SHORTCUTS`:20; [builtin.py](../../python/arqstudiocad/commands/builtin.py), command list; ADR 0025 D2. | Label correctly in prototypes and acceptance criteria. A future implementation must use the common command/action path. |
| F09 — confirmed persistence boundary | Dock/toolbar state is per workspace; unpinned dock names/sides use global `panels/auto_hidden`. Ribbon folding, drafting switches, property section folds and recent colors lack inspected settings persistence. [workspace_switcher.py](../../python/arqstudiocad/ui/workspace_switcher.py), `save_state`:180; [auto_hide.py](../../python/arqstudiocad/ui/auto_hide.py), `_save`:424; `Ribbon._folded`, `PropertyGrid.folded`, `Drafting.__init__`, `color_picker.remember_color`. | Clarify desired session/workspace/application scope before expanding persistence; avoid treating every reset as a defect. |
| F10 — verified busy policy; interaction risk | Runner refuses commands during background work and side panels are disabled, but viewport mouse paths still perform queries and document tab handlers can activate documents. `CommandRunner._refuse_while_busy`; `MainWindow._on_work_started`, `_on_tab_chosen`; `Viewport.mouseMoveEvent`/`pick_at`. | This is not proof of corruption or deadlock. Inspect lock timing and define expected navigation/tab behavior during long saves. Keep policy explicit in prototype busy states. |
| F11 — accessibility and narrow-screen risk, not a demonstrated failure | Ribbon pages have a horizontal group row without an explicit scroll/overflow adaptation; custom side tabs and property delegates paint content; icon state cells rely on visuals. `Ribbon.__init__`:76; `SideTab.paintEvent`:135; `PropertyGridDelegate.paint`:84; `LayersPanel._fill_row`:161. | Validate keyboard-only use, accessible names/roles, focus contrast, 200% scaling and long Spanish labels in a real Qt session. No accessibility failure is asserted from appearance alone. |
| F12 — cross-platform verification limit | Fusion/QSettings/StandardKey are used; CI config names Ubuntu, Windows and macOS runners. [theme.py](../../python/arqstudiocad/ui/theme.py):72; [application.py](../../python/arqstudiocad/application.py):27; [ci.yml](../../.github/workflows/ci.yml), Rust/Python matrices. | Configuration is not proof of a passing build or matching native behavior. Validate real DPI, input devices, file dialogs, docking and packaging on each claimed platform. |

## Strengths to preserve

- Shared actions and data-driven workspaces prevent separate tool logic in
  Ribbon and Classic. Evidence: `CommandActions.by_command`,
  `RibbonGroupWidget.setDefaultAction` call, `WorkspaceSwitcher._widgets`.
- Session reports and typed events make document/selection updates explicit;
  event-handler exceptions are logged without stopping other handlers.
  Evidence: `Session.report`; `EventBus.publish`;
  [test_events.py](../../tests/python/test_events.py), handler-order/failure tests.
- Base-image caching separates hover/selection from full rasterization;
  coordinates are converted using device-pixel ratio. Evidence:
  `Viewport._dirty`, `_set_hover`, `_physical`, `_paint_overlay`.
- Existing protections include background file work, loss/recovery questions,
  failed-edit feedback, grouped undo and editor-tab separation. Evidence:
  `builtin.open_file`, `_save_to`; `QtUiServices.ask_accept_losses`;
  `LayersPanel._edit`; `PropertiesPanel._edit_geometry`; `Session.open_block_editor`.
- Shared panel styles, runtime translation and palette-based themes provide
  reusable visual foundations. Evidence: `panel_style` helpers;
  `MainWindow.retranslate`, `change_theme`.

## Suggested sequence, subject to authorization

1. Use the current-state prototype specification to review layout and
   interactions, preserving actual feature boundaries. Deliverable is defined
   in [mockup-specification](mockup-specification.md); no prototype was built.
2. Validate F01–F04 and F10 with approved timing/event-loop measurements on
   representative drawings, including the reference-machine release gate.
   Existing benchmark guidance is [TESTING_AND_PERFORMANCE](../TESTING_AND_PERFORMANCE.md),
   Metrics and Rules for benchmarks.
3. Continue the Blocks palette through the bounded steps of ADR 0025, with
   its status resolved through the maintainer's existing decision process.
   Maintain Rust previews and one-undo-step INSERT integration.
4. Review icon coverage, token duplication, keyboard access and translation
   width on both themes and layouts; propose focused changes after evidence.

GPU rendering, 3D orbit, BIM UI, rich MTEXT/fonts and general preferences are
governed future work, not quick visual fixes. Respect milestone 02.7's hardware
validation hold and phase 04's parametric prerequisites. Sources:
[MODULES_AND_ROADMAP](../MODULES_AND_ROADMAP.md), §§02,04–05;
[ADR 0015](../architecture/adr/0015-workspaces.md), Look;
[ADR 0022](../architecture/adr/0022-text-entities.md), scope;
[ADR 0024](../architecture/adr/0024-3d-document-model.md), D4–D5.
