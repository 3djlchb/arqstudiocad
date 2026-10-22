# UI design system extracted from code

Snapshot: 2026-10-22. Values below are source constants or layout calls,
not measured screenshot dimensions. Qt widget sizes use logical pixels;
the CAD image uses device pixels. Native decorations and font metrics can
change the final size. See [architecture](architecture.md) for limitations.

## Theme tokens

The application explicitly selects Fusion and applies a QPalette. Dark is
the first/default theme; Light uses Fusion's `standardPalette()` rather than
a project-maintained table of hex values. Do not invent fixed light colors.
Sources: [theme.py](../../python/arqstudiocad/ui/theme.py), `THEMES`:17,
`DARK`:21, `DARK_DISABLED`:43, `palette`:60, `apply_theme`:72;
[application.py](../../python/arqstudiocad/application.py), `run`:39.

| Dark QPalette role | Value | Purpose in the current UI |
|---|---|---|
| Window | `#2b2b2e` | Window/panel surface |
| WindowText, Text, ButtonText | `#e0e0e0` | Normal labels and values |
| Base | `#1e1e20` | Editors, data views and property values |
| AlternateBase | `#303034` | Alternating panel rows |
| PlaceholderText | `#8a8a8a` | Muted labels, redo entries, footer |
| Button, ToolTipBase | `#3a3a3e` | Controls, section bands, tooltips |
| Light | `#4a4a50` | Palette shading |
| Midlight | `#404046` | Palette shading |
| Mid | `#46464c` | Borders and separators |
| Dark | `#232326` | Palette shading |
| Shadow | `#141416` | Palette shading |
| Highlight | `#0a9be0` | Accent and focused editors |
| HighlightedText, BrightText | `#ffffff` | Highlighted text |
| Link | `#4fb8f0` | Links |
| ToolTipText | `#e0e0e0` | Tooltip text |
| Disabled WindowText/Text/ButtonText | `#7a7a7a` | Disabled controls |
| Disabled Highlight | `#46464c` | Disabled highlight |

## Shared visual states

| State / component | Source value or styling | Evidence |
|---|---|---|
| Panel selection | Background `rgba(117, 143, 164, 48)`; top/bottom borders `rgba(0, 140, 255, 230)`, 1 px | [panel_style.py](../../python/arqstudiocad/ui/panel_style.py), `SELECTION_BACKGROUND`, `SELECTION_LINE`, `apply_data_view_style` |
| Palette toggle checked | Same background, complete 1 px accent border, radius 2 px | `toggle_focus_style` |
| General data row | Padding 3 px vertical / 6 px horizontal; bottom 1 px Mid border; alternating surfaces | `apply_data_view_style` |
| Header | Button background/text roles; padding 4 px / 6 px; bottom Mid border | Same function |
| Property row/editor | Row padding 2 px / 5 px; Base value surface; muted label; editor 1 px Mid border and radius 1 px | `apply_property_grid_style`; [property_grid.py](../../python/arqstudiocad/ui/property_grid.py), `PropertyGridDelegate.paint`:84 |
| Property accent combo | Highlight background, HighlightedText foreground | `apply_property_grid_style`, `QComboBox#propertyAccentEditor` |
| Property section | Full-width Button band, ButtonText, border and custom fold indicator | `PropertyGridDelegate._paint_section`:114 |
| Panel pushbutton | Minimum height 26; radius 2; padding 3 px / 9 px; hover accent border; pressed Highlight fill | `apply_panel_action_style` |
| CAD background | RGB `(33, 40, 48)` / `#212830`, unchanged by shell theme | [raster.rs](../../crates/arq-render/src/raster.rs), `Style::default`; `Viewport._render`:514 |
| Selection / window | RGB `(30,144,255)` / `#1e90ff`; rectangle fill alpha 40/255 | [viewport.py](../../python/arqstudiocad/ui/viewport.py), `SELECTED`, `FILL_ALPHA`, `_paint_overlay`:436 |
| Crossing | RGB `(60,200,90)` / `#3cc85a`; dashed border, same alpha | `CROSSING`, `_paint_overlay` |
| Hot grip | RGB `(230,40,40)` / `#e62828` | `HOT`, `_paint_grips`:470 |
| Snap marker | RGB `(255,200,0)` / `#ffc800`; 2 px marker pen | `MARKER`, `_paint_point`:481 |

The fixed panel selection accent differs from QPalette.Highlight. The
viewport interaction colors are also fixed in both themes. These are actual
token families, not evidence that all color roles have been centralized.
Sources: `panel_style` constants, `theme.DARK`, `Viewport` constants.

## Geometry, density and type

