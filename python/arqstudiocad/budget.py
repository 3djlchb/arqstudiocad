"""The budget from the drawing (ADR 0028): each block's quantity in model
space times its unit cost, grouped by category, with subtotals and totals
per currency, never added across currencies. Blocks placed without a cost
are listed apart and never summed.

This module reads the document and builds the budget; the Budget panel
shows it and the export writes it.
"""

from collections.abc import Sequence
from dataclasses import dataclass, field
from decimal import Decimal

from arqstudiocad.core import CadBlockData, CadDocument
from arqstudiocad.currencies import CurrencyRegister, register_of


@dataclass(frozen=True)
class BudgetRow:
    """One block of the budget. ``item`` is its type, or its name when it
    has none; ``cost`` and ``amount`` are decimal text in ``currency``, or
    None for a block without a cost."""

    block: str
    item: str
    material: str
    supplier: str
    quantity: int
    cost: str | None = None
    currency: str | None = None
    amount: str | None = None


@dataclass(frozen=True)
class BudgetGroup:
    """A category's rows and its subtotal per currency; ``category`` is
    empty for the blocks without one. Categories that differ only in case
    are one."""

    category: str
    rows: list[BudgetRow]
    subtotals: dict[str, str]


@dataclass(frozen=True)
class Budget:
    """The groups, by category, then the blocks without a cost and the
    totals per currency, the principal currency first."""

    groups: list[BudgetGroup] = field(default_factory=list)
    without_cost: list[BudgetRow] = field(default_factory=list)
    totals: dict[str, str] = field(default_factory=dict)
    register: CurrencyRegister | None = None

    @property
    def empty(self) -> bool:
        return not self.groups and not self.without_cost


def budget_of(document: CadDocument, entity_ids: Sequence[int] | None = None) -> Budget:
    """``document``'s budget: of the whole drawing, or with ``entity_ids``
    of those references and what they hold only (ADR 0028 D5)."""
    quantities = (
        document.block_quantities
        if entity_ids is None
        else document.block_quantities_of(list(entity_ids))
    )
    register = register_of(document)
    data = document.blocks_data
    priced: dict[str, list[BudgetRow]] = {}
    without_cost: list[BudgetRow] = []
    for block, quantity in quantities.items():
        found = data.get(block, CadBlockData())
        # A cost always has its currency (ADR 0027 D2).
        if found.cost is None:
            without_cost.append(_row(block, found, quantity))
            continue
        # The core writes a cost with its currency's decimals: so is its amount.
        decimals = len(found.cost.partition(".")[2])
        amount = _text(Decimal(found.cost) * quantity, decimals)
        row = _row(block, found, quantity, amount)
        priced.setdefault(found.category.strip().casefold(), []).append(row)
    groups = [
        BudgetGroup(_spelling(rows, data), sorted(rows, key=_order), _sums(register, rows))
        for _, rows in sorted(priced.items(), key=lambda pair: _category_order(pair[0]))
    ]
    every = [row for group in groups for row in group.rows]
    return Budget(groups, sorted(without_cost, key=_order), _sums(register, every), register)


def _row(block: str, data: CadBlockData, quantity: int, amount: str | None = None) -> BudgetRow:
    return BudgetRow(
        block=block,
        item=data.type.strip() or block,
        material=data.material,
        supplier=data.supplier,
        quantity=quantity,
        cost=data.cost,
        currency=data.currency,
        amount=amount,
    )


def _spelling(rows: list[BudgetRow], data: dict[str, CadBlockData]) -> str:
    """A category as its rows spell it, ignoring case: as the first block
    by name spells it."""
    first = min(rows, key=lambda row: row.block.casefold())
    return data[first.block].category.strip()


def _text(amount: Decimal, decimals: int) -> str:
    return f"{amount:.{decimals}f}"


def _sums(register: CurrencyRegister, rows: list[BudgetRow]) -> dict[str, str]:
    """The amounts of ``rows``, each with a cost, summed per currency, the
    principal first, then by code."""
    sums: dict[str, Decimal] = {}
    decimals: dict[str, int] = {}
    for row in rows:
        assert row.amount is not None and row.currency is not None
        sums[row.currency] = sums.get(row.currency, Decimal(0)) + Decimal(row.amount)
        places = len(row.amount.partition(".")[2])
        decimals[row.currency] = max(decimals.get(row.currency, 0), places)
    codes = sorted(sums, key=lambda code: (code != register.principal, code))
    return {code: _text(sums[code], decimals[code]) for code in codes}


def _order(row: BudgetRow) -> tuple[str, str]:
    """Rows by type, then by block name, ignoring case."""
    return (row.item.casefold(), row.block.casefold())


def _category_order(category: str) -> tuple[bool, str]:
    """Categories by name, ignoring case; the blocks without one last."""
    return (not category, category)
