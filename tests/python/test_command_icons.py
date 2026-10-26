"""Command and drafting icons (maintainer, 2026-10-25): the ribbon's draw
commands at 54 px, one button per row, and ORTHO's icon in the status bar
where the theme has artwork for it."""

import pytest
from PySide6.QtCore import QSize, Qt
from PySide6.QtWidgets import QGridLayout
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.ui.command_icons import (
    RIBBON_ICON,
    command_icon,
    drafting_icon,
    draw_icon_files,
)
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import Ribbon, RibbonGroupWidget, RibbonPage
from arqstudiocad.ui.theme import THEMES


@pytest.mark.parametrize("command", ["LINE", "PLINE", "CIRCLE", "ARC"])
def test_the_draw_commands_have_54_px_artwork_in_the_dark_theme(qtbot: QtBot, command: str) -> None:
    # qtbot: drawing a pixmap needs the application.
    dark = draw_icon_files(command, "dark")
    assert sorted(dark) == [18, 24, 32, 48, 54] and all(p.is_file() for p in dark.values())
    light = draw_icon_files(command, "light")
    assert sorted(light) == [24, 32, 48] and all(p.is_file() for p in light.values()), (
        "the light theme scales its 48 px drawing until its own arrives"
    )
    for theme in THEMES:
        pixmap = command_icon(command, theme).pixmap(54, 54)
        assert not pixmap.isNull() and pixmap.width() == 54
    assert draw_icon_files("ERASE", "dark") == {} and command_icon("ERASE", "dark").isNull()


@pytest.mark.parametrize("switch", ["ORTHO", "OSNAP"])
def test_the_drafting_switches_have_icons_in_the_dark_theme_only(switch: str) -> None:
    assert not drafting_icon(switch, "dark").isNull()
    assert drafting_icon(switch, "light").isNull()
    assert drafting_icon("POLAR", "dark").isNull(), "no artwork yet"


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    w = MainWindow(ui=QuietUi())
    qtbot.addWidget(w)
    w.resize(1100, 700)
    w.show()
    qtbot.waitExposed(w)
    return w


def test_the_ribbon_has_one_button_per_row_at_54_px(window: MainWindow) -> None:
    groups = window.findChildren(RibbonGroupWidget)
    assert groups
    for group in groups:
        grid = group.findChild(QGridLayout)
        assert grid is not None
        assert grid.rowCount() == 1, f"{group.group.name}: one row"
    (draw,) = [g for g in groups if "LINE" in g.group.commands]
    line = draw.buttons[draw.group.commands.index("LINE")]
    assert line.iconSize() == QSize(RIBBON_ICON, RIBBON_ICON) == QSize(54, 54)
    assert line.toolButtonStyle() == Qt.ToolButtonStyle.ToolButtonTextUnderIcon


def test_the_switches_show_their_icon_alone_and_their_name_without_artwork(
    window: MainWindow,
) -> None:
    bar = window.drafting_bar
    switches = [
        (bar.ortho_action, bar.ortho_button, "ORTHO", "Orthogonal mode (F8)"),
        (
            bar.osnap_action,
            bar.osnap_button,
            "OSNAP",
            "Object snaps (F3). Right-click to choose them.",
        ),
    ]
    assert window.theme == "dark"
    for action, button, name, tip in switches:
        assert button.toolButtonStyle() == Qt.ToolButtonStyle.ToolButtonIconOnly, name
        assert not action.icon().isNull() and button.iconSize() == QSize(18, 18)
        assert action.text() == name and action.toolTip() == tip, (
            "the name stays for tooltips and screen readers"
        )
    window.change_theme("light")
    for action, button, name, _ in switches:
        assert button.toolButtonStyle() == Qt.ToolButtonStyle.ToolButtonTextOnly, name
        assert action.icon().isNull()
    window.change_theme("dark")
    for _, button, name, _ in switches:
        assert button.toolButtonStyle() == Qt.ToolButtonStyle.ToolButtonIconOnly, name


def test_a_ribbon_wider_than_the_window_scrolls_sideways(qtbot: QtBot, window: MainWindow) -> None:
    """One button per row makes a tab wide: it scrolls, so the window can
    still be narrow (small screens)."""
    pages = window.findChildren(RibbonPage)
    widest = max(pages, key=lambda p: p.sizeHint().width())
    content = widest.widget()
    assert content is not None
    assert window.minimumSizeHint().width() < content.sizeHint().width(), "the window stays narrow"
    window.resize(window.minimumSizeHint().width(), 700)
    (ribbon,) = [r for r in window.findChildren(Ribbon) if r.indexOf(widest) >= 0]
    ribbon.setCurrentWidget(widest)
    qtbot.waitUntil(lambda: widest.horizontalScrollBar().isVisible())
    assert widest.height() >= content.sizeHint().height() + (
        widest.horizontalScrollBar().sizeHint().height()
    ), "room for the buttons and the scroll bar"
