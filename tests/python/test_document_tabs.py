"""The drawings' tabs run the full width of the window, under the ribbon or
the toolbars, and the panels start below them, as AutoCAD's file tabs."""

import pytest
from PySide6.QtCore import QPoint, Qt
from PySide6.QtWidgets import QWidget
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import Viewport
from arqstudiocad.workspaces import LAYOUTS, Layout


def make_window(qtbot: QtBot) -> MainWindow:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    window.resize(1100, 700)
    window.show()
    qtbot.waitExposed(window)
    return window


def top_in_window(window: MainWindow, widget: QWidget) -> int:
    return widget.mapTo(window, QPoint(0, 0)).y()


def test_the_strip_shows_while_a_drawing_is_open(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    assert not window.tabs_strip.isVisible()
    window.runner.execute("NEW")
    assert window.tabs_strip.isVisible() and window.tabs.bar.count() == 1
    window.runner.execute("CLOSE")
    assert not window.tabs_strip.isVisible()


@pytest.mark.parametrize("layout", LAYOUTS)
def test_tabs_are_above_the_panels_and_below_the_tools(qtbot: QtBot, layout: Layout) -> None:
    window = make_window(qtbot)
    window.addDockWidget(Qt.DockWidgetArea.LeftDockWidgetArea, window.properties_panel)
    window.workspaces.show(None, layout)
    window.runner.execute("NEW")
    qtbot.waitUntil(lambda: window.tabs.bar.isVisible())
    strip = window.tabs_strip
    assert window.toolBarArea(strip) == Qt.ToolBarArea.TopToolBarArea
    strip_bottom = top_in_window(window, strip) + strip.height()
    for panel in (*window.panels.values(), window.command_window):
        if panel.isVisible():
            assert top_in_window(window, panel) >= strip_bottom, panel.objectName()
    assert top_in_window(window, window.tabs) >= strip_bottom
    built = window.workspaces.widgets("cad")
    assert built is not None
    tools = [*built.toolbars, built.ribbon_bar]
    shown = [tool for tool in tools if tool is not None and tool.isVisible()]
    assert shown, "the layout shows its tools"
    for tool in shown:
        assert top_in_window(window, tool) + tool.height() <= top_in_window(window, strip)
    assert strip.width() >= window.width() - 4, "the full width"


def test_classic_toolbars_share_one_row(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.workspaces.show(None, "classic")
    window.runner.execute("NEW")
    built = window.workspaces.widgets("cad")
    assert built is not None
    rows = {top_in_window(window, toolbar) for toolbar in built.toolbars}
    assert len(rows) == 1


def test_dragging_a_tab_keeps_each_tab_with_its_drawing(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    for _ in range(3):
        window.runner.execute("NEW")
    names = [window.tabs.tabText(i) for i in range(3)]
    window.tabs.bar.moveTab(0, 2)
    assert [window.tabs.tabText(i) for i in range(3)] == [names[1], names[2], names[0]]
    for index in range(3):
        viewport = window.tabs.widget(index)
        assert isinstance(viewport, Viewport)
        assert viewport.document.name == window.tabs.tabText(index)
    assert window.tabs.currentWidget() is window.tabs.widget(window.tabs.bar.currentIndex())
    window.tabs.bar.setCurrentIndex(0)
    active = window.session.active
    assert active is not None and active.name == names[1]
    window.tabs.bar.moveTab(0, 1)  # the current tab itself
    assert window.tabs.bar.currentIndex() == 1
    current = window.tabs.currentWidget()
    assert isinstance(current, Viewport) and current.document.name == names[1]


def test_the_tab_follows_the_active_drawing(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.runner.execute("NEW")
    first = window.session.active
    window.runner.execute("NEW")
    assert first is not None and window.tabs.bar.currentIndex() == 1
    window.session.activate(first)
    assert window.tabs.bar.currentIndex() == 0


def test_closing_from_the_tab(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.runner.execute("NEW")
    window.runner.execute("NEW")
    window.tabs.bar.tabCloseRequested.emit(0)
    assert window.tabs.count() == 1 and window.tabs.bar.count() == 1
    assert len(window.session.documents) == 1
