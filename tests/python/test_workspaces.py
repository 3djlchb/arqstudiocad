"""Workspaces: a discipline shown in a layout (ADR 0015)."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QStackedWidget, QToolBar
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.i18n import set_language, tr
from arqstudiocad.ui.actions import SHORTCUTS
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import Ribbon
from arqstudiocad.ui.workspace_switcher import WorkspaceSwitcher
from arqstudiocad.workspaces import (
    CAD,
    Discipline,
    RibbonGroup,
    RibbonTab,
    Toolbar,
    WorkspaceRegistry,
    builtin_workspaces,
)

#: A ribbon-only discipline, as BIM and the others will be.
BIM = Discipline(
    name="bim",
    title=lambda: "BIM",
    ribbon=(RibbonTab("model", lambda: "Model", (RibbonGroup("g", lambda: "G", ("UNDO",)),)),),
)


def settings_in(tmp_path: Path) -> QSettings:
    return QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)


def make_window(
    qtbot: QtBot, settings: QSettings | None = None, workspaces: WorkspaceRegistry | None = None
) -> MainWindow:
    window = MainWindow(settings, ui=QuietUi(), workspaces=workspaces)
    qtbot.addWidget(window)
    window.show()
    qtbot.waitExposed(window)
    return window


def with_bim() -> WorkspaceRegistry:
    registry = builtin_workspaces()
    registry.register(BIM)
    return registry


def switcher(window: MainWindow) -> WorkspaceSwitcher:
    return window.workspaces


def toolbars(window: MainWindow) -> list[QToolBar]:
    built = switcher(window).widgets("cad")
    assert built is not None
    return built.toolbars


def ribbon(window: MainWindow, discipline: str = "cad") -> Ribbon:
    built = switcher(window).widgets(discipline)
    assert built is not None and built.ribbon is not None
    return built.ribbon


def ribbon_shown(window: MainWindow, discipline: str = "cad") -> bool:
    built = switcher(window).widgets(discipline)
    return built is not None and built.ribbon_bar is not None and built.ribbon_bar.isVisible()


# The data


def test_every_tool_is_a_command_with_an_action() -> None:
    commands = builtin_registry()
    for discipline in builtin_workspaces():
        for name in discipline.commands():
            assert name in SHORTCUTS, f"{name} has no action"
            assert commands.find(name) is not None, f"{name} is not a command"


def test_only_2d_drafting_has_the_classic_layout() -> None:
    assert CAD.layouts == ("ribbon", "classic")
    assert BIM.layouts == ("ribbon",)


def test_a_discipline_needs_a_ribbon_and_a_unique_name() -> None:
    with pytest.raises(ValueError, match="no ribbon"):
        Discipline("empty", lambda: "Empty", ribbon=())
    registry = builtin_workspaces()
    with pytest.raises(ValueError, match="already registered"):
        registry.register(CAD)


# The window


def test_the_first_run_shows_the_ribbon(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    assert ribbon_shown(window)
    assert not any(toolbar.isVisible() for toolbar in toolbars(window))
    assert switcher(window).button.text() == "2D drafting · Ribbon"
    assert switcher(window).layout_actions["ribbon"].isChecked()


def test_switching_to_classic_and_remembering_it(qtbot: QtBot, tmp_path: Path) -> None:
    settings = settings_in(tmp_path)
    window = make_window(qtbot, settings)
    switcher(window).layout_actions["classic"].trigger()
    assert all(toolbar.isVisible() for toolbar in toolbars(window))
    assert not ribbon_shown(window)
    assert switcher(window).button.text() == "2D drafting · Classic"
    window.close()
    again = make_window(qtbot, settings)
    assert switcher(again).layout == "classic"
    assert all(toolbar.isVisible() for toolbar in toolbars(again))


def test_each_workspace_keeps_its_arrangement(qtbot: QtBot, tmp_path: Path) -> None:
    window = make_window(qtbot, settings_in(tmp_path))
    window.history_panel.hide()
    switcher(window).show(None, "classic")
    window.history_panel.show()
    switcher(window).show(None, "ribbon")
    assert not window.history_panel.isVisible(), "the ribbon's arrangement is back"
    switcher(window).show(None, "classic")
    assert window.history_panel.isVisible()


def test_a_ribbon_only_discipline_disables_classic(qtbot: QtBot, tmp_path: Path) -> None:
    window = make_window(qtbot, settings_in(tmp_path), with_bim())
    switcher(window).show("cad", "classic")
    switcher(window).discipline_actions["bim"].trigger()
    assert switcher(window).layout == "ribbon" and ribbon_shown(window, "bim")
    assert not switcher(window).layout_actions["classic"].isEnabled()
    assert not ribbon_shown(window, "cad")
    assert not any(toolbar.isVisible() for toolbar in toolbars(window))
    switcher(window).show("bim", "classic")
    assert switcher(window).layout == "ribbon", "BIM has no Classic layout"
    switcher(window).discipline_actions["cad"].trigger()
    assert switcher(window).layout == "classic", "2D drafting comes back as it was"
    assert switcher(window).layout_actions["classic"].isEnabled()


def test_only_the_shown_discipline_has_toolbars(qtbot: QtBot) -> None:
    registry = builtin_workspaces()
    registry.register(
        Discipline("topo", lambda: "Topo", BIM.ribbon, (Toolbar("t", lambda: "T", ("REDO",)),))
    )
    window = make_window(qtbot, workspaces=registry)
    switcher(window).show("cad", "classic")
    switcher(window).show("topo", None)
    assert switcher(window).layout == "classic"
    built = switcher(window).widgets("topo")
    assert built is not None and built.toolbars[0].isVisible()
    assert not any(toolbar.isVisible() for toolbar in toolbars(window))


def test_an_unknown_remembered_discipline_falls_back(qtbot: QtBot, tmp_path: Path) -> None:
    settings = settings_in(tmp_path)
    settings.setValue("workspace/discipline", "gone")
    settings.setValue("workspace/cad/layout", "sideways")
    window = make_window(qtbot, settings)
    assert switcher(window).discipline == CAD and switcher(window).layout == "ribbon"


def test_the_workspace_menu_is_in_the_view_menu(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    assert switcher(window).menu.menuAction() in window._view_menu.actions()
    assert switcher(window).button.menu() is switcher(window).menu


# The ribbon


def test_ribbon_tabs_groups_and_buttons(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    shown = ribbon(window)
    assert [shown.tabText(i) for i in range(shown.count())] == ["Home", "View"]
    titles = [group.title.text() for group in shown.groups]
    assert titles == ["File", "Draw", "History", "Layers", "Navigate"]
    new_button = shown.groups[0].buttons[0]
    assert new_button.defaultAction() is window.command_actions["NEW"]
    new_button.click()
    assert window.session.active is not None


def test_double_clicking_a_tab_folds_the_ribbon(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    shown = ribbon(window)
    stack = shown.findChild(QStackedWidget)
    assert stack is not None and stack.isVisible()
    tall = shown.height()
    shown.tabBar().tabBarDoubleClicked.emit(0)
    assert shown.folded and not stack.isVisible()
    qtbot.waitUntil(lambda: shown.height() < tall)
    shown.tabBar().tabBarDoubleClicked.emit(1)
    assert not shown.folded and stack.isVisible()


def test_workspaces_follow_the_language(qtbot: QtBot) -> None:
    window = make_window(qtbot)
    try:
        window.change_language("es")
        assert ribbon(window).tabText(0) == tr("Home") == "Inicio"
        assert ribbon(window).groups[0].title.text() == "Archivo"
        assert switcher(window).button.text() == "Dibujo 2D · Cinta de opciones"
        assert switcher(window).menu.title() == "&Espacio de trabajo"
        switcher(window).show(None, "classic")
        assert toolbars(window)[0].windowTitle() == "Estándar"
    finally:
        set_language("en")
