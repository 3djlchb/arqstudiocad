"""PURGE for blocks (ADR 0025 step 8e, maintainer 2026-10-26): the core
operation, the command with its Purge dialog, -PURGE's prompts, the dialog
itself and the Blocks palette's Purge…, confirmed first. Dynamic blocks
wait for the parametric engine (04.1)."""

import pytest
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QDialog, QMessageBox, QWidget
from pytestqt.qtbot import QtBot
from test_block_editor import LOOKUP
from test_blocks import Harness
from test_blocks_context_menu import actions
from test_blocks_palette import drawing_with_blocks, opened
from test_commands import regression_drawing
from ui_fakes import QuietUi

from arqstudiocad.commands.blocks import purge_named
from arqstudiocad.core import CadApplication, CadDocument, InvalidOperationError
from arqstudiocad.i18n import set_language
from arqstudiocad.ui import purge_dialog
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.purge_dialog import PurgeDialog
from arqstudiocad.ui.qt_services import QtUiServices


def furnish(document: CadDocument) -> None:
    """MESA holds SILLA and is placed nowhere; SOLO, placed nowhere; MURO,
    placed once."""
    a, _ = document.add_line((0, 0), (1, 0))
    chair, _ = document.create_block("SILLA", (0, 0), [a])
    b, _ = document.add_line((0, 0), (2, 0))
    table, _ = document.create_block("MESA", (0, 0), [chair, b])
    c, _ = document.add_line((0, 0), (3, 0))
    alone, _ = document.create_block("SOLO", (0, 0), [c])
    d, _ = document.add_line((0, 0), (4, 0))
    document.create_block("MURO", (0, 0), [d])
    document.delete_entities([table, alone])


def harness(ui: QuietUi | None = None) -> Harness:
    h = Harness(ui)
    furnish(h.document)
    h.steps = len(h.document.undo_history)
    return h


def test_the_api_lists_and_purges_unused_blocks() -> None:
    document = CadApplication().create_document("a")
    furnish(document)
    assert document.unused_blocks == ["MESA", "SOLO"], "SILLA is MESA's"
    purged, changes = document.purge_blocks(["mesa"])
    assert purged == ["MESA", "SILLA"], "nested by default"
    assert document.undo_name == "PURGE"
    assert sorted(changes.blocks_deleted) == ["MESA", "SILLA"]
    with pytest.raises(InvalidOperationError, match="MURO is in use"):
        document.purge_blocks(["MURO"])
    assert document.purge_blocks(["SOLO"], nested=False)[0] == ["SOLO"]
    document.undo()
    document.undo()
    assert document.block_names == ["MESA", "MURO", "SILLA", "SOLO"]


def test_purge_purges_what_the_dialog_chooses() -> None:
    ui = QuietUi(purge_answer=(["MESA"], True))
    h = harness(ui)
    h.run("PURGE")
    assert ui.purge_shown == 1
    assert h.document.block_names == ["MURO", "SOLO"]
    assert h.new_steps() == ["PURGE"] and h.runner.active is None
    assert h.messages()[-1] == "2 blocks purged: MESA, SILLA."


def test_purge_keeps_nested_items_when_asked() -> None:
    ui = QuietUi(purge_answer=(["MESA", "SOLO"], False))
    h = harness(ui)
    h.run("PU")
    assert h.document.block_names == ["MURO", "SILLA"]
    assert h.document.unused_blocks == ["SILLA"]
    assert h.messages()[-1] == "2 blocks purged: MESA, SOLO."


def test_purge_with_nothing_unused_or_closed_changes_nothing() -> None:
    ui = QuietUi()
    h = Harness(ui)
    h.run("PURGE")
    assert h.messages()[-1] == "No unused blocks to purge." and ui.purge_shown == 0
    h = harness(ui)
    h.run("PURGE")
    assert ui.purge_shown == 1 and h.new_steps() == [] and h.runner.active is None


def test_a_block_open_in_its_editor_is_not_purged() -> None:
    ui = QuietUi(purge_answer=(["SOLO", "MESA"], False))
    h = harness(ui)
    h.session.open_block_editor(h.document, "SOLO")
    h.session.activate(h.document)
    h.run("PURGE")
    assert "Not purged: SOLO (open in a block editor)." in h.messages()
    assert h.document.block_names == ["MURO", "SILLA", "SOLO"]
    assert h.messages()[-1] == "1 block purged: MESA."


def test_dash_purge_asks_at_the_command_line() -> None:
    h = harness()
    h.run("-PURGE")
    assert h.prompt().text.startswith("Type of unused objects to purge")
    h.run("B")
    assert h.prompt().text.startswith("Names to purge")
    h.run("m*, nada")
    assert h.prompt().text.startswith("Purge nested items?")
    h.run("N")
    assert h.document.block_names == ["MURO", "SILLA", "SOLO"], "MURO is in use"
    assert h.messages()[-1] == "1 block purged: MESA."
    h.run("-PURGE", "B", "", "")
    assert h.document.block_names == ["MURO"], "every unused one, SILLA now too"
    h.run("-PURGE", "B", "", "")
    assert h.messages()[-1] == "No unused blocks to purge."
    assert h.new_steps() == ["PURGE", "PURGE"]


def test_purge_without_an_interface_asks_and_in_spanish() -> None:
    h = harness()
    h.run("PURGE", "B", "SOLO", "Y")
    assert h.document.block_names == ["MESA", "MURO", "SILLA"]
    set_language("es")
    h.run("-LIMPIA")
    assert h.runner.active == "-PURGE"
    h.runner.cancel()
    h.run("LIMPIA")
    assert h.runner.active == "PURGE"


