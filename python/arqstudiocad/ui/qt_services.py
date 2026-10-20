"""The interface services commands may ask for, built with Qt dialogs
(the :class:`~arqstudiocad.commands.UiServices` protocol)."""

from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Literal

from PySide6.QtWidgets import QFileDialog, QMessageBox, QWidget

from arqstudiocad.commands import ViewControl
from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui import text_dialog


def drawing_filter() -> str:
    return tr("Drawings (*.dwg *.dxf);;DWG drawings (*.dwg);;DXF drawings (*.dxf)")


class QtUiServices:
    """File dialogs and questions shown over ``parent``; views come from the
    function given (the viewport, from step 5.4)."""

    def __init__(
        self,
        parent: QWidget,
        views: Callable[[CadDocument], ViewControl | None] | None = None,
        panels: Callable[[str], bool] | None = None,
    ) -> None:
        self._parent = parent
        self._views = views
        self._panels = panels
        self.last_folder = ""

    def choose_open_path(self) -> Path | None:
        path, _ = QFileDialog.getOpenFileName(
            self._parent, tr("Open drawing"), self.last_folder, drawing_filter()
        )
        return self._remember(path)

    def choose_save_path(self, suggested: str) -> Path | None:
        start = str(Path(self.last_folder) / suggested) if self.last_folder else suggested
        # DXF first until the round-trip corpus makes DWG the default
        # (contract section 10, phase 8).
        filters = tr("DXF drawings (*.dxf);;DWG drawings (*.dwg)")
        path, _ = QFileDialog.getSaveFileName(self._parent, tr("Save drawing as"), start, filters)
        return self._remember(path)

    def ask_save_changes(self, name: str) -> Literal["yes", "no", "cancel"]:
        buttons = (
            QMessageBox.StandardButton.Save
            | QMessageBox.StandardButton.Discard
            | QMessageBox.StandardButton.Cancel
        )
        answer = QMessageBox.question(
            self._parent,
            tr("Unsaved changes"),
            tr("Save changes to {name}?", name=name),
            buttons,
            QMessageBox.StandardButton.Save,
        )
        if answer == QMessageBox.StandardButton.Save:
            return "yes"
        if answer == QMessageBox.StandardButton.Discard:
            return "no"
        return "cancel"

    def report_recovered(self, name: str, problems: Sequence[str]) -> None:
        box = QMessageBox(
            QMessageBox.Icon.Warning,
            tr("Damaged drawing"),
            tr(
                "{name} is damaged. It was opened by recovery and part of the "
                "drawing may be missing. Check it, and save it under a new name "
                "to keep the original.",
                name=name,
            ),
            QMessageBox.StandardButton.Ok,
            self._parent,
        )
        box.setDetailedText("\n".join(problems))
        box.exec()

    def ask_overwrite_damaged(self, name: str) -> Literal["overwrite", "save_as", "cancel"]:
        box = QMessageBox(
            QMessageBox.Icon.Warning,
            tr("Replace the damaged file?"),
            tr(
                "This drawing was recovered from {name}, which is damaged. "
                "Replacing it loses whatever recovery could not read.",
                name=name,
            ),
            QMessageBox.StandardButton.NoButton,
            self._parent,
        )
        save_as = box.addButton(tr("Save as…"), QMessageBox.ButtonRole.AcceptRole)
        replace = box.addButton(tr("Replace"), QMessageBox.ButtonRole.DestructiveRole)
        box.addButton(QMessageBox.StandardButton.Cancel)
        box.setDefaultButton(save_as)
        box.exec()
        clicked = box.clickedButton()
        if clicked == save_as:
            return "save_as"
        if clicked == replace:
            return "overwrite"
        return "cancel"

    def ask_accept_losses(self, name: str, losses: Sequence[str]) -> bool:
        box = QMessageBox(
            QMessageBox.Icon.Warning,
            tr("Data would be lost"),
            tr(
                "Saving as {name} would lose data its format or version cannot keep. "
                "The original file is unchanged until you agree. Save anyway?",
                name=name,
            ),
            QMessageBox.StandardButton.NoButton,
            self._parent,
        )
        box.setDetailedText("\n".join(losses))
        save = box.addButton(tr("Save anyway"), QMessageBox.ButtonRole.DestructiveRole)
        cancel = box.addButton(QMessageBox.StandardButton.Cancel)
        box.setDefaultButton(cancel)
        box.exec()
        return box.clickedButton() == save

    def edit_text(self, title: str, text: str = "") -> str | None:
        return text_dialog.ask_text(self._parent, title, text)

    def view(self, document: CadDocument) -> ViewControl | None:
        return None if self._views is None else self._views(document)

    def show_panel(self, name: str) -> bool:
        return self._panels is not None and self._panels(name)

    def _remember(self, path: str) -> Path | None:
        if not path:
            return None
        chosen = Path(path)
        self.last_folder = str(chosen.parent)
        return chosen
