"""Panels and toolbars move, float and dock on any side, as AutoCAD's
palettes, in both layouts (ADR 0015)."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings, Qt
from PySide6.QtWidgets import QDockWidget, QMainWindow
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.workspaces import LAYOUTS, Layout

SIDES = (
    Qt.DockWidgetArea.LeftDockWidgetArea,
    Qt.DockWidgetArea.RightDockWidgetArea,
    Qt.DockWidgetArea.TopDockWidgetArea,
    Qt.DockWidgetArea.BottomDockWidgetArea,
)
TOOLBAR_SIDES = (
    Qt.ToolBarArea.LeftToolBarArea,
    Qt.ToolBarArea.RightToolBarArea,
    Qt.ToolBarArea.TopToolBarArea,
    Qt.ToolBarArea.BottomToolBarArea,
)
RIGHT = Qt.DockWidgetArea.RightDockWidgetArea


def make_window(qtbot: QtBot, settings: QSettings | None = None) -> MainWindow:
    window = MainWindow(settings, ui=QuietUi())
    qtbot.addWidget(window)
    window.resize(1100, 700)
    window.show()
    qtbot.waitExposed(window)
    return window


def docks(window: MainWindow) -> list[QDockWidget]:
    return [*window.panels.values(), window.command_window]


@pytest.mark.parametrize("layout", LAYOUTS)
def test_every_panel_docks_on_every_side_and_floats(qtbot: QtBot, layout: Layout) -> None:
    window = make_window(qtbot)
    window.workspaces.show(None, layout)
    for dock in docks(window):
        dock.show()  # the Blocks palette starts closed
        features = dock.features()
        assert features & QDockWidget.DockWidgetFeature.DockWidgetMovable
        assert features & QDockWidget.DockWidgetFeature.DockWidgetFloatable
        for side in SIDES:
            window.addDockWidget(side, dock)
            assert window.dockWidgetArea(dock) == side, (dock.objectName(), side)
        dock.setFloating(True)
        assert dock.isFloating() and dock.isVisible()
        dock.setFloating(False)


def test_panels_nest_side_by_side_and_drag_as_tab_groups(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    options = window.dockOptions()
    for option in (
        QMainWindow.DockOption.AllowNestedDocks,
        QMainWindow.DockOption.AllowTabbedDocks,
        QMainWindow.DockOption.GroupedDragging,
    ):
        assert options & option
    # Two panels out of any tab group, side by side in one area.
    left = Qt.DockWidgetArea.LeftDockWidgetArea
    window.addDockWidget(left, window.history_panel)
    window.splitDockWidget(window.history_panel, window.command_window, Qt.Orientation.Horizontal)
    assert window.dockWidgetArea(window.command_window) == left
    qtbot.waitUntil(lambda: window.command_window.x() > window.history_panel.x())
    assert window.command_window.y() == window.history_panel.y()


def test_side_panels_take_the_full_height(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    for corner in (Qt.Corner.BottomLeftCorner, Qt.Corner.TopLeftCorner):
        assert window.corner(corner) == Qt.DockWidgetArea.LeftDockWidgetArea
    for corner in (Qt.Corner.BottomRightCorner, Qt.Corner.TopRightCorner):
        assert window.corner(corner) == RIGHT
    command = window.command_window.geometry()
    assert window.history_panel.geometry().bottom() > command.top(), "beside the command window"
    assert command.right() < window.layers_panel.geometry().left()


def test_classic_toolbars_dock_on_every_side_and_float(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.workspaces.show(None, "classic")
    built = window.workspaces.widgets("cad")
    assert built is not None
    for toolbar in built.toolbars:
        assert toolbar.isMovable() and toolbar.isFloatable()
        for side in TOOLBAR_SIDES:
            window.addToolBar(side, toolbar)
            assert window.toolBarArea(toolbar) == side


def test_a_moved_panel_stays_put_per_workspace_and_across_sessions(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    window = make_window(qtbot, settings)
    left = Qt.DockWidgetArea.LeftDockWidgetArea
    window.addDockWidget(left, window.layers_panel)
    window.workspaces.show(None, "classic")
    assert window.dockWidgetArea(window.layers_panel) == left, "until moved in Classic"
    window.addDockWidget(RIGHT, window.layers_panel)
    window.workspaces.show(None, "ribbon")
    assert window.dockWidgetArea(window.layers_panel) == left, "the Ribbon's arrangement"
    # Moved after the last switch: only saving on quitting keeps this.
    window.history_panel.setFloating(True)
    window.close()
    again = make_window(qtbot, settings)
    assert again.dockWidgetArea(again.layers_panel) == left
    assert again.history_panel.isFloating()


@pytest.mark.parametrize("layout", LAYOUTS)
def test_reset_layout_puts_everything_back(qtbot: QtBot, layout: Layout) -> None:
    window = make_window(qtbot)
    window.workspaces.show(None, layout)
    built = window.workspaces.widgets("cad")
    assert built is not None
    for toolbar in built.toolbars:
        window.addToolBar(Qt.ToolBarArea.LeftToolBarArea, toolbar)
    built.toolbars[0].hide()
    window.addDockWidget(Qt.DockWidgetArea.LeftDockWidgetArea, window.layers_panel)
    window.history_panel.setFloating(True)
    window.command_window.hide()
    window.workspaces.reset_action.trigger()
    assert window.dockWidgetArea(window.layers_panel) == RIGHT
    assert not window.history_panel.isFloating()
    assert window.command_window.isVisible()
    assert window.tabifiedDockWidgets(window.layers_panel) == [window.properties_panel]
    for toolbar in built.toolbars:
        assert window.toolBarArea(toolbar) == Qt.ToolBarArea.TopToolBarArea
        assert toolbar.isVisible() == (layout == "classic")
    assert built.ribbon_bar is not None
    assert built.ribbon_bar.isVisible() == (layout == "ribbon")
