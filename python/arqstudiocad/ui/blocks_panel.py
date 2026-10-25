"""The Blocks palette (ADR 0025): the block definitions of the active
drawing, filtered as the user types, and the details of the one selected.

Laid out as the maintainer's mockup (docs/design/blocks-palette.md): the
sources (Template, Recent, Drawing), a caption naming where the blocks come
from, the search with the grid and list views, the blocks, a footer, and
the Selected block section, the insert options and the actions: Insert,
Edit block and New, which start INSERT, BEDIT and BLOCK. Blocks show as
cards with previews in the grid view, or as rows in the list view; the
previews are drawn in Rust and kept until a change touches a block; a
dynamic block's carries the star tag (ADR 0025 D4).

The Drawing source shows the active drawing's blocks; the Template source a
template's, loaded in the background and never shown in a tab (D7): a block
inserted from it is copied into the drawing first (D6). Recent comes with
its step (D9) and says so.
"""

import math
from collections.abc import Callable, Sequence
from pathlib import Path

from PySide6.QtCore import QMimeData, QPointF, QRectF, QSettings, QSize, Qt, QTimer
from PySide6.QtGui import (
    QColor,
    QDragEnterEvent,
    QDropEvent,
    QIcon,
    QImage,
    QPainter,
    QPalette,
    QPen,
    QPixmap,
    QPolygonF,
    QShowEvent,
)
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QButtonGroup,
    QCheckBox,
    QComboBox,
    QDockWidget,
    QDoubleSpinBox,
    QGridLayout,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QListView,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QSizePolicy,
    QSplitter,
    QStackedWidget,
    QStyle,
    QToolButton,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.commands import CommandRunner
from arqstudiocad.commands.blocks import edit_named, insert_named, load_template
from arqstudiocad.core import CadBlockInfo, CadDocument
from arqstudiocad.events import DocumentChanged, Event
from arqstudiocad.i18n import tr
from arqstudiocad.paths import sample_template
from arqstudiocad.recent_files import RecentFiles, same_file
from arqstudiocad.session import Session
from arqstudiocad.ui.block_drag import block_mime
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.panel_style import (
    SELECTION_BACKGROUND,
    SELECTION_LINE,
    apply_data_view_style,
)
from arqstudiocad.ui.property_grid import PropertyGrid, Section

#: The palette's sources, in the order the mockup shows them.
SOURCES = ("template", "recent", "drawing")
#: Columns of the list view.
NAME, IN_DRAWING = 0, 1
#: The views, and where the chosen one is remembered.
GRID, LIST = "grid", "list"
VIEW_SETTING = "blocks_palette/view"
#: The templates loaded last, newest first, as the settings keep them.
TEMPLATES_SETTING = "blocks_palette/templates"
TEMPLATES_LIMIT = 8
#: Files the palette loads as templates, dropped or chosen (ADR 0025 D5, D7).
TEMPLATE_SUFFIXES = (".dwt", ".dwg", ".dxf")
#: The empty Template source's placeholder picture, in pixels.
EMPTY_ICON = 48
#: Previews in the list's rows, in pixels.
ROW_PREVIEW = 24
#: Previews on the grid's cards, in pixels: fixed, so a wider palette holds
#: more columns rather than bigger cards (maintainer, 2026-10-23).
CARD_PREVIEW = 64
#: Room around a card's preview, in pixels.
CARD_MARGIN = 16
#: The dynamic-block tag: a five-pointed star, a symbol the same in every
#: language, in turquoise on orange, the same in both themes, as the
#: maintainer chose (ADR 0025 D4, 2026-10-23); about 5:1.
TAG_BACKGROUND = "#ffa64d"
TAG_STAR = "#004b4e"
#: The tag's side in pixels: on cards, and in the list's small previews.
TAG_ON_CARDS, TAG_IN_ROWS = 13, 10


def tag_side(size: int) -> int:
    """The tag's side on a preview of ``size`` pixels (logical)."""
    return TAG_IN_ROWS if size <= ROW_PREVIEW else TAG_ON_CARDS


def star(center: QPointF, radius: float) -> QPolygonF:
    """A five-pointed star, its top point up."""
    inner = radius * 0.382  # where the sides of a regular pentagram cross
    points = []
    for i in range(10):
        r = radius if i % 2 == 0 else inner
        angle = math.pi / 2 + i * math.pi / 5
        points.append(QPointF(center.x() + r * math.cos(angle), center.y() - r * math.sin(angle)))
    return QPolygonF(points)


def paint_tag(pixmap: QPixmap, size: int) -> None:
    """Paints the dynamic-block tag in the top-left corner of a preview of
    ``size`` pixels (logical): a filled star on a rounded orange square."""
    side = tag_side(size)
    painter = QPainter(pixmap)
    try:
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(TAG_BACKGROUND))
        painter.drawRoundedRect(0, 0, side, side, 2, 2)
        painter.setBrush(QColor(TAG_STAR))
        painter.drawPolygon(star(QPointF(side / 2, side / 2 + 0.5), side * 0.42))
    finally:
        painter.end()


