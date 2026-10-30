"""The model-space context menu (ADR 0029 D1, D2): a right-click, the Menu
key or Shift+F10 opens Repeat {command} and Options… with nothing selected
and no command running; during a command a right-click acts as Enter. And
OPTIONS (OP, OPCIONES), which the menu's Options… runs."""

import pytest
from PySide6.QtCore import QEvent, QPoint, QRect, Qt
from PySide6.QtGui import QContextMenuEvent, QKeyEvent
from PySide6.QtWidgets import QApplication, QMenu
from pytestqt.qtbot import QtBot
from test_blocks import Harness
from ui_fakes import QuietUi

from arqstudiocad.commands import Command, CommandContext, CommandScript
from arqstudiocad.core import CadDocument
from arqstudiocad.events import CommandCancelled
from arqstudiocad.i18n import command_display_name, set_language
from arqstudiocad.ui.context_menu import inside, model_space_menu
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import Viewport


def quiet(ctx: CommandContext) -> CommandScript:
    return
    yield


def test_the_runner_says_which_command_an_empty_enter_repeats() -> None:
    h = Harness()
    assert h.runner.last is None
    h.run("LINE")
    h.runner.cancel()
    last = h.runner.last
    assert last is not None and last.name == "LINE"
    # A command that does not repeat (a grip edit, Open Recent) leaves it.
    h.runner.start(Command("GRIP", quiet, repeat=False))
    assert h.runner.last is last


def test_a_command_is_named_in_the_active_language() -> None:
    assert command_display_name("LINE") == "LINE"
    try:
        set_language("es")
        assert command_display_name("LINE") == "LINEA"
        assert command_display_name("ZOOM") == "ZOOM", "no name of its own"
    finally:
        set_language("en")


@pytest.mark.parametrize("typed", ["OPTIONS", "op", "OPCIONES"])
def test_options_opens_the_dialog(typed: str) -> None:
    ui = QuietUi()
    h = Harness(ui)
    if typed == "OPCIONES":
        set_language("es")
    try:
        h.run(typed)
    finally:
        set_language("en")
    assert ui.options_shown == 1


def test_options_without_an_interface_says_so() -> None:
    h = Harness()
    h.run("OPTIONS")
    reasons = [e.reason for e in h.events if isinstance(e, CommandCancelled)]
    assert reasons == ["The Options dialog is not available here."]


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    qtbot.addWidget(w)
    w.resize(1000, 700)
    w.show()
    qtbot.waitExposed(w)
    # Not NEW: no command has run yet.
    w.session.new_document()
    return w


def drawing(window: MainWindow) -> tuple[CadDocument, Viewport]:
    document = window.session.active
    assert document is not None
    viewport = window.viewport_for(document)
    assert isinstance(viewport, Viewport)
    return document, viewport


#: Where the tests right-click, in the viewport.
CLICK = QPoint(40, 30)


def right_click(viewport: Viewport, at: QPoint = CLICK) -> None:
    event = QContextMenuEvent(QContextMenuEvent.Reason.Mouse, at, viewport.mapToGlobal(at))
    QApplication.sendEvent(viewport, event)


def opened(window: MainWindow) -> QMenu:
    menu = window.context_menu
    assert menu is not None and menu.isVisible()
    return menu


def test_before_any_command_repeat_is_disabled(window: MainWindow) -> None:
    _, viewport = drawing(window)
    right_click(viewport)
    menu = opened(window)
    texts = [a.text() for a in menu.actions()]
    assert texts == ["Repeat last command", "", "&Options…"]
    repeat = menu.actions()[0]
    assert not repeat.isEnabled() and not repeat.icon().isNull()
    assert menu.actions()[1].isSeparator()
    assert menu.pos() == viewport.mapToGlobal(QPoint(40, 30)), "at the cursor"
    menu.close()


def test_repeat_starts_what_an_empty_enter_starts(window: MainWindow) -> None:
    document, viewport = drawing(window)
    window.runner.execute("LINE")
    window.runner.cancel()
    right_click(viewport)
    menu = opened(window)
    repeat = menu.actions()[0]
    assert repeat.text() == "&Repeat LINE\tEnter"
    assert menu.defaultAction() is repeat and menu.activeAction() is repeat
    window.command_window.input.setText("CIRCLE")
    repeat.trigger()
    assert window.runner.active == "LINE", "what was typed is not sent"
    assert window.command_window.input.text() == ""
    window.runner.cancel()
    assert document.get_entities() == []


