# Options dialog — design brief

Status: proposal from the maintainer's mockups (2026-10-22).
Related decisions: ADR 0006 (i18n), ADR 0007 (graphics API), ADR 0013
(application shell), ADR 0015 (workspaces, panels' look), ADR 0016
(editing workflow), ADR 0017 (DWG/DXF engine, data protections D5).
Companion brief: `modelspace-context-menu.md` (opens this dialog).
Content reference: OpenCADStudio's Options dialog (sections and settings);
layout: a sidebar of tall entries and a content area with a wide primary
action below it.

## 1. Purpose

One place to configure the application and the current drawing, as
AutoCAD's OPTIONS. Today the language, theme and layout are set from
menus; those menus stay, and the dialog shows the same settings.

## 2. Where it lives and how it opens

- **Kind**: modal dialog, about 900 × 640, resizable; remembers its size
  and last section.
- **Command**: `OPTIONS`, alias `OP` (AutoCAD's), Spanish `OPCIONES`.
- **Menus**: Tools › Options…; the model-space context menu's Options….
- No shortcut proposed.

## 3. Layout (top to bottom)

1. **Title**: "Options".
2. **Body**, side by side:
   - **Sidebar** (about 190 px): General, Files, Open and save, Display,
     Drawing, 3D modeling, Selection, User preferences, Graphics. Tall rows
     (about 46 px), centered text, thin separators; the current one filled
     (§9). Default: General, or the last section used.
   - **Content** (Base, scrolls vertically): the section's subsections,
     **each a `QGroupBox` titled with the subsection name**. Inside: rows
     of a muted label (about 200 px) and its control; help text under a
     control in a smaller muted font; some group boxes have a "Restore
     defaults" button at their top right.
3. **Action row** under the content: Reset section (left), OK (primary,
   takes the free width), Apply, Cancel.

### Sections (controls and defaults)

**General** (`01-general.png`)
- *Language*: Interface language — combo: System language, Español,
  English (default: System language).
- *Applications*: "Installed plug-ins and their sources" → Plug-ins…;
  "Keyboard shortcuts" → Keyboard shortcuts…; "Command aliases" → Command
  aliases….
- *Plotting*: "Plot device, paper, scale and plot styles" → Plot and page
  setup…; checkbox "Show page setup for new layouts" (off).

**Files** (`02-files.png`)
- Intro: "Where the application keeps its own files."
- *Locations*: each row shows a name and its folder: Configuration (Open
  folder), Plot styles (Open folder, Change…), Plug-ins (Open folder),
  Autosave files (Open folder), Fonts (Open folder, Change…), Color books
  (Open folder).
- *Default template for new drawings*: path field, placeholder "Blank
  drawing"; Browse….

**Open and save** (`03-open-and-save.png`)
- *Open and save*: Default save format — combo (DWG 2013 default, DWG
  2018, DWG 2010, DXF 2013) with help "Used the first time a new drawing is
  saved. Existing drawings keep their file type and version."; checkbox
  "Open .dwg and .dxf files with {app}" (off) with help "Also installs the
  application and file-type icons the desktop shows."; checkbox "Show
  values and parameter names on constraint markers" (on) with help.
- *File safety*: Autosave slider in minutes (10; 0 turns it off) with help
  "(SAVETIME)"; checkbox "Keep a .bak copy when overwriting a drawing
  (ISAVEBAK)" (on). The data protections of ADR 0017 D5 are always on and
  are not shown as options.

**Display** (`04-display.png`, `10-crosshair-preview-light-orange.png`)
- *Model space · Appearance* (Restore defaults): Drawing area · Mode —
  Follow the theme (default), Dark background, Light background; Paper ·
  Background (swatch + `#RRGGBB`, white); Paper surround (swatch +
  `#RRGGBB`, gray); Grid · Opacity (18 %); help about following the theme.
- *Crosshair*: Size — slider and number field kept in sync, 1–100 %
  (default 5), help "Percent of the drawing area. At 100 % the lines cross
  the whole screen."; **crosshair preview** to the right (§3.1); Type —
  Crosshair (default), Arrow, Small cross; Crosshair color, Object-snap
  marker color, Command-line text color — swatch + `#RRGGBB` field, empty =
  automatic contrast; help about leaving it empty.
- *Lineweight*: Display scale in model space (100 %).

**Drawing** (`05-drawing.png`)
- *Drawing*: Drawing rotation (0, degrees) with help "(SNAPANG)"; Polar
  tracking increment (1°, 5°, 10°, 15°, 22.5°, 30°, 45°, 90°; default 1°);
  Object-snap marker size (5 px).
- *Snap and grid*: "Snap modes, grid spacing and object snaps" → Drafting
  settings….

**3D modeling** (`06-3d-modeling.png`)
- *Display tools*: Show the navigation cube (NAVVCUBE), Show the UCS icon
  (UCSICON), Place the UCS icon at the origin (UCSICON Origin) — all on.
- *Display resolution* · applies to the current drawing: Isolines per
  surface (4); Show silhouette edges of solids (DISPSILH, on); Surface
  density U (6) and V (6); Surface type — Cubic B-spline (6), Quadratic
  B-spline (5), Bézier (8).
- *Solid · History* · applies to the current drawing: Record compound solid
  history (SOLIDHIST, off); Show solid history — As set per solid,
  Always, Never.

**Selection** (`07-selection.png`)
- *Selection*: Pickbox size (3) with help.
- *Selection modes*: Shift adds to the selection (PICKADD), Dragging draws
  a rectangle instead of a lasso (PICKDRAG), Clicking overlapping objects
  opens a chooser — all off.
- *Visual effects · Settings* (Restore defaults): transparent fill for
  selection areas (SELECTIONAREA, on); Selection opacity (12 %) with help
  (SELECTIONAREAOPACITY); Window color and Crossing color — "0: Theme
  (default)"; Show selection effect (SELECTIONEFFECT, on); Highlight color
  "0: Theme (default)"; Highlight opacity (100 %); Highlight extra width
  (0 px).

**User preferences** (`08-user-preferences.png`)
- *SpaceMouse*: Enable SpaceMouse (on); Status (read-only, e.g.
  "3DxWare unavailable"); Navigation — Follow context (default), Pan only,
  Pan and zoom, with help; Reverse pan direction (off); Pan speed (100 %);
  Open 3Dconnexion settings… with help; collapsible "Connection details".
- *Navigation style*: Orbit — "CAD: Shift + middle button orbits"
  (default), "Middle button orbits", "Right button orbits".
- *Zoom*: Reverse wheel zoom (ZOOMWHEEL, off); Touchpad: scrolling pans,
  Ctrl + scroll or pinch zooms (off); Zoom factor — slider and number (60)
  with "{percent} % per notch".
- *Right-click in model space*: With nothing selected — Show the context
  menu (default), Repeat the last command; During a command — Acts as
  Enter (default), Show the context menu. Controls
  `modelspace-context-menu.md`.

**Graphics** (`09-graphics.png`)
- Intro: "Saved at once. Changes take effect the next time {app} starts."
- *This session* (read-only): GPU, Driver, Backend, Selected by
  (automatic / pinned).
- *Next launch*: Graphics backend — Automatic (recommended), Vulkan,
  DirectX 12, Metal, OpenGL, CPU (fallback rasterizer); Use the
  compatibility renderer (for GPUs without shader storage buffers) (off);
  Prefer OpenGL on older GPUs (automatic backend only) (on); help about
  falling back and about `--backend` / `WGPU_BACKEND` overriding the page.

### 3.1 Crosshair preview

A small box (about 150 × 100 px) to the right of the size controls, with
the caption "Preview", painted live (before Apply):

- background: the drawing-area color of the selected mode;
- the crosshair in the chosen color, or the automatic contrast color
  (light on a dark background, dark on a light one) when the field is
  empty or invalid;
- the pickbox at the center, the arms broken at it;
- each arm is *size % × half the preview's width*; at 100 % the lines cross
  the whole box.

## 4. States

| State | What is shown |
|---|---|
| No drawing open | Settings marked "applies to the current drawing" are disabled; tooltip "Open a drawing to change its settings". |
| Unapplied changes | Apply enabled; otherwise disabled (`01-general.png` shows it enabled for the mockup). |
| Invalid value (e.g. `#12G`) | Field border in an error color, tooltip "Enter a color as #RRGGBB, or leave it empty"; OK and Apply keep the last valid value. |
| A setting whose feature does not exist yet | Not shown (decided per setting in the ADR, §11). |
| Read-only values (Graphics › This session, SpaceMouse status) | Plain text, not fields. |
| Long content | The content area scrolls; the sidebar and action row stay. |
| Light theme | Same layout with the Qt palette; `10-crosshair-preview-light-orange.png` shows the preview with a light drawing area. |

## 5. Actions and events

| # | Trigger | Where | What happens | Command run | Undo step | Command-line echo | Updates |
|---|---|---|---|---|---|---|---|
| 1 | OPTIONS / OP, Tools › Options…, context menu | Anywhere | Opens the dialog at the last section | OPTIONS | none | `_OPTIONS` when typed | — |
| 2 | Click a sidebar entry | Sidebar | Shows that section; pending changes are kept | — | none | — | Content |
| 3 | Change any control | Content | Marks the dialog changed; previews update live | — | none | — | Apply enabled; crosshair preview |
| 4 | Click "Apply" | Action row | Applies all pending changes; dialog stays open | — | one for drawing settings, none for application settings | — | Viewport, theme, language… |
| 5 | Click "OK" or Enter | Action row | Applies and closes | — | as 4 | — | as 4 |
| 6 | Click "Cancel", Esc or the close button | Action row / title | Discards unapplied changes and closes | — | none | — | — |
| 7 | Click "Reset section" | Action row | Sets the current section's controls to their defaults (pending until Apply/OK) | — | none | — | Controls |
| 8 | Click "Restore defaults" | Group box | Same as 7 for that group only | — | none | — | Controls |
| 9 | Click a "…" button (Plug-ins…, Keyboard shortcuts…, Command aliases…, Plot and page setup…, Drafting settings…, Open 3Dconnexion settings…) | Content | Opens that dialog or tool (each its own work) | per tool | — | — | — |
| 10 | Click "Open folder" | Files | Opens the folder in the system file manager | — | none | — | — |
| 11 | Click "Change…" / "Browse…" | Files | Folder or file chooser; sets the path | — | none | — | Field |
| 12 | Move the crosshair slider or type the size | Display | Slider and number stay in sync; preview updates | — | none | — | Preview |
| 13 | Change Drawing area · Mode or Crosshair color | Display | Preview background / crosshair color update | — | none | — | Preview, swatch |
| 14 | Change Graphics › Next launch | Graphics | Saved at once (as the page says) | — | none | — | Settings file |

The dialog reacts to: **language changed** (retranslates); **theme
changed** (restyles; "Follow the theme" preview follows it); **drawing
switched** while open (drawing settings reload — or the dialog is modal and
this cannot happen, §11).

## 6. Keyboard

- Ctrl+Tab / Ctrl+Shift+Tab or Ctrl+PgDn / Ctrl+PgUp move between
  sections; Up/Down in the sidebar when it has focus.
- Tab order: sidebar → content controls top to bottom → Reset section →
  OK → Apply → Cancel.
- Enter = OK (except in multi-line fields); Esc = Cancel.
- Sliders: arrows ±1, PgUp/PgDn ±10.

## 7. Data shown and where it comes from

| Field | Example | Source (if known) | Exists today? |
|---|---|---|---|
| Interface language | Español | `application.choose_language`, `--lang`, settings | Yes |
| Theme | Dark | `theme.py`, `THEME_SETTING` | Yes |
| Workspace layout | Ribbon | `workspace_switcher.py` | Yes |
| Application settings store | — | `application.open_settings()` (QSettings, INI) | Yes |
| Last repeating command (for the context menu) | LINE | `CommandRunner` | Yes |
| Crosshair size / color / type | 5 % | Viewport | Unknown (does the viewport draw a crosshair?) |
| Drawing settings (SNAPANG, ISOLINES, DISPSILH, SURFU/V, SURFTYPE, SOLIDHIST, SHOWHIST) | 0, 4, on… | DWG header variables through the core API | Unknown |
| SAVETIME autosave | 10 | — | No |
| ISAVEBAK | on | — | Unknown |
| Graphics session info (GPU, driver, backend) | [detected GPU] | wgpu probe (`gpu.py`, ADR 0007) | Partly |
| Default template | — | — | No |
| SpaceMouse | — | 3DxWare SDK | No |
| Plot device / page setup | — | — | No (phase 04) |
| Plug-ins, shortcuts, aliases dialogs | — | — | Unknown |

## 8. Strings

Variable names in parentheses (SAVETIME, PICKADD…) are kept as they are
in both languages. Paths and hardware values are not catalog strings.

| English source | Spanish | Notes |
|---|---|---|
| Options | Opciones | Dialog title |
| OPTIONS | OPCIONES | Command name |
| General | General | Section |
| Files | Archivos | Section |
| Open and save | Abrir y guardar | Section |
| Display | Visualización | Section |
| Drawing | Dibujo | Section; group |
| 3D modeling | Modelado 3D | Section |
| Selection | Selección | Section; group |
| User preferences | Preferencias de usuario | Section |
| Graphics | Gráficos | Section |
| Reset section | Restablecer sección | Action row |
| OK | Aceptar | Action row (primary) |
| Apply | Aplicar | Action row |
| Cancel | Cancelar | Action row |
| Close | Cerrar | Title button accessible name |
| Restore defaults | Restaurar valores predeterminados | Group button |
| Language | Idioma | Group |
| Interface language | Idioma de la interfaz | |
| System language | Idioma del sistema | Combo item; "Español" and "English" are shown in their own language |
| Applications | Aplicaciones | Group |
| Installed plug-ins and their sources | Complementos instalados y sus orígenes | |
| Plug-ins… | Complementos… | Button |
| Keyboard shortcuts | Atajos de teclado | |
| Keyboard shortcuts… | Atajos de teclado… | Button |
| Command aliases | Alias de comandos | |
| Command aliases… | Alias de comandos… | Button |
| Plotting | Trazado | Group |
| Plot device, paper, scale and plot styles | Dispositivo de trazado, papel, escala y estilos de trazado | |
| Plot and page setup… | Trazar y configurar página… | Button |
| Show page setup for new layouts | Mostrar la configuración de página para las presentaciones nuevas | Checkbox |
| Where the application keeps its own files. | Dónde guarda la aplicación sus propios archivos. | Files intro |
| Locations | Ubicaciones | Group |
| Configuration | Configuración | |
| Plot styles | Estilos de trazado | |
| Plug-ins | Complementos | |
| Autosave files | Archivos de guardado automático | |
| Fonts | Fuentes | |
| Color books | Libros de colores | |
| Open folder | Abrir carpeta | Button |
| Change… | Cambiar… | Button |
| Default template for new drawings | Plantilla predeterminada para dibujos nuevos | Group |
| Default template | Plantilla predeterminada | Field accessible name |
| Blank drawing | Dibujo en blanco | Placeholder |
| Browse… | Examinar… | Button |
| Default save format | Formato de guardado predeterminado | |
| Used the first time a new drawing is saved. Existing drawings keep their file type and version. | Se usa al guardar por primera vez un dibujo nuevo. Los dibujos existentes conservan su tipo y versión de archivo. | Help |
| Open .dwg and .dxf files with {app} | Abrir los archivos .dwg y .dxf con {app} | Checkbox |
| Also installs the application and file-type icons the desktop shows. | También instala los iconos de la aplicación y de los tipos de archivo que muestra el escritorio. | Help |
| Show values and parameter names on constraint markers | Mostrar valores y nombres de parámetros en los marcadores de restricción | Checkbox |
| On by default. Turn it off to see only the constraint symbol in the viewport, without the value or parameter name covering nearby geometry. | Activado por defecto. Desactívalo para ver solo el símbolo de la restricción en la ventana gráfica, sin que el valor o el nombre del parámetro tapen la geometría cercana. | Help |
| File safety | Seguridad de archivos | Group |
| Autosave | Guardado automático | Slider label |
| {count} min | {count} min | Slider value |
| Minutes between recovery saves to a .sv$ file; 0 turns it off (SAVETIME). | Minutos entre guardados de recuperación en un archivo .sv$; 0 lo desactiva (SAVETIME). | Help |
| Keep a .bak copy when overwriting a drawing (ISAVEBAK) | Conservar una copia .bak al sobrescribir un dibujo (ISAVEBAK) | Checkbox |
| Model space · Appearance | Espacio modelo · Apariencia | Group |
| Drawing area · Mode | Área de dibujo · Modo | |
| Follow the theme | Adaptar al tema | Combo item |
| Dark background | Fondo oscuro | Combo item |
| Light background | Fondo claro | Combo item |
| Paper · Background | Papel · Fondo | |
| Paper surround | Entorno del papel | |
| #RRGGBB | #RRGGBB | Placeholder |
| Grid · Opacity | Rejilla · Opacidad | |
| {percent} % | {percent} % | Slider value |
| The canvas background, the grid and the default line colors follow the active theme. | El fondo del lienzo, la rejilla y los colores de línea predeterminados se adaptan al tema activo. | Help |
| Crosshair | Cursor en cruz | Group; Type combo item |
| Size | Tamaño | |
| Crosshair size, in percent | Tamaño del cursor en cruz, en porcentaje | Number field accessible name |
| Percent of the drawing area. At 100 % the lines cross the whole screen. | Porcentaje del área de dibujo. Con 100 % las líneas cruzan toda la pantalla. | Help |
| Preview | Vista previa | Preview caption |
| Crosshair preview at {percent} % | Vista previa del cursor en cruz al {percent} % | Preview accessible name |
| Type | Tipo | |
| Arrow | Flecha | Combo item |
| Small cross | Cruz pequeña | Combo item |
| Crosshair color | Color del cursor | |
| Object-snap marker color | Color del marcador de referencia | |
| Command-line text color | Color del texto de la línea de comandos | |
| #RRGGBB or empty | #RRGGBB o vacío | Placeholder |
| Leave the color empty to keep the viewport's automatic contrast. | Deja el color vacío para mantener el contraste automático de la ventana gráfica. | Help |
| Enter a color as #RRGGBB, or leave it empty | Escribe un color como #RRGGBB o déjalo vacío | Invalid-value tooltip; not in mockup |
| Lineweight | Grosor de línea | Group |
| Display scale in model space | Escala de visualización en el modelo | |
| Drawing rotation | Rotación de dibujo | |
| degrees | grados | Unit |
| Turns the crosshair and the snap grid in the active UCS (SNAPANG). | Gira la mira y la rejilla de forzado en el SCP activo (SNAPANG). | Help |
| Polar tracking increment | Incremento de rastreo polar | Items "1°"… use the locale's decimal separator (22.5° / 22,5°) |
| Object-snap marker size | Tamaño del marcador de referencia | |
| {count} px | {count} px | Slider value |
| Snap and grid | Forzado y rejilla | Group |
| Snap modes, grid spacing and object snaps | Modos de forzado, espaciado de rejilla y referencia a objetos | |
| Drafting settings… | Parámetros de dibujo… | Button |
| Display tools | Herramientas de visualización | Group |
| Show the navigation cube (NAVVCUBE) | Mostrar el cubo de navegación (NAVVCUBE) | |
| Show the UCS icon (UCSICON) | Mostrar el icono SCP (UCSICON) | |
| Place the UCS icon at the origin (UCSICON Origin) | Situar el icono SCP en el origen (UCSICON Origen) | |
| Display resolution | Resolución de visualización | Group |
| applies to the current drawing | se aplica al dibujo actual | Group title note |
| Isolines per surface | Isolíneas por superficie | |
| Show silhouette edges of solids (DISPSILH) | Mostrar aristas de silueta en sólidos (DISPSILH) | |
| Surface density U | Densidad de superficie U | |
| Surface density V | Densidad de superficie V | |
| Surface type | Tipo de superficie | |
| Cubic B-spline (6) | B-spline cúbica (6) | Combo item |
| Quadratic B-spline (5) | B-spline cuadrática (5) | Combo item |
| Bézier (8) | Bézier (8) | Combo item |
| Solid · History | Sólido · Historial | Group |
| Record compound solid history (SOLIDHIST) | Registrar el historial de sólidos compuestos (SOLIDHIST) | |
| Show solid history | Mostrar el historial de sólidos | |
| As set per solid | Según cada sólido | Combo item |
| Always | Siempre | Combo item |
| Never | Nunca | Combo item |
| Pickbox size | Cuadro de selección: Tamaño | |
| Sets both the visible pickbox and the click area. | Controla tanto el cuadro de selección visible como el área de clic. | Help |
| Selection modes | Modos de selección | Group |
| Shift adds to the selection (PICKADD) | Usar Mayús para añadir a la selección (PICKADD) | |
| Dragging draws a rectangle instead of a lasso (PICKDRAG) | Arrastrar dibuja un rectángulo en lugar de un lazo (PICKDRAG) | |
| Clicking overlapping objects opens a chooser | Al pulsar sobre objetos superpuestos se abre un selector | |
| Visual effects · Settings | Efectos visuales · Configuración | Group |
| Show selection areas with a transparent fill (SELECTIONAREA) | Indicar el área de selección con relleno transparente (SELECTIONAREA) | |
| Selection opacity | Selección: Opacidad | |
| Transparency of window and crossing selection areas (SELECTIONAREAOPACITY). | Transparencia de las áreas de selección por ventana y por captura (SELECTIONAREAOPACITY). | Help |
| Window selection color | Selección: Ventana: Color | |
| Crossing selection color | Selección: Captura: Color | |
| 0: Theme (default) | 0: Tema (predeterminado) | Combo item; other items are ACI colors "{index}: {name}" |
| Show selection effect (SELECTIONEFFECT) | Mostrar efecto de selección (SELECTIONEFFECT) | |
| Highlight color | Selección: Resaltado: Color | |
| Highlight opacity | Opacidad del resaltado | |
| Highlight extra width | Grosor extra del resaltado | |
| SpaceMouse | SpaceMouse | Group (product name, not translated) |
| Enable SpaceMouse | Activar SpaceMouse | |
| Status | Estado | |
| 3DxWare unavailable | 3DxWare no disponible | Status value |
| Navigation | Navegación | |
| Follow context | Según el contexto | Combo item |
| Pan only | Solo encuadre | Combo item |
| Pan and zoom | Encuadre y zoom | Combo item |
| Pan and zoom on paper sheets; full navigation in model views. | Encuadre y zoom en las hojas de papel; navegación completa en las vistas del modelo. | Help |
| Reverse pan direction | Invertir la dirección del encuadre | |
| Pan speed | Velocidad de encuadre | |
| Open 3Dconnexion settings… | Abrir la configuración de 3Dconnexion… | Button |
| Configure 3D navigation, buttons and radial menus in the 3Dconnexion settings. Pan only and Pan and zoom share the push/tilt mapping and the pan speed shown here. | Configura la navegación 3D, los botones y los menús radiales en la configuración de 3Dconnexion. Solo encuadre y Encuadre y zoom comparten la asignación de empuje e inclinación y la velocidad de encuadre que se muestran aquí. | Help |
| Connection details | Detalles de conexión | Collapsible |
| Navigation style | Estilo de navegación | Group |
| Orbit | Orbitar | Label |
| CAD: Shift + middle button orbits | CAD: Mayús + botón central orbita | Combo item |
| Middle button orbits | Botón central orbita | Combo item |
| Right button orbits | Botón derecho orbita | Combo item |
| Zoom | Zoom | Group |
| Reverse wheel zoom (ZOOMWHEEL) | Invertir el zoom de la rueda del ratón (ZOOMWHEEL) | |
| Touchpad: scrolling pans; Ctrl + scroll or pinch zooms | Panel táctil: el desplazamiento encuadra; Ctrl + desplazamiento o pellizco hace zoom | |
| Zoom factor | Factor de zoom | Also the number field's accessible name |
| {percent} % per notch | {percent} % por paso | |
| Right-click in model space | Clic derecho en el espacio modelo | Group |
| With nothing selected | Sin objetos seleccionados | |
| Show the context menu | Mostrar el menú contextual | Combo item |
| Repeat the last command | Repetir el último comando | Combo item |
| During a command | Durante un comando | |
| Acts as Enter | Equivale a Intro | Combo item |
| Saved at once. Changes take effect the next time {app} starts. | Se guarda al momento. Los cambios se aplican la próxima vez que se inicie {app}. | Graphics intro |
| This session | Esta sesión | Group |
| GPU | GPU | |
| Driver | Controlador | |
| Backend | Motor | |
| Selected by | Elegido por | |
| automatic | automático | Value |
| pinned | fijado | Value |
| Next launch | Próximo inicio | Group |
| Graphics backend | Motor gráfico | |
| Automatic (recommended) | Automático (recomendado) | Combo item; Vulkan, DirectX 12, Metal, OpenGL are names |
| CPU (fallback rasterizer) | CPU (rasterizador de respaldo) | Combo item |
| Use the compatibility renderer (for GPUs without shader storage buffers) | Usar el renderizador de compatibilidad (para GPU sin búferes de almacenamiento de shaders) | |
| Prefer OpenGL on older GPUs (automatic backend only) | Preferir OpenGL en GPU antiguas (solo con el motor automático) | |
| If a pinned backend fails to start, the next launch falls back to automatic selection. The --backend option and the WGPU_BACKEND variable override this page. | Si un motor fijado no logra iniciar, el siguiente inicio vuelve a la selección automática. La opción --backend y la variable WGPU_BACKEND tienen prioridad sobre esta página. | Help |
| Open a drawing to change its settings | Abre un dibujo para cambiar sus ajustes | Disabled tooltip; not in mockup |
| Options… | Opciones… | Tools menu item (shared with the context menu) |

## 9. Colors and look

- Dialog: Window; sidebar: a darker shade (Dark/Shadow side, the mockup
  uses the title-bar shade), entries Text on it; content area: Base; group
  box frames: Mid with the title in Text; labels and help: PlaceholderText;
  fields: Window/Base with a Mid border.
- Current sidebar entry and OK: `#0b6fa6` with white text (deliberate:
  Highlight `#0a9be0` with white is about 3.0:1; `#0b6fa6` about 5.5:1),
  the same primary color as the Blocks palette.
- Swatches show the chosen color with a Mid border.
- Crosshair preview: drawing-area color of the selected mode (Follow the
  theme = the viewport's own background), crosshair color as §3.1.
- Standard Qt widgets: `QListWidget` sidebar, `QScrollArea` content,
  `QGroupBox` per subsection, `QSlider` + `QSpinBox` pairs, `QComboBox`,
  `QCheckBox`, `QDialogButtonBox`-like row (custom, for the wide OK).

## 10. Out of scope / later

- The dialogs opened by the "…" buttons (plug-ins, shortcuts, aliases,
  page setup, drafting settings, 3Dconnexion).
- SpaceMouse support itself, plotting (phase 04), NAVVCUBE and 3D display
  options until the 3D viewport exists (milestone 02.8, step 2 onward).
- Any setting whose feature does not exist yet: hidden until it does
  (§11).

## 11. Open decisions for the maintainer

1. Which settings are wired in the first version? Proposal: General
   (language), Display (drawing-area mode, crosshair if the viewport draws
   one), User preferences › Right-click, Graphics (backend, if the probe
   allows it); the rest hidden until their feature lands. Or show them
   disabled with "Not available yet"?
2. Does "Theme" (dark/light) also appear in General, as the mockup's
   earlier version had, or only in the View menu?
3. Is the dialog strictly modal (simpler: no drawing switch while open)?
4. Default save format: DWG 2013 (OpenCADStudio's) or the version of the
   drawing opened, as contract §10 prefers?
5. Keep AutoCAD variable names in parentheses in the labels, as mocked?

## 12. Suggested order of work

1. ADR for the dialog (and the context menu), deciding §11.
2. Dialog shell: sidebar, scrolling content with group boxes, action row
   and its semantics, OPTIONS command and Tools › Options….
3. General and User preferences › Right-click (with the context menu).
4. Display, with the crosshair preview widget.
5. Graphics, with the probe's information.
6. Other sections as their features land.

## 13. Acceptance criteria

- OPTIONS, OP, OPCIONES, Tools › Options… and the context menu open the
  dialog at the last section used.
- Every subsection is a titled group box; the sidebar switches sections
  without losing pending changes.
- Apply applies and keeps the dialog open; OK applies and closes; Cancel
  and Esc discard; Reset section and Restore defaults reset only their
  scope.
- The crosshair preview follows size, color and drawing-area mode live,
  before Apply; empty or invalid color uses the automatic contrast.
- Drawing settings change the drawing in one undo step; application
  settings persist across restarts.
- Correct in both themes and both languages; all strings in the catalogs
  with keys in alphabetical order.

## 14. Mockup reference

Images in `docs/design/options-dialog/` (each section captured at full
height; the real dialog scrolls):

- `01-general.png` — General: Language, Applications, Plotting.
- `02-files.png` — Files: Locations and the default template.
- `03-open-and-save.png` — Open and save, File safety.
- `04-display.png` — Model space · Appearance, Crosshair with the preview,
  Lineweight.
- `05-drawing.png` — Drawing, Snap and grid.
- `06-3d-modeling.png` — Display tools, Display resolution, Solid ·
  History.
- `07-selection.png` — Selection, Selection modes, Visual effects.
- `08-user-preferences.png` — SpaceMouse, Navigation style, Zoom,
  Right-click in model space.
- `09-graphics.png` — This session, Next launch.
- `10-crosshair-preview-light-orange.png` — Display with Light background,
  crosshair color `#FF8800` and size 40 %, showing the live preview.

Paths and hardware values in the mockups are placeholders.
