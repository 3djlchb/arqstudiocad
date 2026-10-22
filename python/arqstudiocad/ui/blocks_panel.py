"""The Blocks palette (ADR 0025): the block definitions of the active
drawing, filtered as the user types, and the details of the one selected.

Laid out as the maintainer's mockup (docs/design/blocks-palette.md): the
sources (Template, Recent, Drawing), a caption naming where the blocks come
from, the search with the grid and list views, the blocks, a footer, and
the Selected block section, the insert options and the actions: Insert,
Edit block and New, which start INSERT, BEDIT and BLOCK. Only the Drawing
source and the list view work yet; the others come with their steps (ADR
0025 D9) and say so.
"""

import math

from PySide6.QtCore import Qt
from PySide6.QtGui import QPalette, QShowEvent
from PySide6.QtWidgets import (
    QApplication,
    QButtonGroup,
    QCheckBox,
    QDockWidget,
    QDoubleSpinBox,
    QGridLayout,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QPushButton,
    QSizePolicy,
    QSplitter,
    QStyle,
    QToolButton,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.commands import CommandRunner
from arqstudiocad.commands.blocks import edit_named, insert_named
from arqstudiocad.core import CadBlockInfo, CadDocument
from arqstudiocad.events import Event
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.panel_style import apply_data_view_style
from arqstudiocad.ui.property_grid import PropertyGrid, Section

#: The palette's sources, in the order the mockup shows them.
SOURCES = ("template", "recent", "drawing")
#: Columns of the list view.
NAME, IN_DRAWING = 0, 1


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
    """The blocks of the active drawing, without anonymous or layout blocks."""

    def __init__(
        self,
        session: Session,
        events: QtEventBridge,
        runner: CommandRunner | None = None,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("blocks_panel")
        self._session = session
        self._runner = runner
        #: The drawing changed while the palette was hidden.
        self._stale = True
        self._filling = False
        #: Each block's references, counted at the last refresh.
        self._references: dict[str, int] = {}

        self.sources: dict[str, QToolButton] = {}
        self._source_group = QButtonGroup(self)
        source_row = QHBoxLayout()
        source_row.setSpacing(0)
        for source in SOURCES:
            button = QToolButton()
            button.setObjectName(f"blocks_source_{source}")
            button.setCheckable(True)
            button.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
            # Only the drawing's blocks yet (ADR 0025 D9).
            button.setEnabled(source == "drawing")
            self._source_group.addButton(button)
            source_row.addWidget(button)
            self.sources[source] = button
        self.sources["drawing"].setChecked(True)
        self.source_caption = QLabel()

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
        self.list_view.setChecked(True)
        # The grid shows previews, which come in step 3 (ADR 0025 D9).
        self.grid_view.setEnabled(False)
        search_row = QHBoxLayout()
        search_row.addWidget(self.search, 1)
        search_row.addWidget(self.grid_view)
        search_row.addWidget(self.list_view)

        self.table = QTreeWidget()
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
        self.footer = QLabel()
        self.details = PropertyGrid()
        self.details.caption.hide()

        top = QWidget()
        column = QVBoxLayout(top)
        column.setContentsMargins(4, 4, 4, 0)
        column.addLayout(source_row)
        column.addWidget(self.source_caption)
        column.addLayout(search_row)
        column.addWidget(self.table, 1)
        column.addWidget(self.footer)
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
        self.scale_label, self.rotation_label = QLabel(), QLabel()
        self.options = QWidget()
        grid = QGridLayout(self.options)
        grid.setContentsMargins(4, 2, 4, 2)
        grid.addWidget(self.scale_label, 0, 0)
        grid.addWidget(self.scale, 0, 1)
        grid.addWidget(self.rotation_label, 0, 2)
        grid.addWidget(self.rotation, 0, 3)
        grid.addWidget(self.on_screen, 1, 0, 1, 4)
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
        colors = self.palette()
        muted = colors.color(QPalette.ColorRole.PlaceholderText).name()
        window = colors.color(QPalette.ColorRole.Window).name()
        button = colors.color(QPalette.ColorRole.Button).name()
        text = colors.color(QPalette.ColorRole.ButtonText).name()
        mid = colors.color(QPalette.ColorRole.Mid).name()
        highlight = colors.color(QPalette.ColorRole.Highlight).name()
        for label in (self.source_caption, self.footer):
            label.setStyleSheet(f"color: {muted};")
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
        # The property grid's section band, and Insert as the primary action.
        self.options_band.setStyleSheet(
            f"QToolButton {{ background-color: {button}; color: {text}; border: none; "
            f"padding: 3px 4px; font-weight: bold; text-align: left; }}"
        )
        highlighted = colors.color(QPalette.ColorRole.HighlightedText).name()
        self.insert_button.setStyleSheet(
            f"QPushButton {{ background-color: {highlight}; color: {highlighted}; "
            f"border: 1px solid {highlight}; border-radius: 2px; padding: 4px 8px; }}"
            f"QPushButton:disabled {{ background-color: {window}; color: {muted}; "
            f"border-color: {mid}; }}"
        )

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Blocks"))
        for source, button in self.sources.items():
            button.setText(source_title(source))
            button.setToolTip("" if button.isEnabled() else tr("Not available yet."))
        self.search.setPlaceholderText(tr("Search blocks"))
        self.grid_view.setToolTip(tr("Grid view (not available yet)"))
        self.list_view.setToolTip(tr("List view"))
        self.table.setHeaderLabels([tr("Name"), tr("In drawing")])
        self.options_band.setText(tr("Insert options"))
        self.scale_label.setText(tr("Scale"))
        self.rotation_label.setText(tr("Rotation"))
        self.on_screen.setText(tr("Specify on screen"))
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

    def names(self) -> list[str]:
        """The active drawing's blocks that match the search, case-insensitively."""
        document = self.document()
        if document is None:
            return []
        wanted = self.search.text().strip().casefold()
        return [name for name in document.block_names if wanted in name.casefold()]

    def shown(self) -> list[str]:
        """The names the list shows, in order."""
        return [self._row(i).text(NAME) for i in range(self.table.topLevelItemCount())]

    def selected(self) -> str | None:
        item = self.table.currentItem()
        return item.text(NAME) if item is not None else None

    def select(self, name: str) -> bool:
        """Selects block ``name`` in the list; False if it is not shown."""
        for index in range(self.table.topLevelItemCount()):
            item = self._row(index)
            if item.text(NAME) == name:
                self.table.setCurrentItem(item)
                return True
        return False

    def refresh(self, event: Event | str | None = None) -> None:
        if not self.isVisible():
            # Counting references reads the whole drawing: not while closed.
            self._stale = True
            return
        self._stale = False
        document = self.document()
        chosen = self.selected()
        self._filling = True
        try:
            self.table.clear()
            if document is None:
                self.table.setEnabled(False)
                self.source_caption.setText("")
                self.footer.setText(tr("No drawing is open."))
            else:
                self.table.setEnabled(True)
                self.source_caption.setText(tr("Definitions in {name}", name=document.name))
                self._references = references = document.block_references
                shown = self.names()
                for name in shown:
                    count = references.get(name, 0)
                    item = QTreeWidgetItem([name, str(count)])
                    item.setTextAlignment(
                        IN_DRAWING, Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter
                    )
                    self.table.addTopLevelItem(item)
                if not document.block_names:
                    self.footer.setText(tr("The drawing has no blocks."))
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
            )
        )

    def edit_selected(self) -> None:
        """Opens the selected block in the block editor, through BEDIT."""
        name = self.selected()
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
        drawing = self.document() is not None
        chosen = drawing and self.selected() is not None
        self.insert_button.setEnabled(chosen)
        self.edit_button.setEnabled(chosen)
        self.new_button.setEnabled(drawing)

    def _fold_options(self, unfolded: bool) -> None:
        self.options.setVisible(unfolded)
        self.options_band.setArrowType(
            Qt.ArrowType.DownArrow if unfolded else Qt.ArrowType.RightArrow
        )

    def _row(self, index: int) -> QTreeWidgetItem:
        item = self.table.topLevelItem(index)
        assert item is not None
        return item

    def _on_current_changed(self) -> None:
        if not self._filling:
            self._show_details()

    def _show_details(self) -> None:
        self._update_actions()
        document = self.document()
        name = self.selected()
        info = document.block_info(name) if document is not None and name else None
        if document is None or info is None:
            self.details.show_sections("", [])
            return
        count = self._references.get(info.name, 0)
        self.details.show_sections("", [self._section(document, info, count)])

    @staticmethod
    def _section(document: CadDocument, info: CadBlockInfo, references: int) -> Section:
        rows = [(tr("Name"), info.name)]
        if info.description:
            rows.append((tr("Description"), info.description))
        rows += [
            (tr("Type"), tr("Dynamic block") if info.is_dynamic else tr("Static")),
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