def test_options_in_the_menu_runs_options(window: MainWindow) -> None:
    _, viewport = drawing(window)
    right_click(viewport)
    menu = opened(window)
    menu.actions()[2].trigger()
    assert isinstance(window.ui, QuietUi) and window.ui.options_shown == 1


def test_over_an_object_it_opens_with_a_selection_it_does_not(window: MainWindow) -> None:
    document, viewport = drawing(window)
    line, _ = document.add_line((-1000, -1000), (1000, 1000))
    right_click(viewport, viewport.rect().center())
    opened(window).close()
    window.context_menu = None
    window.session.select(document, [line])
    right_click(viewport)
    assert window.context_menu is None, "the object menu is later work"


def test_during_a_command_a_right_click_is_enter(window: MainWindow) -> None:
    document, viewport = drawing(window)
    window.runner.execute("LINE")
    window.runner.execute("0,0")
    window.runner.execute("10,0")
    right_click(viewport)
    assert window.runner.active is None, "LINE ended, as with Enter"
    assert window.context_menu is None
    assert [e.kind for e in document.get_entities()] == ["LINE"]


def test_the_keyboard_opens_it_once(window: MainWindow) -> None:
    _, viewport = drawing(window)
    keyboard = QContextMenuEvent(QContextMenuEvent.Reason.Keyboard, QPoint(0, 0), QPoint(0, 0))
    QApplication.sendEvent(viewport, keyboard)  # the Menu key
    first = opened(window)
    first.close()
    shift_f10 = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_F10, Qt.KeyboardModifier.ShiftModifier)
    QApplication.sendEvent(viewport, shift_f10)
    second = opened(window)
    assert second is not first
    second.close()
    # A platform that also reports Shift+F10 as a context-menu event.
    QApplication.sendEvent(
        viewport, QContextMenuEvent(QContextMenuEvent.Reason.Keyboard, QPoint(0, 0), QPoint(0, 0))
    )
    assert window.context_menu is second, "not opened twice"


def test_from_the_keyboard_it_opens_at_the_cursor_or_the_center(
    window: MainWindow, monkeypatch: pytest.MonkeyPatch
) -> None:
    _, viewport = drawing(window)
    inner = viewport.mapToGlobal(QPoint(25, 35))
    monkeypatch.setattr("arqstudiocad.ui.viewport.QCursor.pos", lambda: inner)
    assert viewport.menu_point() == inner
    outer = viewport.mapToGlobal(QPoint(-50, -50))
    monkeypatch.setattr("arqstudiocad.ui.viewport.QCursor.pos", lambda: outer)
    assert viewport.menu_point() == viewport.mapToGlobal(viewport.rect().center())


def test_near_a_corner_the_menu_opens_inside_the_drawing(window: MainWindow) -> None:
    _, viewport = drawing(window)
    corner = viewport.rect().bottomRight() - QPoint(2, 2)
    right_click(viewport, corner)
    menu = opened(window)
    area = QRect(viewport.mapToGlobal(QPoint(0, 0)), viewport.size())
    assert area.contains(QRect(menu.pos(), menu.sizeHint()))
    menu.close()


def test_the_menu_stays_inside_the_drawing(window: MainWindow) -> None:
    _, viewport = drawing(window)
    menu = model_space_menu(None, viewport, lambda: None, lambda: None)
    size = menu.sizeHint()
    area = QRect(100, 100, 400, 300)
    assert inside(menu, QPoint(150, 120), area) == QPoint(150, 120)
    corner = inside(menu, QPoint(499, 399), area)
    assert corner == QPoint(500 - size.width(), 400 - size.height())
    assert inside(menu, QPoint(50, 50), area) == QPoint(100, 100)


def test_the_menu_in_spanish(window: MainWindow) -> None:
    _, viewport = drawing(window)
    try:
        set_language("es")
        window.runner.execute("LINEA")
        window.runner.cancel()
        right_click(viewport)
        menu = opened(window)
        assert [a.text() for a in menu.actions()] == ["&Repetir LINEA\tIntro", "", "&Opciones…"]
        menu.close()
        fresh = model_space_menu(None, viewport, lambda: None, lambda: None)
        assert fresh.actions()[0].text() == "Repetir el último comando"
    finally:
        set_language("en")


def test_tools_holds_options(window: MainWindow) -> None:
    assert window.command_actions["OPTIONS"] in window._tools_menu.actions()
    assert window.command_actions["OPTIONS"].text() == "&Options…"
