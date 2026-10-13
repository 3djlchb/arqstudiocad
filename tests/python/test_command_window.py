"""The command window and the command menus (ADR 0013, D5.1 and D5.4)."""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

import pytest
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from pytestqt.qtbot import QtBot

from arqstudiocad.commands import CommandContext, CommandRunner, ViewControl
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadDocument
from arqstudiocad.events import EventBus
from arqstudiocad.product import APP_NAME
from arqstudiocad.session import Session
from arqstudiocad.ui.command_window import CommandWindow
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.main_window import MainWindow


@dataclass
class Zooms:
    count: int = 0

    def zoom_extents(self) -> bool:
        self.count += 1
        return True

    def zoom(self, factor: float) -> None:
        self.count += 1


@dataclass
class QuietUi:
    """No dialogs: every file dialog is cancelled; one view for everything."""

    zooms: Zooms = field(default_factory=Zooms)

    def choose_open_path(self) -> Path | None:
        return None

    def choose_save_path(self, suggested: str) -> Path | None:
        return None

    def ask_save_changes(self, name: str) -> Literal["yes", "no", "cancel"]:
        return "no"

    def view(self, document: CadDocument) -> ViewControl | None:
        return self.zooms


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    w = MainWindow(ui=QuietUi())
    qtbot.addWidget(w)
    return w


def type_line(
    qtbot: QtBot, window: CommandWindow, text: str, key: Qt.Key = Qt.Key.Key_Return
) -> None:
    QTest.keyClicks(window.input, text)
    QTest.keyClick(window.input, key)


def history(window: CommandWindow) -> list[str]:
    return window.history.toPlainText().splitlines()


def test_typing_a_command(qtbot: QtBot, window: MainWindow) -> None:
    cw = window.command_window
    assert cw.prompt.text() == "Command:"
    type_line(qtbot, cw, "new")
    assert len(window.session.documents) == 1
    assert history(cw) == ["Command: new", "New drawing: Drawing1"]
    assert window.windowTitle() == f"Drawing1 — {APP_NAME}"
    assert cw.input.text() == ""


def test_space_is_enter_and_up_recalls(qtbot: QtBot, window: MainWindow) -> None:
    cw = window.command_window
    type_line(qtbot, cw, "new", Qt.Key.Key_Space)
    assert len(window.session.documents) == 1
    QTest.keyClick(cw.input, Qt.Key.Key_Up)
    assert cw.input.text() == "new"
    QTest.keyClick(cw.input, Qt.Key.Key_Return)
    assert len(window.session.documents) == 2
    QTest.keyClick(cw.input, Qt.Key.Key_Return)  # empty: repeats NEW
    assert len(window.session.documents) == 3


def test_prompts_and_esc(qtbot: QtBot, window: MainWindow) -> None:
    cw = window.command_window
    type_line(qtbot, cw, "new")
    type_line(qtbot, cw, "z")
    assert cw.prompt.text() == "Zoom [Extents(E)/In(I)/Out(O)] <E>:"
    assert cw.input.completer() is None, "answers are not completed as commands"
    QTest.keyClick(cw.input, Qt.Key.Key_Escape)
    assert cw.prompt.text() == "Command:"
    assert history(cw)[-2:] == ["Command: z", "*Cancel*"]
    assert cw.input.completer() is not None
    type_line(qtbot, cw, "z")
    type_line(qtbot, cw, "")  # the default, Extents
    assert window.runner.active is None
    assert history(cw)[-1] == "Zoom [Extents(E)/In(I)/Out(O)] <E>:"


def test_space_is_a_character_while_typing_text(
    qtbot: QtBot, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Without an interface, SAVEAS asks for the file name on the line. Work
    # in a temporary folder: if Space ever sent the line, "my" would be saved.
    monkeypatch.chdir(tmp_path)
    bus = EventBus()
    bridge = QtEventBridge(bus)
    session = Session(bus)
    runner = CommandRunner(builtin_registry(), CommandContext(session, None))
    cw = CommandWindow(runner, bridge)
    qtbot.addWidget(cw)
    type_line(qtbot, cw, "new")
    type_line(qtbot, cw, "saveas")
    assert cw.prompt.text() == "Save as <Drawing1.dxf>:"
    QTest.keyClicks(cw.input, "my plan")
    assert cw.input.text() == "my plan", "the space stayed in the name"
    QTest.keyClick(cw.input, Qt.Key.Key_Escape)
    assert runner.active is None


def test_menu_actions_start_commands(qtbot: QtBot, window: MainWindow) -> None:
    window.command_actions["NEW"].trigger()
    document = window.session.active
    assert document is not None
    _, changes = document.add_line((0, 0), (1, 1))
    window.session.report(document, changes)
    assert window.windowTitle() == f"Drawing1* — {APP_NAME}", "starred while modified"
    window.command_actions["UNDO"].trigger()
    assert document.get_entities() == []
    assert history(window.command_window)[-1] == "Undone: LINE"
    assert window.windowTitle() == f"Drawing1 — {APP_NAME}"
    window.command_actions["CLOSE"].trigger()
    assert window.session.documents == [] and window.windowTitle() == APP_NAME


def test_a_menu_action_cancels_the_running_command(qtbot: QtBot, window: MainWindow) -> None:
    cw = window.command_window
    type_line(qtbot, cw, "new")
    type_line(qtbot, cw, "zoom")
    assert window.runner.active == "ZOOM"
    window.command_actions["NEW"].trigger()
    assert window.runner.active is None
    assert "*Cancel*" in history(cw) and len(window.session.documents) == 2
