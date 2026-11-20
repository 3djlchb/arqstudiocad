"""Menus opened inside a window holding native widgets (ui/popup.py).

Qt's "must be a top level window" comes from the Windows platform, not
from the offscreen one the tests run on: what these check is the menu's
owner, which decides it (seen gone in the application on Windows).
"""

from PySide6.QtCore import QPoint, Qt
from PySide6.QtWidgets import QMenu, QVBoxLayout, QWidget
from pytestqt.qtbot import QtBot

from arqstudiocad.ui.popup import popup


def native_child(qtbot: QtBot) -> tuple[QWidget, QWidget]:
    """A window holding a native child, as the GPU viewport and the panels
    Qt makes native beside it."""
    window = QWidget()
    qtbot.addWidget(window)
    child = QWidget(window)
    QVBoxLayout(window).addWidget(child)
    child.setAttribute(Qt.WidgetAttribute.WA_NativeWindow)
    window.resize(200, 100)
    window.show()
    qtbot.waitExposed(window)
    assert child.internalWinId(), "the child has a native window"
    return window, child


def test_a_menu_opens_owned_by_the_top_level_window(qtbot: QtBot) -> None:
    window, child = native_child(qtbot)
    menu = QMenu(child)
    menu.addAction("Probe")
    popup(menu, child.mapToGlobal(QPoint(10, 10)))
    qtbot.waitUntil(menu.isVisible)
    assert menu.parentWidget() is window
    menu.close()


def test_a_menu_of_the_window_itself_stays_as_it_is(qtbot: QtBot) -> None:
    window, _child = native_child(qtbot)
    menu = QMenu(window)
    menu.addAction("Probe")
    popup(menu, window.mapToGlobal(QPoint(10, 10)))
    qtbot.waitUntil(menu.isVisible)
    assert menu.parentWidget() is window
    menu.close()
