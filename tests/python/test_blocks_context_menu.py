"""The Blocks palette's context menu (ADR 0025 step 8b, maintainer
2026-10-25): Insert and Insert with options always, Copy to drawing for a
block of another file, Edit block for the drawing's, Redefine from template
when the drawing's block of that name differs, confirmed seeing both."""

import math

import pytest
from PySide6.QtCore import QPoint
from PySide6.QtGui import QAction, QContextMenuEvent
from PySide6.QtWidgets import QAbstractScrollArea, QApplication, QMenu
from pytestqt.qtbot import QtBot
from test_block_editor import LOOKUP
from test_blocks_palette import (
    drawing_with_blocks,
    history,
    opened,
    references,
    with_template,
)
from test_commands import regression_drawing
from ui_fakes import QuietUi

from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import set_language, tr
from arqstudiocad.ui import block_dialogs
from arqstudiocad.ui.blocks_panel import BlocksPanel, InsertOptions
from arqstudiocad.ui.main_window import MainWindow


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


def actions(panel: BlocksPanel) -> dict[str, QAction]:
    """The selected block's menu items, by name, in order."""
    menu = panel.block_menu()
    assert menu is not None
    return {a.objectName(): a for a in menu.actions() if not a.isSeparator()}


def test_the_drawings_blocks_can_be_inserted_and_edited(window: MainWindow) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa")
    assert panel.block_menu() is None, "nothing selected"
    panel.select("Mesa")
    found = actions(panel)
    assert list(found) == [
        "insert",
        "insert_options",
        "edit",
        "rename",
        "select",
        "block_data",
        "purge",
    ]
    assert [a.text() for a in found.values()] == [
        "Insert",
        "Insert with options…",
        "Edit block",
        "Rename…",
        "Select in drawing",
        "Block data…",
        "Purge…",
    ]
    purge = found.pop("purge")
    assert all(a.isEnabled() for a in found.values())
    assert not purge.isEnabled(), "Mesa is in use"
    found["edit"].trigger()
    editor = window.session.active
    assert editor is not None and window.session.block_editor_of(editor) is not None


def test_insert_with_options_uses_them_once(
    window: MainWindow, monkeypatch: pytest.MonkeyPatch
) -> None:
    panel = opened(window)
    document = drawing_with_blocks(window, "Mesa")
    panel.select("Mesa")
    asked: list[tuple[str, InsertOptions]] = []

    def answer(name: str, options: InsertOptions, parent: object) -> InsertOptions | None:
        asked.append((name, options))
        return InsertOptions(2.0, 90.0, False, False)

    monkeypatch.setattr(block_dialogs, "ask_insert_options", answer)
    before = len(references(document, "Mesa"))
    steps = len(document.undo_history)
    actions(panel)["insert_options"].trigger()
    assert asked == [("Mesa", InsertOptions(1.0, 0.0, True, False))], "the palette's, first"
    assert document.undo_history[steps:] == ["INSERT"] and window.runner.active is None
    (placed,) = references(document, "Mesa")[before:]
    details = dict(document.get_entity(placed).details)
    assert math.isclose(float(details["Rotation"]), 90.0), details
    assert panel.insert_options() == InsertOptions(1.0, 0.0, True, False), "the palette's kept"
    monkeypatch.setattr(block_dialogs, "ask_insert_options", lambda *a: None)
    actions(panel)["insert_options"].trigger()
    assert document.undo_history[steps:] == ["INSERT"], "cancelled: nothing"


def test_a_templates_block_is_copied_without_inserting_it(qtbot: QtBot, window: MainWindow) -> None:
    panel = with_template(qtbot, window)
    document = window.session.new_document()
    panel.select("Tree")
    found = actions(panel)
    assert list(found) == ["insert", "insert_options", "copy"], "the template's: no edit"
    steps = len(document.undo_history)
    found["copy"].trigger()
    assert "Tree" in document.block_names and references(document, "Tree") == []
    assert document.undo_history[steps:] == ["COPYBLOCK"], "one undo step"
    assert history(window)[-1] == "Block Tree copied to the drawing."
    actions(panel)["copy"].trigger()
    assert history(window)[-1] == "The drawing already has block Tree."
    assert document.undo_history[steps:] == ["COPYBLOCK"], "nothing copied twice"


def clashing_wc(window: MainWindow) -> CadDocument:
    """A drawing whose WC differs from the template's and has 2 references."""
    document = drawing_with_blocks(window, "WC")
    document.insert_block("WC", (5, 5))
    return document


def test_redefine_from_template_is_confirmed_seeing_both(
    qtbot: QtBot, window: MainWindow, monkeypatch: pytest.MonkeyPatch
) -> None:
    panel = with_template(qtbot, window)
    document = clashing_wc(window)
    panel.refresh()
    template = panel.template
    assert template is not None
    panel.select("Tree")
    assert "redefine" not in actions(panel), "the drawing has no Tree"
    panel.select("WC")
    found = actions(panel)
    assert list(found) == ["insert", "insert_options", "copy", "redefine"]
    asked: list[tuple[object, ...]] = []

    def refuse(*shown: object) -> bool:
        asked.append(shown)
        return False

    monkeypatch.setattr(block_dialogs, "confirm_redefine", refuse)
    steps = len(document.undo_history)
    found["redefine"].trigger()
    assert asked == [(template, document, "WC", "WC", 2, panel)]
    assert document.undo_history[steps:] == [], "cancelled: nothing"
    monkeypatch.setattr(block_dialogs, "confirm_redefine", lambda *a: True)
    actions(panel)["redefine"].trigger()
    assert document.block_clashes("WC", template) == [], "the template's now"
    assert document.undo_history[steps:] == ["COPYBLOCK"], "one undo step"
    assert history(window)[-1] == "Block WC redefined from arqstudiocad-sample.dwt."
    assert "redefine" not in actions(panel), "the same block now"
    window.runner.execute("UNDO")
    assert document.block_clashes("WC", template) == ["WC"], "undone in one step"


