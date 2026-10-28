"""The Purge dialog (PURGE, ADR 0025 step 8e, maintainer 2026-10-26): the
drawing's unused blocks, each with a box and its preview, and "Purge nested
items", checked by default as AutoCAD's. A dynamic block is listed without
a box, saying why, until the parametric engine (milestone 04.1). Only
blocks are purged for now."""

from PySide6.QtCore import QSize, Qt
from PySide6.QtGui import QIcon
from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDialogButtonBox,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui.blocks_panel import preview_pixmap

#: The previews' side, in pixels.
PURGE_PREVIEW = 32


class PurgeDialog(QDialog):
    """Chooses which of ``document``'s unused blocks to purge."""

    def __init__(self, document: CadDocument, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("purge_dialog")
        self.setWindowTitle(tr("Purge"))
        self.document = document
        self.blocks = QListWidget()
        self.blocks.setIconSize(QSize(PURGE_PREVIEW, PURGE_PREVIEW))
        for name in document.unused_blocks:
            item = QListWidgetItem(name)
            pixmap = preview_pixmap(
                document, name, PURGE_PREVIEW, self.palette(), self.devicePixelRatioF()
            )
            if pixmap is not None:
                item.setIcon(QIcon(pixmap))
            if document.is_dynamic_block(name):
                # Its parameters wait for the parametric engine (04.1).
                item.setFlags(item.flags() & ~Qt.ItemFlag.ItemIsEnabled)
                item.setText(f"{name} — {tr('Dynamic block: it cannot be purged yet.')}")
                item.setData(Qt.ItemDataRole.UserRole, None)
            else:
                item.setFlags(item.flags() | Qt.ItemFlag.ItemIsUserCheckable)
                item.setCheckState(Qt.CheckState.Unchecked)
                item.setData(Qt.ItemDataRole.UserRole, name)
            self.blocks.addItem(item)
        self.blocks.itemChanged.connect(lambda _item: self._show())
        self.nested = QCheckBox(tr("Purge nested items"))
        self.nested.setChecked(True)
        self.count = QLabel()
        self.buttons = QDialogButtonBox()
        self.purge_button = QPushButton(tr("Purge"))
        self.purge_all_button = QPushButton(tr("Purge All"))
        close = QPushButton(tr("Close"))
        for button, role in (
            (self.purge_button, QDialogButtonBox.ButtonRole.AcceptRole),
            (self.purge_all_button, QDialogButtonBox.ButtonRole.ActionRole),
            (close, QDialogButtonBox.ButtonRole.RejectRole),
        ):
            self.buttons.addButton(button, role)
        self.purge_button.clicked.connect(self.accept)
        self.purge_all_button.clicked.connect(self._purge_all)
        close.clicked.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addWidget(QLabel(tr("Unused blocks")))
        layout.addWidget(self.blocks, 1)
        layout.addWidget(self.count)
        layout.addWidget(self.nested)
        layout.addWidget(self.buttons)
        self.resize(420, 380)
        self._show()

    def purgeable(self) -> list[QListWidgetItem]:
        """The items that can be checked: the unused blocks not dynamic."""
        items = (self.blocks.item(i) for i in range(self.blocks.count()))
        return [i for i in items if i.data(Qt.ItemDataRole.UserRole) is not None]

    def chosen(self) -> list[str]:
        """The blocks checked, in the list's order."""
        return [
            i.data(Qt.ItemDataRole.UserRole)
            for i in self.purgeable()
            if i.checkState() == Qt.CheckState.Checked
        ]

    def _show(self) -> None:
        chosen = len(self.chosen())
        self.count.setText(
            tr("{count} of {total} chosen", count=chosen, total=len(self.purgeable()))
        )
        self.purge_button.setEnabled(chosen > 0)
        self.purge_all_button.setEnabled(bool(self.purgeable()))

    def _purge_all(self) -> None:
        for item in self.purgeable():
            item.setCheckState(Qt.CheckState.Checked)
        self.accept()


def purge_blocks(
    document: CadDocument, parent: QWidget | None = None
) -> tuple[list[str], bool] | None:
    """Shows the Purge dialog; the blocks chosen and whether to purge nested
    items, or None if it was closed."""
    dialog = PurgeDialog(document, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted or not dialog.chosen():
        return None
    return dialog.chosen(), dialog.nested.isChecked()
