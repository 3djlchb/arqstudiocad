"""Editing a block's data for budgets (ADR 0027 step 4a, maintainer
2026-10-27): the Block Data dialog, BLOCKDATA and -BLOCKDATA (the block of
the references selected, one picked, or one named), the Blocks palette's
Block data… and its Budget data section. Costs are typed and shown in
their currency's number format (D3a)."""

import pytest
from PySide6.QtWidgets import QDialog, QDialogButtonBox, QWidget
from pytestqt.qtbot import QtBot
from test_blocks import Harness
from test_blocks_context_menu import actions
from test_blocks_palette import drawing_with_blocks, opened
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadBlockData, CadCurrency, CadDocument
from arqstudiocad.currencies import register_of
from arqstudiocad.i18n import set_language
from arqstudiocad.ui import block_data_dialog
from arqstudiocad.ui.block_data_dialog import BlockDataDialog, suggestions
from arqstudiocad.ui.blocks_panel import budget_section
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.qt_services import QtUiServices

USD = CadCurrency("USD", "US dollar", "$", 2, True)
PEN = CadCurrency("PEN", "Peruvian sol", "S/", 2, True, ".", ",")
EUR = CadCurrency("EUR", "Euro", "€", 2, False, ".", ",")
TABLE = CadBlockData(category="Mobiliario", type="Mesa", cost="1250.5", currency="PEN")


def furnish(document: CadDocument) -> list[int]:
    """Blocks MESA, SILLA and LAMPARA, each placed once; PEN is written
    1.250,50 and EUR is not active. Returns the references."""
    document.set_currencies([USD, PEN, EUR], "USD")
    placed = []
    for index, name in enumerate(("MESA", "SILLA", "LAMPARA")):
        line, _ = document.add_line((index, 0), (index, 1))
        reference, _ = document.create_block(name, (0, 0), [line])
        placed.append(reference)
    document.set_block_data("MESA", TABLE)
    document.set_block_data("SILLA", CadBlockData(category="mobiliario", type="Silla"))
    document.set_block_data("LAMPARA", CadBlockData(category="Iluminación"))
    return placed


def drawing() -> CadDocument:
    document = CadApplication().create_document("Plano")
    furnish(document)
    return document


# The dialog


def dialog(qtbot: QtBot, document: CadDocument, name: str) -> BlockDataDialog:
    shown = BlockDataDialog(document, name)
    qtbot.addWidget(shown)
    return shown


def ok(shown: BlockDataDialog) -> bool:
    return shown.buttons.button(QDialogButtonBox.StandardButton.Ok).isEnabled()


def test_suggestions_are_the_values_the_drawing_uses() -> None:
    document = drawing()
    # Once ignoring case, as MESA spells it (before SILLA by name).
    assert suggestions(document, "category") == ["Iluminación", "Mobiliario"]
    assert suggestions(document, "type") == ["Mesa", "Silla"]
    # Sorted by value, not by block: ARMARIO comes first, Zócalo last.
    line, _ = document.add_line((9, 0), (9, 1))
    document.create_block("ARMARIO", (0, 0), [line])
    document.set_block_data("ARMARIO", CadBlockData(category="Zócalo"))
    assert suggestions(document, "category") == ["Iluminación", "Mobiliario", "Zócalo"]


def test_the_dialog_shows_the_blocks_data_in_its_currencys_format(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "MESA")
    assert shown.windowTitle() == "Block data: MESA"
    assert shown.category.currentText() == "Mobiliario"
    assert [shown.category.itemText(i) for i in range(2)] == ["Iluminación", "Mobiliario"]
    assert shown.kind.currentText() == "Mesa"
    assert shown.cost.text() == "1.250,50", "PEN's own format"
    assert shown.currency.currentData() == "PEN"
    assert [shown.currency.itemData(i) for i in range(2)] == ["USD", "PEN"], "principal first"
    assert shown.currency.count() == 2, "EUR is not active"
    assert shown.data() == CadBlockData(
        category="Mobiliario", type="Mesa", cost="1250.50", currency="PEN"
    )


def test_a_new_cost_proposes_the_principal_currency(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "LAMPARA")
    assert shown.cost.text() == "" and shown.currency.currentData() == "USD"
    assert shown.data() == CadBlockData(category="Iluminación"), "no cost"
    shown.cost.setText("1,234.5")
    shown.material.setText(" Bronce ")
    shown.category.setCurrentText(" Iluminación ")
    shown.supplier.setText("Luces SAC")
    assert ok(shown)
    assert shown.data() == CadBlockData(
        category="Iluminación",
        material="Bronce",
        supplier="Luces SAC",
        cost="1234.5",
        currency="USD",
    )


