"""The Block Definition dialog's Budget data section (ADR 0027 step 4b,
maintainer 2026-10-26): the block data fields of the Block Data dialog,
optional and folded by default, written with the block in one undo step;
its Currencies… button closes the dialog to edit the currencies and BLOCK
opens it again."""

from PySide6.QtWidgets import QDialog
from pytestqt.qtbot import QtBot
from test_blocks import Harness, dialog_harness
from ui_fakes import QuietUi

from arqstudiocad.commands import BlockDefinitionState
from arqstudiocad.core import CadApplication, CadBlockData, CadCurrency, CadDocument
from arqstudiocad.currencies import CurrencyRegister
from arqstudiocad.events import CommandCancelled, CommandFinished, DocumentChanged
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.block_definition_dialog import BlockDefinitionDialog

USD = CadCurrency("USD", "US dollar", "$", 2, True)
PEN = CadCurrency("PEN", "Peruvian sol", "S/", 2, True, ".", ",")
TABLE = CadBlockData(category="Mobiliario", type="Mesa", cost="1250.50", currency="PEN")


def drawing() -> CadDocument:
    document = CadApplication().create_document("Plano")
    document.set_currencies([USD, PEN], "USD")
    line, _ = document.add_line((0, 0), (1, 1))
    reference, _ = document.create_block("SILLA", (0, 0), [line])
    document.delete_entities([reference])
    document.set_block_data("SILLA", CadBlockData(category="Mobiliario"))
    return document


def dialog(qtbot: QtBot, state: BlockDefinitionState) -> BlockDefinitionDialog:
    shown = BlockDefinitionDialog(drawing(), state)
    qtbot.addWidget(shown)
    return shown


# The section


def test_the_section_is_folded_by_default(qtbot: QtBot) -> None:
    shown = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,)))
    assert shown.data_toggle.text() == "Budget data"
    assert not shown.data_toggle.isChecked() and shown.data_box.isHidden()
    assert shown.state().data == CadBlockData() and not shown.state().data_shown
    shown.data_toggle.click()
    assert not shown.data_box.isHidden() and shown.state().data_shown
    shown.data_toggle.click()
    assert shown.data_box.isHidden()


def test_the_section_shows_what_it_holds_and_gives_it_back(qtbot: QtBot) -> None:
    shown = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,), data=TABLE))
    assert shown.data_toggle.isChecked() and not shown.data_box.isHidden(), "unfolded"
    fields = shown.data_fields
    assert fields.category.currentText() == "Mobiliario"
    assert [fields.category.itemText(i) for i in range(fields.category.count())] == [
        "Mobiliario"
    ], "the drawing's values suggested"
    assert fields.cost.text() == "1.250,50" and fields.currency.currentData() == "PEN"
    assert shown.state().data == TABLE
    fields.material.setText("Roble")
    fields.cost.setText("99")
    assert shown.state().data == CadBlockData(
        category="Mobiliario", type="Mesa", material="Roble", cost="99", currency="PEN"
    )
    # Unfolded before the dialog closed to pick: unfolded again.
    again = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,), data_shown=True))
    assert again.data_toggle.isChecked()


def test_data_that_cannot_be_written_disables_ok_and_is_not_kept(qtbot: QtBot) -> None:
    shown = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,), data=TABLE))
    assert shown.ok_button.isEnabled()
    shown.data_fields.cost.setText("1250.50")  # PEN writes 1.250,50
    assert not shown.ok_button.isEnabled()
    assert shown.problem_label.text() == (
        "Budget data: 1250.50 is not an amount: write it as 1.250,50."
    )
    assert shown.state().data == TABLE, "what it had"
    shown.data_fields.cost.setText("")
    assert shown.ok_button.isEnabled()


def test_currencies_closes_the_dialog_for_the_command(qtbot: QtBot) -> None:
    shown = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,)))
    shown.data_toggle.click()
    shown.edit_currencies.click()
    assert shown.result() == QDialog.DialogCode.Accepted and shown.action == "currencies"
    assert shown.state().name == "MESA" and shown.state().data_shown


def test_the_section_in_spanish(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,)))
    assert shown.data_toggle.text() == "Datos de presupuesto"
    assert shown.edit_currencies.text() == "Monedas…"


# BLOCK


def furnished(*answers: tuple[str, dict[str, object]] | None) -> tuple[Harness, QuietUi]:
    h, ui = dialog_harness(*answers)
    h.document.set_currencies([USD, PEN], "USD")
    h.session.select(h.document, [h.a, h.b])
    h.steps = len(h.document.undo_history)
    return h, ui


def test_block_writes_the_block_and_its_data_in_one_step() -> None:
    h, _ui = furnished(("ok", {"name": "MESA", "data": TABLE}))
    h.run("BLOCK")
    assert h.document.block_data("MESA") == TABLE
    assert h.new_steps() == ["BLOCK"] and h.runner.active is None
    assert h.messages()[-1] == "Block MESA made from 2 objects."
    changed = [e for e in h.events if isinstance(e, DocumentChanged)]
    assert changed[-1].changes.blocks_created == ("MESA",), "the interface hears of it"
    h.document.undo()
    assert h.document.block_names == [], "both undone"


