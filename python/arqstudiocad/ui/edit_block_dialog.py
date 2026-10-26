"""The Edit Block Definition dialog (ADR 0023 D3), as AutoCAD's and
progeCAD's: the block to edit, by name or from the drawing's list, with its
preview and description. BEDIT shows it, and so does a double click on a
block reference, with that block chosen (maintainer, 2026-10-25). A dynamic
block is shown but cannot be chosen until the parametric engine (D3a)."""

from PySide6.QtCore import Qt
from PySide6.QtGui import QPalette
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QGridLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui.blocks_panel import preview_pixmap

#: The preview's side, in pixels.
EDIT_PREVIEW = 128


class EditBlockDialog(QDialog):
    """Chooses a block of ``document`` to open in the block editor."""

    def __init__(
        self, document: CadDocument, chosen: str | None = None, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("edit_block_dialog")
        self.setWindowTitle(tr("Edit Block Definition"))
        self.document = document
        self.name = QLineEdit()
        self.name.textEdited.connect(self._typed)
        self.blocks = QListWidget()
        self.blocks.addItems(document.block_names)
        self.blocks.currentItemChanged.connect(self._listed)
        # A double click on a block of the list edits it, as OK.
        self.blocks.itemDoubleClicked.connect(lambda _item: self._accept_if_ready())
        colors = self.palette()
        self.preview = QLabel()
        self.preview.setFixedSize(EDIT_PREVIEW + 2, EDIT_PREVIEW + 2)
        self.preview.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.preview.setStyleSheet(
            f"background-color: {colors.color(QPalette.ColorRole.Base).name()}; "
            f"border: 1px solid {colors.color(QPalette.ColorRole.Mid).name()};"
        )
        self.description = QLabel()
        self.description.setWordWrap(True)
        self.description.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        self.problem_label = QLabel()
        self.problem_label.setWordWrap(True)
        self.buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        ok = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        assert isinstance(ok, QPushButton)
        self.ok_button = ok
        ok.setText(tr("OK"))
        cancel = self.buttons.button(QDialogButtonBox.StandardButton.Cancel)
        assert cancel is not None
        cancel.setText(tr("Cancel"))
        self.buttons.accepted.connect(self._accept_if_ready)
        self.buttons.rejected.connect(self.reject)

        grid = QGridLayout()
        grid.addWidget(QLabel(tr("Block to edit")), 0, 0)
        grid.addWidget(self.name, 1, 0)
        grid.addWidget(self.blocks, 2, 0, 3, 1)
        grid.addWidget(QLabel(tr("Preview")), 1, 1)
        grid.addWidget(self.preview, 2, 1)
        grid.addWidget(QLabel(tr("Description")), 3, 1)
        grid.addWidget(self.description, 4, 1)
        grid.setColumnStretch(0, 1)
        layout = QVBoxLayout(self)
        layout.addLayout(grid, 1)
        layout.addWidget(self.problem_label)
        layout.addWidget(self.buttons)
        self.resize(460, 320)
        if chosen is not None:
            self.name.setText(chosen)
            self._typed(chosen)
        elif self.blocks.count():
            self.blocks.setCurrentRow(0)
        self._show()

    def chosen(self) -> str | None:
        """The block chosen, as the drawing spells it; None if the name typed
        names none."""
        item = self.blocks.currentItem()
        return None if item is None else item.text()

    def problem(self) -> str | None:
        """Why the block chosen cannot be edited; None when it can."""
        name = self.chosen()
        if name is None:
            typed = self.name.text().strip()
            return tr("There is no block named {name}.", name=typed) if typed else ""
        if self.document.is_dynamic_block(name):
            return tr("{name} is a dynamic block: it cannot be edited yet.", name=name)
        return None

    def _typed(self, text: str) -> None:
        """A name typed selects the block of that name, ignoring case."""
        # A fixed-string match ignores case.
        found = self.blocks.findItems(text.strip(), Qt.MatchFlag.MatchFixedString)
        self.blocks.blockSignals(True)
        if found:
            self.blocks.setCurrentItem(found[0])
            self.blocks.scrollToItem(found[0])
        else:
            self.blocks.setCurrentRow(-1)
        self.blocks.blockSignals(False)
        self._show()

    def _listed(self, item: QListWidgetItem | None) -> None:
        if item is not None:
            self.name.setText(item.text())
        self._show()

    def _show(self) -> None:
        """The chosen block's preview and description, and whether OK can
        edit it."""
        name = self.chosen()
        self.preview.clear()
        self.description.clear()
        if name is not None:
            pixmap = preview_pixmap(
                self.document, name, EDIT_PREVIEW, self.palette(), self.devicePixelRatioF()
            )
            if pixmap is not None:
                self.preview.setPixmap(pixmap)
            info = self.document.block_info(name)
            self.description.setText(info.description if info is not None else "")
        problem = self.problem()
        self.problem_label.setText(problem or "")
        self.problem_label.setVisible(bool(problem))
        self.ok_button.setEnabled(problem is None)

    def _accept_if_ready(self) -> None:
        if self.problem() is None:
            self.accept()


def choose_block_to_edit(
    document: CadDocument, chosen: str | None, parent: QWidget | None = None
) -> str | None:
    """Shows the Edit Block Definition dialog; the block chosen, or None if
    it was cancelled."""
    dialog = EditBlockDialog(document, chosen, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.chosen()
