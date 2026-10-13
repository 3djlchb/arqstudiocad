"""A property grid after OpenCADStudio's (ADR 0015): a caption naming what
is shown, then foldable sections of name and value rows.

Read-only for now; editors per value (color, layer, linetype…) come with
selection in milestone 2.5.
"""

from collections.abc import Sequence
from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtGui import QFont, QPalette
from PySide6.QtWidgets import (
    QApplication,
    QHeaderView,
    QLabel,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

#: Where a section title keeps its key and its title without the arrow.
KEY = Qt.ItemDataRole.UserRole
TITLE = Qt.ItemDataRole.UserRole + 1


@dataclass(frozen=True)
class Section:
    """Rows under a foldable title; ``key`` keeps it folded across refreshes
    and languages."""

    key: str
    title: str
    rows: Sequence[tuple[str, str]]


class PropertyGrid(QWidget):
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
        header = self.tree.header()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.Interactive)
        header.setStretchLastSection(True)
        self.tree.itemClicked.connect(self._on_clicked)
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
                title = QTreeWidgetItem([self._arrow(section.key) + section.title])
                title.setData(0, KEY, section.key)
                title.setData(0, TITLE, section.title)
                title.setFont(0, bold)
                self._paint_title(title)
                self.tree.addTopLevelItem(title)
                title.setFirstColumnSpanned(True)
                for name, value in section.rows:
                    row = QTreeWidgetItem([name, value])
                    row.setToolTip(1, value)
                    title.addChild(row)
                title.setExpanded(section.key not in self.folded)
            self.tree.resizeColumnToContents(0)
        finally:
            self._filling = False

    def restyle(self) -> None:
        """Paints the section titles in the theme's colors."""
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
                    rows.append((row.text(0), row.text(1)))
            shown.append((str(title.data(0, TITLE)), rows))
        return shown

    def _arrow(self, key: str) -> str:
        return "▸ " if key in self.folded else "▾ "

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
        item.setText(0, self._arrow(key) + str(item.data(0, TITLE)))