def tag_pixmap(ratio: float = 1.0) -> QPixmap:
    """The tag alone, as the footer's legend shows it."""
    side = TAG_ON_CARDS
    pixmap = QPixmap(round(side * ratio), round(side * ratio))
    pixmap.setDevicePixelRatio(ratio)
    pixmap.fill(Qt.GlobalColor.transparent)
    paint_tag(pixmap, side * 4)
    return pixmap


def empty_icon(color: QColor, ratio: float = 1.0) -> QPixmap:
    """The empty Template source's placeholder, as the mockup draws it: three
    dashed squares and a plus where the fourth would be."""
    side = EMPTY_ICON
    pixmap = QPixmap(round(side * ratio), round(side * ratio))
    pixmap.setDevicePixelRatio(ratio)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    try:
        pen = QPen(color, 1.5)
        pen.setStyle(Qt.PenStyle.DashLine)
        painter.setPen(pen)
        cell = side / 2 - 4
        for x, y in ((2, 2), (side / 2 + 2, 2), (2, side / 2 + 2)):
            painter.drawRect(QRectF(x, y, cell, cell))
        pen.setStyle(Qt.PenStyle.SolidLine)
        painter.setPen(pen)
        middle = side * 3 / 4
        painter.drawLine(QPointF(middle - 6, middle), QPointF(middle + 6, middle))
        painter.drawLine(QPointF(middle, middle - 6), QPointF(middle, middle + 6))
    finally:
        painter.end()
    return pixmap


def template_file(path: str) -> Path | None:
    """A file the palette loads as a template, if ``path`` names one."""
    file = Path(path)
    return file if file.suffix.lower() in TEMPLATE_SUFFIXES else None


def preview_pixmap(
    document: CadDocument, name: str, size: int, colors: QPalette, ratio: float
) -> QPixmap | None:
    """Block ``name``'s preview, ``size`` logical pixels square, in its
    layers' colors as the viewport draws them over the theme's Base color,
    with the dynamic-block tag (ADR 0025 D3, D4); None if it cannot be drawn."""
    pixels = max(1, round(size * ratio))
    base = colors.color(QPalette.ColorRole.Base)
    data = document.block_preview(name, pixels, (base.red(), base.green(), base.blue()))
    if data is None:
        return None
    image = QImage(data, pixels, pixels, QImage.Format.Format_RGBA8888_Premultiplied).copy()
    pixmap = QPixmap.fromImage(image)
    pixmap.setDevicePixelRatio(ratio)
    if document.is_dynamic_block(name):
        paint_tag(pixmap, size)
    return pixmap


class PreviewCache:
    """Block previews as icons, plain and selected, per drawing, size and
    name; a drawing's are forgotten when a change touches any of its
    blocks, since a block may hold another (ADR 0025 D3)."""

    def __init__(self) -> None:
        self._icons: dict[tuple[int, int, str], QIcon] = {}
        #: How many previews were drawn, for tests and measurements.
        self.drawn = 0

    def icon(
        self, document: CadDocument, name: str, size: int, colors: QPalette, ratio: float
    ) -> QIcon:
        """Block ``name``'s preview in its layers' colors, as the viewport
        draws them over the theme's Base color (maintainer, 2026-10-23)."""
        key = (document.id, size, name)
        found = self._icons.get(key)
        if found is None:
            found = QIcon()
            pixmap = preview_pixmap(document, name, size, colors, ratio)
            if pixmap is not None:
                # The same picture when selected: the card's frame shows the
                # selection, and Qt would otherwise tint the colors.
                found.addPixmap(pixmap, QIcon.Mode.Normal)
                found.addPixmap(pixmap, QIcon.Mode.Selected)
                self.drawn += 1
            self._icons[key] = found
        return found

    def forget(self, document_id: int) -> None:
        for key in [k for k in self._icons if k[0] == document_id]:
            del self._icons[key]

    def clear(self) -> None:
        self._icons.clear()


#: What a drag of a block carries, by its name (None: no drag).
DragPayload = Callable[[str], QMimeData | None]


def no_drag(name: str) -> QMimeData | None:
    return None


def drag_only(view: QListWidget | QTreeWidget) -> None:
    """Blocks are dragged out onto the drawing, never moved within."""
    view.setDragEnabled(True)
    view.setDragDropMode(QAbstractItemView.DragDropMode.DragOnly)
    view.setDefaultDropAction(Qt.DropAction.CopyAction)


class BlockTable(QTreeWidget):
    """The list view: a row per block, each draggable onto the drawing."""

    def __init__(self) -> None:
        super().__init__()
        self.payload: DragPayload = no_drag
        drag_only(self)

    def mimeData(self, items: Sequence[QTreeWidgetItem]) -> QMimeData:  # noqa: N802 (Qt API)
        mime = self.payload(items[0].text(NAME)) if items else None
        return mime if mime is not None else QMimeData()


