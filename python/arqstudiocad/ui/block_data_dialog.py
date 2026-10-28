"""A block's data for budgets (ADR 0027): its category and type, with the
values the drawing already uses suggested, its material and supplier, and
its cost: an amount typed in its currency's number format (D3a) and one of
the drawing's active currencies, the principal one first.

The fields are one widget, shown by the Block Data dialog (BLOCKDATA, step
4a) and by the Block Definition dialog's Budget data section (step 4b).
They only propose the data: the command writes it, and the core checks it
again."""

from PySide6.QtCore import Signal
from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadBlockData, CadDocument, CadError
from arqstudiocad.currencies import format_amount, parse_amount, register_of
from arqstudiocad.i18n import tr
from arqstudiocad.ui.currencies_dialog import ok_cancel

#: The longest text a field holds, as the core accepts.
MAX_TEXT = 255


def suggestions(document: CadDocument, field: str) -> list[str]:
    """The values the drawing's blocks already give ``field`` (``category``
    or ``type``), each once ignoring case, as the first block by name spells
    it, sorted."""
    found: dict[str, str] = {}
    every = document.blocks_data
    for name in sorted(every, key=str.casefold):
        value = getattr(every[name], field).strip()
        if value:
            found.setdefault(value.casefold(), value)
    return sorted(found.values(), key=str.casefold)


