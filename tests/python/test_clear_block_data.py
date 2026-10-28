"""Clearing a block's budget data at once (maintainer, 2026-10-28): the
Block Data dialog's Clear data empties every field, OK confirming; the
Blocks palette's Clear block data asks first, then clears it in one undo
step. Data that does not read can be cleared too."""

import pytest
from PySide6.QtWidgets import QMessageBox, QWidget
from pytestqt.qtbot import QtBot
from test_block_data_editors import TABLE, dialog, drawing, furnish, ok
from test_blocks import Harness
from test_blocks_context_menu import actions
from test_blocks_palette import drawing_with_blocks, opened
from ui_fakes import QuietUi

from arqstudiocad.commands.budget import clear_data_named
from arqstudiocad.core import CadBlockData, CadCurrency
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.blocks_panel import has_block_data
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.qt_services import QtUiServices

PEN = CadCurrency("PEN", "Peruvian sol", "S/", 2, True, ".", ",")
USD = CadCurrency("USD", "US dollar", "$", 2, True)


def test_clear_data_empties_the_fields_and_ok_confirms(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "MESA")
    assert shown.clear_button.text() == "Clear data"
    shown.material.setText("Roble")
    shown.clear_button.click()
    fields = (shown.category.currentText(), shown.kind.currentText())
    assert fields == ("", "") and shown.material.text() == ""
    assert shown.supplier.text() == "" and shown.cost.text() == ""
    assert ok(shown) and shown.data() == CadBlockData(), "OK clears the block's data"
    assert shown.currency.currentData() == "PEN", "the currency stays chosen"
    set_language("es")
    spanish = dialog(qtbot, drawing(), "MESA")
    assert spanish.clear_button.text() == "Borrar datos"


def test_clear_data_through_blockdata_clears_it_in_one_step() -> None:
    ui = QuietUi(block_data_answer=CadBlockData())
    h = Harness(ui)
    placed = furnish(h.document)
    h.steps = len(h.document.undo_history)
    h.session.select(h.document, [placed[0]])
    h.run("BLOCKDATA")
    assert h.document.block_data("MESA") is None and h.new_steps() == ["BLOCKDATA"]
    h.document.undo()
    assert h.document.block_data("MESA") == CadBlockData(
        category="Mobiliario", type="Mesa", cost="1250.50", currency="PEN"
    )


def harness(ui: QuietUi | None = None) -> Harness:
    h = Harness(ui)
    furnish(h.document)
    h.steps = len(h.document.undo_history)
    return h


def test_clearing_asks_first_and_is_one_undo_step() -> None:
    ui = QuietUi(clear_data_confirm=False)
    h = harness(ui)
    h.runner.start(clear_data_named("mesa"))
    assert ui.asked[-1] == "clear data MESA"
    assert h.document.block_data("MESA") is not None and h.new_steps() == []
    ui.clear_data_confirm = True
    h.runner.start(clear_data_named("mesa"))
    assert h.document.block_data("MESA") is None
    assert h.new_steps() == ["BLOCKDATA"] and h.runner.active is None
    assert h.messages()[-1] == "Block data of MESA cleared."
    h.runner.start(clear_data_named("MESA"))
    assert h.messages()[-1] == "MESA has no block data." and h.new_steps() == ["BLOCKDATA"]
    h.runner.start(clear_data_named("NADA"))
    assert h.messages()[-1] == "There is no such block."
    h.document.undo()
    assert h.document.block_data("MESA") is not None


def test_clearing_in_spanish() -> None:
    h = harness(QuietUi())
    set_language("es")
    h.runner.start(clear_data_named("SILLA"))
    assert h.messages()[-1] == "Datos del bloque SILLA borrados."


def test_the_service_asks_with_no_as_the_default(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch
) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    services = QtUiServices(parent)
    seen: list[tuple[str, str, bool]] = []

    def answer(box: QMessageBox) -> int:
        no = box.defaultButton() is box.button(QMessageBox.StandardButton.No)
        seen.append((box.windowTitle(), box.text(), no))
        return QMessageBox.StandardButton.Yes.value

    monkeypatch.setattr(QMessageBox, "exec", answer)
    assert services.ask_clear_block_data("MESA")
    assert seen == [("Clear block data", "Clear the budget data of block MESA?", True)]


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


def test_the_palette_clears_a_blocks_data(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    window.session.edit(document, lambda d: d.set_currencies([USD, PEN], "USD"))
    window.session.edit(document, lambda d: d.set_block_data("Mesa", TABLE))
    panel.select("Silla")
    assert not actions(panel)["clear_block_data"].isEnabled(), "no data to clear"
    panel.select("Mesa")
    found = actions(panel)
    assert list(found)[5:7] == ["block_data", "clear_block_data"]
    assert found["clear_block_data"].text() == "Clear block data"
    found["clear_block_data"].trigger()
    assert isinstance(window.ui, QuietUi) and window.ui.asked[-1] == "clear data Mesa"
    assert document.block_data("Mesa") is None
    panel.select("Mesa")
    assert [title for title, _ in panel.details.sections()] == ["Selected block"]


def test_data_that_does_not_read_counts_as_data(monkeypatch: pytest.MonkeyPatch) -> None:
    document = drawing()
    assert has_block_data(document, "MESA")
    document.set_block_data("MESA", CadBlockData())
    assert not has_block_data(document, "MESA")

    def unreadable(self: object, name: str) -> CadBlockData:
        from arqstudiocad.core import InvalidOperationError

        raise InvalidOperationError("block MESA's data could not be read")

    monkeypatch.setattr(type(document), "block_data", unreadable, raising=False)
    assert has_block_data(document, "MESA"), "it can be cleared"
