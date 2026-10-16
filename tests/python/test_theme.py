"""Dark and light themes (ADR 0015)."""

from pathlib import Path

from PySide6.QtCore import QSettings
from PySide6.QtGui import QPalette
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.theme import THEMES, Theme, as_theme, palette


def window_color(app: QApplication) -> int:
    return app.palette().color(QPalette.ColorRole.Window).lightness()


def current(window: MainWindow) -> Theme:
    return window.theme


def test_dark_is_the_default_and_light_is_lighter() -> None:
    assert THEMES[0] == "dark"
    dark, light = palette("dark"), palette("light")
    for role in (QPalette.ColorRole.Window, QPalette.ColorRole.Base):
        assert dark.color(role).lightness() < 80 < light.color(role).lightness()
    assert dark.color(QPalette.ColorRole.Text).lightness() > 180
    disabled = dark.color(QPalette.ColorGroup.Disabled, QPalette.ColorRole.Text)
    assert disabled.lightness() < dark.color(QPalette.ColorRole.Text).lightness()
    assert as_theme("light") == "light" and as_theme("purple") is None and as_theme(None) is None


def test_the_window_switches_and_remembers_the_theme(qtbot: QtBot, tmp_path: Path) -> None:
    app = QApplication.instance()
    assert isinstance(app, QApplication)
    original, original_style = app.palette(), app.style().name()
    app.setStyle("Windows")
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    window = MainWindow(settings, ui=QuietUi())
    qtbot.addWidget(window)
    try:
        assert current(window) == "dark" and window.theme_actions["dark"].isChecked()
        window.theme_actions["light"].trigger()
        assert current(window) == "light" and window_color(app) > 128
        assert app.style().name().lower() == "fusion"
        window.theme_actions["dark"].trigger()
        assert window_color(app) < 80
        window.change_theme("light")
        again = MainWindow(settings, ui=QuietUi())
        qtbot.addWidget(again)
        assert again.theme == "light" and again.theme_actions["light"].isChecked()
    finally:
        app.setStyle(original_style)
        app.setPalette(original)


def test_pins_and_sections_are_redrawn_in_the_new_colors(qtbot: QtBot) -> None:
    app = QApplication.instance()
    assert isinstance(app, QApplication)
    original = app.palette()
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    window.show()
    qtbot.waitExposed(window)
    window.runner.execute("NEW")
    title_bar = window.auto_hide.title_bars[window.layers_panel]
    section = window.properties_panel.grid.tree.topLevelItem(0)
    assert section is not None
    layers = window.layers_panel.table
    grid = window.properties_panel.grid.tree
    try:
        window.change_theme("light")
        light_pin = title_bar.pin.icon().pixmap(16, 16).toImage()
        light_section = section.background(0).color()
        assert window.layers_panel.table.showGrid() is False
        assert layers.alternatingRowColors()
        assert app.palette().base().color().name() in layers.styleSheet()
        assert app.palette().base().color().name() in grid.styleSheet()
        window.change_theme("dark")
        assert title_bar.pin.icon().pixmap(16, 16).toImage() != light_pin
        assert section.background(0).color() != light_section
        assert app.palette().base().color().name() in layers.styleSheet()
        assert app.palette().base().color().name() in grid.styleSheet()
    finally:
        app.setPalette(original)