| Component | Verifiable source dimension | Source |
|---|---|---|
| Main window | Initial `resize(1024, 700)`; not a fixed/minimum window size | [main_window.py](../../python/arqstudiocad/ui/main_window.py), `MainWindow.__init__` |
| Command dock | Initial requested height 140; resizable | `COMMAND_WINDOW_HEIGHT`:72 and `resizeDocks` |
| Busy indicator | Maximum width 120; range 0–0 | `MainWindow.__init__` |
| Viewport | Minimum 64 × 64; render width/height = rounded logical size × DPR | `Viewport.__init__`:132, `resizeEvent`:285 |
| Pick / drag threshold | Pick half-aperture 4; drag threshold 4 logical px | `PICK_PIXELS`, `DRAG_PIXELS` |
| Snap / grips | Snap half-aperture 10; marker half-size 6; grip half-side 5 | `SNAP_PIXELS`, `MARKER_PIXELS`, `GRIP_PIXELS` |
| Ribbon | Three buttons per column, grid spacing 2; group margins `(4,2,4,2)`; icon 24 × 24 | [ribbon.py](../../python/arqstudiocad/ui/ribbon.py), `ROWS`:32, `RibbonGroupWidget.__init__`:36 |
| Ribbon page | Margins 2 on all sides; trailing stretch | `Ribbon.__init__`:76 |
| Layers | Row height 25; body margins 3; spacing 2; action-row spacing 4 | [layers_panel.py](../../python/arqstudiocad/ui/layers_panel.py), `LayersPanel.__init__`:75 |
| Generic panel table | Default row height 26; actual row-height handling differs for list/tree/table | `apply_data_view_style` |
| Blocks panel | Body margins 4; native list sizing; no card dimensions exist yet | [blocks_panel.py](../../python/arqstudiocad/ui/blocks_panel.py), `__init__`:29 |
| Property grid | Caption margins `(6,3,6,3)`; indentation 10; two columns, hidden header | `PropertyGrid.__init__`:195 |
| Auto-hide | Leave timer 400 ms; remembered overlay thickness at least 180; fallback 240, clipped to central area | [auto_hide.py](../../python/arqstudiocad/ui/auto_hide.py), `LEAVE_DELAY_MS`:37, `unpin`:236, `_overlay_rect`:354 |
| Panel title | Margins `(4,1,1,1)`, spacing 0, pin/close icon size 12 × 12 | `PanelTitleBar.__init__`:81 |
| Color dialog | Preferred 900 × 720; nominal min 650 × 500, clipped to screen; available-size margins 32 horizontal / 48 vertical | [color_picker.py](../../python/arqstudiocad/ui/color_picker.py), `_fit_to_screen`:193 |
| Color buttons | Regular 28 × 26; compact 22 × 20; preview 30 × 24 | `_color_button`:105; `ColorPickerDialog.__init__`:135 |
| Text dialog | Initial 420 × 220 | [text_dialog.py](../../python/arqstudiocad/ui/text_dialog.py), `TextDialog.__init__`:20 |

No single explicit application font family, font size or fixed Ribbon height
is set in the inspected styling path. Command history/prompt/input use Qt's
system FixedFont; TextDialog uses system GeneralFont; property section text
is bold. A browser prototype may choose a fallback font, but must label that
choice as an approximation. Evidence: `CommandWindow.__init__`:38,
`TextDialog.__init__`, `PropertyGrid.show_sections`:227, `theme.apply_theme`.

## Icons and resources

`command_icon` maps NEW, OPEN, SAVE, SAVEAS, UNDO and REDO to action SVGs,
and LINE/CIRCLE/ARC to 24/32/48 px drawing SVG variants. Theme directories
are `dark-themes` and `clear-themes`; unmapped commands return an empty
QIcon and the Ribbon uses text only. Layers use state SVGs, theme-dependent
lock icons and generated 14 × 14 color swatches. Pin icons are painted in
the current text color. Sources:
[command_icons.py](../../python/arqstudiocad/ui/command_icons.py), `command_icon`:21;
`RibbonGroupWidget.__init__`; `LayersPanel._fill_row`, `_lock_state_icon`:49,
`swatch`:66; `auto_hide.pin_icon`:54; [paths.py](../../python/arqstudiocad/paths.py),
`icons_dir`:26; [resources/icons](../../resources/icons).

No new assets were created or copied. Reusing artwork in a future prototype
must retain repository attribution and license obligations in `THIRD_PARTY.md`.
Authority: [AGENTS.md](../../AGENTS.md), non-negotiable source-reuse rule.

## Translation and settings rules for prototypes

Use English source strings with a Spanish catalog layer, preserve command
names/option keys and `{app}` substitution, and allow translated labels to
change width. Runtime UI retranslation flows through `LanguageChanged` and
`MainWindow.retranslate`; Qt standard dialog buttons use `.qm` translators.
Sources: [i18n/__init__.py](../../python/arqstudiocad/i18n/__init__.py), `tr`,
`localized_commands`; `MainWindow.change_language`, `retranslate`;
[qt_translations.py](../../python/arqstudiocad/ui/qt_translations.py), `QtTranslations.apply`:12.

Global Preferences/Options is a future design described in ADR 0015, not a
current modal to reproduce. Actual preferences are menu choices and layout
state. The proposed dynamic-block badge (`DB`/`BD`, `#004b4e` on `#ffa64d`)
is an ADR 0025 D4 target, absent from `BlocksPanel`; keep it in a separate
future-design board. Source: [ADR 0015](../architecture/adr/0015-workspaces.md),
Look; [ADR 0025](../architecture/adr/0025-blocks-palette.md), D4.
