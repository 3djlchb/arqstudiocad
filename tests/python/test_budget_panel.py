"""The budget (ADR 0028 step 3): the model built from the drawing, grouped
by category with subtotals per currency, blocks without a cost apart, the
whole drawing or the selection; and the Budget panel showing it, opened by
BUDGET (PRESUPUESTO) or its button in the Palettes group."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings, Qt
from PySide6.QtWidgets import QTabBar
from pytestqt.qtbot import QtBot
from test_blocks_palette import palette_button
from ui_fakes import QuietUi

from arqstudiocad.budget import BudgetRow, budget_of
from arqstudiocad.core import CadApplication, CadBlockData, CadCurrency, CadDocument
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow

USD = CadCurrency("USD", "US dollar", "$", 2, True)
EUR = CadCurrency("EUR", "Euro", "€", 2, True, ".", ",")


def furnish(document: CadDocument) -> tuple[int, int]:
    """A dining set, JUEGO (four SILLA and a MESA), placed twice; a SILLA
    alone; three LAMPARA, a SOFA, an INODORO, two PUERTA; ROTULO placed
    nowhere. Returns a JUEGO's id and the lone SILLA's."""
    document.set_currencies([USD, EUR], "USD")

    def block(name: str, x: float) -> int:
        line, _ = document.add_line((x, 0), (x, 1))
        return document.create_block(name, (x, 0), [line])[0]

    chairs = [block("SILLA", 0)] + [document.insert_block("SILLA", (x, 0))[0] for x in (1, 2, 3)]
    first, _ = document.create_block("JUEGO", (0, 0), [*chairs, block("MESA", 4)])
    document.insert_block("JUEGO", (0, 10))
    alone, _ = document.insert_block("SILLA", (0, 20))
    block("LAMPARA", 5)
    document.insert_block("LAMPARA", (5, 5))
    document.insert_block("LAMPARA", (5, 6))
    block("SOFA", 6)
    block("INODORO", 7)
    block("PUERTA", 8)
    document.insert_block("PUERTA", (8, 5))
    title = block("ROTULO", 9)
    document.delete_entities([title])
    for name, data in {
        "SILLA": CadBlockData(
            category="MOBILIARIO",
            type="Silla",
            material="Madera",
            supplier="Muebles SA",
            cost="50.00",
            currency="USD",
        ),
        "MESA": CadBlockData(
            category="Mobiliario", type="Mesa", material="Madera", cost="120.5", currency="USD"
        ),
        "SOFA": CadBlockData(category="mobiliario", type="Asiento", cost="1500", currency="EUR"),
        "LAMPARA": CadBlockData(cost="80", currency="EUR"),
        "INODORO": CadBlockData(category="Sanitarios", type="Inodoro"),
        "ROTULO": CadBlockData(cost="10", currency="USD"),
    }.items():
        document.set_block_data(name, data)
    return first, alone


def test_the_budget_groups_by_category_and_never_mixes_currencies() -> None:
    document = CadApplication().create_document("a")
    furnish(document)
    budget = budget_of(document)
    [furniture, rest] = budget.groups
    assert furniture.category == "Mobiliario", "one group ignoring case, as MESA spells it"
    assert furniture.rows == [
        BudgetRow("SOFA", "Asiento", "", "", 1, "1500.00", "EUR", "1500.00"),
        BudgetRow("MESA", "Mesa", "Madera", "", 2, "120.50", "USD", "241.00"),
        BudgetRow("SILLA", "Silla", "Madera", "Muebles SA", 9, "50.00", "USD", "450.00"),
    ], "by type, then by name; nested chairs multiplied"
    assert list(furniture.subtotals.items()) == [("USD", "691.00"), ("EUR", "1500.00")]
    assert rest.category == "", "no category, last"
    assert rest.rows == [BudgetRow("LAMPARA", "LAMPARA", "", "", 3, "80.00", "EUR", "240.00")]
    assert rest.subtotals == {"EUR": "240.00"}
    assert [(r.block, r.item, r.quantity, r.cost) for r in budget.without_cost] == [
        ("INODORO", "Inodoro", 1, None),
        ("JUEGO", "JUEGO", 2, None),
        ("PUERTA", "PUERTA", 2, None),
    ], "with data but no cost, or no data; ROTULO is placed nowhere"
    assert list(budget.totals.items()) == [("USD", "691.00"), ("EUR", "1740.00")], (
        "the principal first"
    )


def test_selection_only_counts_the_chosen_references() -> None:
    document = CadApplication().create_document("a")
    first, alone = furnish(document)
    budget = budget_of(document, [first, alone])
    [furniture] = budget.groups
    assert [(r.block, r.quantity, r.amount) for r in furniture.rows] == [
        ("MESA", 1, "120.50"),
        ("SILLA", 5, "250.00"),
    ]
    assert [r.block for r in budget.without_cost] == ["JUEGO"]
    assert budget.totals == {"USD": "370.50"}
    assert budget_of(document, []).empty


def test_amounts_keep_their_currencys_decimals() -> None:
    document = CadApplication().create_document("a")
    yen = CadCurrency("JPY", "Japanese yen", "¥", 0, True)
    document.set_currencies([USD, yen], "USD")
    line, _ = document.add_line((0, 0), (1, 0))
    document.create_block("VIGA", (0, 0), [line])
    document.insert_block("VIGA", (5, 0))
    document.set_block_data("VIGA", CadBlockData(cost="1500", currency="JPY"))
    budget = budget_of(document)
    assert [(r.cost, r.amount) for r in budget.groups[0].rows] == [("1500", "3000")]
    assert budget.totals == {"JPY": "3000"}


def test_an_empty_drawing_has_an_empty_budget() -> None:
    budget = budget_of(CadApplication().create_document("a"))
    assert budget.empty and budget.totals == {}


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    ui.panel_for = w.show_panel
    qtbot.addWidget(w)
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    return w


def opened(window: MainWindow) -> CadDocument:
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    window.runner.execute("BUDGET")
    return document


def test_budget_opens_the_panel_as_a_tab_among_the_palettes(window: MainWindow) -> None:
    panel = window.budget_panel
    assert panel.isHidden(), "closed at first"
    assert not palette_button(window, "BUDGET").isChecked()
    # A palette beside Layers, Properties and Blocks (maintainer, 2026-11-19),
    # no longer across the bottom by the command window. Qt lists only open
    # docks as tabified.
    assert window.dockWidgetArea(panel) == Qt.DockWidgetArea.RightDockWidgetArea
    assert window.dockWidgetArea(window.command_window) == Qt.DockWidgetArea.BottomDockWidgetArea
    window.runner.execute("BUDGET")
    assert panel.isVisible()
    (bar,) = [
        bar
        for bar in window.findChildren(QTabBar)
        if panel.windowTitle() in [bar.tabText(i) for i in range(bar.count())]
    ]
    assert bar.tabText(bar.currentIndex()) == panel.windowTitle(), "its tab in front"
    assert window.layers_panel.windowTitle() in [bar.tabText(i) for i in range(bar.count())]
    assert panel in window.tabifiedDockWidgets(window.layers_panel)
    assert palette_button(window, "BUDGET").isChecked()
    assert panel.footer.text() == "No drawing is open."
    assert not panel.table.isEnabled()
    window.runner.execute("NEW")
    assert panel.table.isEnabled()
    assert panel.footer.text() == "No block is placed in model space."


def _window_with(qtbot: QtBot, settings: QSettings) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(settings, ui=ui)
    ui.panel_for = w.show_panel
    qtbot.addWidget(w)
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    return w


def _to_the_bottom(window: MainWindow) -> None:
    """The Budget panel where it was until 2026-11-19: across the bottom,
    above the command window, open."""
    panel = window.budget_panel
    window.addDockWidget(Qt.DockWidgetArea.BottomDockWidgetArea, panel)
    window.splitDockWidget(panel, window.command_window, Qt.Orientation.Vertical)
    panel.show()
    assert window.dockWidgetArea(panel) == Qt.DockWidgetArea.BottomDockWidgetArea


def test_an_arrangement_saved_with_the_panel_at_the_bottom_moves_it_once(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    first = _window_with(qtbot, settings)
    _to_the_bottom(first)
    first.close()
    second = _window_with(qtbot, settings)
    panel = second.budget_panel
    assert second.dockWidgetArea(panel) == Qt.DockWidgetArea.RightDockWidgetArea
    assert panel in second.tabifiedDockWidgets(second.layers_panel)
    assert not panel.isHidden(), "open as it was"
    assert second.dockWidgetArea(second.command_window) == Qt.DockWidgetArea.BottomDockWidgetArea
    # Put back at the bottom by hand, it stays there.
    _to_the_bottom(second)
    second.close()
    third = _window_with(qtbot, settings)
    assert third.dockWidgetArea(third.budget_panel) == Qt.DockWidgetArea.BottomDockWidgetArea
    third.close()


def test_a_closed_panel_at_the_bottom_moves_closed(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    first = _window_with(qtbot, settings)
    _to_the_bottom(first)
    first.budget_panel.hide()
    first.close()
    second = _window_with(qtbot, settings)
    assert second.dockWidgetArea(second.budget_panel) == Qt.DockWidgetArea.RightDockWidgetArea
    assert second.budget_panel.isHidden(), "closed as it was"
    second.runner.execute("BUDGET")
    assert second.budget_panel.isVisible()
    assert second.budget_panel in second.tabifiedDockWidgets(second.layers_panel)


def test_the_panel_shows_groups_subtotals_and_totals(window: MainWindow) -> None:
    document = opened(window)
    furnish(document)
    panel = window.budget_panel
    panel.refresh()
    assert panel.rows() == [
        ["Mobiliario", "", "", "", "", "", ""],
        ["Asiento", "", "", "each", "1", "€ 1.500,00", "€ 1.500,00"],
        ["Mesa", "Madera", "", "each", "2", "$ 120.50", "$ 241.00"],
        ["Silla", "Madera", "Muebles SA", "each", "9", "$ 50.00", "$ 450.00"],
        ["Subtotal Mobiliario", "", "", "", "", "", "$ 691.00"],
        ["Subtotal Mobiliario", "", "", "", "", "", "€ 1.500,00"],
        ["No category", "", "", "", "", "", ""],
        ["LAMPARA", "", "", "each", "3", "€ 80,00", "€ 240,00"],
        ["Subtotal No category", "", "", "", "", "", "€ 240,00"],
        ["Without cost (3)", "", "", "", "", "", ""],
        ["Inodoro", "", "", "each", "1", "", ""],
        ["JUEGO", "", "", "each", "2", "", ""],
        ["PUERTA", "", "", "each", "2", "", ""],
        ["Total USD", "", "", "", "", "", "$ 691.00"],
        ["Total EUR", "", "", "", "", "", "€ 1.740,00"],
    ]
    assert panel.footer.text() == "3 blocks have no cost: they are not summed."
    head = panel.table.topLevelItem(0)
    assert head is not None and head.isExpanded() and head.isFirstColumnSpanned()
    sofa = head.child(0)
    assert sofa is not None and sofa.toolTip(0) == "Block SOFA"


def test_the_panel_follows_the_drawing_and_the_selection(window: MainWindow) -> None:
    document = opened(window)
    first, alone = furnish(document)
    panel = window.budget_panel
    # A change reported: the door gets a cost and leaves Without cost.
    door = CadBlockData(category="Carpintería", cost="200", currency="USD")
    window.session.report(document, document.set_block_data("PUERTA", door))
    assert panel.rows()[0] == ["Carpintería", "", "", "", "", "", ""]
    assert panel.footer.text() == "2 blocks have no cost: they are not summed."
    panel.selection_only.setChecked(True)
    assert panel.footer.text() == "No block among the selected objects."
    window.session.select(document, [first])
    assert [r.block for r in panel.budget.groups[0].rows] == ["MESA", "SILLA"], "no sofa"
    assert panel.footer.text() == "1 block has no cost: it is not summed."
    window.session.select(document, [alone])
    assert panel.budget.groups[0].rows[1].quantity == 5
    panel.selection_only.setChecked(False)
    assert panel.budget.totals == {"USD": "1091.00", "EUR": "1740.00"}


def test_a_closed_panel_catches_up_when_opened(window: MainWindow) -> None:
    document = opened(window)
    panel = window.budget_panel
    window.close_panel(panel)
    furnish(document)
    lamp = CadBlockData(cost="90", currency="EUR")
    window.session.report(document, document.set_block_data("LAMPARA", lamp))
    assert panel.budget.empty, "not read while closed"
    window.runner.execute("BUDGET")
    assert panel.budget.totals == {"USD": "691.00", "EUR": "1770.00"}


def test_the_panel_in_spanish(window: MainWindow) -> None:
    try:
        set_language("es")
        window.retranslate()
        document = opened(window)
        furnish(document)
        window.runner.execute("PRESUPUESTO")
        panel = window.budget_panel
        assert panel.windowTitle() == "Presupuesto"
        header = panel.table.headerItem()
        assert [header.text(c) for c in range(7)] == [
            "Partida",
            "Material",
            "Proveedor",
            "Unidad",
            "Cantidad",
            "P. unitario",
            "Parcial",
        ]
        panel.refresh()
        assert ["LAMPARA", "", "", "und", "3", "€ 80,00", "€ 240,00"] in panel.rows()
        assert ["Sin categoría", "", "", "", "", "", ""] in panel.rows()
        assert panel.footer.text() == "3 bloques no tienen costo: no se suman."
    finally:
        set_language("en")
