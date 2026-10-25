"""The clash dialog (ADR 0025 D6): a template's blocks whose names the
drawing uses for other contents, each with the template's block on the left
and the drawing's on the right, 96 pixels each (maintainer, 2026-10-25), and
what to do with it: use the drawing's, redefine it from the template, or
copy it under another name. A dynamic block can only be used as the drawing
has it until the parametric engine."""

from collections.abc import Sequence

from PySide6.QtGui import QPalette
from PySide6.QtWidgets import (
    QButtonGroup,
    QDialog,
    QDialogButtonBox,
    QGridLayout,
    QGroupBox,
    QLabel,
    QLineEdit,
    QPushButton,
    QRadioButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.commands import BlockClash, ClashChoice
from arqstudiocad.i18n import tr
from arqstudiocad.ui.blocks_panel import preview_pixmap

#: Each preview's side, in pixels (maintainer, 2026-10-25).
CLASH_PREVIEW = 96
#: Characters a block name cannot hold, as the core refuses them.
FORBIDDEN = set('<>/\\":;?*|,=`')


class ClashRow(QGroupBox):
    """One clashing block: both previews and the three choices."""

    def __init__(self, clash: BlockClash, parent: QWidget | None = None) -> None:
        super().__init__(clash.drawing_name, parent)
        self.clash = clash
        ratio = self.devicePixelRatioF()
        colors = self.palette()
        # Framed as the palette's cards, so both read as samples side by side.
        frame = (
            f"background-color: {colors.color(QPalette.ColorRole.Base).name()}; "
            f"border: 1px solid {colors.color(QPalette.ColorRole.Mid).name()};"
        )
        self.template_preview = QLabel()
        self.drawing_preview = QLabel()
        for label, document, name in (
            (self.template_preview, clash.template, clash.name),
            (self.drawing_preview, clash.drawing, clash.drawing_name),
        ):
            label.setFixedSize(CLASH_PREVIEW + 2, CLASH_PREVIEW + 2)
            label.setStyleSheet(frame)
            pixmap = preview_pixmap(document, name, CLASH_PREVIEW, colors, ratio)
            if pixmap is not None:
                label.setPixmap(pixmap)
        self.template_caption = QLabel(tr("In the template: {name}", name=clash.name))
        self.drawing_caption = QLabel(tr("In the drawing: {name}", name=clash.drawing_name))
        self.keep = QRadioButton(tr("Use the drawing's"))
        self.redefine = QRadioButton(tr("Redefine from the template"))
        self.rename = QRadioButton(tr("Copy with another name"))
        self.new_name = QLineEdit(clash.suggested)
        group = QButtonGroup(self)
        for button in (self.keep, self.redefine, self.rename):
            group.addButton(button)
        self.keep.setChecked(True)
        self.rename.toggled.connect(self.new_name.setEnabled)
        self.new_name.setEnabled(False)
        grid = QGridLayout(self)
        grid.addWidget(self.template_preview, 0, 0)
        grid.addWidget(self.drawing_preview, 0, 1)
        grid.addWidget(self.template_caption, 1, 0)
        grid.addWidget(self.drawing_caption, 1, 1)
        grid.addWidget(self.keep, 2, 0, 1, 2)
        grid.addWidget(self.redefine, 3, 0, 1, 2)
        grid.addWidget(self.rename, 4, 0)
        grid.addWidget(self.new_name, 4, 1)
        self.note = QLabel()
        self.note.setWordWrap(True)
        if clash.dynamic:
            # Until the parametric engine (maintainer, 2026-10-25).
            reason = tr(
                "{name} is a dynamic block: until the parametric engine, only "
                "the drawing's can be used.",
                name=clash.drawing_name,
            )
            for button in (self.redefine, self.rename):
                button.setEnabled(False)
                button.setToolTip(reason)
            self.note.setText(reason)
            grid.addWidget(self.note, 5, 0, 1, 2)
        else:
            self.note.hide()

    def choice(self) -> ClashChoice:
        if self.redefine.isChecked():
            return ClashChoice("redefine")
        if self.rename.isChecked():
            return ClashChoice("rename", self.new_name.text().strip())
        return ClashChoice("keep")


class BlockClashDialog(QDialog):
    """Asks what to do with each clashing block before INSERT copies it."""

    def __init__(self, clashes: Sequence[BlockClash], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("block_clash_dialog")
        self.setWindowTitle(tr("Blocks with the same name"))
        intro = QLabel(
            tr(
                "The drawing already has blocks with these names and other "
                "contents. Choose what to do with each; nothing changes until "
                "you insert."
            )
        )
        intro.setWordWrap(True)
        self.rows = [ClashRow(clash) for clash in clashes]
        body = QWidget()
        column = QVBoxLayout(body)
        for row in self.rows:
            column.addWidget(row)
            for widget in (row.keep, row.redefine, row.rename):
                widget.toggled.connect(self._check)
            row.new_name.textChanged.connect(self._check)
        column.addStretch(1)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setWidget(body)
        self.problem_label = QLabel()
        self.problem_label.setWordWrap(True)
        self.buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        self.insert_button = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        assert isinstance(self.insert_button, QPushButton)
        self.insert_button.setText(tr("Insert"))
        cancel = self.buttons.button(QDialogButtonBox.StandardButton.Cancel)
        assert cancel is not None
        cancel.setText(tr("Cancel"))
        self.buttons.accepted.connect(self.accept)
        self.buttons.rejected.connect(self.reject)
        layout = QVBoxLayout(self)
        layout.addWidget(intro)
        layout.addWidget(scroll, 1)
        layout.addWidget(self.problem_label)
        layout.addWidget(self.buttons)
        self.resize(460, min(640, 200 + 230 * len(self.rows)))
        self._check()

    def choices(self) -> dict[str, ClashChoice]:
        """The choice made for each clashing block, by its template name."""
        return {row.clash.name: row.choice() for row in self.rows}

    def problem(self) -> str | None:
        """Why the choices cannot be applied: a new name that is invalid,
        taken by the drawing, or chosen twice; None when they can."""
        chosen: set[str] = set()
        for row in self.rows:
            choice = row.choice()
            if choice.action != "rename":
                continue
            name = choice.name
            taken = {n.casefold() for n in row.clash.drawing.block_names}
            if not name or name.startswith("*") or FORBIDDEN & set(name):
                return tr("{name} is not a valid block name.", name=name)
            if name.casefold() in taken or name.casefold() in chosen:
                return tr("A block named {name} exists in the drawing.", name=name)
            chosen.add(name.casefold())
        return None

    def _check(self) -> None:
        problem = self.problem()
        self.problem_label.setText(problem or "")
        self.problem_label.setVisible(problem is not None)
        self.insert_button.setEnabled(problem is None)


def resolve_block_clashes(
    clashes: Sequence[BlockClash], parent: QWidget | None = None
) -> dict[str, ClashChoice] | None:
    """Shows the clash dialog; the choices, or None if it was cancelled."""
    dialog = BlockClashDialog(clashes, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.choices()
