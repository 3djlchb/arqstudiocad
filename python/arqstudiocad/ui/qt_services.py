"""The interface services commands may ask for, built with Qt dialogs
(the :class:`~arqstudiocad.commands.UiServices` protocol)."""

from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Literal

from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QFileDialog, QInputDialog, QMessageBox, QWidget

from arqstudiocad.commands import (
    BlockClash,
    BlockDefinitionState,
    BlockDialogAction,
    BlockObjectsChoice,
    ClashChoice,
    ViewControl,
)
from arqstudiocad.core import CadBlockData, CadDocument
from arqstudiocad.currencies import CurrencyRegister
from arqstudiocad.i18n import tr
from arqstudiocad.ui import (
    block_clash_dialog,
    block_data_dialog,
    block_definition_dialog,
    currencies_dialog,
    edit_block_dialog,
    purge_dialog,
    rename_block_dialog,
    text_dialog,
)

#: The Block Definition dialog's last choice for the objects (ADR 0023 D2a).
BLOCK_OBJECTS_SETTING = "block_definition/objects"


def drawing_filter() -> str:
    return tr(
        "Drawings (*.dwg *.dxf *.dwt);;DWG drawings (*.dwg);;DXF "
        "drawings (*.dxf);;Drawing templates (*.dwt)"
    )