def test_what_cannot_be_written_disables_ok(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "MESA")
    shown.cost.setText("1250.50")  # PEN writes 1.250,50
    assert not ok(shown) and shown.data() is None
    assert shown.problem.text() == "1250.50 is not an amount: write it as 1.250,50."
    shown.cost.setText("12,345")
    assert shown.problem.text() == "A cost in PEN has at most 2 decimals."
    assert not ok(shown)
    shown.cost.setText("12,34")
    assert ok(shown) and shown.problem.text() == ""
    shown.cost.setText("")
    assert ok(shown) and shown.data() == CadBlockData(category="Mobiliario", type="Mesa")


def test_changing_the_currency_rewrites_the_amount(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "MESA")
    shown.currency.setCurrentIndex(0)  # USD: the drawing's 1,250.50
    assert shown.cost.text() == "1,250.50"
    assert shown.data() == CadBlockData(
        category="Mobiliario", type="Mesa", cost="1250.50", currency="USD"
    )
    shown.cost.setText("nada")
    shown.currency.setCurrentIndex(1)
    assert shown.cost.text() == "nada", "what is not an amount stays as typed"
    # There and back: each change reads the amount in the format it was in.
    shown.cost.setText("1.250,50")
    shown.currency.setCurrentIndex(0)
    shown.currency.setCurrentIndex(1)
    assert shown.cost.text() == "1.250,50"


def test_a_cost_in_a_currency_no_longer_active_must_change(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch
) -> None:
    document = drawing()
    document.set_currencies([USD, PEN, CadCurrency("BOB", "Boliviano")], "USD")
    document.set_block_data("SILLA", CadBlockData(cost="80", currency="BOB"))
    # As a drawing edited in another program may hold it: BOB, used, inactive.
    held = register_of(document)
    inactive = CadCurrency("BOB", "Boliviano", active=False)
    monkeypatch.setattr(
        block_data_dialog,
        "register_of",
        lambda d: held._replace(currencies=[USD, PEN, inactive]),
    )
    shown = dialog(qtbot, document, "SILLA")
    assert shown.currency.currentText() == "BOB (not active)"
    assert shown.problem.text() == (
        "BOB is not an active currency: activate it in Currencies, or choose another."
    )
    assert not ok(shown)
    shown.currency.setCurrentIndex(0)
    assert ok(shown) and shown.data() == CadBlockData(cost="80.00", currency="USD")


def test_the_dialog_in_spanish(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, drawing(), "MESA")
    assert shown.windowTitle() == "Datos de bloque: MESA"
    assert shown.buttons.button(QDialogButtonBox.StandardButton.Ok).text() == "Aceptar"
    shown.cost.setText("x")
    assert shown.problem.text() == "x no es un importe: escríbalo como 1.250,50."


