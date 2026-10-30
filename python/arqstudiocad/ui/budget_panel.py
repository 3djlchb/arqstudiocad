"""The Budget panel (ADR 0028 D1, D2, D4, D5): the active drawing's budget,
grouped by category, each group ending with its subtotal per currency
(maintainer, 2026-10-29), then the blocks placed without a cost, then the
totals per currency. It follows the drawing as it changes; with Selection
only, it follows the selection too. Export… writes what it shows to Excel
or CSV through BUDGETEXPORT (D6).
"""

from PySide6.QtCore import Qt
from PySide6.QtGui import QFont, QPalette, QShowEvent
from PySide6.QtWidgets import (
    QCheckBox,
    QDockWidget,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QMainWindow,
    QPushButton,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.budget import Budget, BudgetRow, budget_of
from arqstudiocad.commands import CommandRunner
from arqstudiocad.commands.budget import export_command
from arqstudiocad.core import CadDocument
from arqstudiocad.currencies import CurrencyRegister
from arqstudiocad.events import Event, SelectionChanged
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.panel_style import apply_data_view_style

#: The columns, in order.
ITEM, MATERIAL, SUPPLIER, UNIT, QUANTITY, COST, AMOUNT = range(7)
#: Columns of numbers, aligned right.
NUMBERS = (QUANTITY, COST, AMOUNT)
#: Its height when first opened docked: about ten rows.
FIRST_HEIGHT = 340


class BudgetPanel(QDockWidget):
    """The budget of the active drawing, or of its selection."""

    def __init__(
        self,
        session: Session,
        events: QtEventBridge,
        runner: CommandRunner | None = None,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("budget_panel")
        self._session = session
        self._runner = runner
        #: The drawing changed while the panel was hidden.
        self._stale = True
        #: Not yet opened: its first opening gives it room.
        self._sized = False
        #: The budget shown, for tests and the export.
        self.budget = Budget()
        body = QWidget()
        layout = QVBoxLayout(body)
        layout.setContentsMargins(6, 6, 6, 6)
        self.selection_only = QCheckBox()
        self.selection_only.toggled.connect(lambda _checked: self.refresh())
        self.export_button = QPushButton()
        self.export_button.clicked.connect(self.export)
        top = QHBoxLayout()
        top.addWidget(self.selection_only)
        top.addStretch(1)
        top.addWidget(self.export_button)
        layout.addLayout(top)
        self.caption = QLabel()
        self.caption.setWordWrap(True)
        layout.addWidget(self.caption)
        self.table = QTreeWidget()
        self.table.setColumnCount(7)
        self.table.setRootIsDecorated(True)
        self.table.setUniformRowHeights(True)
        header = self.table.header()
        header.setStretchLastSection(False)
        header.setSectionResizeMode(ITEM, QHeaderView.ResizeMode.Stretch)
        for column in range(MATERIAL, AMOUNT + 1):
            header.setSectionResizeMode(column, QHeaderView.ResizeMode.ResizeToContents)
        apply_data_view_style(self.table)
        layout.addWidget(self.table)
        self.footer = QLabel()
        self.footer.setWordWrap(True)
        layout.addWidget(self.footer)
        self.setWidget(body)
        for signal in (events.active_document_changed, events.document_changed):
            signal.connect(self.refresh)
        events.selection_changed.connect(self._selection_changed)
        self.retranslate()

    def restyle(self) -> None:
        """Refresh palette-based colors after a theme change."""
        apply_data_view_style(self.table)
        self._stale = True
        self.refresh()

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Budget"))
        self.selection_only.setText(tr("Selection only"))
        self.export_button.setText(tr("Export…"))
        self.export_button.setToolTip(tr("Writes the budget shown to Excel or CSV."))
        self.caption.setText(
            tr("Quantities: references in model space, those inside other blocks included.")
        )
        self.table.setHeaderLabels(
            [
                tr("Item"),
                tr("Material"),
                tr("Supplier"),
                tr("Unit"),
                tr("Quantity"),
                tr("Unit cost"),
                tr("Amount"),
            ]
        )
        self._stale = True
        self.refresh()

    def document(self) -> CadDocument | None:
        document = self._session.active
        return document if document is not None and document.is_open else None

    def _selection_changed(self, event: SelectionChanged) -> None:
        if self.selection_only.isChecked():
            self.refresh(event)

    def refresh(self, event: Event | None = None) -> None:
        if not self.isVisible():
            # The budget reads the whole drawing: not while closed.
            self._stale = True
            return
        self._stale = False
        document = self.document()
        self.table.clear()
        if document is None:
            self.budget = Budget()
            self.table.setEnabled(False)
            self.export_button.setEnabled(False)
            self.footer.setText(tr("No drawing is open."))
            return
        self.table.setEnabled(True)
        ids = self._session.selection(document) if self.selection_only.isChecked() else None
        self.budget = budget_of(document, ids)
        self._fill(self.budget)
        self.export_button.setEnabled(not self.budget.empty and self._runner is not None)
        self.footer.setText(self._footer(ids))

    def export(self) -> None:
        """Export…: BUDGETEXPORT of what the panel shows."""
        if self._runner is not None:
            self._runner.start(export_command(self.selection_only.isChecked()))

    def _footer(self, ids: tuple[int, ...] | None) -> str:
        budget = self.budget
        if budget.empty:
            if ids is not None:
                return tr("No block among the selected objects.")
            return tr("No block is placed in model space.")
        count = len(budget.without_cost)
        if count == 1:
            return tr("1 block has no cost: it is not summed.")
        if count > 1:
            return tr("{count} blocks have no cost: they are not summed.", count=count)
        return ""

    def _fill(self, budget: Budget) -> None:
        register = budget.register
        assert register is not None or budget.empty
        for group in budget.groups:
            title = group.category or tr("No category")
            head = self._heading(title)
            for row in group.rows:
                head.addChild(self._row(row, register))
            for code, amount in group.subtotals.items():
                head.addChild(
                    self._sum(tr("Subtotal {category}", category=title), code, amount, register)
                )
            self.table.addTopLevelItem(head)
            # Only an item in the tree spans its row.
            head.setFirstColumnSpanned(True)
            head.setExpanded(True)
        if budget.without_cost:
            head = self._heading(
                tr("Without cost ({count})", count=len(budget.without_cost)), muted=True
            )
            for row in budget.without_cost:
                head.addChild(self._row(row, register))
            self.table.addTopLevelItem(head)
            # Only an item in the tree spans its row.
            head.setFirstColumnSpanned(True)
            head.setExpanded(True)
        for code, amount in budget.totals.items():
            self.table.addTopLevelItem(
                self._sum(tr("Total {currency}", currency=code), code, amount, register)
            )

    def _heading(self, title: str, muted: bool = False) -> QTreeWidgetItem:
        item = QTreeWidgetItem([title])
        font = QFont(self.table.font())
        font.setBold(True)
        item.setFont(ITEM, font)
        if muted:
            item.setForeground(ITEM, self.palette().color(QPalette.ColorRole.PlaceholderText))
        return item

    def _row(self, row: BudgetRow, register: CurrencyRegister | None) -> QTreeWidgetItem:
        cost = amount = ""
        if register is not None and row.cost is not None and row.currency is not None:
            cost = register.cost_text(row.cost, row.currency)
        if register is not None and row.amount is not None and row.currency is not None:
            amount = register.cost_text(row.amount, row.currency)
        item = QTreeWidgetItem(
            [
                row.item,
                row.material,
                row.supplier,
                tr("each"),
                str(row.quantity),
                cost,
                amount,
            ]
        )
        if row.item != row.block:
            item.setToolTip(ITEM, tr("Block {name}", name=row.block))
        self._align(item)
        return item

    def _sum(
        self, title: str, code: str, amount: str, register: CurrencyRegister | None
    ) -> QTreeWidgetItem:
        text = register.cost_text(amount, code) if register is not None else amount
        item = QTreeWidgetItem([title, "", "", "", "", "", text])
        font = QFont(self.table.font())
        font.setBold(True)
        item.setFont(ITEM, font)
        item.setFont(AMOUNT, font)
        self._align(item)
        return item

    @staticmethod
    def _align(item: QTreeWidgetItem) -> None:
        for column in NUMBERS:
            item.setTextAlignment(
                column, Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter
            )

    def rows(self) -> list[list[str]]:
        """Every row shown, children after their parent, as text: for tests."""
        shown: list[list[str]] = []

        def add(item: QTreeWidgetItem) -> None:
            shown.append([item.text(c) for c in range(self.table.columnCount())])
            for index in range(item.childCount()):
                child = item.child(index)
                if child is not None:
                    add(child)

        for index in range(self.table.topLevelItemCount()):
            top = self.table.topLevelItem(index)
            if top is not None:
                add(top)
        return shown

    def showEvent(self, event: QShowEvent) -> None:  # noqa: N802 (Qt API)
        # Opened again: catch up with what changed while it was closed.
        super().showEvent(event)
        window = self.parentWidget()
        if not self._sized and isinstance(window, QMainWindow) and not self.isFloating():
            # Docked while closed, it would open a few pixels high.
            self._sized = True
            window.resizeDocks([self], [FIRST_HEIGHT], Qt.Orientation.Vertical)
        if self._stale:
            self.refresh()
