"""The Block Definition dialog (ADR 0023 D2a), as AutoCAD's and progeCAD's:
the block's name and description, its base point, the objects and what
happens to them, its behavior and units, and an optional Budget data
section, folded by default (ADR 0027 step 4b). BLOCK shows it; it closes
to pick the base point, select objects or edit the currencies, and BLOCK
opens it again with what it held. Annotative waits for annotation scales
(phase 04)."""

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QButtonGroup,
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFormLayout,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QRadioButton,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.commands import BlockDefinitionState, BlockDialogAction, BlockObjectsChoice
from arqstudiocad.core import CadBlockData, CadDocument
from arqstudiocad.i18n import tr
from arqstudiocad.ui.block_data_dialog import BlockDataFields
from arqstudiocad.ui.blocks_panel import units_name

#: Characters a block name cannot hold, as the core refuses them.
FORBIDDEN = set('<>/\\":;?*|,=`')
#: DWG's INSUNITS codes, in the order the list shows them.
UNITS = tuple(range(22))
#: How far a typed coordinate may go, either way.
COORDINATE_LIMIT = 1e12


class BlockDefinitionDialog(QDialog):
    """BLOCK's dialog over ``document``, filled from ``state``; ``objects``
    is the choice to show when ``state`` leaves it to the interface."""

    def __init__(
        self,
        document: CadDocument,
        state: BlockDefinitionState,
        objects: BlockObjectsChoice = "convert",
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("block_definition_dialog")
        self.setWindowTitle(tr("Block Definition"))
        self.document = document
        self.ids = state.ids
        #: Why it closed: OK, or to pick on screen.
        self.action: BlockDialogAction = "ok"

        self.name = QComboBox()
        self.name.setEditable(True)
        self.name.addItems(document.block_names)
        self.name.setCurrentText(state.name)
        self.name.editTextChanged.connect(self._check)
        self.description = QLineEdit(state.description)

        # Base point.
        self.base_on_screen = QCheckBox(tr("Specify on screen"))
        self.base_on_screen.setChecked(state.base_on_screen)
        self.pick_point = QPushButton(tr("Pick point"))
        self.pick_point.clicked.connect(lambda: self._close_for("pick_point"))
        self.base_x = self._coordinate(state.base[0])
        self.base_y = self._coordinate(state.base[1])
        base = QGroupBox(tr("Base point"))
        base_form = QFormLayout(base)
        base_form.addRow(self.base_on_screen)
        base_form.addRow(self.pick_point)
        base_form.addRow("X:", self.base_x)
        base_form.addRow("Y:", self.base_y)

        # Objects.
        self.objects_on_screen = QCheckBox(tr("Specify on screen"))
        self.objects_on_screen.setChecked(state.objects_on_screen)
        self.select_objects = QPushButton(tr("Pick objects"))
        self.select_objects.clicked.connect(lambda: self._close_for("select_objects"))
        self.retain = QRadioButton(tr("Retain objects"))
        self.convert = QRadioButton(tr("Convert to block"))
        self.delete = QRadioButton(tr("Delete objects"))
        self.choices = QButtonGroup(self)
        for button in (self.retain, self.convert, self.delete):
            self.choices.addButton(button)
        chosen = state.objects or objects
        {"retain": self.retain, "convert": self.convert, "delete": self.delete}[chosen].setChecked(
            True
        )
        self.count = QLabel()
        self.count.setWordWrap(True)
        objects_box = QGroupBox(tr("Objects"))
        objects_column = QVBoxLayout(objects_box)
        for widget in (
            self.objects_on_screen,
            self.select_objects,
            self.retain,
            self.convert,
            self.delete,
            self.count,
        ):
            objects_column.addWidget(widget)

        # Behavior.
        self.annotative = QCheckBox(tr("Annotative"))
        self.annotative.setEnabled(False)
        self.annotative.setToolTip(tr("Annotation scales come with a later phase."))
        self.scale_uniformly = QCheckBox(tr("Scale uniformly"))
        self.scale_uniformly.setChecked(state.scale_uniformly)
        self.explodable = QCheckBox(tr("Allow exploding"))
        self.explodable.setChecked(state.explodable)
        self.units = QComboBox()
        for code in UNITS:
            self.units.addItem(units_name(code), code)
        self.units.setCurrentIndex(max(0, self.units.findData(state.units)))
        behavior = QGroupBox(tr("Behavior"))
        behavior_form = QFormLayout(behavior)
        behavior_form.addRow(self.annotative)
        behavior_form.addRow(self.scale_uniformly)
        behavior_form.addRow(self.explodable)
        behavior_form.addRow(tr("Block unit:"), self.units)

        # Budget data (ADR 0027 step 4b): optional, folded unless it holds
        # something or was unfolded before the dialog closed to pick.
        self._data = state.data
        self.data_fields = BlockDataFields(document, state.data)
        self.data_fields.changed.connect(self._check)
        self.edit_currencies = QPushButton(tr("Currencies…"))
        self.edit_currencies.clicked.connect(lambda: self._close_for("currencies"))
        currencies_row = QHBoxLayout()
        currencies_row.addStretch(1)
        currencies_row.addWidget(self.edit_currencies)
        self.data_box = QWidget()
        data_column = QVBoxLayout(self.data_box)
        data_column.setContentsMargins(16, 0, 0, 0)
        data_column.addWidget(self.data_fields)
        data_column.addLayout(currencies_row)
        self.data_toggle = QToolButton()
        self.data_toggle.setObjectName("budget_data_toggle")
        self.data_toggle.setText(tr("Budget data"))
        self.data_toggle.setCheckable(True)
        self.data_toggle.setAutoRaise(True)
        self.data_toggle.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextBesideIcon)
        self.data_toggle.setChecked(state.data_shown or state.data != CadBlockData())
        self.data_toggle.toggled.connect(lambda _on: self._fold())
        self._fold()

        self.open_editor = QCheckBox(tr("Open in block editor"))
        self.open_editor.setChecked(state.open_editor)
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
        self.buttons.accepted.connect(lambda: self._close_for("ok"))
        self.buttons.rejected.connect(self.reject)

        names = QFormLayout()
        names.addRow(tr("Name:"), self.name)
        names.addRow(tr("Description:"), self.description)
        groups = QGridLayout()
        groups.addWidget(base, 0, 0)
        groups.addWidget(objects_box, 0, 1)
        groups.addWidget(behavior, 0, 2)
        bottom = QHBoxLayout()
        bottom.addWidget(self.open_editor)
        bottom.addStretch(1)
        bottom.addWidget(self.buttons)
        layout = QVBoxLayout(self)
        layout.addLayout(names)
        layout.addLayout(groups)
        layout.addWidget(self.data_toggle)
        layout.addWidget(self.data_box)
        layout.addWidget(self.problem_label)
        layout.addLayout(bottom)
        for box in (self.base_on_screen, self.objects_on_screen):
            box.toggled.connect(self._check)
        self._check()

    def _coordinate(self, value: float) -> QDoubleSpinBox:
        box = QDoubleSpinBox()
        box.setDecimals(4)
        box.setRange(-COORDINATE_LIMIT, COORDINATE_LIMIT)
        box.setValue(value)
        box.setAlignment(Qt.AlignmentFlag.AlignRight)
        return box

    def state(self) -> BlockDefinitionState:
        """What the dialog holds now."""
        objects: BlockObjectsChoice = (
            "retain"
            if self.retain.isChecked()
            else "delete"
            if self.delete.isChecked()
            else "convert"
        )
        return BlockDefinitionState(
            name=self.name.currentText().strip(),
            description=self.description.text(),
            base=(self.base_x.value(), self.base_y.value()),
            base_on_screen=self.base_on_screen.isChecked(),
            ids=self.ids,
            objects_on_screen=self.objects_on_screen.isChecked(),
            objects=objects,
            scale_uniformly=self.scale_uniformly.isChecked(),
            explodable=self.explodable.isChecked(),
            units=int(self.units.currentData()),
            open_editor=self.open_editor.isChecked(),
            # What cannot be written yet keeps the data it had.
            data=self.data_fields.data() or self._data,
            data_shown=self.data_toggle.isChecked(),
        )

    def _fold(self) -> None:
        """Shows or hides the Budget data section, its arrow following."""
        shown = self.data_toggle.isChecked()
        self.data_box.setVisible(shown)
        self.data_toggle.setArrowType(Qt.ArrowType.DownArrow if shown else Qt.ArrowType.RightArrow)
        self.adjustSize()

    def problem(self) -> str | None:
        """Why OK cannot make the block yet; None when it can."""
        name = self.name.currentText().strip()
        if not name:
            return tr("Type the block's name.")
        if name.startswith("*") or FORBIDDEN & set(name):
            return tr("{name} is not a valid block name.", name=name)
        taken = next(
            (n for n in self.document.block_names if n.casefold() == name.casefold()), None
        )
        if taken is not None and self.document.is_dynamic_block(taken):
            return tr("{name} is a dynamic block: it cannot be redefined yet.", name=taken)
        if not self.ids and not self.objects_on_screen.isChecked():
            return tr("No objects are selected.")
        data = self.data_fields.problem()
        if data:
            return tr("Budget data: {problem}", problem=data)
        return None

    def _check(self) -> None:
        """The objects' count, the coordinates and pick buttons as on
        screen says, and whether OK can go on."""
        on_screen = self.base_on_screen.isChecked()
        for widget in (self.pick_point, self.base_x, self.base_y):
            widget.setEnabled(not on_screen)
        self.select_objects.setEnabled(not self.objects_on_screen.isChecked())
        if self.objects_on_screen.isChecked():
            self.count.setText(tr("Objects are selected after OK."))
        elif self.ids:
            self.count.setText(tr("{count} objects selected", count=len(self.ids)))
        else:
            self.count.setText("⚠ " + tr("No objects are selected."))
        problem = self.problem()
        self.problem_label.setText(problem or "")
        self.problem_label.setVisible(problem is not None)
        self.ok_button.setEnabled(problem is None)

    def _close_for(self, action: BlockDialogAction) -> None:
        if action == "ok" and self.problem() is not None:
            return
        self.action = action
        self.accept()


def define_block(
    document: CadDocument,
    state: BlockDefinitionState,
    objects: BlockObjectsChoice = "convert",
    parent: QWidget | None = None,
) -> tuple[BlockDialogAction, BlockDefinitionState] | None:
    """Shows the dialog; why it closed and what it held, or None if it was
    cancelled."""
    dialog = BlockDefinitionDialog(document, state, objects, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.action, dialog.state()
