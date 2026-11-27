"""Choose a loaded linetype for a layer, with an AutoCAD-style load action."""

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QDialog,
    QHBoxLayout,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui.linetype_manager_dialog import LinetypeManagerDialog
from arqstudiocad.ui.linetype_preview import linetype_preview


class SelectLinetypeDialog(QDialog):
    """Lists loaded patterns and opens the linetype loader on demand."""

    def __init__(self, document: CadDocument, current: str, parent: QWidget | None = None):
        super().__init__(parent)
        self._document = document
        self._selected_linetype: str | None = None
        self.setWindowTitle(tr("Select Linetype"))
        self.setMinimumSize(380, 300)
        self._loaded = QListWidget(self)
        self._load = QPushButton(tr("Load..."), self)
        self._ok = QPushButton(tr("OK"), self)
        cancel = QPushButton(tr("Cancel"), self)
        buttons = QHBoxLayout()
        buttons.addWidget(self._load)
        buttons.addStretch(1)
        buttons.addWidget(self._ok)
        buttons.addWidget(cancel)
        layout = QVBoxLayout(self)
        layout.addWidget(self._loaded)
        layout.addLayout(buttons)
        self._load.clicked.connect(self._load_linetypes)
        self._ok.clicked.connect(self._accept_selection)
        self._loaded.itemDoubleClicked.connect(lambda _item: self._accept_selection())
        cancel.clicked.connect(self.reject)
        self._refresh(current)

    @property
    def selected_linetype(self) -> str | None:
        return self._selected_linetype

    def _refresh(self, current: str | None = None) -> None:
        selected_name = current or self._current_name() or "Continuous"
        self._loaded.clear()
        definitions = {
            name.casefold(): pattern
            for name, _description, pattern in self._document.get_linetype_definitions()
        }
        for name in self._document.get_linetypes():
            item = QListWidgetItem(
                linetype_preview(tuple(definitions.get(name.casefold(), []))), name
            )
            item.setData(Qt.ItemDataRole.UserRole, name)
            self._loaded.addItem(item)
            if name.casefold() == selected_name.casefold():
                self._loaded.setCurrentItem(item)
        self._ok.setEnabled(self._loaded.currentItem() is not None)

    def _current_name(self) -> str | None:
        item = self._loaded.currentItem()
        return None if item is None else str(item.data(Qt.ItemDataRole.UserRole))

    def _load_linetypes(self) -> None:
        dialog = LinetypeManagerDialog(self._document, self, load_only=True)
        if dialog.exec() == QDialog.DialogCode.Accepted:
            self._refresh()

    def _accept_selection(self) -> None:
        self._selected_linetype = self._current_name()
        if self._selected_linetype is not None:
            self.accept()


def choose_linetype(
    document: CadDocument, current: str, parent: QWidget | None = None
) -> str | None:
    dialog = SelectLinetypeDialog(document, current, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.selected_linetype
