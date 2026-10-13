"""Pinned and auto-hidden panels, as AutoCAD's palettes (ADR 0015)."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, QRect, QSettings, Qt
from PySide6.QtGui import QEnterEvent
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.events import WorkFinished, WorkStarted
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.auto_hide import AutoHide, PanelTitleBar, SideTab
from arqstudiocad.ui.main_window import MainWindow

RIGHT = Qt.DockWidgetArea.RightDockWidgetArea
LEFT = Qt.DockWidgetArea.LeftDockWidgetArea


def make_window(qtbot: QtBot, settings: QSettings | None = None) -> MainWindow:
    ui = QuietUi()
    window = MainWindow(settings, ui=ui)
    ui.panel_for = window.show_panel
    qtbot.addWidget(window)
    window.resize(1100, 700)
    window.show()
    qtbot.waitExposed(window)
    return window


def drawing(window: MainWindow) -> QRect:
    """Where the drawing is: the window's central widget."""
    central = window.centralWidget()
    assert central is not None
    return central.geometry()


def pins(window: MainWindow) -> AutoHide:
    return window.auto_hide


def title_bar(window: MainWindow) -> PanelTitleBar:
    return pins(window).title_bars[window.layers_panel]


def tab(window: MainWindow) -> SideTab:
    return pins(window).tabs[window.layers_panel]


def hover(widget: SideTab) -> None:
    QApplication.sendEvent(widget, QEnterEvent(QPointF(2, 2), QPointF(2, 2), QPointF(2, 2)))