def test_block_without_data_writes_none() -> None:
    h, _ui = furnished(("ok", {"name": "MESA"}))
    h.run("BLOCK")
    assert h.document.block_data("MESA") is None and h.new_steps() == ["BLOCK"]


def test_a_redefined_block_keeps_its_data_unless_given() -> None:
    h, ui = furnished(("ok", {"name": "MESA", "data": TABLE}))
    h.run("BLOCK")
    line, _ = h.document.add_line((0, 0), (3, 0))
    h.session.select(h.document, [line])
    ui.block_dialog.append(("ok", {"name": "mesa"}))
    h.run("BLOCK")
    assert h.messages()[-1] == "Block MESA redefined from 1 objects."
    assert h.document.block_data("MESA") == TABLE, "empty data keeps it"
    line, _ = h.document.add_line((0, 0), (4, 0))
    h.session.select(h.document, [line])
    ui.block_dialog.append(("ok", {"name": "MESA", "data": CadBlockData(material="Pino")}))
    h.run("BLOCK")
    assert h.document.block_data("MESA") == CadBlockData(material="Pino"), "given: replaced"


def test_data_the_core_refuses_makes_nothing() -> None:
    inactive = CadBlockData(cost="5", currency="EUR")
    h, ui = furnished(("ok", {"name": "MESA", "data": inactive}), None)
    h.run("BLOCK")
    assert h.messages()[-1] == ("MESA cannot be made: EUR is not an active currency of the drawing")
    assert h.document.block_names == [] and h.new_steps() == [], "rolled back"
    assert len(ui.block_dialog_shown) == 2, "the dialog again"


def test_the_currencies_button_edits_them_and_opens_the_dialog_again() -> None:
    euro = CadCurrency("EUR", "Euro", "€", 2, True, ".", ",")
    h, ui = furnished(
        ("currencies", {"name": "MESA", "data_shown": True}),
        ("ok", {"data": CadBlockData(cost="10", currency="EUR")}),
    )
    ui.currencies_answer = CurrencyRegister([USD, PEN, euro], "USD", ",", ".")
    h.run("BLOCK")
    assert h.document.currencies == ([USD, PEN, euro], "USD")
    assert ui.block_dialog_shown[1].name == "MESA" and ui.block_dialog_shown[1].data_shown
    assert h.document.block_data("MESA") == CadBlockData(cost="10.00", currency="EUR")
    assert h.new_steps() == ["CURRENCIES", "BLOCK"]
    # Cancelled, or refused by the core: the dialog again, nothing changed.
    h2, ui2 = furnished(("currencies", {"name": "SOFA"}), None)
    ui2.currencies_answer = None
    h2.run("BLOCK")
    assert len(ui2.currencies_shown) == 1 and len(ui2.block_dialog_shown) == 2
    ui2.block_dialog = [("currencies", {}), None]
    ui2.currencies_answer = CurrencyRegister([PEN], "USD", ",", ".")
    h2.run("BLOCK")
    assert h2.messages()[-1].startswith("The currencies were not changed:")
    assert h2.new_steps() == []


def test_currencies_cancelled_is_a_cancelled_command() -> None:
    h, ui = furnished()
    h.run("CURRENCIES")
    assert any(isinstance(e, CommandCancelled) for e in h.events)
    ui.currencies_answer = CurrencyRegister([USD, PEN], "PEN", ",", ".")
    h.events.clear()
    h.run("CURRENCIES")
    assert any(isinstance(e, CommandFinished) for e in h.events)
    assert not any(isinstance(e, CommandCancelled) for e in h.events)
    assert h.document.currencies[1] == "PEN"


def test_budget_data_is_never_required(qtbot: QtBot) -> None:
    # Optional (maintainer, 2026-10-28): folded or open, empty data makes the block.
    shown = dialog(qtbot, BlockDefinitionState(name="MESA", ids=(1,)))
    assert shown.ok_button.isEnabled() and shown.problem() is None
    shown.data_toggle.click()
    assert shown.ok_button.isEnabled(), "open and empty"
    shown.data_fields.category.setCurrentText("Mobiliario")
    shown.data_fields.category.setCurrentText("")
    assert shown.ok_button.isEnabled(), "typed and erased"
    # Only data typed wrong holds OK back, and the section never hides why.
    shown.data_fields.cost.setText("doce")  # not an amount
    assert not shown.ok_button.isEnabled()
    shown.data_toggle.click()
    assert shown.data_toggle.isChecked() and not shown.data_box.isHidden(), "stays open"
    shown.data_fields.cost.setText("")
    shown.data_toggle.click()
    assert shown.data_box.isHidden() and shown.ok_button.isEnabled()
