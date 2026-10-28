"""The Currencies dialog (CURRENCIES, ADR 0027 D3, maintainer 2026-10-27):
the drawing's currency register as a table, one row per currency with its
Active box, its Principal choice, code, name, symbol, decimals and how
many blocks' costs use it. Names and symbols are edited in place;
currencies are added from the ISO 4217 catalog, with a search, or made by
the user. The dialog only proposes a register: the command writes it, and
the core refuses what would break a cost."""

import re

from PySide6.QtCore import QModelIndex, QPersistentModelIndex, QRectF, Qt
from PySide6.QtGui import QPainter, QPalette, QPen
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QSpinBox,
    QStyle,
    QStyledItemDelegate,
    QStyleOptionViewItem,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadCurrency, CadDocument
from arqstudiocad.currencies import (
    CATALOG,
    DECIMAL,
    THOUSANDS,
    CatalogCurrency,
    CurrencyRegister,
    format_amount,
    shown_name,
)
from arqstudiocad.i18n import tr

#: The table's columns.
ACTIVE, PRINCIPAL, CODE, NAME, SYMBOL, DECIMALS, USES = range(7)
#: A code of the user's own: 3 to 10 capital letters or digits, as the core
#: accepts.
OWN_CODE = re.compile(r"[A-Z0-9]{3,10}")
#: Where a Principal cell keeps whether its currency is the principal one.
IS_PRINCIPAL = Qt.ItemDataRole.UserRole


def ok_cancel(dialog: QDialog) -> QDialogButtonBox:
    """OK and Cancel, in the interface's language, accepting and rejecting
    ``dialog``."""
    buttons = QDialogButtonBox(
        QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
    )
    for which, text in (
        (QDialogButtonBox.StandardButton.Ok, tr("OK")),
        (QDialogButtonBox.StandardButton.Cancel, tr("Cancel")),
    ):
        button = buttons.button(which)
        assert button is not None
        button.setText(text)
    buttons.accepted.connect(dialog.accept)
    buttons.rejected.connect(dialog.reject)
    return buttons


class RadioDelegate(QStyledItemDelegate):
    """Draws a Principal cell as a radio button: a ring in the text's color,
    filled with a dot when chosen, dimmed when the currency is inactive. The
    style's own radio is faint over a table in the dark theme."""

    def paint(
        self,
        painter: QPainter,
        option: QStyleOptionViewItem,
        index: QModelIndex | QPersistentModelIndex,
    ) -> None:
        cell = QStyleOptionViewItem(option)
        self.initStyleOption(cell, index)
        widget = cell.widget
        style = widget.style() if widget is not None else QApplication.style()
        style.drawControl(QStyle.ControlElement.CE_ItemViewItem, cell, painter, widget)
        palette = cell.palette
        selected = bool(cell.state & QStyle.StateFlag.State_Selected)
        enabled = bool(index.flags() & Qt.ItemFlag.ItemIsEnabled)
        group = QPalette.ColorGroup.Normal if enabled else QPalette.ColorGroup.Disabled
        ink = palette.color(
            group,
            QPalette.ColorRole.HighlightedText if selected else QPalette.ColorRole.Text,
        )
        side = style.pixelMetric(QStyle.PixelMetric.PM_ExclusiveIndicatorWidth)
        area = cell.rect
        ring = QRectF(
            area.x() + (area.width() - side) / 2 + 0.5,
            area.y() + (area.height() - side) / 2 + 0.5,
            side - 1,
            side - 1,
        )
        painter.save()
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setPen(QPen(ink, 1.2))
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawEllipse(ring)
        if index.data(IS_PRINCIPAL):
            dot = ink if selected else palette.color(group, QPalette.ColorRole.Highlight)
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(dot)
            inset = side * 0.27
            painter.drawEllipse(ring.adjusted(inset, inset, -inset, -inset))
        painter.restore()


