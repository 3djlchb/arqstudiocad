"""RENAME for blocks (ADR 0025 step 8c): the core operation, the command
with its dialog, -RENAME's prompts, the Rename dialog itself and the Blocks
palette's Rename…, which the Recent source follows."""

from pathlib import Path

import pytest
from PySide6.QtWidgets import QDialog, QWidget
from pytestqt.qtbot import QtBot
from test_block_editor import LOOKUP
from test_blocks import Harness
from test_blocks_context_menu import actions
from test_blocks_palette import drawing_with_blocks, opened
from test_commands import regression_drawing
from ui_fakes import QuietUi

from arqstudiocad.commands.blocks import rename_named
from arqstudiocad.core import CadApplication, CadDocument, InvalidOperationError
from arqstudiocad.events import BlockRenamed
from arqstudiocad.i18n import set_language
from arqstudiocad.recent_blocks import CREATED, RecentBlocks
from arqstudiocad.ui import rename_block_dialog
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.qt_services import QtUiServices
from arqstudiocad.ui.rename_block_dialog import RenameBlockDialog


def harness(*answers: tuple[str, str] | None, ui: bool = True) -> tuple[Harness, QuietUi]:
    """MARCO and OTRO, made from a line each and placed once."""
    quiet = QuietUi(rename_answers=list(answers))
    h = Harness(quiet if ui else None)
    h.document.create_block("MARCO", (10, 10), [h.a])
    h.document.create_block("OTRO", (0, 0), [h.b])
    h.steps = len(h.document.undo_history)
    return h, quiet


def renamed(h: Harness) -> list[BlockRenamed]:
    return [e for e in h.events if isinstance(e, BlockRenamed)]


def test_renaming_through_the_api_is_one_step_and_refuses_what_it_cannot() -> None:
    doc = CadApplication().create_document("a")
    a, _ = doc.add_line((0, 0), (2, 0))
    reference, _ = doc.create_block("MESA", (0, 0), [a])
    changes = doc.rename_block("mesa", "Mesa grande")
    assert doc.undo_name == "RENAME" and doc.block_names == ["Mesa grande"]
    assert changes.blocks_created == ("Mesa grande",) and changes.blocks_deleted == ("MESA",)
    assert reference in changes.modified
    assert doc.block_references == {"Mesa grande": 1}
    with pytest.raises(InvalidOperationError, match="not a valid block name"):
        doc.rename_block("Mesa grande", "A/B")
    with pytest.raises(InvalidOperationError, match="no block named"):
        doc.rename_block("NADA", "OTRA")
    doc.undo()
    assert doc.block_names == ["MESA"]
    lookup = CadApplication().open_document(regression_drawing(LOOKUP))
    with pytest.raises(InvalidOperationError, match="dynamic"):
        lookup.rename_block("My_Look_Block", "Otro")


def test_rename_renames_the_block_chosen_in_the_dialog() -> None:
    h, ui = harness(("MARCO", "Marco de ventana"))
    h.run("RENAME")
    assert ui.rename_chosen == [None], "nothing chosen first"
    assert h.document.block_names == ["Marco de ventana", "OTRO"]
    assert h.document.block_references == {"Marco de ventana": 1, "OTRO": 1}
    assert h.new_steps() == ["RENAME"] and h.runner.active is None
    assert h.messages()[-1] == "Block MARCO renamed to Marco de ventana."
    assert renamed(h) == [BlockRenamed(h.document.id, "MARCO", "Marco de ventana")]


@pytest.mark.parametrize("typed", ["REN", "CAMBIANOM"])
def test_ren_and_the_spanish_name_start_it(typed: str) -> None:
    h, ui = harness(None)
    if typed == "CAMBIANOM":
        set_language("es")
    h.run(typed)
    assert ui.rename_chosen == [None] and h.new_steps() == []