def test_panels_start_pinned_with_a_pin_in_the_title(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    for panel in window.panels.values():
        assert pins(window).is_pinned(panel) and panel.isVisible()
        bar = panel.titleBarWidget()
        assert isinstance(bar, PanelTitleBar) and bar.pinned
        assert bar.pin.toolTip() == "Auto-hide"
    assert not any(bar.isVisible() for bar in pins(window).bars.values())


def test_the_title_bar_floats_and_closes(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    assert not hasattr(title_bar(window), "float_button"), "pin and close, as OpenCADStudio"
    QTest.mouseDClick(title_bar(window).title, Qt.MouseButton.LeftButton)
    assert window.layers_panel.isFloating(), "double-clicking the title floats it"
    assert title_bar(window).pin.isHidden(), "a floating panel has nothing to collapse to"
    QTest.mouseDClick(title_bar(window).title, Qt.MouseButton.LeftButton)
    assert not window.layers_panel.isFloating() and not title_bar(window).pin.isHidden()
    title_bar(window).close_button.click()
    assert not window.layers_panel.isVisible()


def test_unpinning_collapses_to_a_tab_on_its_side(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    viewport_width = drawing(window).width()
    title_bar(window).pin.click()
    for other in (window.properties_panel, window.history_panel):
        pins(window).title_bars[other].pin.click()
    assert not pins(window).is_pinned(window.layers_panel)
    assert not window.layers_panel.isVisible()
    assert tab(window).text() == "Layers" and tab(window).side == RIGHT
    assert pins(window).bars[RIGHT].isVisible()
    assert tab(window).height() > tab(window).width(), "written along the side"
    assert not pins(window).overlays[window.layers_panel].isVisible()
    assert not window.layers_panel.toggleViewAction().isEnabled()
    assert len(pins(window).tabs) == 3
    qtbot.waitUntil(lambda: drawing(window).width() > viewport_width)


def test_hovering_the_tab_slides_the_panel_out_over_the_drawing(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    hover(tab(window))
    overlay = pins(window).overlays[window.layers_panel]
    assert overlay.isVisible() and pins(window).open is window.layers_panel
    central = drawing(window)
    assert overlay.geometry().right() == central.right(), "against its side"
    assert overlay.geometry().height() == central.height()
    assert overlay.isAncestorOf(window.layers_panel.table)
    window.runner.execute("NEW")
    window.layers_panel.new_layer()
    assert window.layers_panel.table.rowCount() == 2, "it works while slid out"


def test_clicking_the_drawing_slides_it_back(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.runner.execute("NEW")
    title_bar(window).pin.click()
    tab(window).click()
    overlay = pins(window).overlays[window.layers_panel]
    assert overlay.isVisible()
    QTest.mouseClick(window.layers_panel.table.viewport(), Qt.MouseButton.LeftButton)
    assert overlay.isVisible(), "a click inside keeps it out"
    shown = window.tabs.currentWidget()
    assert shown is not None
    QTest.mouseClick(shown, Qt.MouseButton.LeftButton)
    assert not overlay.isVisible() and pins(window).open is None


def test_leaving_it_slides_it_back_unless_it_has_the_keyboard(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch
) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    hover(tab(window))
    overlay = pins(window).overlays[window.layers_panel]
    monkeypatch.setattr(AutoHide, "_under_mouse", staticmethod(lambda widget: widget is overlay))
    pins(window)._check_leave()
    assert overlay.isVisible(), "the mouse is on it"
    monkeypatch.setattr(AutoHide, "_under_mouse", staticmethod(lambda widget: False))
    window.layers_panel.table.setFocus()
    monkeypatch.setattr(
        QApplication, "focusWidget", staticmethod(lambda: window.layers_panel.table)
    )
    pins(window)._check_leave()
    assert overlay.isVisible(), "typing in it"
    monkeypatch.setattr(QApplication, "focusWidget", staticmethod(lambda: None))
    qtbot.waitUntil(lambda: not overlay.isVisible(), timeout=2000)


def test_pinning_again_keeps_it_open_where_it_was(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.addDockWidget(LEFT, window.history_panel)
    pins(window).title_bars[window.history_panel].pin.click()
    assert pins(window).tabs[window.history_panel].side == LEFT
    hover(pins(window).tabs[window.history_panel])
    overlay = pins(window).overlays[window.history_panel]
    assert overlay.geometry().left() == drawing(window).left()
    assert overlay.title_bar.pin.toolTip() == "Keep open"
    overlay.title_bar.pin.click()
    assert pins(window).is_pinned(window.history_panel)
    assert window.history_panel.isVisible() and window.dockWidgetArea(window.history_panel) == LEFT
    assert window.history_panel.widget() is window.history_panel.list
    assert window.history_panel not in pins(window).tabs
    assert not pins(window).bars[LEFT].isVisible()
    assert window.history_panel.toggleViewAction().isEnabled()


def test_closing_an_unpinned_panel(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    tab(window).click()
    pins(window).overlays[window.layers_panel].title_bar.close_button.click()
    assert pins(window).is_pinned(window.layers_panel)
    assert not window.layers_panel.isVisible() and not pins(window).bars[RIGHT].isVisible()
    window.layers_panel.toggleViewAction().trigger()
    assert window.layers_panel.isVisible()


def test_the_layer_command_slides_out_an_unpinned_panel(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    window.runner.execute("NEW")
    window.runner.execute("LA")
    assert pins(window).overlays[window.layers_panel].isVisible()


def test_unpinned_panels_survive_workspaces_restarts_and_reset(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    window = make_window(qtbot, settings)
    title_bar(window).pin.click()
    window.workspaces.show(None, "classic")
    window.workspaces.show(None, "ribbon")
    assert not window.layers_panel.isVisible() and pins(window).bars[RIGHT].isVisible()
    window.close()
    again = make_window(qtbot, settings)
    assert not pins(again).is_pinned(again.layers_panel)
    assert not again.layers_panel.isVisible() and tab(again).isVisible()
    again.workspaces.reset_action.trigger()
    assert pins(again).is_pinned(again.layers_panel) and again.layers_panel.isVisible()
    assert not pins(again).bars[RIGHT].isVisible()


def test_a_floating_panel_cannot_be_unpinned(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    window.layers_panel.setFloating(True)
    assert not pins(window).unpin(window.layers_panel)
    assert pins(window).is_pinned(window.layers_panel)


def test_unpinned_panels_wait_while_background_work_runs(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    overlay = pins(window).overlays[window.layers_panel]
    window.bus.publish(WorkStarted("Saving…"))
    assert not overlay.isEnabled()
    assert not pins(window).title_bars[window.history_panel].pin.isEnabled()
    pins(window).unpin(window.history_panel)  # as restoring a session does
    assert not pins(window).overlays[window.history_panel].isEnabled(), "unpinned while busy"
    window.bus.publish(WorkFinished())
    assert overlay.isEnabled() and pins(window).overlays[window.history_panel].isEnabled()


def test_one_panel_slides_out_at_a_time(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    pins(window).title_bars[window.history_panel].pin.click()
    hover(tab(window))
    hover(pins(window).tabs[window.history_panel])
    assert pins(window).overlays[window.history_panel].isVisible()
    assert not pins(window).overlays[window.layers_panel].isVisible()


def test_a_workspace_saved_before_unpinning_keeps_it_collapsed(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    window = make_window(qtbot, settings)
    window.workspaces.show(None, "classic")
    window.workspaces.show(None, "ribbon")
    title_bar(window).pin.click()
    window.workspaces.show(None, "classic")  # its arrangement still shows Layers
    assert not window.layers_panel.isVisible()
    assert pins(window).bars[RIGHT].isVisible() and tab(window).isVisible()


def test_pins_and_tabs_follow_the_language(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    try:
        window.change_language("es")
        assert tab(window).text() == "Capas"
        assert pins(window).overlays[window.layers_panel].title_bar.pin.toolTip() == (
            "Mantener abierto"
        )
        assert pins(window).title_bars[window.history_panel].pin.toolTip() == (
            "Ocultar automáticamente"
        )
    finally:
        set_language("en")


def test_the_open_panel_follows_the_window_size(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    title_bar(window).pin.click()
    hover(tab(window))
    overlay = pins(window).overlays[window.layers_panel]
    window.resize(1300, 800)
    qtbot.waitUntil(lambda: overlay.geometry().right() == drawing(window).right())
    assert overlay.geometry().height() == drawing(window).height()


def test_hover_events_reach_only_tabs(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    QApplication.sendEvent(window.layers_panel, QEvent(QEvent.Type.Enter))
    assert pins(window).open is None
