"""The Rename dialog (RENAME, ADR 0025 step 8c): a block of the drawing and
the name it takes; OK waits for a name it can take and says why not. Only
blocks are renamed for now, and not dynamic ones until the parametric
engine (milestone 04.1)."""

from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui.block_definition_dialog import FORBIDDEN


class RenameBlockDialog(QDialog):
    """Chooses a block of ``document``, ``chosen`` first, and its new name."""

    def __init__(
        self, document: CadDocument, chosen: str | None = None, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("rename_block_dialog")
        self.setWindowTitle(tr("Rename"))
        self.document = document
        self.old = QComboBox()
        self.old.addItems(document.block_names)
        if chosen is not None:
            found = next(
                (
                    i
                    for i in range(self.old.count())
                    if self.old.itemText(i).lower() == chosen.lower()
                ),
                -1,
            )
            self.old.setCurrentIndex(found if found >= 0 else 0)
        self.new_name = QLineEdit(self.old.currentText())
        self.new_name.selectAll()
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
        self.old.currentTextChanged.connect(self._chose)
        self.new_name.textChanged.connect(lambda _text: self._show())

        form = QFormLayout()
        form.addRow(tr("Block"), self.old)
        form.addRow(tr("Rename to"), self.new_name)
        layout = QVBoxLayout(self)
        layout.addLayout(form)
        layout.addWidget(self.problem_label)
        layout.addWidget(self.buttons)
        self.resize(380, 0)
        self._show()

    def chosen(self) -> tuple[str, str]:
        """The block chosen, as the drawing spells it, and its new name."""
        return self.old.currentText(), self.new_name.text().strip()

    def problem(self) -> str | None:
        """Why OK cannot rename yet; None when it can."""
        old, new = self.chosen()
        if not old:
            return tr("The drawing has no blocks to rename.")
        if self.document.is_dynamic_block(old):
            return tr("{name} is a dynamic block: it cannot be renamed yet.", name=old)
        if not new or new == old:
            return tr("Type the block's new name.")
        if FORBIDDEN & set(new):  # "*" among them: no anonymous name
            return tr("{name} is not a valid block name.", name=new)
        taken = next(
            (n for n in self.document.block_names if n.lower() == new.lower()),
            None,
        )
        if taken is not None and taken != old:
            return tr("A block named {name} exists in the drawing.", name=taken)
        return None

    def _chose(self, name: str) -> None:
        """Another block chosen: its name, to be typed over."""
        self.new_name.setText(name)
        self.new_name.selectAll()

    def _show(self) -> None:
        problem = self.problem()
        self.problem_label.setText(problem or "")
        self.problem_label.setVisible(bool(problem))
        self.ok_button.setEnabled(problem is None)

    def _accept_if_ready(self) -> None:
        if self.problem() is None:
            self.accept()


def rename_block(
    document: CadDocument, chosen: str | None, parent: QWidget | None = None
) -> tuple[str, str] | None:
    """Shows the Rename dialog; the block chosen and its new name, or None if
    it was cancelled."""
    dialog = RenameBlockDialog(document, chosen, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.chosen()
