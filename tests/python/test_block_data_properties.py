"""The Properties panel's Block data group (ADR 0027 step 5): with
references of one block selected, the definition's budget data is shown and
edited in place, each field optional, one undo step each; the cost is
typed in its currency's format, and the data can be cleared, confirmed
first."""

import pytest
from PySide6.QtWidgets import QComboBox, QMessageBox
from pytestqt.qtbot import QtBot
from test_panels import property_item
from ui_fakes import QuietUi

from arqstudiocad.core import CadBlockData, CadCurrency, CadDocument
from arqstudiocad.events import CommandMessage
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.properties_panel import block_data_section, selected_block
from arqstudiocad.ui.property_grid import Property

USD = CadCurrency("USD", "US dollar", "$", 2, True)
PEN = CadCurrency("PEN", "Peruvian sol", "S/", 2, True, ".", ",")
TABLE = CadBlockData(category="Mobiliario", type="Mesa", cost="1250.50", currency="PEN")


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


def furnished(window: MainWindow) -> tuple[CadDocument, list[int]]:
    """MESA placed twice with data; SILLA placed once without."""
    window.runner.execute("NEW")
    document = window.session.active
    assert document is not None
    euro = CadCurrency("EUR", "Euro", "€", 2, False)
    document.set_currencies([USD, PEN, euro], "USD")  # EUR not active
    placed = []
    for index, name in enumerate(("MESA", "SILLA")):
        line, _ = document.add_line((index, 0), (index, 1))
        reference, _ = document.create_block(name, (0, 0), [line])
        placed.append(reference)
    second, _ = document.insert_block("MESA", (5, 5))
    placed.append(second)
    document.set_block_data("MESA", TABLE)
    window.session.report(document, document.set_block_data("MESA", TABLE))
    return document, placed


def rows(window: MainWindow) -> dict[str, str]:
    return dict(window.properties_panel.rows())


def sections(window: MainWindow) -> list[str]:
    return [title for title, _ in window.properties_panel.grid.sections()]


def test_the_group_shows_for_references_of_one_block(window: MainWindow) -> None:
    document, placed = furnished(window)
    window.session.select(document, [placed[0], placed[2]])
    assert sections(window)[1] == "Block data", "after General"
    shown = rows(window)
    assert shown["Applies to"] == "Every reference of MESA"
    assert (shown["Category"], shown["Type"]) == ("Mobiliario", "Mesa")
    assert (shown["Material"], shown["Supplier"]) == ("", "")
    assert shown["Cost"] == "1.250,50", "PEN's own format"
    assert shown["Currency"] == "PEN (S/)"
    assert "Clear block data" in shown
    # Two blocks, or a line, show no group.
    window.session.clear_selection(document)
    window.session.select(document, [placed[1]])
    assert "Block data" in sections(window), "SILLA has none yet, but can"
    assert "Clear block data" not in rows(window), "nothing to clear"
    window.session.clear_selection(document)
    window.session.select(document, [placed[0], placed[1]])
    assert "Block data" not in sections(window)
    line, _ = document.add_line((9, 9), (10, 9))
    window.session.clear_selection(document)
    window.session.select(document, [line])
    assert "Block data" not in sections(window)


def test_each_field_is_edited_in_one_undo_step(window: MainWindow) -> None:
    document, placed = furnished(window)
    window.session.select(document, [placed[0]])
    property_item(window, "Material").setText(1, " Roble ")
    assert document.block_data("MESA") == CadBlockData(
        category="Mobiliario", type="Mesa", material="Roble", cost="1250.50", currency="PEN"
    )
    assert document.undo_history[-1] == "BLOCKDATA"
    property_item(window, "Category").setText(1, "")
    data = document.block_data("MESA")
    assert data is not None and data.category == "", "optional"
    property_item(window, "Cost").setText(1, "2.000")
    data = document.block_data("MESA")
    assert data is not None and (data.cost, data.currency) == ("2000.00", "PEN")
    assert rows(window)["Cost"] == "2.000,00"
    window.runner.execute("UNDO")
    data = document.block_data("MESA")
    assert data is not None and data.cost == "1250.50"