class BlockGrid(QListWidget):
    """The grid view: cards of a preview and a name, all one size; as many
    columns as the palette's width holds; each draggable onto the drawing."""

    def __init__(self) -> None:
        super().__init__()
        self.payload: DragPayload = no_drag
        self.setViewMode(QListView.ViewMode.IconMode)
        self.setResizeMode(QListView.ResizeMode.Adjust)
        self.setMovement(QListView.Movement.Static)
        self.setUniformItemSizes(True)
        self.setWrapping(True)
        self.setTextElideMode(Qt.TextElideMode.ElideRight)
        self.setSpacing(2)
        self.setIconSize(QSize(CARD_PREVIEW, CARD_PREVIEW))
        side = CARD_PREVIEW + CARD_MARGIN
        self.setGridSize(QSize(side, side + self.fontMetrics().height() + 4))
        # IconMode makes items movable within: only out, onto the drawing.
        drag_only(self)

    def mimeData(self, items: Sequence[QListWidgetItem]) -> QMimeData:  # noqa: N802 (Qt API)
        mime = self.payload(items[0].text()) if items else None
        return mime if mime is not None else QMimeData()

    def card_size(self) -> int:
        """A card's preview side, in pixels."""
        return CARD_PREVIEW


def source_title(source: str) -> str:
    return {
        "template": tr("Template"),
        "recent": tr("Recent"),
        "drawing": tr("Drawing"),
    }[source]


def units_name(code: int) -> str:
    """A block's insertion units, from DWG's INSUNITS code."""
    names = {
        0: tr("Unitless"),
        1: tr("Inches"),
        2: tr("Feet"),
        3: tr("Miles"),
        4: tr("Millimeters"),
        5: tr("Centimeters"),
        6: tr("Meters"),
        7: tr("Kilometers"),
        8: tr("Microinches"),
        9: tr("Mils"),
        10: tr("Yards"),
        11: tr("Angstroms"),
        12: tr("Nanometers"),
        13: tr("Microns"),
        14: tr("Decimeters"),
        15: tr("Decameters"),
        16: tr("Hectometers"),
        17: tr("Gigameters"),
        18: tr("Astronomical units"),
        19: tr("Light years"),
        20: tr("Parsecs"),
        21: tr("US survey feet"),
    }
    return names.get(code, tr("Unitless"))


def type_text(info: CadBlockInfo) -> str:
    """Static, or Dynamic block with the parameters it shows, if read."""
    if not info.is_dynamic:
        return tr("Static")
    if not info.parameters:
        return tr("Dynamic block")
    return tr("Dynamic block · {parameters}", parameters=", ".join(info.parameters))


def attributes_text(count: int) -> str:
    if count == 0:
        return tr("No attributes")
    if count == 1:
        return tr("1 attribute")
    return tr("{count} attributes", count=count)


def references_text(count: int) -> str:
    if count == 0:
        return tr("Not inserted")
    if count == 1:
        return tr("1 reference")
    return tr("{count} references", count=count)


