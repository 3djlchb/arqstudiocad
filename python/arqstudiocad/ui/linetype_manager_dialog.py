"""Manage simple linetypes loaded in the current drawing."""

from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QDialog,
    QFileDialog,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadDocument, CadError
from arqstudiocad.i18n import tr
from arqstudiocad.ui.linetype_preview import linetype_preview


class LinetypeManagerDialog(QDialog):
    """Load simple A-aligned definitions from `.lin` into a drawing."""

    def __init__(
        self, document: CadDocument, parent: QWidget | None = None, *, load_only: bool = False
    ) -> None:
        super().__init__(parent)
        self._document = document
        self._load_only = load_only
        self.setWindowTitle(tr("Load or Reload Linetypes") if load_only else tr("Linetype Manager"))
        self.setMinimumSize(460, 360)
        self._loaded = QListWidget(self)
        self._available = QListWidget(self)
        self._available.setSelectionMode(QListWidget.SelectionMode.MultiSelection)
        self._definitions: list[tuple[str, str, list[float]]] = []
        self._source = QLabel(tr("No LIN file selected"), self)
        self._load_file = QPushButton(tr("Load LIN file…"), self)
        self._load_selected = QPushButton(tr("Load selected"), self)
        self._create = QPushButton(tr("Create"), self)
        self._edit = QPushButton(tr("Edit"), self)
        self._rename = QPushButton(tr("Rename"), self)
        self._delete = QPushButton(tr("Delete"), self)
        self._export = QPushButton(tr("Export…"), self)
        self._reload = QPushButton(tr("Reload from LIN…"), self)
        close = QPushButton(tr("Cancel") if load_only else tr("Close"), self)
        layout = QVBoxLayout(self)
        loaded_label = QLabel(tr("Loaded in drawing"), self)
        layout.addWidget(loaded_label)
        layout.addWidget(self._loaded)
        manage = QHBoxLayout()
        for button in (
            self._create,
            self._edit,
            self._rename,
            self._delete,
            self._export,
            self._reload,
        ):
            manage.addWidget(button)
        layout.addLayout(manage)
        layout.addWidget(self._source)
        layout.addWidget(self._available)
        buttons = QHBoxLayout()
        buttons.addWidget(self._load_file)
        buttons.addWidget(self._load_selected)
        buttons.addStretch(1)
        buttons.addWidget(close)
        layout.addLayout(buttons)
        self._load_file.clicked.connect(self._choose_file)
        self._load_selected.clicked.connect(self._load)
        self._create.clicked.connect(lambda: self._edit_definition())
        self._edit.clicked.connect(self._edit_selected)
        self._rename.clicked.connect(self._rename_selected)
        self._delete.clicked.connect(self._delete_selected)
        self._export.clicked.connect(self._export_definitions)
        self._reload.clicked.connect(self._reload_selected)
        self._loaded.currentItemChanged.connect(lambda *_args: self._update_actions())
        close.clicked.connect(self.reject if load_only else self.accept)
        if load_only:
            self._loaded.hide()
            loaded_label.hide()
            for button in (
                self._create,
                self._edit,
                self._rename,
                self._delete,
                self._export,
                self._reload,
            ):
                button.hide()
        self._refresh_loaded()

    def _refresh_loaded(self) -> None:
        self._loaded.clear()
        patterns = {
            name.casefold(): pattern
            for name, _description, pattern in self._document.get_linetype_definitions()
        }
        for name in self._document.get_linetypes():
            item = QListWidgetItem(linetype_preview(tuple(patterns.get(name.casefold(), []))), name)
            self._loaded.addItem(item)
        self._update_actions()

    def _update_actions(self) -> None:
        name = self._selected_loaded_name()
        editable = name is not None and name.casefold() != "continuous"
        for button in (self._edit, self._rename, self._delete, self._reload):
            button.setEnabled(editable)

    def _choose_file(self) -> None:
        path, _ = QFileDialog.getOpenFileName(
            self,
            tr("Load linetypes"),
            "",
            tr("Linetype definitions (*.lin);;All files (*)"),
        )
        if not path:
            return
        try:
            contents = Path(path).read_text(encoding="utf-8-sig")
            self._definitions = CadDocument.parse_linetype_file(contents)
        except (OSError, ValueError) as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))
            return
        self._source.setText(path)
        self._available.clear()
        loaded = {name.casefold() for name in self._document.get_linetypes()}
        for name, description, pattern in self._definitions:
            item = QListWidgetItem(f"{name} — {description}")
            item.setIcon(linetype_preview(tuple(pattern)))
            item.setData(Qt.ItemDataRole.UserRole, name)
            item.setFlags(item.flags() | Qt.ItemFlag.ItemIsUserCheckable)
            item.setCheckState(
                Qt.CheckState.Unchecked if name.casefold() in loaded else Qt.CheckState.Checked
            )
            if name.casefold() in loaded:
                item.setFlags(item.flags() & ~Qt.ItemFlag.ItemIsEnabled)
            self._available.addItem(item)

    def _load(self) -> None:
        selected = {
            item.data(Qt.ItemDataRole.UserRole)
            for index in range(self._available.count())
            if (item := self._available.item(index)).checkState() == Qt.CheckState.Checked
        }
        definitions = [item for item in self._definitions if item[0] in selected]
        if not definitions:
            return
        try:
            self._document.load_linetypes(definitions)
        except (CadError, ValueError, OSError) as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))
            return
        self._refresh_loaded()
        self._available.clear()
        self._definitions.clear()
        if self._load_only:
            self.accept()

    def _selected_loaded_name(self) -> str | None:
        item = self._loaded.currentItem()
        return None if item is None else item.text()

    def _edit_selected(self) -> None:
        name = self._selected_loaded_name()
        if name is None:
            return
        definition = next(
            (entry for entry in self._document.get_linetype_definitions() if entry[0] == name), None
        )
        if definition is None:
            QMessageBox.information(
                self,
                tr("Linetype Manager"),
                tr("This linetype uses text or shapes and cannot be edited here."),
            )
            return
        self._edit_definition(definition)

    def _edit_definition(self, definition: tuple[str, str, list[float]] | None = None) -> None:
        name, description, pattern = definition or ("", "", [])
        name, accepted = QInputDialog.getText(self, tr("Linetype Manager"), tr("Name"), text=name)
        if not accepted:
            return
        description, accepted = QInputDialog.getText(
            self, tr("Linetype Manager"), tr("Description"), text=description
        )
        if not accepted:
            return
        pattern_text, accepted = QInputDialog.getText(
            self,
            tr("Linetype Manager"),
            tr("Pattern values (positive dash, negative gap, zero dot)"),
            text=",".join(map(str, pattern)),
        )
        if not accepted:
            return
        try:
            values = [float(value.strip()) for value in pattern_text.split(",")]
            self._document.set_linetype(name, description, values)
        except (CadError, ValueError) as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))
            return
        self._refresh_loaded()

    def _rename_selected(self) -> None:
        old_name = self._selected_loaded_name()
        if old_name is None:
            return
        new_name, accepted = QInputDialog.getText(
            self, tr("Linetype Manager"), tr("New name"), text=old_name
        )
        if not accepted:
            return
        try:
            self._document.rename_linetype(old_name, new_name)
        except (CadError, ValueError) as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))
            return
        self._refresh_loaded()

    def _delete_selected(self) -> None:
        name = self._selected_loaded_name()
        if name is None:
            return
        if (
            QMessageBox.question(
                self, tr("Linetype Manager"), tr("Delete linetype {name}?", name=name)
            )
            != QMessageBox.StandardButton.Yes
        ):
            return
        try:
            self._document.delete_linetype(name)
        except (CadError, ValueError) as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))
            return
        self._refresh_loaded()

    def _export_definitions(self) -> None:
        path, _ = QFileDialog.getSaveFileName(
            self, tr("Export linetypes"), "", tr("Linetype definitions (*.lin)")
        )
        if not path:
            return
        definitions = [
            item
            for item in self._document.get_linetype_definitions()
            if item[0].casefold() not in {"continuous", "bylayer", "byblock"}
        ]
        if not definitions:
            QMessageBox.information(
                self, tr("Linetype Manager"), tr("There are no custom simple linetypes to export.")
            )
            return
        contents = (
            "\n".join(
                f"*{name},{description}\nA," + ",".join(format(value, ".12g") for value in pattern)
                for name, description, pattern in definitions
            )
            + "\n"
        )
        try:
            Path(path).write_text(contents, encoding="utf-8")
        except OSError as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))

    def _reload_selected(self) -> None:
        name = self._selected_loaded_name()
        if name is None:
            return
        path, _ = QFileDialog.getOpenFileName(
            self, tr("Reload linetype"), "", tr("Linetype definitions (*.lin)")
        )
        if not path:
            return
        try:
            contents = Path(path).read_text(encoding="utf-8-sig")
            definitions = CadDocument.parse_linetype_file(contents)
            definition = next(
                (item for item in definitions if item[0].casefold() == name.casefold()), None
            )
            if definition is None:
                raise ValueError(tr("The selected linetype is not in this file."))
            self._document.set_linetype(*definition)
        except (CadError, OSError, ValueError) as error:
            QMessageBox.warning(self, tr("Linetype Manager"), str(error))
            return
        self._refresh_loaded()