def test_a_cost_typed_wrong_is_said_and_changes_nothing(window: MainWindow) -> None:
    document, placed = furnished(window)
    said: list[str] = []
    window.session.bus.subscribe(CommandMessage, lambda e: said.append(e.text))
    window.session.select(document, [placed[0]])
    steps = len(document.undo_history)
    property_item(window, "Cost").setText(1, "1250.50")  # PEN writes 1.250,50
    assert said[-1] == "1250.50 is not an amount: write it as 1.250,50."
    assert document.block_data("MESA") == TABLE and len(document.undo_history) == steps
    assert rows(window)["Cost"] == "1.250,50", "shown as it was"
    property_item(window, "Cost").setText(1, "")
    data = document.block_data("MESA")
    assert data is not None and data.cost is None and data.currency is None, "no cost"


def test_the_currency_moves_the_cost_or_waits_for_one(window: MainWindow) -> None:
    document, placed = furnished(window)
    window.session.select(document, [placed[0]])
    combo = window.properties_panel.grid.tree.itemWidget(property_item(window, "Currency"), 1)
    assert isinstance(combo, QComboBox)
    assert [combo.itemData(i) for i in range(combo.count())] == ["USD", "PEN"]
    combo.setCurrentIndex(0)
    data = document.block_data("MESA")
    assert data is not None and (data.cost, data.currency) == ("1250.50", "USD")
    assert rows(window)["Cost"] == "1,250.50", "USD's format"
    # SILLA has no cost: the currency chosen waits, then the cost uses it.
    window.session.clear_selection(document)
    window.session.select(document, [placed[1]])
    assert rows(window)["Currency"] == "USD ($)", "the principal"
    combo = window.properties_panel.grid.tree.itemWidget(property_item(window, "Currency"), 1)
    assert isinstance(combo, QComboBox)
    combo.setCurrentIndex(1)
    assert document.block_data("SILLA") is None, "nothing written yet"
    assert rows(window)["Currency"] == "PEN (S/)"
    property_item(window, "Cost").setText(1, "80")
    assert document.block_data("SILLA") == CadBlockData(cost="80.00", currency="PEN")


def test_clearing_asks_first(window: MainWindow, monkeypatch: pytest.MonkeyPatch) -> None:
    document, placed = furnished(window)
    window.session.select(document, [placed[0]])
    answers = [QMessageBox.StandardButton.No, QMessageBox.StandardButton.Yes]
    defaults: list[bool] = []

    def answer(box: QMessageBox) -> int:
        defaults.append(box.defaultButton() is box.button(QMessageBox.StandardButton.No))
        return answers.pop(0).value

    monkeypatch.setattr(QMessageBox, "exec", answer)
    grid = window.properties_panel.grid
    grid.tree.itemDoubleClicked.emit(property_item(window, "Clear block data"), 1)
    assert document.block_data("MESA") == TABLE, "No"
    grid.tree.itemDoubleClicked.emit(property_item(window, "Clear block data"), 1)
    assert document.block_data("MESA") is None and defaults == [True, True]
    assert document.undo_history[-1] == "BLOCKDATA"
    assert "Clear block data" not in rows(window)


def test_the_group_in_spanish(window: MainWindow) -> None:
    document, placed = furnished(window)
    set_language("es")
    window.properties_panel.retranslate()
    window.session.select(document, [placed[0]])
    assert "Datos de bloque" in sections(window)
    shown = rows(window)
    assert shown["Se aplica a"] == "Todas las referencias de MESA"
    assert shown["Categoría"] == "Mobiliario"


def test_the_section_and_the_block_of_a_selection() -> None:
    from arqstudiocad.core import CadApplication

    document = CadApplication().create_document("Plano")
    line, _ = document.add_line((0, 0), (1, 0))
    reference, _ = document.create_block("MESA", (0, 0), [line])
    assert selected_block(document, document.get_entities([reference])) == "MESA"
    other, _ = document.add_line((0, 0), (2, 0))
    assert selected_block(document, document.get_entities([reference, other])) is None
    section = block_data_section(document, "MESA", currency="USD")
    assert section.title == "Block data"
    names = [row.name if isinstance(row, Property) else row[0] for row in section.rows]
    assert names == ["Applies to", "Category", "Type", "Material", "Supplier", "Cost", "Currency"]
