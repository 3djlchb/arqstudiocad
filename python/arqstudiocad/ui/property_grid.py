"""A property grid after OpenCADStudio's (ADR 0015): a caption naming what
is shown, then foldable sections of name and value rows.

Read-only for now; editors per value (color, layer, linetype…) come with
selection in milestone 2.5.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from typing import cast

from PySide6.QtCore import QModelIndex, QPersistentModelIndex, Qt, Signal
from PySide6.QtGui import QColor, QFont, QIcon, QPainter, QPalette, QPen, QPixmap
from PySide6.QtWidgets import (
    QApplication,
    QComboBox,
    QCompleter,
    QHeaderView,
    QLabel,
    QStyledItemDelegate,
    QStyleOptionViewItem,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.i18n import tr
from arqstudiocad.ui.panel_style import apply_property_grid_style

#: Where a section title keeps its key and its title without the arrow.
KEY = Qt.ItemDataRole.UserRole
TITLE = Qt.ItemDataRole.UserRole + 1
EDIT_KEY = Qt.ItemDataRole.UserRole + 2


@dataclass(frozen=True)
class Choice:
    """One option in a property editor, optionally painted with a swatch."""

    label: str
    value: object
    color: tuple[int, int, int] | None = None


@dataclass(frozen=True)
class Property:
    """One displayed property, with an optional editor."""

    key: str
    name: str
    value: str
    editable: bool = False
    choices: Sequence[Choice] = ()
    choice_value: object = None
    accent: bool = False
    searchable: bool = False


@dataclass(frozen=True)
class Section:
    """Rows under a foldable title; ``key`` keeps it folded across refreshes
    and languages."""

    key: str
    title: str
    rows: Sequence[tuple[str, str] | Property]


class PropertyGridDelegate(QStyledItemDelegate):
    """Paints property fields and section bands after the reference palette."""

    def __init__(self, tree: QTreeWidget) -> None:
        super().__init__(tree)
        self.tree = tree

    def paint(
        self,
        painter: QPainter,
        option: QStyleOptionViewItem,
        index: QModelIndex | QPersistentModelIndex,
    ) -> None:
        index = cast(QModelIndex, index)
        item = self.tree.itemFromIndex(index)
        if item is None:
            super().paint(painter, option, index)
            return

        column = index.column()
        if item.parent() is None:
            if column == 0:
                self._paint_section(painter, option, item)
            return
        if column == 0:
            option = QStyleOptionViewItem(option)
            option.palette.setColor(
                QPalette.ColorRole.Text,
                QColor(str(self.tree.property("propertyLabelColor") or "#8a8a8a")),
            )
            super().paint(painter, option, index)
            return
        if column == 1:
            self._paint_value(painter, option, index)
            return
        super().paint(painter, option, index)

    def _paint_section(
        self, painter: QPainter, option: QStyleOptionViewItem, item: QTreeWidgetItem
    ) -> None:
        rect = option.rect
        painter.save()
        painter.fillRect(
            rect,
            QColor(str(self.tree.property("propertySectionColor") or "#3a3a3e")),
        )
        painter.setPen(QPen(QColor(str(self.tree.property("propertyBorderColor") or "#46464c"))))
        painter.drawLine(rect.left(), rect.bottom(), rect.right(), rect.bottom())
        painter.setFont(item.font(0))
        painter.setPen(QColor(str(self.tree.property("propertySectionTextColor") or "#e0e0e0")))
        text_rect = rect.adjusted(7, 0, -22, 0)
        painter.drawText(
            text_rect,
            Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter,
            str(item.data(0, TITLE)),
        )
        center_x = rect.right() - 12
        center_y = rect.center().y()
        if item.isExpanded():
            points = (
                (center_x - 4, center_y - 2),
                (center_x, center_y + 2),
                (center_x + 4, center_y - 2),
            )
        else:
            points = (
                (center_x - 2, center_y - 4),
                (center_x + 2, center_y),
                (center_x - 2, center_y + 4),
            )
        painter.setPen(
            QPen(QColor(str(self.tree.property("propertySectionTextColor") or "#e0e0e0")), 1.5)
        )
        painter.drawLine(*points[0], *points[1])
        painter.drawLine(*points[1], *points[2])
        painter.restore()

    def _paint_value(
        self, painter: QPainter, option: QStyleOptionViewItem, index: QModelIndex
    ) -> None:
        rect = option.rect.adjusted(1, 2, -2, -2)
        painter.save()
        painter.fillRect(rect, QColor(str(self.tree.property("propertyBaseColor") or "#1e1e20")))
        painter.setPen(QPen(QColor(str(self.tree.property("propertyBorderColor") or "#46464c"))))
        painter.drawRect(rect)
        text_rect = rect.adjusted(6, 0, -4, 0)
        text = str(index.data() or "")
        painter.setFont(option.font)
        metrics = painter.fontMetrics()
        painter.setPen(QColor(str(self.tree.property("propertyTextColor") or "#e0e0e0")))
        painter.drawText(
            text_rect,
            Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter,
            metrics.elidedText(text, Qt.TextElideMode.ElideRight, text_rect.width()),
        )
        painter.restore()


class PropertyGrid(QWidget):
    property_edited = Signal(str, object)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.caption = QLabel()
        self.caption.setContentsMargins(6, 3, 6, 3)
        self.caption.setAutoFillBackground(True)
        self.caption.setBackgroundRole(QPalette.ColorRole.Base)
        self.tree = QTreeWidget()
        self.tree.setColumnCount(2)
        self.tree.setHeaderHidden(True)
        self.tree.setRootIsDecorated(False)
        self.tree.setIndentation(10)
        self.tree.setAlternatingRowColors(False)
        self.tree.setSelectionMode(QTreeWidget.SelectionMode.NoSelection)
        self.tree.setItemDelegate(PropertyGridDelegate(self.tree))
        apply_property_grid_style(self.tree)
        header = self.tree.header()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.Interactive)
        header.setStretchLastSection(True)
        self.tree.itemClicked.connect(self._on_clicked)
        self.tree.itemChanged.connect(self._on_item_changed)
        self.tree.itemExpanded.connect(self._remember)
        self.tree.itemCollapsed.connect(self._remember)
        column = QVBoxLayout(self)
        column.setContentsMargins(0, 0, 0, 0)
        column.setSpacing(0)
        column.addWidget(self.caption)
        column.addWidget(self.tree, 1)
        #: The sections the user folded, by key.
        self.folded: set[str] = set()
        self._filling = False

    def show_sections(self, caption: str, sections: Sequence[Section]) -> None:
        self.caption.setText(caption)
        self._filling = True
        try:
            self.tree.clear()
            bold = QFont(self.font())
            bold.setBold(True)
            for section in sections:
                title = QTreeWidgetItem([section.title])
                title.setData(0, KEY, section.key)
                title.setData(0, TITLE, section.title)
                title.setFont(0, bold)
                self._paint_title(title)
                self.tree.addTopLevelItem(title)
                title.setFirstColumnSpanned(True)
                for prop in section.rows:
                    if isinstance(prop, Property):
                        row = QTreeWidgetItem([prop.name, prop.value])
                        title.addChild(row)
                        row.setData(1, EDIT_KEY, prop.key)
                        if prop.choices:
                            editor = QComboBox(self.tree)
                            editor.setEditable(prop.searchable)
                            if prop.searchable:
                                editor.setInsertPolicy(QComboBox.InsertPolicy.NoInsert)
                            if prop.accent:
                                editor.setObjectName("propertyAccentEditor")
                            for choice in prop.choices:
                                if choice.color is None:
                                    editor.addItem(choice.label, choice.value)
                                else:
                                    editor.addItem(
                                        _swatch(choice.color), choice.label, choice.value
                                    )
                            selected = next(
                                (
                                    index
                                    for index, choice in enumerate(prop.choices)
                                    if choice.value == prop.choice_value
                                ),
                                -1,
                            )
                            editor.setCurrentIndex(selected)
                            if prop.searchable:
                                completer = editor.completer()
                                if completer is not None:
                                    completer.setCaseSensitivity(Qt.CaseSensitivity.CaseInsensitive)
                                    completer.setFilterMode(Qt.MatchFlag.MatchContains)
                                    completer.setCompletionMode(
                                        QCompleter.CompletionMode.PopupCompletion
                                    )
                                editor.setToolTip(tr("Search colors by name, index, or RGB value."))
                            editor.currentIndexChanged.connect(
                                lambda index, combo=editor, key=prop.key: self._choice_changed(
                                    key, combo, index
                                )
                            )
                            self.tree.setItemWidget(row, 1, editor)
                        elif prop.editable:
                            row.setFlags(row.flags() | Qt.ItemFlag.ItemIsEditable)
                        row.setToolTip(1, prop.value)
                    else:
                        name, value = prop
                        row = QTreeWidgetItem([name, value])
                        title.addChild(row)
                        row.setToolTip(1, value)
                title.setExpanded(section.key not in self.folded)
            self.tree.resizeColumnToContents(0)
        finally:
            self._filling = False

    def restyle(self) -> None:
        """Refreshes the palette-specific property appearance for the theme."""
        apply_property_grid_style(self.tree)
        for index in range(self.tree.topLevelItemCount()):
            title = self.tree.topLevelItem(index)
            if title is not None:
                self._paint_title(title)

    def _paint_title(self, title: QTreeWidgetItem) -> None:
        # The application's palette: the theme, even before it reaches here.
        button = QApplication.palette(self).button()
        title.setBackground(0, button)
        title.setBackground(1, button)

    def sections(self) -> list[tuple[str, list[tuple[str, str]]]]:
        """What the grid shows: each section's title and rows."""
        shown = []
        for index in range(self.tree.topLevelItemCount()):
            title = self.tree.topLevelItem(index)
            if title is None:
                continue
            rows = []
            for child in range(title.childCount()):
                row = title.child(child)
                if row is not None:
                    editor = self.tree.itemWidget(row, 1)
                    value = editor.currentText() if isinstance(editor, QComboBox) else row.text(1)
                    rows.append((row.text(0), value))
            shown.append((str(title.data(0, TITLE)), rows))
        return shown

    def _on_clicked(self, item: QTreeWidgetItem) -> None:
        """One click on a section title folds or unfolds it (rows have
        nothing to fold)."""
        item.setExpanded(not item.isExpanded())

    def _remember(self, item: QTreeWidgetItem) -> None:
        key = item.data(0, KEY)
        if not isinstance(key, str) or self._filling:
            return
        if item.isExpanded():
            self.folded.discard(key)
        else:
            self.folded.add(key)
        self.tree.viewport().update()

    def _choice_changed(self, key: str, editor: QComboBox, index: int) -> None:
        if self._filling or index < 0:
            return
        self.property_edited.emit(key, editor.itemData(index))

    def _on_item_changed(self, item: QTreeWidgetItem, column: int) -> None:
        if self._filling or column != 1:
            return
        key = item.data(1, EDIT_KEY)
        if isinstance(key, str):
            self.property_edited.emit(key, item.text(1))


_VARIES = "__varies__"


def _swatch(rgb: tuple[int, int, int]) -> QIcon:
    pixmap = QPixmap(14, 14)
    pixmap.fill(QColor(*rgb))
    painter = QPainter(pixmap)
    painter.setPen(QPalette().color(QPalette.ColorRole.Mid))
    painter.drawRect(0, 0, 13, 13)
    painter.end()
    return QIcon(pixmap)