class QtUiServices:
    """File dialogs and questions shown over ``parent``; views come from the
    function given (the viewport, from step 5.4)."""

    def __init__(
        self,
        parent: QWidget,
        views: Callable[[CadDocument], ViewControl | None] | None = None,
        panels: Callable[[str], bool] | None = None,
        settings: QSettings | None = None,
        options: Callable[[], bool] | None = None,
    ) -> None:
        self._parent = parent
        self._views = views
        self._panels = panels
        self._options = options
        self._settings = settings
        self.last_folder = ""

    def choose_open_path(self) -> Path | None:
        path, _ = QFileDialog.getOpenFileName(
            self._parent, tr("Open drawing"), self.last_folder, drawing_filter()
        )
        return self._remember(path)

    def choose_template_path(self) -> Path | None:
        filters = tr("Templates and drawings (*.dwt *.dwg *.dxf)")
        path, _ = QFileDialog.getOpenFileName(
            self._parent, tr("Load template"), self.last_folder, filters
        )
        return self._remember(path)

    def choose_save_path(self, suggested: str) -> Path | None:
        start = str(Path(self.last_folder) / suggested) if self.last_folder else suggested
        # DXF first until the round-trip corpus makes DWG the default
        # (contract section 10, phase 8).
        filters = tr("DXF drawings (*.dxf);;DWG drawings (*.dwg)")
        path, _ = QFileDialog.getSaveFileName(self._parent, tr("Save drawing as"), start, filters)
        return self._remember(path)

    def choose_budget_export_path(self, suggested: str) -> Path | None:
        start = str(Path(self.last_folder) / suggested) if self.last_folder else suggested
        excel, comma = tr("Excel workbooks (*.xlsx)"), tr("CSV files (*.csv)")
        path, chosen = QFileDialog.getSaveFileName(
            self._parent, tr("Export budget"), start, f"{excel};;{comma}"
        )
        if path and Path(path).suffix.lower() not in (".xlsx", ".csv"):
            # A name typed without its type takes the chosen filter's.
            path += ".csv" if chosen == comma else ".xlsx"
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

    def report_block_name_taken(self, name: str) -> None:
        QMessageBox(
            QMessageBox.Icon.Warning,
            tr("Block name in use"),
            tr("A block named {name} exists in the drawing. Type another name.", name=name),
            QMessageBox.StandardButton.Ok,
            self._parent,
        ).exec()

    def ask_block_name_in_template(self, name: str, template: str) -> bool:
        box = QMessageBox(
            QMessageBox.Icon.Information,
            tr("Block name in the template"),
            tr(
                "The template {template} has a block named {name}. You can make this "
                "block: inserting {name} from the template will ask what to do.",
                template=template,
                name=name,
            ),
            QMessageBox.StandardButton.NoButton,
            self._parent,
        )
        go_on = box.addButton(tr("Continue"), QMessageBox.ButtonRole.AcceptRole)
        box.addButton(tr("Another name"), QMessageBox.ButtonRole.RejectRole)
        box.setDefaultButton(go_on)
        box.exec()
        return box.clickedButton() == go_on

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

    def choose_block(self, names: Sequence[str], title: str) -> str | None:
        name, chosen = QInputDialog.getItem(
            self._parent, title, tr("Block:"), list(names), 0, False
        )
        return name if chosen and name else None

    def choose_block_to_edit(self, document: CadDocument, chosen: str | None) -> str | None:
        return edit_block_dialog.choose_block_to_edit(document, chosen, self._parent)

    def define_block(
        self, document: CadDocument, state: BlockDefinitionState
    ) -> tuple[BlockDialogAction, BlockDefinitionState] | None:
        stored = self._settings.value(BLOCK_OBJECTS_SETTING) if self._settings else None
        remembered: BlockObjectsChoice = (
            stored if stored in ("retain", "convert", "delete") else "convert"
        )
        answer = block_definition_dialog.define_block(document, state, remembered, self._parent)
        if answer is not None and answer[0] == "ok" and self._settings is not None:
            # The choice made last is the next one's (ADR 0023 D2a).
            self._settings.setValue(BLOCK_OBJECTS_SETTING, answer[1].objects)
        return answer

    def rename_block(self, document: CadDocument, chosen: str | None) -> tuple[str, str] | None:
        return rename_block_dialog.rename_block(document, chosen, self._parent)

    def purge_blocks(self, document: CadDocument) -> tuple[list[str], bool] | None:
        return purge_dialog.purge_blocks(document, self._parent)

    def edit_currencies(self, document: CadDocument) -> CurrencyRegister | None:
        return currencies_dialog.edit_currencies(document, self._parent)

    def edit_block_data(self, document: CadDocument, name: str) -> CadBlockData | None:
        return block_data_dialog.edit_block_data(document, name, self._parent)

    def ask_clear_block_data(self, name: str) -> bool:
        box = QMessageBox(
            QMessageBox.Icon.Question,
            tr("Clear block data"),
            tr("Clear the budget data of block {name}?", name=name),
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            self._parent,
        )
        box.setDefaultButton(QMessageBox.StandardButton.No)
        return box.exec() == QMessageBox.StandardButton.Yes

    def ask_purge_block(self, name: str) -> bool:
        box = QMessageBox(
            QMessageBox.Icon.Question,
            tr("Purge"),
            tr("Purge block {name} from the drawing?", name=name),
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            self._parent,
        )
        box.setDefaultButton(QMessageBox.StandardButton.No)
        return box.exec() == QMessageBox.StandardButton.Yes

    def ask_redefine_block(self, name: str, references: int) -> bool:
        if references == 1:
            change = tr("1 reference in the drawing will change.")
        else:
            change = tr("{count} references in the drawing will change.", count=references)
        box = QMessageBox(
            QMessageBox.Icon.Question,
            tr("Redefine block"),
            tr(
                "A block named {name} exists in the drawing. Redefine it with the "
                "objects selected? {change}",
                name=name,
                change=change,
            ),
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            self._parent,
        )
        box.setDefaultButton(QMessageBox.StandardButton.No)
        return box.exec() == QMessageBox.StandardButton.Yes

    def resolve_block_clashes(self, clashes: Sequence[BlockClash]) -> dict[str, ClashChoice] | None:
        return block_clash_dialog.resolve_block_clashes(clashes, self._parent)

    def edit_text(self, title: str, text: str = "") -> str | None:
        return text_dialog.ask_text(self._parent, title, text)

    def view(self, document: CadDocument) -> ViewControl | None:
        return None if self._views is None else self._views(document)

    def show_panel(self, name: str) -> bool:
        return self._panels is not None and self._panels(name)

    def show_options(self) -> bool:
        return self._options is not None and self._options()

    def _remember(self, path: str) -> Path | None:
        if not path:
            return None
        chosen = Path(path)
        self.last_folder = str(chosen.parent)
        return chosen
