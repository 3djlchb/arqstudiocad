"""The layers panel (ADR 0013, D5.6), opened by the LAYER command.

A table of the active drawing's layers: current, name, on, frozen, locked,
color, linetype, plotted. Every change goes through the session, so it is one
undo step, reaches the viewport and the other panels by events, and a refused
change (freezing the current layer…) is reported on the command line.
"""

from collections.abc import Callable
from functools import cache

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QIcon, QPalette, QPixmap
from PySide6.QtWidgets import (
    QApplication,
    QDockWidget,
    QHBoxLayout,
    QMenu,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadChangeSet, CadColor, CadDocument, CadError, CadLayerInfo
from arqstudiocad.events import CommandMessage, DocumentChanged, Event
from arqstudiocad.i18n import tr
from arqstudiocad.paths import icons_dir
from arqstudiocad.session import Session
from arqstudiocad.ui.color_picker import (
    ColorPickerDialog,
    recent_colors,
    remember_color,
)
from arqstudiocad.ui.color_picker import color_name as color_name
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.panel_style import apply_data_view_style, apply_panel_action_style

CURRENT, NAME, ON, FROZEN, LOCKED, PLOT, COLOR, LINETYPE = range(8)
LAYER_ID = Qt.ItemDataRole.UserRole


@cache
def _layer_state_icon(name: str) -> QIcon:
    return QIcon(str(icons_dir() / "icons_layers" / name))


def _lock_state_icon(locked: bool) -> QIcon:
    dark = QApplication.palette().color(QPalette.ColorRole.Window).lightness() < 128
    folder = "dark-themes" if dark else "clear-themes"
    if dark:
        filename = "lock-on-dark.svg" if locked else "lock-off-dark.svg"
    else:
        filename = "lock-on.svg" if locked else "lock-off.svg"
    return _layer_state_icon(f"{folder}/{filename}")


def _quick_color_choices() -> tuple[CadColor, ...]:
    """Standard colors followed by distinct colors used recently this session."""
    standard = tuple(CadColor.from_index(index) for index in range(1, 10))
    recent = tuple(color for color in recent_colors() if color not in standard)
    return (*standard, *recent)


def swatch(color: CadColor) -> QIcon:
    pixmap = QPixmap(14, 14)
    pixmap.fill(QColor(*color.display_rgb()))
    return QIcon(pixmap)


class LayersPanel(QDockWidget):
    """The active drawing's layers."""

    def __init__(
        self, session: Session, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("layers_panel")
        self._session = session
        self._filling = False

        self.new_button = QPushButton()
        self.delete_button = QPushButton()
        self.current_button = QPushButton()
        self.new_button.clicked.connect(self.new_layer)
        self.delete_button.clicked.connect(self.delete_selected)
        self.current_button.clicked.connect(self.set_selected_current)
        buttons = QHBoxLayout()
        buttons.setContentsMargins(2, 2, 2, 3)
        buttons.setSpacing(4)
        for button in (self.new_button, self.delete_button, self.current_button):
            apply_panel_action_style(button)
            buttons.addWidget(button)
        buttons.addStretch(1)

        self.table = QTableWidget(0, 8)
        self.table.setSelectionBehavior(QTableWidget.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QTableWidget.SelectionMode.SingleSelection)
        self.table.verticalHeader().hide()
        apply_data_view_style(self.table, row_height=25)
        self.table.itemChanged.connect(self._on_item_changed)
        self.table.cellDoubleClicked.connect(self._on_double_click)
        self.table.cellClicked.connect(self._on_click)

        body = self._body = QWidget()
        layout = QVBoxLayout(body)
        layout.setContentsMargins(3, 3, 3, 3)
        layout.setSpacing(2)
        layout.addLayout(buttons)
        layout.addWidget(self.table, 1)
        self.setWidget(body)

        events.active_document_changed.connect(self.refresh)
        events.document_changed.connect(self._on_document_changed)
        self.retranslate()

    def restyle(self) -> None:
        """Rebuilds palette-based styles when the application theme changes."""
        apply_data_view_style(self.table, row_height=25)
        for button in (self.new_button, self.delete_button, self.current_button):
            apply_panel_action_style(button)
        self.refresh()

    # Reading the drawing

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Layers"))
        self.new_button.setText(tr("New"))
        self.delete_button.setText(tr("Delete"))
        self.current_button.setText(tr("Set current"))
        self.table.setHorizontalHeaderLabels(
            [
                tr("Current"),
                tr("Name"),
                tr("On"),
                tr("Frozen"),
                tr("Locked"),
                tr("Plot"),
                tr("Color"),
                tr("Linetype"),
            ]
        )
        self.refresh()

    def refresh(self, event: Event | None = None) -> None:
        """Fills the table from the active drawing."""
        document = self._session.active
        layers = [] if document is None else document.get_layers()
        current = None if document is None else document.current_layer.id
        self._filling = True
        try:
            self.table.setRowCount(len(layers))
            for row, layer in enumerate(layers):
                self._fill_row(row, layer, layer.id == current)
        finally:
            self._filling = False
        self._body.setEnabled(document is not None)
        self.table.resizeColumnsToContents()

    def _fill_row(self, row: int, layer: CadLayerInfo, current: bool) -> None:
        def cell(text: str = "", editable: bool = False) -> QTableWidgetItem:
            item = QTableWidgetItem(text)
            item.setData(LAYER_ID, layer.id)
            flags = Qt.ItemFlag.ItemIsSelectable | Qt.ItemFlag.ItemIsEnabled
            if editable:
                flags |= Qt.ItemFlag.ItemIsEditable
            item.setFlags(flags)
            return item

        color = cell(color_name(layer.color))
        color.setIcon(swatch(layer.color))
        current_item = cell()
        current_item.setIcon(
            _layer_state_icon("layer-state-on.svg" if current else "layer-state-off.svg")
        )
        self.table.setItem(row, CURRENT, current_item)
        self.table.setItem(row, NAME, cell(layer.name, editable=True))
        visibility_item = cell()
        visibility_item.setIcon(
            _layer_state_icon("bulb-on.svg" if layer.is_visible else "bulb-off.svg")
        )
        self.table.setItem(row, ON, visibility_item)
        frozen_item = cell()
        frozen_item.setIcon(_layer_state_icon("sun-on.svg" if layer.is_frozen else "sun-off.svg"))
        self.table.setItem(row, FROZEN, frozen_item)
        locked_item = cell()
        locked_item.setIcon(_lock_state_icon(layer.is_locked))
        self.table.setItem(row, LOCKED, locked_item)
        plot_item = cell()
        plot_item.setIcon(
            _layer_state_icon("printer-on.svg" if layer.is_plottable else "printer-off.svg")
        )
        self.table.setItem(row, PLOT, plot_item)
        self.table.setItem(row, COLOR, color)
        self.table.setItem(row, LINETYPE, cell(layer.linetype))

    def _on_document_changed(self, event: DocumentChanged) -> None:
        changes = event.changes
        tables = changes.layers_created or changes.layers_modified or changes.layers_deleted
        if tables or changes.document_changed:
            self.refresh()

    # Changing the drawing

    def layer_id_at(self, row: int) -> int | None:
        item = self.table.item(row, NAME)
        return None if item is None else int(item.data(LAYER_ID))

    def selected_layer_id(self) -> int | None:
        rows = self.table.selectionModel().selectedRows()
        return self.layer_id_at(rows[0].row()) if rows else None

    def new_layer(self) -> int | None:
        """Creates Layer1, Layer2… (the first free name) and starts editing
        its name."""
        document = self._session.active
        if document is None:
            return None
        taken = {layer.name.upper() for layer in document.get_layers()}
        number = 1
        while tr("Layer{number}", number=number).upper() in taken:
            number += 1
        name = tr("Layer{number}", number=number)
        created: list[int] = []

        def create(d: CadDocument) -> CadChangeSet:
            layer_id, changes = d.create_layer(name)
            created.append(layer_id)
            return changes

        if not self._edit(create) or not created:
            return None
        for row in range(self.table.rowCount()):
            if self.layer_id_at(row) == created[0]:
                self.table.selectRow(row)
                item = self.table.item(row, NAME)
                if item is not None:
                    self.table.editItem(item)
        return created[0]

    def delete_selected(self) -> None:
        layer_id = self.selected_layer_id()
        if layer_id is not None:
            self._edit(lambda d: d.delete_layer(layer_id))

    def set_selected_current(self) -> None:
        layer_id = self.selected_layer_id()
        if layer_id is not None:
            self._edit(lambda d: d.set_current_layer(layer_id))

    def set_color(self, layer_id: int, color: CadColor) -> None:
        if self._edit(lambda d: d.set_layer_color(layer_id, color)):
            remember_color(color)

    def _on_item_changed(self, item: QTableWidgetItem) -> None:
        if self._filling or item.column() != NAME:
            return
        layer_id = int(item.data(LAYER_ID))
        name = item.text().strip()
        self._edit(lambda d: d.rename_layer(layer_id, name))

    def _on_double_click(self, row: int, column: int) -> None:
        if column == CURRENT:
            layer_id = self.layer_id_at(row)
            if layer_id is not None:
                self._edit(lambda d: d.set_current_layer(layer_id))

    def _on_click(self, row: int, column: int) -> None:
        layer_id = self.layer_id_at(row)
        if layer_id is None:
            return
        if column == CURRENT:
            self._edit(lambda d: d.set_current_layer(layer_id))
            return
        if column == ON:
            document = self._session.active
            if document is not None:
                visible = document.get_layer(layer_id).is_visible
                self._edit(lambda d: d.set_layer_visible(layer_id, not visible))
            return
        if column == FROZEN:
            document = self._session.active
            if document is not None:
                frozen = document.get_layer(layer_id).is_frozen
                self._edit(lambda d: d.set_layer_frozen(layer_id, not frozen))
            return
        if column == LOCKED:
            document = self._session.active
            if document is not None:
                locked = document.get_layer(layer_id).is_locked
                self._edit(lambda d: d.set_layer_locked(layer_id, not locked))
            return
        if column == PLOT:
            document = self._session.active
            if document is not None:
                plottable = document.get_layer(layer_id).is_plottable
                self._edit(lambda d: d.set_layer_plottable(layer_id, not plottable))
            return
        if column != COLOR:
            return
        menu = QMenu(self)
        menu.setStyleSheet(
            "QMenu::item:selected { background-color: rgba(117, 143, 164, 48); "
            "border-top: 1px solid rgba(0, 140, 255, 230); "
            "border-bottom: 1px solid rgba(0, 140, 255, 230); "
            "color: palette(text); }"
        )
        choices = _quick_color_choices()
        for choice_index, choice in enumerate(choices):
            if choice_index == 9:
                menu.addSeparator()
            action = menu.addAction(swatch(choice), color_name(choice))
            action.triggered.connect(lambda _checked=False, c=choice: self.set_color(layer_id, c))
        if len(choices) == 9:
            menu.addSeparator()
        other = menu.addAction(tr("More colors…"))
        other.triggered.connect(lambda: self._pick_color(layer_id))
        item = self.table.item(row, column)
        if item is not None:
            rect = self.table.visualItemRect(item)
            menu.popup(self.table.viewport().mapToGlobal(rect.bottomLeft()))

    def _pick_color(self, layer_id: int) -> None:
        document = self._session.active
        current = None if document is None else document.get_layer(layer_id).color
        picker = ColorPickerDialog(current, allow_special=False, parent=self)
        if picker.exec() == ColorPickerDialog.DialogCode.Accepted:
            self.set_color(layer_id, picker.selected_color())

    def _edit(self, operation: Callable[[CadDocument], CadChangeSet]) -> bool:
        """One change through the session; a refused one is reported on the
        command line and the table shows the drawing as it is."""
        document = self._session.active
        if document is None:
            return False
        try:
            self._session.edit(document, operation)
        except CadError as error:
            self._session.bus.publish(CommandMessage(tr("Error: {message}", message=str(error))))
            self.refresh()
            return False
        return True