class CurrenciesDialog(QDialog):
    """Edits ``document``'s currency register; nothing is written here."""

    def __init__(self, document: CadDocument, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("currencies_dialog")
        self.setWindowTitle(tr("Currencies"))
        self.uses = document.currency_uses
        currencies, principal = document.currencies
        self.table = QTableWidget(0, 7)
        self.table.setHorizontalHeaderLabels(
            [
                tr("Active"),
                tr("Principal"),
                tr("Code"),
                tr("Name"),
                tr("Symbol"),
                tr("Decimals"),
                tr("Blocks"),
            ]
        )
        self.table.verticalHeader().hide()
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.table.setItemDelegateForColumn(PRINCIPAL, RadioDelegate(self.table))
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(NAME, QHeaderView.ResizeMode.Stretch)
        for currency in currencies:
            self._add_row(currency, currency.code == principal)
        self.table.itemChanged.connect(lambda _item: self._show())
        self.table.itemSelectionChanged.connect(self._show)
        self.table.cellClicked.connect(self._clicked)

        self.add_catalog = QPushButton(tr("Add from catalog…"))
        self.add_own = QPushButton(tr("Own currency…"))
        self.remove = QPushButton(tr("Remove"))
        self.add_catalog.clicked.connect(self._from_catalog)
        self.add_own.clicked.connect(self._own)
        self.remove.clicked.connect(self._remove)
        edit = QHBoxLayout()
        for button in (self.add_catalog, self.add_own, self.remove):
            edit.addWidget(button)
        edit.addStretch(1)
        self.note = QLabel(
            tr(
                "A currency a block's cost uses stays active, and the principal "
                "currency is proposed first for a new cost."
            )
        )
        self.note.setWordWrap(True)

        # How amounts are written (maintainer, 2026-10-27).
        thousands, decimal = document.number_format
        names = {",": tr("Comma (,)"), ".": tr("Point (.)"), " ": tr("Space")}
        self.thousands = QComboBox()
        for separator in THOUSANDS:
            self.thousands.addItem(names[separator], separator)
        self.decimal = QComboBox()
        for separator in DECIMAL:
            self.decimal.addItem(names[separator], separator)
        self.thousands.setCurrentIndex(THOUSANDS.index(thousands))
        self.decimal.setCurrentIndex(DECIMAL.index(decimal))
        self.example = QLabel()
        self.thousands.currentIndexChanged.connect(lambda _i: self._separators(self.thousands))
        self.decimal.currentIndexChanged.connect(lambda _i: self._separators(self.decimal))
        numbers = QGroupBox(tr("Number format"))
        form = QFormLayout(numbers)
        form.addRow(tr("Thousands separator"), self.thousands)
        form.addRow(tr("Decimal separator"), self.decimal)
        form.addRow(tr("Example"), self.example)
        self.buttons = ok_cancel(self)

        layout = QVBoxLayout(self)
        layout.addWidget(self.table, 1)
        layout.addLayout(edit)
        layout.addWidget(self.note)
        layout.addWidget(numbers)
        layout.addWidget(self.buttons)
        self.resize(640, 460)
        self._separators(self.decimal)
        self._show()

    def number_format(self) -> tuple[str, str]:
        """The thousands and decimal separators chosen."""
        return self.thousands.currentData(), self.decimal.currentData()

    def _separators(self, changed: QComboBox) -> None:
        """Keeps the two separators apart: the one not just changed takes
        another, and the example follows."""
        thousands, decimal = self.number_format()
        if thousands == decimal:
            other = self.decimal if changed is self.thousands else self.thousands
            choices = DECIMAL if other is self.decimal else THOUSANDS
            taken = thousands if other is self.decimal else decimal
            other.blockSignals(True)
            other.setCurrentIndex(next(i for i, s in enumerate(choices) if s != taken))
            other.blockSignals(False)
        self.example.setText(format_amount("1234567.89", *self.number_format()))

    # The register the dialog proposes

    def codes(self) -> list[str]:
        """The codes, in the table's order."""
        return [self._item(row, CODE).text() for row in range(self.table.rowCount())]

    def principal_row(self) -> int:
        """The row of the principal currency."""
        rows = range(self.table.rowCount())
        return next(r for r in rows if self._item(r, PRINCIPAL).data(IS_PRINCIPAL))

    def can_be_principal(self, row: int) -> bool:
        """Whether row ``row``'s currency may become the principal one: it
        is active."""
        return bool(self._item(row, PRINCIPAL).flags() & Qt.ItemFlag.ItemIsEnabled)

    def set_principal(self, row: int) -> None:
        """Makes row ``row``'s currency the principal one, if it is active."""
        if not self.can_be_principal(row):
            return
        self.table.blockSignals(True)
        for r in range(self.table.rowCount()):
            self._item(r, PRINCIPAL).setData(IS_PRINCIPAL, r == row)
        self.table.blockSignals(False)
        self._show()

    def register(self) -> CurrencyRegister:
        """The currencies as the table shows them, the principal one's code
        and the number format. A catalog name shown translated is kept in
        English."""
        currencies = []
        for row in range(self.table.rowCount()):
            name_item = self._item(row, NAME)
            stored = name_item.data(Qt.ItemDataRole.UserRole)
            name = name_item.text().strip()
            if name == shown_name(stored):
                name = stored
            currencies.append(
                CadCurrency(
                    self._item(row, CODE).text(),
                    name,
                    self._item(row, SYMBOL).text().strip(),
                    int(self._item(row, DECIMALS).text()),
                    self._item(row, ACTIVE).checkState() == Qt.CheckState.Checked,
                )
            )
        return CurrencyRegister(
            currencies, self.codes()[self.principal_row()], *self.number_format()
        )

    # Rows

    def _add_row(self, currency: CadCurrency, principal: bool) -> None:
        row = self.table.rowCount()
        self.table.insertRow(row)
        uses = self.uses.get(currency.code, 0)
        active = QTableWidgetItem()
        active.setFlags(Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsUserCheckable)
        active.setCheckState(
            Qt.CheckState.Checked if currency.is_active else Qt.CheckState.Unchecked
        )
        self.table.setItem(row, ACTIVE, active)
        choice = QTableWidgetItem()
        choice.setData(IS_PRINCIPAL, principal)
        self.table.setItem(row, PRINCIPAL, choice)
        fixed = Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsSelectable
        editable = fixed | Qt.ItemFlag.ItemIsEditable
        name = QTableWidgetItem(shown_name(currency.name))
        name.setData(Qt.ItemDataRole.UserRole, currency.name)
        for column, item, flags in (
            (CODE, QTableWidgetItem(currency.code), fixed),
            (NAME, name, editable),
            (SYMBOL, QTableWidgetItem(currency.symbol), editable),
            (DECIMALS, QTableWidgetItem(str(currency.decimals)), fixed),
            (USES, QTableWidgetItem(str(uses)), fixed),
        ):
            item.setFlags(flags)
            self.table.setItem(row, column, item)

    def _item(self, row: int, column: int) -> QTableWidgetItem:
        item = self.table.item(row, column)
        assert item is not None
        return item

    def _uses(self, row: int) -> int:
        return int(self._item(row, USES).text())

    def _clicked(self, row: int, column: int) -> None:
        if column == PRINCIPAL:
            self.set_principal(row)

    def _show(self) -> None:
        """Enables what may change: a currency a cost uses, or the
        principal one, stays active; neither can be removed; only an active
        one can be principal. OK waits for a name on every row."""
        principal = self.principal_row()
        self.table.blockSignals(True)
        for row in range(self.table.rowCount()):
            active = self._item(row, ACTIVE)
            # Both are active already: the core keeps them so.
            locked = self._uses(row) > 0 or row == principal
            flags = Qt.ItemFlag.ItemIsUserCheckable
            active.setFlags(flags if locked else flags | Qt.ItemFlag.ItemIsEnabled)
            on = active.checkState() == Qt.CheckState.Checked
            choice = Qt.ItemFlag.ItemIsSelectable
            self._item(row, PRINCIPAL).setFlags(
                choice | Qt.ItemFlag.ItemIsEnabled if on else choice
            )
        self.table.blockSignals(False)
        self.table.viewport().update()
        row = self.table.currentRow()
        selected = bool(self.table.selectedItems())
        self.remove.setEnabled(selected and self._uses(row) == 0 and row != principal)
        named = all(self._item(r, NAME).text().strip() for r in range(self.table.rowCount()))
        ok = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        ok.setEnabled(named)

    def _from_catalog(self) -> None:
        dialog = CatalogDialog(set(self.codes()), self)
        if dialog.exec() == QDialog.DialogCode.Accepted and (chosen := dialog.chosen()):
            self._append(CadCurrency(chosen.code, chosen.name, chosen.symbol, chosen.decimals))

    def _own(self) -> None:
        dialog = OwnCurrencyDialog(set(self.codes()), self)
        if dialog.exec() == QDialog.DialogCode.Accepted:
            self._append(dialog.currency())

    def _append(self, currency: CadCurrency) -> None:
        self.table.blockSignals(True)
        self._add_row(currency, False)
        self.table.blockSignals(False)
        self.table.selectRow(self.table.rowCount() - 1)
        self._show()

    def _remove(self) -> None:
        row = self.table.currentRow()
        if row < 0 or self._uses(row) > 0 or row == self.principal_row():
            return
        # No refresh may see the row half removed.
        self.table.blockSignals(True)
        self.table.removeRow(row)
        self.table.blockSignals(False)
        self._show()


class CatalogDialog(QDialog):
    """Chooses one ISO 4217 currency the register lacks, with a search by
    code or name in the reader's language."""

    def __init__(self, taken: set[str], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("currency_catalog_dialog")
        self.setWindowTitle(tr("Add from catalog"))
        self.search = QLineEdit()
        self.search.setPlaceholderText(tr("Search by code or name"))
        self.search.setClearButtonEnabled(True)
        self.list = QListWidget()
        for currency in CATALOG:
            if currency.code in taken:
                continue
            text = f"{currency.code} — {shown_name(currency.name)}"
            if currency.symbol:
                text += f" ({currency.symbol})"
            item = QListWidgetItem(text)
            item.setData(Qt.ItemDataRole.UserRole, currency)
            self.list.addItem(item)
        self.search.textChanged.connect(self._filter)
        self.list.itemDoubleClicked.connect(lambda _item: self.accept())
        self.list.currentItemChanged.connect(lambda *_: self._show())
        self.buttons = ok_cancel(self)
        layout = QVBoxLayout(self)
        layout.addWidget(self.search)
        layout.addWidget(self.list, 1)
        layout.addWidget(self.buttons)
        self.resize(380, 420)
        self._show()

    def shown(self) -> list[str]:
        """The codes the search leaves visible, in order."""
        return [
            self.list.item(i).data(Qt.ItemDataRole.UserRole).code
            for i in range(self.list.count())
            if not self.list.item(i).isHidden()
        ]

    def chosen(self) -> CatalogCurrency | None:
        """The currency chosen: the search keeps the first one it finds
        chosen, or none."""
        item = self.list.currentItem()
        if item is None:
            return None
        currency: CatalogCurrency = item.data(Qt.ItemDataRole.UserRole)
        return currency

    def _filter(self, text: str) -> None:
        wanted = text.strip().casefold()
        first = None
        for i in range(self.list.count()):
            item = self.list.item(i)
            hidden = bool(wanted) and wanted not in item.text().casefold()
            item.setHidden(hidden)
            if not hidden and first is None:
                first = item
        if first is None:
            self.list.setCurrentRow(-1)
        else:
            self.list.setCurrentItem(first)
        self._show()

    def _show(self) -> None:
        ok = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        ok.setEnabled(self.chosen() is not None)


class OwnCurrencyDialog(QDialog):
    """Makes a currency of the user's own: a code the register does not
    hold, a name, a symbol and its decimals."""

    def __init__(self, taken: set[str], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("own_currency_dialog")
        self.setWindowTitle(tr("Own currency"))
        self.taken = taken
        self.code = QLineEdit()
        self.code.setMaxLength(10)
        self.code.setPlaceholderText(tr("3 to 10 capital letters or digits"))
        self.name = QLineEdit()
        self.symbol = QLineEdit()
        self.decimals = QSpinBox()
        self.decimals.setRange(0, 4)
        self.decimals.setValue(2)
        self.problem = QLabel()
        self.buttons = ok_cancel(self)
        for field in (self.code, self.name):
            field.textChanged.connect(lambda _text: self._show())
        form = QFormLayout()
        form.addRow(tr("Code"), self.code)
        form.addRow(tr("Name"), self.name)
        form.addRow(tr("Symbol"), self.symbol)
        form.addRow(tr("Decimals"), self.decimals)
        layout = QVBoxLayout(self)
        layout.addLayout(form)
        layout.addWidget(self.problem)
        layout.addWidget(self.buttons)
        self._show()

    def currency(self) -> CadCurrency:
        """The currency described, active."""
        return CadCurrency(
            self.code.text().strip().upper(),
            self.name.text().strip(),
            self.symbol.text().strip(),
            self.decimals.value(),
        )

    def _show(self) -> None:
        code = self.code.text().strip().upper()
        if not OWN_CODE.fullmatch(code):
            problem = tr("The code is 3 to 10 capital letters or digits.")
        elif code in self.taken:
            problem = tr("The drawing already has {code}.", code=code)
        elif not self.name.text().strip():
            problem = tr("Give the currency a name.")
        else:
            problem = ""
        self.problem.setText(problem)
        ok = self.buttons.button(QDialogButtonBox.StandardButton.Ok)
        ok.setEnabled(not problem)


def edit_currencies(
    document: CadDocument, parent: QWidget | None = None
) -> CurrencyRegister | None:
    """Shows the Currencies dialog; the register proposed, or None if it was
    cancelled."""
    dialog = CurrenciesDialog(document, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return None
    return dialog.register()