class BlocksPanel(QDockWidget):
    """The blocks of the active drawing or of a template, without anonymous
    or layout blocks."""

    def __init__(
        self,
        session: Session,
        events: QtEventBridge,
        runner: CommandRunner | None = None,
        parent: QWidget | None = None,
        settings: QSettings | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("blocks_panel")
        self._session = session
        self._runner = runner
        self._settings = settings
        self.previews = PreviewCache()
        self._syncing = False
        #: The drawing changed while the palette was hidden.
        self._stale = True
        self._filling = False
        #: Each block's references in the active drawing, by upper-case
        #: name, counted at the last refresh.
        self._references: dict[str, int] = {}
        #: The source shown: one of SOURCES.
        self._source = "drawing"
        #: The template loaded, read-only and never in a tab (ADR 0025 D7).
        self.template: CadDocument | None = None
        stored = settings.value(TEMPLATES_SETTING, []) if settings is not None else []
        # An INI file gives back a list of one as a plain string.
        paths = [stored] if isinstance(stored, str) else stored
        self.recent_templates = RecentFiles(
            paths if isinstance(paths, list) else [], TEMPLATES_LIMIT
        )
        # A template dropped on the palette is loaded (ADR 0025 D5).
        self.setAcceptDrops(True)

        self.sources: dict[str, QToolButton] = {}
        self._source_group = QButtonGroup(self)
        source_row = QHBoxLayout()
        source_row.setSpacing(0)
        for source in SOURCES:
            button = QToolButton()
            button.setObjectName(f"blocks_source_{source}")
            button.setCheckable(True)
            button.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
            # Recent comes with its step (ADR 0025 D9).
            button.setEnabled(source != "recent")
            button.clicked.connect(lambda _=False, s=source: self.set_source(s))
            self._source_group.addButton(button)
            source_row.addWidget(button)
            self.sources[source] = button
        self.sources["drawing"].setChecked(True)
        self.source_caption = QLabel()
        # The Template source's row: the templates used last, and a folder
        # button to load another.
        self.template_combo = QComboBox()
        self.template_combo.setObjectName("blocks_template")
        self.template_combo.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self.template_combo.activated.connect(self._on_template_chosen)
        self.load_button = QToolButton()
        self.load_button.setObjectName("blocks_load_template")
        self.load_button.setAutoRaise(True)
        self.load_button.clicked.connect(lambda: self.load_template())
        self.template_row = QWidget()
        template_row = QHBoxLayout(self.template_row)
        template_row.setContentsMargins(0, 0, 0, 0)
        template_row.addWidget(self.template_combo, 1)
        template_row.addWidget(self.load_button)

        self.search = QLineEdit()
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(self.refresh)
        style = QApplication.style()
        self.grid_view = QToolButton()
        self.grid_view.setIcon(style.standardIcon(QStyle.StandardPixmap.SP_FileDialogContentsView))
        self.list_view = QToolButton()
        self.list_view.setIcon(style.standardIcon(QStyle.StandardPixmap.SP_FileDialogDetailedView))
        views = QButtonGroup(self)
        for button in (self.grid_view, self.list_view):
            button.setCheckable(True)
            button.setAutoRaise(True)
            views.addButton(button)
        self.grid_view.toggled.connect(self._on_view_chosen)
        search_row = QHBoxLayout()
        search_row.addWidget(self.search, 1)
        search_row.addWidget(self.grid_view)
        search_row.addWidget(self.list_view)

        self.table = BlockTable()
        self.table.setColumnCount(2)
        self.table.setRootIsDecorated(False)
        self.table.setUniformRowHeights(True)
        header = self.table.header()
        header.setStretchLastSection(False)
        header.setSectionResizeMode(NAME, QHeaderView.ResizeMode.Stretch)
        header.setSectionResizeMode(IN_DRAWING, QHeaderView.ResizeMode.ResizeToContents)
        self.table.currentItemChanged.connect(self._on_current_changed)
        # Enter or a double click inserts, as the Insert button.
        self.table.itemActivated.connect(self.insert_selected)
        self.table.setIconSize(QSize(ROW_PREVIEW, ROW_PREVIEW))
        self.grid = BlockGrid()
        self.grid.currentItemChanged.connect(self._on_card_chosen)
        self.grid.itemActivated.connect(self.insert_selected)
        self.table.payload = self.grid.payload = self._drag_payload
        #: Whether INSERT runs for a block being dragged over the drawing.
        self._dropping = False
        #: A template's block being dragged whose clashes are asked about
        #: once it is dropped: a dialog cannot open during a drag.
        self._pending_drop: str | None = None
        # The Template source with no template loaded, as the mockup's.
        self.empty = QWidget()
        self.empty.setObjectName("blocks_no_template")
        self.empty_icon = QLabel()
        self.empty_title = QLabel()
        self.empty_text = QLabel()
        self.empty_text.setWordWrap(True)
        self.empty_load = QPushButton()
        self.empty_load.setObjectName("blocks_empty_load")
        self.empty_load.clicked.connect(lambda: self.load_template())
        self.empty_recent_label = QLabel()
        self.empty_recent = QVBoxLayout()
        empty = QVBoxLayout(self.empty)
        empty.addStretch(1)
        for widget in (self.empty_icon, self.empty_title, self.empty_text):
            widget.setAlignment(Qt.AlignmentFlag.AlignCenter)
            empty.addWidget(widget)
        empty.addWidget(self.empty_load, 0, Qt.AlignmentFlag.AlignHCenter)
        self.empty_recent_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        empty.addWidget(self.empty_recent_label)
        empty.addLayout(self.empty_recent)
        empty.addStretch(2)
        self.views = QStackedWidget()
        self.views.addWidget(self.grid)
        self.views.addWidget(self.table)
        self.views.addWidget(self.empty)
        chosen = self._settings.value(VIEW_SETTING, GRID) if self._settings else GRID
        (self.list_view if chosen == LIST else self.grid_view).setChecked(True)
        self._on_view_chosen()
        self.footer = QLabel()
        # The tag's legend: the star as it looks, then what it means.
        self.legend_tag = QLabel()
        self.legend_text = QLabel()
        self.legend = QWidget()
        legend_row = QHBoxLayout(self.legend)
        legend_row.setContentsMargins(0, 0, 0, 0)
        legend_row.setSpacing(4)
        legend_row.addWidget(self.legend_tag)
        legend_row.addWidget(self.legend_text, 1)
        self.drag_hint = QLabel()
        self.details = PropertyGrid()
        self.details.caption.hide()

        top = QWidget()
        column = QVBoxLayout(top)
        column.setContentsMargins(4, 4, 4, 0)
        column.addLayout(source_row)
        column.addWidget(self.source_caption)
        column.addWidget(self.template_row)
        column.addLayout(search_row)
        column.addWidget(self.views, 1)
        column.addWidget(self.footer)
        column.addWidget(self.legend)
        column.addWidget(self.drag_hint)
        splitter = QSplitter(Qt.Orientation.Vertical)
        splitter.setChildrenCollapsible(False)
        splitter.addWidget(top)
        splitter.addWidget(self.details)
        splitter.setStretchFactor(0, 3)
        splitter.setStretchFactor(1, 2)

        # Insert options: a foldable section band, as the mockup's.
        self.options_band = QToolButton()
        self.options_band.setCheckable(True)
        self.options_band.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextBesideIcon)
        self.options_band.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        self.options_band.toggled.connect(self._fold_options)
        self.scale = QDoubleSpinBox()
        self.scale.setDecimals(3)
        self.scale.setRange(0.001, 1_000_000.0)
        self.scale.setValue(1.0)
        self.rotation = QDoubleSpinBox()
        self.rotation.setDecimals(2)
        self.rotation.setRange(-360.0, 360.0)
        self.rotation.setSuffix("°")
        self.on_screen = QCheckBox()
        self.on_screen.setChecked(True)
        self.explode = QCheckBox()
        self.scale_label, self.rotation_label = QLabel(), QLabel()
        self.options = QWidget()
        grid = QGridLayout(self.options)
        grid.setContentsMargins(4, 2, 4, 2)
        grid.addWidget(self.scale_label, 0, 0)
        grid.addWidget(self.scale, 0, 1)
        grid.addWidget(self.rotation_label, 0, 2)
        grid.addWidget(self.rotation, 0, 3)
        grid.addWidget(self.on_screen, 1, 0, 1, 2)
        grid.addWidget(self.explode, 1, 2, 1, 2)
        grid.setColumnStretch(1, 1)
        grid.setColumnStretch(3, 1)
        self.options_band.setChecked(True)
        self._fold_options(True)

        self.insert_button = QPushButton()
        self.insert_button.setObjectName("blocks_insert")
        self.edit_button = QPushButton()
        self.new_button = QPushButton()
        self.insert_button.clicked.connect(self.insert_selected)
        self.edit_button.clicked.connect(self.edit_selected)
        self.new_button.clicked.connect(self.new_block)
        actions = QHBoxLayout()
        actions.setContentsMargins(4, 4, 4, 4)
        for action_button in (self.insert_button, self.edit_button, self.new_button):
            actions.addWidget(action_button, 1)

        body = QWidget()
        rows = QVBoxLayout(body)
        rows.setContentsMargins(0, 0, 0, 0)
        rows.setSpacing(0)
        rows.addWidget(splitter, 1)
        rows.addWidget(self.options_band)
        rows.addWidget(self.options)
        rows.addLayout(actions)
        self.setWidget(body)
        for signal in (events.active_document_changed, events.document_changed):
            signal.connect(self.refresh)
        self.restyle()
        self.retranslate()

    def restyle(self) -> None:
        """Refresh palette-based colors after a theme change."""
        apply_data_view_style(self.table)
        self.details.restyle()
        # Previews are drawn over the theme's Base color: drawn again.
        self.previews.clear()
        colors = self.palette()
        muted = colors.color(QPalette.ColorRole.PlaceholderText).name()
        window = colors.color(QPalette.ColorRole.Window).name()
        button = colors.color(QPalette.ColorRole.Button).name()
        text = colors.color(QPalette.ColorRole.ButtonText).name()
        mid = colors.color(QPalette.ColorRole.Mid).name()
        highlight = colors.color(QPalette.ColorRole.Highlight).name()
        for label in (
            self.source_caption,
            self.footer,
            self.legend_text,
            self.drag_hint,
            self.empty_text,
            self.empty_recent_label,
        ):
            label.setStyleSheet(f"color: {muted};")
        self.empty_title.setStyleSheet(f"color: {text}; font-weight: bold;")
        self.empty_icon.setPixmap(
            empty_icon(colors.color(QPalette.ColorRole.PlaceholderText), self.devicePixelRatioF())
        )
        # A segmented control: the active source on the Button color with a
        # line in Highlight under it, as the mockup.
        segments = (
            f"QToolButton {{ background-color: {window}; color: {text}; "
            f"border: 1px solid {mid}; padding: 4px 8px; }}"
            f"QToolButton:checked {{ background-color: {button}; "
            f"border-bottom: 2px solid {highlight}; }}"
            f"QToolButton:disabled {{ color: {muted}; }}"
        )
        for source_button in self.sources.values():
            source_button.setStyleSheet(segments)
        base = colors.color(QPalette.ColorRole.Base).name()
        alternate = colors.color(QPalette.ColorRole.AlternateBase).name()
        self.grid.setStyleSheet(
            f"QListWidget {{ background-color: {base}; color: {text}; border: none; "
            f"outline: none; }}"
            f"QListWidget::item {{ background-color: {alternate}; border: 1px solid {mid}; "
            f"border-radius: 2px; }}"
            f"QListWidget::item:selected {{ background-color: {SELECTION_BACKGROUND}; "
            f"border: 1px solid {SELECTION_LINE}; color: {text}; }}"
        )
        # The property grid's section band, and Insert as the primary action.
        self.options_band.setStyleSheet(
            f"QToolButton {{ background-color: {button}; color: {text}; border: none; "
            f"padding: 3px 4px; font-weight: bold; text-align: left; }}"
        )
        highlighted = colors.color(QPalette.ColorRole.HighlightedText).name()
        primary = (
            f"QPushButton {{ background-color: {highlight}; color: {highlighted}; "
            f"border: 1px solid {highlight}; border-radius: 2px; padding: 4px 8px; }}"
            f"QPushButton:disabled {{ background-color: {window}; color: {muted}; "
            f"border-color: {mid}; }}"
        )
        self.insert_button.setStyleSheet(primary)
        self.empty_load.setStyleSheet(primary)

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Blocks"))
        for source, button in self.sources.items():
            button.setText(source_title(source))
            button.setToolTip("" if button.isEnabled() else tr("Not available yet."))
        self.template_combo.setPlaceholderText(tr("No template is loaded"))
        self.load_button.setIcon(
            QApplication.style().standardIcon(QStyle.StandardPixmap.SP_DirOpenIcon)
        )
        self.load_button.setToolTip(tr("Load template…"))
        self.empty_title.setText(tr("No template is loaded"))
        self.empty_text.setText(
            tr(
                "Load a .dwt or .dwg file to see its blocks here. "
                "You can also drop the file on this panel."
            )
        )
        self.empty_load.setText(tr("Load template…"))
        self.empty_recent_label.setText(tr("Recently used"))
        self._fill_templates()
        self.search.setPlaceholderText(tr("Search blocks"))
        self.grid_view.setToolTip(tr("Grid view"))
        self.list_view.setToolTip(tr("List view"))
        self.table.setHeaderLabels([tr("Name"), tr("In drawing")])
        self.legend_tag.setPixmap(tag_pixmap(self.devicePixelRatioF()))
        self.legend_tag.setToolTip(tr("Dynamic block"))
        self.legend_text.setText(tr("Dynamic block"))
        self.options_band.setText(tr("Insert options"))
        self.scale_label.setText(tr("Scale"))
        self.rotation_label.setText(tr("Rotation"))
        self.on_screen.setText(tr("Specify on screen"))
        self.explode.setText(tr("Explode"))
        self.explode.setToolTip(
            tr("Insert the block's contents instead of a reference, as EXPLODE does.")
        )
        self.drag_hint.setText(tr("Drag a block onto the drawing to insert it."))
        self.on_screen.setToolTip(
            tr("Pick the insertion point in the drawing; otherwise the block goes at the origin.")
        )
        self.insert_button.setText(tr("Insert"))
        self.edit_button.setText(tr("Edit block"))
        self.new_button.setText(tr("New…"))
        self._stale = True
        self.refresh()

    def document(self) -> CadDocument | None:
        document = self._session.active
        return document if document is not None and document.is_open else None

    def source(self) -> str:
        """The source shown: ``"template"``, ``"recent"`` or ``"drawing"``."""
        return self._source

    def source_document(self) -> CadDocument | None:
        """Where the blocks shown come from: the template or the drawing."""
        return self.template if self._source == "template" else self.document()

    def set_source(self, source: str) -> None:
        """Shows the blocks of ``source``, one of SOURCES."""
        self._source = source
        self.sources[source].setChecked(True)
        self.refresh()

    def load_template(self, path: Path | None = None) -> None:
        """Loads ``path``, or a file the user chooses, as the template, in the
        background (ADR 0025 D7); the Template source shows it once loaded."""
        if self._runner is not None:
            self._runner.start(load_template(path, self._template_loaded))
        elif path is not None:
            self._template_loaded(self._session.app.open_document(path))

    def _template_loaded(self, template: CadDocument) -> None:
        old = self.template
        if old is not None:
            self.previews.forget(old.id)
            self._session.app.close_document(old.id)
        self.template = self._session.template = template
        if template.path is not None:
            self.recent_templates.add(template.path)
            if self._settings is not None:
                self._settings.setValue(TEMPLATES_SETTING, self.recent_templates.to_strings())
        self._fill_templates()
        self.set_source("template")

    def templates(self) -> list[Path]:
        """The templates offered: those used last, then the sample template
        when it is not among them (ADR 0025 D7)."""
        offered = self.recent_templates.paths
        sample = sample_template()
        if sample.is_file() and not any(same_file(sample, p) for p in offered):
            offered.append(sample)
        return offered

    def _template_title(self, path: Path) -> str:
        return tr("Sample template") if same_file(path, sample_template()) else path.name

    def _fill_templates(self) -> None:
        """The combo and the empty state's buttons list the templates offered."""
        offered = self.templates()
        self.template_combo.clear()
        for path in offered:
            self.template_combo.addItem(self._template_title(path), str(path))
            self.template_combo.setItemData(
                self.template_combo.count() - 1, str(path), Qt.ItemDataRole.ToolTipRole
            )
        loaded = self.template.path if self.template is not None else None
        current = next(
            (i for i, path in enumerate(offered) if loaded is not None and same_file(path, loaded)),
            -1,
        )
        self.template_combo.setCurrentIndex(current)
        while self.empty_recent.count():
            item = self.empty_recent.takeAt(0)
            widget = item.widget() if item is not None else None
            if widget is not None:
                widget.deleteLater()
        for path in offered:
            button = QPushButton(self._template_title(path))
            button.setToolTip(str(path))
            button.clicked.connect(lambda _=False, chosen=path: self.load_template(chosen))
            self.empty_recent.addWidget(button)
        self.empty_recent_label.setVisible(bool(offered))

    def _on_template_chosen(self, index: int) -> None:
        chosen = self.template_combo.itemData(index)
        loaded = self.template.path if self.template is not None else None
        if chosen and (loaded is None or not same_file(Path(chosen), loaded)):
            self.load_template(Path(chosen))

    def dragEnterEvent(self, event: QDragEnterEvent) -> None:  # noqa: N802 (Qt API)
        if self._dropped_template(event.mimeData()) is not None:
            event.acceptProposedAction()
        else:
            event.ignore()

    def dropEvent(self, event: QDropEvent) -> None:  # noqa: N802 (Qt API)
        path = self._dropped_template(event.mimeData())
        if path is None:
            event.ignore()
            return
        event.acceptProposedAction()
        self.load_template(path)

    @staticmethod
    def _dropped_template(mime: QMimeData) -> Path | None:
        """The one template file a drop carries, if it carries one."""
        files = [url.toLocalFile() for url in mime.urls() if url.isLocalFile()]
        return template_file(files[0]) if len(files) == 1 else None

    def names(self) -> list[str]:
        """The blocks of the source shown that match the search,
        case-insensitively."""
        document = self.source_document()
        if document is None:
            return []
        wanted = self.search.text().strip().casefold()
        return [name for name in document.block_names if wanted in name.casefold()]

    def shown(self) -> list[str]:
        """The names the views show, in order."""
        return [self._row(i).text(NAME) for i in range(self.table.topLevelItemCount())]

    def view(self) -> str:
        """The view shown: ``GRID`` or ``LIST``."""
        return GRID if self.grid_view.isChecked() else LIST

    def selected(self) -> str | None:
        item = self.table.currentItem()
        return item.text(NAME) if item is not None else None

    def select(self, name: str) -> bool:
        """Selects block ``name`` in both views; False if it is not shown."""
        for index in range(self.table.topLevelItemCount()):
            item = self._row(index)
            if item.text(NAME) == name:
                self.table.setCurrentItem(item)
                return True
        return False

    def refresh(self, event: Event | str | None = None) -> None:
        if isinstance(event, DocumentChanged) and (
            event.changes.blocks_created
            or event.changes.blocks_modified
            or event.changes.blocks_deleted
        ):
            self.previews.forget(event.document_id)
        if not self.isVisible():
            # Counting references reads the whole drawing: not while closed.
            self._stale = True
            return
        self._stale = False
        drawing = self.document()
        document = self.source_document()
        from_template = self._source == "template"
        self.template_row.setVisible(from_template)
        self.source_caption.setVisible(not from_template)
        self._show_view()
        chosen = self.selected()
        self._filling = True
        try:
            self.table.clear()
            self.grid.clear()
            self._references = {
                name.upper(): count
                for name, count in (drawing.block_references if drawing else {}).items()
            }
            references = self._references
            if document is None:
                self.legend.setVisible(False)
                self.drag_hint.setVisible(False)
                self.table.setEnabled(False)
                self.source_caption.setText("")
                self.footer.setText("" if from_template else tr("No drawing is open."))
            else:
                self.table.setEnabled(True)
                self.source_caption.setText(tr("Definitions in {name}", name=document.name))
                shown = self.names()
                dynamic = {name for name in shown if document.is_dynamic_block(name)}
                self.legend.setVisible(bool(dynamic))
                # Dragged onto a drawing, which there must be.
                self.drag_hint.setVisible(bool(shown) and drawing is not None)
                for name in shown:
                    count = references.get(name.upper(), 0)
                    item = QTreeWidgetItem([name, str(count)])
                    item.setTextAlignment(
                        IN_DRAWING, Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter
                    )
                    item.setIcon(NAME, self._icon(document, name, ROW_PREVIEW))
                    self.table.addTopLevelItem(item)
                    card = QListWidgetItem(self._icon(document, name, self.grid.card_size()), name)
                    card.setToolTip(name)
                    if name in dynamic:
                        # The tag is drawn: say it to screen readers and tooltips.
                        card.setToolTip(f"{name}\n{tr('Dynamic block')}")
                        card.setData(Qt.ItemDataRole.AccessibleDescriptionRole, tr("Dynamic block"))
                        item.setToolTip(NAME, card.toolTip())
                        item.setData(
                            NAME, Qt.ItemDataRole.AccessibleDescriptionRole, tr("Dynamic block")
                        )
                    self.grid.addItem(card)
                if not document.block_names:
                    self.footer.setText(
                        tr("The template has no blocks.")
                        if from_template
                        else tr("The drawing has no blocks.")
                    )
                elif not shown:
                    self.footer.setText(tr("No block matches the search."))
                elif len(shown) == 1:
                    self.footer.setText(tr("1 block"))
                else:
                    self.footer.setText(tr("{count} blocks", count=len(shown)))
            if chosen is not None:
                self.select(chosen)
        finally:
            self._filling = False
        self._show_details()

    def insert_selected(self) -> None:
        """Inserts the selected block with the insert options, through
        INSERT: one undo step (ADR 0025 D5)."""
        name = self.selected()
        if name is None or self._runner is None or self.document() is None:
            return
        self._runner.start(
            insert_named(
                name,
                self.scale.value(),
                math.radians(self.rotation.value()),
                self.on_screen.isChecked(),
                self.explode.isChecked(),
                self._inserting_from(),
            )
        )

    def _inserting_from(self) -> CadDocument | None:
        """The template a block inserted now is copied from; None for the
        drawing's own blocks."""
        return self.template if self._source == "template" else None

    def start_drop_insert(self, name: str) -> None:
        """A block dragged from here came over the drawing: INSERT runs on
        screen with the insert options, so the block follows the cursor
        until it is dropped (ADR 0025 D5)."""
        document = self.document()
        if self._runner is None or document is None:
            return
        template = self._inserting_from()
        if template is not None and document.block_clashes(name, template):
            self._pending_drop = name
            return
        self._dropping = True
        self._runner.start(
            insert_named(
                name,
                self.scale.value(),
                math.radians(self.rotation.value()),
                True,
                self.explode.isChecked(),
                self._inserting_from(),
            )
        )

    def end_drop_insert(self, dropped: bool) -> None:
        """The drag ended: dropped, the block is placed; otherwise the
        INSERT it started is cancelled and nothing was inserted."""
        runner = self._runner
        if self._dropping and not dropped and runner is not None and runner.active == "INSERT":
            runner.cancel()
        self._dropping = False
        self._pending_drop = None

    def drop_insert_at(self, point: tuple[float, float]) -> None:
        """A template's block whose names clash was dropped at ``point``: once
        the drag is over, INSERT asks about the clashes and places it there
        (ADR 0025 D5, D6)."""
        name, runner = self._pending_drop, self._runner
        if name is None or runner is None:
            return
        command = insert_named(
            name,
            self.scale.value(),
            math.radians(self.rotation.value()),
            False,
            self.explode.isChecked(),
            self._inserting_from(),
            at=(point[0], point[1]),
        )
        QTimer.singleShot(0, lambda: runner.start(command))

    def _drag_payload(self, name: str) -> QMimeData | None:
        # For the active drawing, which a template's block is copied into.
        document = self.document()
        return None if document is None else block_mime(document.id, name)

    def edit_selected(self) -> None:
        """Opens the selected block in the block editor, through BEDIT."""
        name = self.selected()
        if self._source != "drawing":
            return
        if name is not None and self._runner is not None and self.document() is not None:
            self._runner.start(edit_named(name))

    def new_block(self) -> None:
        """Makes a new block from objects, through BLOCK."""
        if self._runner is None or self.document() is None:
            return
        command = self._runner.registry.find("BLOCK")
        if command is not None:
            self._runner.start(command)

    def _update_actions(self) -> None:
        document = self.document()
        source = self.source_document()
        name = self.selected()
        chosen = document is not None and source is not None and name is not None
        self.insert_button.setEnabled(chosen)
        self.new_button.setEnabled(document is not None)
        # A dynamic block's parameters wait for the parametric engine
        # (ADR 0023 D3a): it is inserted, never edited, and the button says why.
        dynamic = source is not None and name is not None and source.is_dynamic_block(name)
        if self._source != "drawing":
            # A template is never written (ADR 0025 D7).
            self.edit_button.setEnabled(False)
            self.edit_button.setToolTip(tr("Only the drawing's blocks can be edited."))
            return
        self.edit_button.setEnabled(chosen and not dynamic)
        self.edit_button.setToolTip(
            tr("{name} is a dynamic block: it cannot be edited yet.", name=name) if dynamic else ""
        )

    def _fold_options(self, unfolded: bool) -> None:
        self.options.setVisible(unfolded)
        self.options_band.setArrowType(
            Qt.ArrowType.DownArrow if unfolded else Qt.ArrowType.RightArrow
        )

    def _row(self, index: int) -> QTreeWidgetItem:
        item = self.table.topLevelItem(index)
        assert item is not None
        return item

    def _show_view(self) -> None:
        """The grid or the list, as chosen; the empty state while the Template
        source has no template."""
        if self._source == "template" and self.template is None:
            self.views.setCurrentWidget(self.empty)
        else:
            self.views.setCurrentWidget(self.grid if self.view() == GRID else self.table)

    def _icon(self, document: CadDocument, name: str, size: int) -> QIcon:
        return self.previews.icon(document, name, size, self.palette(), self.devicePixelRatioF())

    def _on_current_changed(self) -> None:
        # The grid follows the list's selection.
        name = self.selected()
        self._syncing = True
        try:
            card = next(
                (
                    self.grid.item(i)
                    for i in range(self.grid.count())
                    if self.grid.item(i).text() == name
                ),
                None,
            )
            if card is None:
                self.grid.clearSelection()
            else:
                self.grid.setCurrentItem(card)
        finally:
            self._syncing = False
        if not self._filling:
            self._show_details()

    def _on_card_chosen(self, card: QListWidgetItem | None) -> None:
        if not self._syncing and card is not None:
            self.select(card.text())

    def _on_view_chosen(self) -> None:
        self._show_view()
        if self._settings is not None:
            self._settings.setValue(VIEW_SETTING, self.view())

    def _show_details(self) -> None:
        self._update_actions()
        document = self.source_document()
        name = self.selected()
        info = document.block_info(name) if document is not None and name else None
        if document is None or info is None:
            self.details.show_sections("", [])
            return
        count = self._references.get(info.name.upper(), 0)
        self.details.show_sections("", [self._section(document, info, count)])

    @staticmethod
    def _section(document: CadDocument, info: CadBlockInfo, references: int) -> Section:
        rows = [(tr("Name"), info.name)]
        if info.description:
            rows.append((tr("Description"), info.description))
        rows += [
            (tr("Type"), type_text(info)),
            (tr("Units"), units_name(info.units)),
            (tr("Attributes"), attributes_text(info.attributes)),
            (tr("In drawing"), references_text(references)),
            (tr("Source"), document.name),
        ]
        return Section("selected_block", tr("Selected block"), rows)

    def showEvent(self, event: QShowEvent) -> None:  # noqa: N802 (Qt API)
        # Opened again: catch up with what changed while it was closed.
        super().showEvent(event)
        if self._stale:
            self.refresh()