def test_the_service_shows_the_dialog(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    services = QtUiServices(parent)
    monkeypatch.setattr(block_data_dialog, "edit_block_data", lambda document, name, parent: TABLE)
    assert services.edit_block_data(drawing(), "MESA") == TABLE
    monkeypatch.undo()
    monkeypatch.setattr(BlockDataDialog, "exec", lambda self: QDialog.DialogCode.Rejected.value)
    assert block_data_dialog.edit_block_data(drawing(), "MESA") is None
    monkeypatch.setattr(BlockDataDialog, "exec", lambda self: QDialog.DialogCode.Accepted.value)
    assert block_data_dialog.edit_block_data(drawing(), "LAMPARA") == CadBlockData(
        category="Iluminación"
    )


# BLOCKDATA and -BLOCKDATA


def harness(ui: QuietUi | None = None) -> tuple[Harness, list[int]]:
    h = Harness(ui)
    placed = furnish(h.document)
    h.steps = len(h.document.undo_history)
    return h, placed


def test_blockdata_edits_the_block_of_the_selected_references() -> None:
    ui = QuietUi(block_data_answer=CadBlockData(category="Comedor", cost="99", currency="USD"))
    h, placed = harness(ui)
    h.session.select(h.document, [placed[0]])
    h.run("BLOCKDATA")
    assert ui.block_data_shown == ["MESA"]
    assert h.document.block_data("MESA") == CadBlockData(
        category="Comedor", cost="99.00", currency="USD"
    )
    assert h.new_steps() == ["BLOCKDATA"] and h.runner.active is None
    assert h.messages()[-1] == "Block data of MESA saved."


def test_blockdata_asks_for_a_reference_or_a_name() -> None:
    ui = QuietUi(block_data_answer=CadBlockData(material="Pino"))
    h, placed = harness(ui)
    h.session.select(h.document, placed[:2])  # two blocks
    h.run("BLOCKDATA")
    assert h.prompt().text == "Select a block reference [Name(N)]:"
    h.run("")
    assert h.messages()[-1] == "Select references of one block."
    assert h.session.selection(h.document) == ()
    h.run("N", "nada")
    assert h.messages()[-1] == "There is no such block."
    h.run("N", "silla")
    assert ui.block_data_shown == ["SILLA"]
    assert h.document.block_data("SILLA") == CadBlockData(material="Pino")
    # Picked: Enter with one reference selected.
    h.run("BLOCKDATA")
    h.session.select(h.document, [placed[2]])
    h.run("")
    assert ui.block_data_shown[-1] == "LAMPARA"


def test_blockdata_cancelled_unchanged_or_refused_changes_nothing() -> None:
    ui = QuietUi()
    h, placed = harness(ui)
    h.session.select(h.document, [placed[0]])
    before = h.messages()
    h.run("BLOCKDATA")
    assert h.messages() == before, "cancelled quietly"
    h.session.deselect(h.document, placed)
    h.run("BLOCKDATA", "")
    assert h.runner.active is None, "Enter with nothing selected ends it"
    ui.block_data_answer = TABLE
    h.session.select(h.document, [placed[0]])
    h.run("BLOCKDATA")
    assert "Block data of MESA saved." not in h.messages(), "the same data"
    ui.block_data_answer = CadBlockData(cost="5", currency="EUR")
    h.session.select(h.document, [placed[0]])
    h.run("BLOCKDATA")
    assert h.messages()[-1] == (
        "The block data was not changed: EUR is not an active currency of the drawing"
    )
    assert h.new_steps() == []


def test_dash_blockdata_asks_each_field() -> None:
    h, _placed = harness()
    h.run("-BLOCKDATA", "N", "MESA")
    assert h.prompt().text == "Category <Mobiliario>:"
    h.run("", "Comedor", ".")  # keep, change, clear
    assert h.prompt().text == "Supplier:"
    h.run("")
    assert h.prompt().text == "Currency [USD/PEN] <PEN>:"
    h.run("")
    assert h.prompt().text == "Cost (a point for none) <1.250,50>:"
    h.run("1250.50")  # not PEN's format
    assert h.messages()[-1] == "1250.50 is not an amount: write it as 1.250,50."
    h.run("2.000")
    assert h.document.block_data("MESA") == CadBlockData(
        category="Mobiliario", type="Comedor", cost="2000.00", currency="PEN"
    )
    assert h.new_steps() == ["BLOCKDATA"]
    h.run("-BLOCKDATA", "N", "MESA", "", "", "", "", "USD", ".")
    assert h.document.block_data("MESA") == CadBlockData(category="Mobiliario", type="Comedor")


def test_without_an_interface_blockdata_asks_and_in_spanish() -> None:
    h, placed = harness()
    h.session.select(h.document, [placed[1]])
    h.run("BLOCKDATA", "", "", "", "Maderera", "", "80")
    assert h.document.block_data("SILLA") == CadBlockData(
        category="mobiliario", type="Silla", supplier="Maderera", cost="80.00", currency="USD"
    )
    set_language("es")
    h.run("DATOSBLOQUE")
    assert h.runner.active == "BLOCKDATA"
    h.runner.cancel()
    h.run("-DATOSBLOQUE")
    assert h.runner.active == "-BLOCKDATA"


# The Blocks palette


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


def test_the_palettes_block_data_edits_and_shows_it(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    window.session.edit(document, lambda d: d.set_currencies([USD, PEN], "USD"))
    panel.select("Mesa")
    titles = [title for title, _rows in panel.details.sections()]
    assert titles == ["Selected block"], "no data yet"
    assert isinstance(window.ui, QuietUi)
    window.ui.block_data_answer = TABLE
    found = actions(panel)
    assert list(found)[4:6] == ["select", "block_data"]
    found["block_data"].trigger()
    assert window.ui.block_data_shown == ["Mesa"]
    assert document.block_data("Mesa") == CadBlockData(
        category="Mobiliario", type="Mesa", cost="1250.50", currency="PEN"
    )
    panel.select("Mesa")
    title, rows = panel.details.sections()[-1]
    assert title == "Budget data"
    assert list(rows) == [
        ("Category", "Mobiliario"),
        ("Type", "Mesa"),
        ("Cost", "S/ 1.250,50"),
    ]


def test_the_budget_section(qtbot: QtBot) -> None:
    document = drawing()
    assert budget_section(document, "LAMPARA").rows == [("Category", "Iluminación")]  # type: ignore[union-attr]
    document.set_block_data("LAMPARA", CadBlockData())
    assert budget_section(document, "LAMPARA") is None
    document.set_block_data("LAMPARA", CadBlockData(cost="7", currency="USD"))
    assert budget_section(document, "LAMPARA").rows == [("Cost", "$ 7.00")]  # type: ignore[union-attr]
    set_language("es")
    section = budget_section(document, "MESA")
    assert section is not None and section.title == "Datos de presupuesto"
    assert section.rows[0] == ("Categoría", "Mobiliario")


def test_the_principal_currency_comes_first(qtbot: QtBot) -> None:
    document = drawing()
    document.set_currencies([USD, PEN, EUR], "PEN")
    shown = dialog(qtbot, document, "LAMPARA")
    assert [shown.currency.itemData(i) for i in range(2)] == ["PEN", "USD"]
    assert shown.currency.currentData() == "PEN"