def test_a_name_it_cannot_take_shows_the_dialog_again() -> None:
    h, ui = harness(("MARCO", "otro"), ("MARCO", "A/B"), None)
    h.run("RENAME")
    assert h.messages()[-2:] == [
        "A block named OTRO exists in the drawing.",
        "A/B is not a valid block name.",
    ]
    assert ui.rename_chosen == [None, "MARCO", "MARCO"], "again with that block"
    assert h.document.block_names == ["MARCO", "OTRO"] and h.new_steps() == []
    assert renamed(h) == []


def test_only_the_case_can_change() -> None:
    h, _ = harness(("MARCO", "Marco"))
    h.run("RENAME")
    assert h.document.block_names == ["Marco", "OTRO"] and h.new_steps() == ["RENAME"]


def test_a_block_open_in_its_editor_is_not_renamed() -> None:
    h, _ = harness(("marco", "NUEVO"), None)
    h.session.open_block_editor(h.document, "MARCO")
    h.session.activate(h.document)
    h.run("RENAME")
    assert "Close the block editor of MARCO before renaming it." in h.messages()
    assert h.document.block_names == ["MARCO", "OTRO"]


def test_a_drawing_without_blocks_has_nothing_to_rename() -> None:
    ui = QuietUi()
    h = Harness(ui)
    h.run("RENAME")
    assert h.messages()[-1] == "The drawing has no blocks to rename."
    assert ui.rename_chosen == [], "no dialog"


def test_dash_rename_asks_at_the_command_line() -> None:
    h, ui = harness()
    h.run("-RENAME")
    assert h.prompt().text.startswith("Object type to rename")
    h.run("B", "nada")
    assert h.messages()[-1] == "There is no such block."
    assert h.prompt().text == "Old block name:"
    h.run("marco")
    assert h.prompt().text == "New block name:"
    h.run("A/B")
    assert h.messages()[-1] == "A/B is not a valid block name."
    h.run("MARCO")
    assert h.messages()[-1] == "MARCO is already called so."
    h.run("Puerta")
    assert h.document.block_names == ["OTRO", "Puerta"] and h.new_steps() == ["RENAME"]
    assert ui.rename_chosen == [], "no dialog"
    set_language("es")
    h.run("-CAMBIANOM")
    assert h.runner.active == "-RENAME"
    h.runner.cancel()
    h.run("-REN")
    assert h.runner.active == "-RENAME"


def test_without_an_interface_rename_asks_too() -> None:
    h, _ = harness(ui=False)
    h.run("RENAME", "B", "MARCO", "NUEVO")
    assert h.document.block_names == ["NUEVO", "OTRO"]


def test_dash_rename_refuses_a_dynamic_block() -> None:
    h, _ = harness(ui=False)
    lookup = h.session.open_document(regression_drawing(LOOKUP))
    h.run("-RENAME", "B", "my_look_block")
    assert h.messages()[-1] == "My_Look_Block is a dynamic block: it cannot be renamed yet."
    assert h.runner.active is None and "My_Look_Block" in lookup.block_names


def test_rename_named_chooses_the_block_first() -> None:
    h, ui = harness(None)
    h.runner.start(rename_named("otro"))
    assert ui.rename_chosen == ["otro"]


def test_the_recent_list_follows_a_renamed_block(tmp_path: Path) -> None:
    recent = RecentBlocks()
    recent.add("MARCO", None, CREATED, document_id=1)
    recent.add("marco", tmp_path / "otro.dwg", CREATED, document_id=2)
    recent.renamed(1, "marco", "Marco nuevo")
    assert [(b.name, b.document_id) for b in recent.blocks] == [
        ("marco", 2),
        ("Marco nuevo", 1),
    ], "only that drawing's"


# The Rename dialog


def drawing() -> CadDocument:
    document = CadApplication().create_document("Plano")
    for name in ("MESA", "SILLA"):
        line, _ = document.add_line((0, 0), (1, 1))
        document.create_block(name, (0, 0), [line])
    return document


def dialog(qtbot: QtBot, document: CadDocument, chosen: str | None) -> RenameBlockDialog:
    shown = RenameBlockDialog(document, chosen)
    qtbot.addWidget(shown)
    return shown