class BlockDataFields(QWidget):
    """The fields of a block's data over ``document``, filled from ``data``;
    ``changed`` says when what they hold, or its problem, may differ."""

    changed = Signal()

    def __init__(
        self, document: CadDocument, data: CadBlockData, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.register = register_of(document)
        self.category = self._suggesting(suggestions(document, "category"), data.category)
        self.kind = self._suggesting(suggestions(document, "type"), data.type)
        self.material = QLineEdit(data.material)
        self.supplier = QLineEdit(data.supplier)
        for field in (self.material, self.supplier):
            field.setMaxLength(MAX_TEXT)

        # The cost: an amount and one of the active currencies.
        self.currency = QComboBox()
        active = [c for c in self.register.currencies if c.is_active]
        active.sort(key=lambda c: c.code != self.register.principal)
        for currency in active:
            self.currency.addItem(
                f"{currency.code} ({currency.symbol})" if currency.symbol else currency.code,
                currency.code,
            )
        if data.currency is not None and self.currency.findData(data.currency) < 0:
            # Not an active currency of the register (ADR 0027 D3).
            self.currency.addItem(tr("{code} (not active)", code=data.currency), data.currency)
        if data.currency is not None:
            self.currency.setCurrentIndex(self.currency.findData(data.currency))
        self._shown_format = self._format()
        self.cost = QLineEdit(
            format_amount(data.cost, *self._shown_format) if data.cost is not None else ""
        )
        self.cost.setPlaceholderText(tr("No cost"))
        cost = QHBoxLayout()
        cost.addWidget(self.cost, 1)
        cost.addWidget(self.currency)

        self.form = QFormLayout(self)
        self.form.setContentsMargins(0, 0, 0, 0)
        self.form.addRow(tr("Category"), self.category)
        self.form.addRow(tr("Type"), self.kind)
        self.form.addRow(tr("Material"), self.material)
        self.form.addRow(tr("Supplier"), self.supplier)
        self.form.addRow(tr("Cost"), cost)
        self.cost.textChanged.connect(lambda _text: self._typed())
        self.currency.currentIndexChanged.connect(lambda _i: self._currency_changed())
        self._typed()

    def data(self) -> CadBlockData | None:
        """The data the fields hold, or None while one cannot be written."""
        if self.problem():
            return None
        amount = self._amount()
        return CadBlockData(
            category=self.category.currentText().strip(),
            type=self.kind.currentText().strip(),
            material=self.material.text().strip(),
            supplier=self.supplier.text().strip(),
            cost=amount,
            currency=self.currency.currentData() if amount is not None else None,
        )

    def clear(self) -> None:
        """Empties every field: OK then writes no data, which clears the
        block's (maintainer, 2026-10-28)."""
        for box in (self.category, self.kind):
            box.setCurrentText("")
        for field in (self.material, self.supplier, self.cost):
            field.clear()

    def problem(self) -> str:
        """Why the fields cannot be written, or empty."""
        try:
            amount = self._amount()
        except ValueError as error:
            return str(error)
        if amount is None:
            return ""
        code = self.currency.currentData()
        currency = self.register.get(code)
        if currency is None or not currency.is_active:
            return tr(
                "{code} is not an active currency: activate it in Currencies, or choose another.",
                code=code,
            )
        decimals = len(amount.partition(".")[2])
        if decimals > currency.decimals:
            return tr(
                "A cost in {code} has at most {decimals} decimals.",
                code=code,
                decimals=currency.decimals,
            )
        return ""

    def _suggesting(self, values: list[str], current: str) -> QComboBox:
        box = QComboBox()
        box.setEditable(True)
        box.addItems(values)
        box.setCurrentText(current)
        line = box.lineEdit()
        assert line is not None
        line.setMaxLength(MAX_TEXT)
        return box

    def _format(self) -> tuple[str, str]:
        return self.register.format_of(self.currency.currentData() or "")

    def _amount(self) -> str | None:
        """The amount typed as the core reads it; None when empty. Raises
        ValueError when it is not an amount in the currency's format."""
        typed = self.cost.text().strip()
        return parse_amount(typed, *self._format()) if typed else None

    def _currency_changed(self) -> None:
        """The amount typed keeps its value, written in the new currency's
        format."""
        typed = self.cost.text().strip()
        if typed:
            try:
                amount = parse_amount(typed, *self._shown_format)
            except ValueError:
                amount = None
            if amount is not None:
                self.cost.setText(format_amount(amount, *self._format()))
        self._shown_format = self._format()
        self._typed()

    def _typed(self) -> None:
        example = format_amount("1250.50", *self._format())
        self.cost.setToolTip(tr("Write it as {example}.", example=example))
        self.changed.emit()


class BlockDataDialog(QDialog):
    """Edits block ``name``'s data for budgets; nothing is written here."""

    def __init__(self, document: CadDocument, name: str, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("block_data_dialog")
        self.setWindowTitle(tr("Block data: {name}", name=name))
        self.note = QLabel()
        self.note.setWordWrap(True)
        try:
            data = document.block_data(name) or CadBlockData()
        except CadError:
            data = CadBlockData()
            self.note.setText(tr("The block's data could not be read: saving replaces it."))
        self.note.setVisible(bool(self.note.text()))
        self.fields = BlockDataFields(document, data)
        fields = self.fields
        self.category, self.kind = fields.category, fields.kind
        self.material, self.supplier = fields.material, fields.supplier
        self.cost, self.currency = fields.cost, fields.currency
        self.problem = QLabel()
        self.problem.setWordWrap(True)
        self.buttons = ok_cancel(self)
        # Empties the fields; OK confirms, Cancel keeps the data.
        self.clear_button = self.buttons.addButton(
            tr("Clear data"), QDialogButtonBox.ButtonRole.ResetRole
        )
        self.clear_button.setObjectName("clear_data")
        self.clear_button.setToolTip(tr("Empties every field; OK then clears the block's data."))
        self.clear_button.clicked.connect(fields.clear)
        layout = QVBoxLayout(self)
        layout.addWidget(self.note)
        layout.addWidget(fields)
        layout.addWidget(self.problem)
        layout.addWidget(self.buttons)
        fields.changed.connect(self._show)
        self.resize(420, 0)
        self._show()

    def data(self) -> CadBlockData | None:
        """The data the fields hold, or None while one cannot be written."""
        return self.fields.data()

    def _show(self) -> None:
        problem = self.fields.problem()
        self.problem.setText(problem)
        ok = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        ok.setEnabled(not problem)


def edit_block_data(
    document: CadDocument, name: str, parent: QWidget | None = None
) -> CadBlockData | None:
    """Shows the Block Data dialog for block ``name``; the data proposed, or
    None if it was cancelled."""
    dialog = BlockDataDialog(document, name, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.data()