def test_a_dynamic_block_is_never_redefined(qtbot: QtBot, window: MainWindow) -> None:
    """Its parameters wait for the parametric engine (ADR 0023 D3a)."""
    panel = with_template(qtbot, window, regression_drawing(LOOKUP))
    drawing_with_blocks(window, "My_Look_Block")
    panel.refresh()
    panel.select("My_Look_Block")
    assert list(actions(panel)) == ["insert", "insert_options", "copy"]


def test_a_recent_block_of_another_drawing_can_be_copied(window: MainWindow) -> None:
    panel = opened(window)
    first = drawing_with_blocks(window, "Mesa")
    panel.select("Mesa")
    panel.on_screen.setChecked(False)
    panel.insert_button.click()
    panel.set_source("recent")
    panel.select("Mesa")
    assert "copy" not in actions(panel), "the active drawing's own"
    second = window.session.new_document()
    panel.select("Mesa")
    actions(panel)["copy"].trigger()
    assert "Mesa" in second.block_names and references(second, "Mesa") == []
    assert first.block_names == ["Mesa"]


def menu_event(view: QAbstractScrollArea, at: QPoint, keyboard: bool = False) -> None:
    """A context menu asked for at ``at`` in the view's viewport, where Qt
    delivers it, by the mouse or the keyboard."""
    reason = QContextMenuEvent.Reason.Keyboard if keyboard else QContextMenuEvent.Reason.Mouse
    target = view.viewport()
    QApplication.sendEvent(target, QContextMenuEvent(reason, at, target.mapToGlobal(at)))


def test_a_right_click_or_the_menu_key_shows_the_blocks_menu(
    window: MainWindow, monkeypatch: pytest.MonkeyPatch
) -> None:
    panel = opened(window)
    drawing_with_blocks(window, "Mesa", "Silla")
    shown: list[list[str]] = []
    monkeypatch.setattr(
        QMenu,
        "popup",
        lambda menu, at, action=None: shown.append([a.text() for a in menu.actions()]),
    )
    panel.select("Mesa")
    card = panel.grid.item(1)
    menu_event(panel.grid, panel.grid.visualItemRect(card).center())
    assert panel.selected() == "Silla" and shown[-1][0] == "Insert", "the block under the mouse"
    menu_event(panel.grid, QPoint(-50, -50))
    assert len(shown) == 1 and panel.selected() == "Silla", "off the blocks: no menu"
    menu_event(panel.grid, QPoint(-50, -50), keyboard=True)
    assert len(shown) == 2 and panel.selected() == "Silla", "the Menu key: the block selected"
    panel.list_view.click()
    first = panel.table.topLevelItem(0)
    assert first is not None
    menu_event(panel.table, panel.table.visualItemRect(first).center())
    assert panel.selected() == "Mesa" and len(shown) == 3, "the list too"


def test_without_a_drawing_nothing_can_be_inserted(qtbot: QtBot, window: MainWindow) -> None:
    panel = with_template(qtbot, window)
    panel.select("Tree")
    found = actions(panel)
    assert not any(found[k].isEnabled() for k in ("insert", "insert_options", "copy"))
    found["copy"].trigger()
    panel.copy_selected()
    assert window.runner.active is None and window.session.documents == []


def test_copyblock_typed_copies_from_the_loaded_template(qtbot: QtBot, window: MainWindow) -> None:
    document = window.session.new_document()
    window.runner.execute("COPYBLOCK")
    assert history(window)[-1].endswith("No template is loaded.")
    with_template(qtbot, window)
    set_language("es")
    for typed in ("COPIARBLOQUE", "nada", "door_090"):
        window.runner.execute(typed)
    assert "Door_090" in document.block_names and references(document, "Door_090") == []
    assert tr("There is no such block.") in history(window), "asked again"


def test_the_menu_dialogs(qtbot: QtBot, window: MainWindow) -> None:
    dialog = block_dialogs.InsertOptionsDialog("Mesa", InsertOptions(1.5, 45.0, False, True))
    qtbot.addWidget(dialog)
    assert dialog.windowTitle() == "Insert Mesa" and dialog.insert_button.text() == "Insert"
    assert dialog.options() == InsertOptions(1.5, 45.0, False, True)
    dialog.scale.setValue(3.0)
    dialog.on_screen.setChecked(True)
    assert dialog.options() == InsertOptions(3.0, 45.0, True, True)

    panel = with_template(qtbot, window)
    document = clashing_wc(window)
    assert panel.template is not None
    for count, text in ((1, "1 reference in the drawing will change."), (2, "2 references")):
        redefine = block_dialogs.RedefineDialog(panel.template, document, "WC", "WC", count)
        qtbot.addWidget(redefine)
        assert text in redefine.text.text() and redefine.redefine_button.text() == "Redefine"
        assert not redefine.template_preview.pixmap().isNull()
        assert not redefine.drawing_preview.pixmap().isNull()