def test_the_dialog_starts_on_the_block_chosen(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "silla")
    assert shown.windowTitle() == "Rename"
    assert [shown.old.itemText(i) for i in range(shown.old.count())] == ["MESA", "SILLA"]
    assert shown.chosen() == ("SILLA", "SILLA"), "its name, to be typed over"
    assert shown.problem_label.text() == "Type the block's new name."
    assert not shown.ok_button.isEnabled()
    shown.old.setCurrentText("MESA")
    assert shown.new_name.text() == "MESA"
    assert dialog(qtbot, drawing(), None).chosen()[0] == "MESA", "the first, else"


def test_ok_waits_for_a_name_the_block_can_take(qtbot: QtBot) -> None:
    shown = dialog(qtbot, drawing(), "MESA")
    for typed, problem in [
        ("silla", "A block named SILLA exists in the drawing."),
        ("A/B", "A/B is not a valid block name."),
        ("*U2", "*U2 is not a valid block name."),
        ("  ", "Type the block's new name."),
    ]:
        shown.new_name.setText(typed)
        assert shown.problem_label.text() == problem, typed
        assert not shown.ok_button.isEnabled(), typed
        shown.buttons.accepted.emit()
        assert shown.result() != QDialog.DialogCode.Accepted, typed
    shown.new_name.setText(" Mesa ")
    assert shown.problem() is None and shown.ok_button.isEnabled(), "only the case"
    assert shown.chosen() == ("MESA", "Mesa")
    shown.buttons.accepted.emit()
    assert shown.result() == QDialog.DialogCode.Accepted


def test_a_dynamic_block_cannot_be_renamed_in_the_dialog(qtbot: QtBot) -> None:
    document = CadApplication().open_document(regression_drawing(LOOKUP))
    shown = dialog(qtbot, document, "My_Look_Block")
    shown.new_name.setText("Otro")
    assert shown.problem_label.text() == (
        "My_Look_Block is a dynamic block: it cannot be renamed yet."
    )
    assert not shown.ok_button.isEnabled()


def test_the_dialog_in_spanish(qtbot: QtBot) -> None:
    set_language("es")
    shown = dialog(qtbot, drawing(), "MESA")
    assert shown.windowTitle() == "Cambiar nombre"
    assert shown.problem_label.text() == "Escriba el nombre nuevo del bloque."


def test_the_services_show_the_dialog(qtbot: QtBot, monkeypatch: pytest.MonkeyPatch) -> None:
    parent = QWidget()
    qtbot.addWidget(parent)
    seen: list[str | None] = []

    def answer(document: CadDocument, chosen: str | None, parent: object) -> tuple[str, str]:
        seen.append(chosen)
        return "MESA", "Mesa"

    monkeypatch.setattr(rename_block_dialog, "rename_block", answer)
    assert QtUiServices(parent).rename_block(drawing(), "MESA") == ("MESA", "Mesa")
    assert seen == ["MESA"]


# The Blocks palette's Rename…


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


def test_the_palettes_rename_selects_the_new_name(window: MainWindow) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa", "Silla")
    assert isinstance(window.ui, QuietUi)
    window.ui.rename_answers = [("Mesa", "Mesa redonda")]
    panel.select("Mesa")
    found = actions(panel)
    assert found["rename"].isEnabled()
    steps = len(document.undo_history)
    found["rename"].trigger()
    assert window.ui.rename_chosen == ["Mesa"]
    assert document.undo_history[steps:] == ["RENAME"] and window.runner.active is None
    assert panel.shown() == ["Mesa redonda", "Silla"]
    assert panel.selected() == "Mesa redonda", "the renamed block stays selected"


def test_the_palette_does_not_offer_renaming_a_dynamic_block(window: MainWindow) -> None:
    panel = opened(window)
    document = window.session.open_document(regression_drawing(LOOKUP))
    window.session.activate(document)
    panel.select("My_Look_Block")
    assert not actions(panel)["rename"].isEnabled()
