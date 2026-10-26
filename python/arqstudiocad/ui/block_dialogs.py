"""Two small dialogs of the Blocks palette's context menu (ADR 0025, step
8b, maintainer 2026-10-25): Insert with options, which inserts once with
other options than the palette's, and the confirmation before redefining a
drawing's block from the template, showing both blocks."""

from PySide6.QtGui import QPalette
from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFormLayout,
    QGridLayout,
    QLabel,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui.blocks_panel import InsertOptions, preview_pixmap

#: Each preview's side in the redefine confirmation, as the clash dialog's.
REDEFINE_PREVIEW = 96


class InsertOptionsDialog(QDialog):
    """Insert with options: the palette's insert options, changed for this
    insertion only."""

    def __init__(self, name: str, options: InsertOptions, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("insert_options_dialog")
        self.setWindowTitle(tr("Insert {name}", name=name))
        self.scale = QDoubleSpinBox()
        self.scale.setDecimals(3)
        self.scale.setRange(0.001, 1_000_000.0)
        self.scale.setValue(options.scale)
        self.rotation = QDoubleSpinBox()
        self.rotation.setDecimals(2)
        self.rotation.setRange(-360.0, 360.0)
        self.rotation.setSuffix("°")
        self.rotation.setValue(options.rotation)
        self.on_screen = QCheckBox(tr("Specify on screen"))
        self.on_screen.setChecked(options.on_screen)
        self.explode = QCheckBox(tr("Explode"))
        self.explode.setChecked(options.explode)
        form = QFormLayout()
        form.addRow(tr("Scale"), self.scale)
        form.addRow(tr("Rotation"), self.rotation)
        form.addRow(self.on_screen)
        form.addRow(self.explode)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        ok = buttons.button(QDialogButtonBox.StandardButton.Ok)
        assert isinstance(ok, QPushButton)
        self.insert_button = ok
        ok.setText(tr("Insert"))
        cancel = buttons.button(QDialogButtonBox.StandardButton.Cancel)
        assert cancel is not None
        cancel.setText(tr("Cancel"))
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout = QVBoxLayout(self)
        layout.addLayout(form)
        layout.addWidget(buttons)

    def options(self) -> InsertOptions:
        return InsertOptions(
            self.scale.value(),
            self.rotation.value(),
            self.on_screen.isChecked(),
            self.explode.isChecked(),
        )


def ask_insert_options(
    name: str, options: InsertOptions, parent: QWidget | None = None
) -> InsertOptions | None:
    """The options to insert block ``name`` with once; None if cancelled."""
    dialog = InsertOptionsDialog(name, options, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.options()


class RedefineDialog(QDialog):
    """Asks before a drawing's block is redefined from the template: both
    blocks side by side, the template's on the left, as the clash dialog,
    and how many references will change."""

    def __init__(
        self,
        template: CadDocument,
        drawing: CadDocument,
        name: str,
        drawing_name: str,
        references: int,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("redefine_dialog")
        self.setWindowTitle(tr("Redefine block"))
        colors = self.palette()
        frame = (
            f"background-color: {colors.color(QPalette.ColorRole.Base).name()}; "
            f"border: 1px solid {colors.color(QPalette.ColorRole.Mid).name()};"
        )
        ratio = self.devicePixelRatioF()
        self.template_preview = QLabel()
        self.drawing_preview = QLabel()
        for label, document, block in (
            (self.template_preview, template, name),
            (self.drawing_preview, drawing, drawing_name),
        ):
            label.setFixedSize(REDEFINE_PREVIEW + 2, REDEFINE_PREVIEW + 2)
            label.setStyleSheet(frame)
            pixmap = preview_pixmap(document, block, REDEFINE_PREVIEW, colors, ratio)
            if pixmap is not None:
                label.setPixmap(pixmap)
        previews = QGridLayout()
        previews.addWidget(self.template_preview, 0, 0)
        previews.addWidget(self.drawing_preview, 0, 1)
        previews.addWidget(QLabel(tr("In the template: {name}", name=name)), 1, 0)
        previews.addWidget(QLabel(tr("In the drawing: {name}", name=drawing_name)), 1, 1)
        if references == 1:
            change = tr("1 reference in the drawing will change.")
        else:
            change = tr("{count} references in the drawing will change.", count=references)
        self.text = QLabel(
            tr(
                "Redefine {name} with the template's block? {change} One undo takes it back.",
                name=drawing_name,
                change=change,
            )
        )
        self.text.setWordWrap(True)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        ok = buttons.button(QDialogButtonBox.StandardButton.Ok)
        assert isinstance(ok, QPushButton)
        self.redefine_button = ok
        ok.setText(tr("Redefine"))
        cancel = buttons.button(QDialogButtonBox.StandardButton.Cancel)
        assert cancel is not None
        cancel.setText(tr("Cancel"))
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout = QVBoxLayout(self)
        layout.addWidget(self.text)
        layout.addLayout(previews)
        layout.addWidget(buttons)


def confirm_redefine(
    template: CadDocument,
    drawing: CadDocument,
    name: str,
    drawing_name: str,
    references: int,
    parent: QWidget | None = None,
) -> bool:
    """Whether to redefine the drawing's block ``drawing_name`` from the
    template's ``name``."""
    dialog = RedefineDialog(template, drawing, name, drawing_name, references, parent)
    return dialog.exec() == QDialog.DialogCode.Accepted
