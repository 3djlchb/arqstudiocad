"""The window that takes several lines of text (ADR 0022 D6): the MTEXT
command's text and an MTEXT's contents in Properties. Formatting (bold,
fonts, colors) is phase 04's MTEXT editor."""

from PySide6.QtGui import QFontDatabase
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QPlainTextEdit,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.i18n import tr


class TextDialog(QDialog):
    """Plain text of several lines, with OK and Cancel."""

    def __init__(self, title: str, text: str = "", parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("text_dialog")
        self.setWindowTitle(title)
        self.editor = QPlainTextEdit()
        self.editor.setPlainText(text)
        self.editor.setFont(QFontDatabase.systemFont(QFontDatabase.SystemFont.GeneralFont))
        self.editor.setPlaceholderText(tr("Type the text; Enter starts a new paragraph."))
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout = QVBoxLayout(self)
        layout.addWidget(self.editor, 1)
        layout.addWidget(buttons)
        self.resize(420, 220)
        self.editor.setFocus()

    def text(self) -> str:
        return self.editor.toPlainText()


def ask_text(parent: QWidget | None, title: str, text: str = "") -> str | None:
    """The text typed, or None if the window was cancelled."""
    dialog = TextDialog(title, text, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.text()