def test_a_dynamic_block_is_not_purged() -> None:
    h = Harness()
    document = h.session.open_document(regression_drawing(LOOKUP))
    shown, *_ = document.find_block_references("My_Look_Block")
    document.delete_entities(shown)
    assert "My_Look_Block" in document.unused_blocks
    h.run("-PURGE", "B", "my_look*", "")
    assert h.messages()[-2:] == [
        "Not purged: My_Look_Block (dynamic blocks).",
        "No unused blocks to purge.",
    ]
    h.runner.start(purge_named("My_Look_Block"))
    assert h.messages()[-1] == "My_Look_Block is a dynamic block: it cannot be purged yet."
    assert "My_Look_Block" in document.block_names


def test_purge_named_asks_first_and_refuses_a_block_in_use() -> None:
    ui = QuietUi(purge_confirm=False)
    h = harness(ui)
    h.runner.start(purge_named("solo"))
    assert ui.asked[-1] == "purge SOLO" and "SOLO" in h.document.block_names
    ui.purge_confirm = True
    h.runner.start(purge_named("solo"))
    assert "SOLO" not in h.document.block_names and h.new_steps() == ["PURGE"]
    h.runner.start(purge_named("MURO"))
    assert h.messages()[-1] == "MURO is in use: it cannot be purged."
    h.runner.start(purge_named("NADA"))
    assert h.messages()[-1] == "There is no such block."


# The Purge dialog


def dialog(qtbot: QtBot, document: CadDocument) -> PurgeDialog:
    shown = PurgeDialog(document)
    qtbot.addWidget(shown)
    return shown


def drawing() -> CadDocument:
    document = CadApplication().create_document("Plano")
    furnish(document)
    return document


def test_the_dialog_lists_unused_blocks_unchecked(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing())
    assert shown.windowTitle() == "Purge"
    items = [shown.blocks.item(i) for i in range(shown.blocks.count())]
    assert [i.text() for i in items] == ["MESA", "SOLO"]
    assert all(i.checkState() == Qt.CheckState.Unchecked for i in items)
    assert shown.nested.isChecked(), "as AutoCAD's"
    assert not shown.purge_button.isEnabled() and shown.purge_all_button.isEnabled()
    assert shown.count.text() == "0 of 2 chosen"
    items[1].setCheckState(Qt.CheckState.Checked)
    assert shown.chosen() == ["SOLO"] and shown.purge_button.isEnabled()
    shown.purge_all_button.click()
    assert shown.result() == QDialog.DialogCode.Accepted
    assert shown.chosen() == ["MESA", "SOLO"]


def test_the_dialog_shows_a_dynamic_block_without_a_box(qtbot: QtBot) -> None:
    document = CadApplication().open_document(regression_drawing(LOOKUP))
    shown_refs, *_ = document.find_block_references("My_Look_Block")
    document.delete_entities(shown_refs)
    shown = dialog(qtbot, document)
    (item,) = [
        shown.blocks.item(i)
        for i in range(shown.blocks.count())
        if shown.blocks.item(i).text().startswith("My_Look_Block")
    ]
    assert item.text() == "My_Look_Block — Dynamic block: it cannot be purged yet."
    assert not item.flags() & Qt.ItemFlag.ItemIsEnabled
    assert item not in shown.purgeable()


def test_the_dialog_in_spanish(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, drawing())
    assert shown.windowTitle() == "Limpiar"
    assert shown.nested.text() == "Limpiar elementos anidados"
    assert shown.purge_all_button.text() == "Limpiar todo"


def test_the_services_show_the_dialog_and_ask(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch
) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    services = QtUiServices(parent)
    monkeypatch.setattr(purge_dialog, "purge_blocks", lambda document, parent: (["SOLO"], False))
    assert services.purge_blocks(drawing()) == (["SOLO"], False)
    seen: list[str] = []

    def answer(box: QMessageBox) -> int:
        seen.append(box.text())
        return QMessageBox.StandardButton.Yes.value

    monkeypatch.setattr(QMessageBox, "exec", answer)
    assert services.ask_purge_block("SOLO")
    assert seen == ["Purge block SOLO from the drawing?"]


# The Blocks palette's Purge…


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


def test_the_palettes_purge_removes_an_unused_block(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    (placed,) = document.find_block_references("Silla")[0]
    window.session.edit(document, lambda d: d.delete_entities([placed]))
    panel.select("Mesa")
    assert not actions(panel)["purge"].isEnabled(), "Mesa is in use"
    panel.select("Silla")
    found = actions(panel)
    assert found["purge"].isEnabled()
    steps = len(document.undo_history)
    found["purge"].trigger()
    assert isinstance(window.ui, QuietUi) and window.ui.asked[-1] == "purge Silla"
    assert document.block_names == ["Mesa"] and document.undo_history[steps:] == ["PURGE"]
    assert panel.shown() == ["Mesa"]


def test_the_palette_does_not_offer_purging_a_dynamic_block(window: MainWindow) -> None:
    panel = opened(window)
    document = window.session.open_document(regression_drawing(LOOKUP))
    window.session.activate(document)
    shown, *_ = document.find_block_references("My_Look_Block")
    window.session.edit(document, lambda d: d.delete_entities(shown))
    assert "My_Look_Block" in document.unused_blocks
    panel.select("My_Look_Block")
    assert not actions(panel)["purge"].isEnabled()
