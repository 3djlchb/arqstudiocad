"""Document tabs, background open and save, and quitting with unsaved
changes (ADR 0013, D5.7). The window's real worker thread is used."""

from pathlib import Path

import pytest
from gil_helpers import big_dxf
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.events import CommandCancelled, DocumentSaved, Event
from arqstudiocad.ui.main_window import MainWindow


@pytest.fixture
def ui() -> QuietUi:
    return QuietUi()


@pytest.fixture
def window(qtbot: QtBot, ui: QuietUi) -> MainWindow:
    w = MainWindow(ui=ui)
    qtbot.addWidget(w)
    w.resize(900, 600)
    w.show()
    qtbot.waitExposed(w)
    return w


def history(window: MainWindow) -> list[str]:
    return window.command_window.history.toPlainText().splitlines()


def wait_idle(qtbot: QtBot, window: MainWindow) -> None:
    qtbot.waitUntil(lambda: window.runner.busy is None, timeout=30_000)


def test_opening_runs_in_the_background(
    qtbot: QtBot, window: MainWindow, ui: QuietUi, tmp_path: Path
) -> None:
    ui.open_path = big_dxf(tmp_path / "grande.dxf", 50_000)
    window.command_actions.run("OPEN")
    # The command now waits for the worker; the window keeps going.
    assert window.runner.busy == "Opening grande.dxf…"
    assert window._busy.isVisible() and window.statusBar().currentMessage() == "Opening grande.dxf…"
    window.runner.execute("NEW")
    assert history(window)[-1] == "Please wait: Opening grande.dxf…"
    window.runner.cancel()
    assert window.runner.busy is not None, "opening cannot be interrupted"
    wait_idle(qtbot, window)
    assert not window._busy.isVisible() and window.statusBar().currentMessage() == ""
    assert window.tabs.count() == 1 and window.tabs.tabText(0) == "grande.dxf"
    assert history(window)[-1].startswith("Opened grande.dxf: 50000 entities")
    assert window.session.active is not None and len(window.session.documents) == 1


def test_a_failed_open_adds_nothing(
    qtbot: QtBot, window: MainWindow, ui: QuietUi, tmp_path: Path
) -> None:
    events: list[Event] = []
    window.bus.subscribe(CommandCancelled, events.append)
    ui.open_path = tmp_path / "no-existe.dwg"
    window.command_actions.run("OPEN")
    wait_idle(qtbot, window)
    assert window.tabs.count() == 0 and window.session.documents == []
    assert history(window)[-1].startswith("Error: ")
    assert isinstance(events[-1], CommandCancelled) and events[-1].reason.startswith("io_error")


def test_saving_runs_in_the_background(
    qtbot: QtBot, window: MainWindow, ui: QuietUi, tmp_path: Path
) -> None:
    saved: list[Event] = []
    window.bus.subscribe(DocumentSaved, saved.append)
    window.command_actions.run("NEW")
    document = window.session.active
    assert document is not None
    _, changes = document.add_line((0, 0), (10, 10))
    window.session.report(document, changes)
    assert window.tabs.tabText(0) == "Drawing1*"
    ui.save_path = tmp_path / "planta.dwg"
    window.command_actions.run("SAVE")
    assert window.runner.busy == "Saving planta.dwg…"
    assert saved == [], "announced only when written"
    wait_idle(qtbot, window)
    assert saved == [DocumentSaved(document.id, tmp_path / "planta.dwg")]
    assert (tmp_path / "planta.dwg").is_file()
    assert window.tabs.tabText(0) == "planta.dwg" and window.tabs.tabToolTip(0) == str(
        tmp_path / "planta.dwg"
    )


def test_tabs_follow_the_documents(qtbot: QtBot, window: MainWindow, ui: QuietUi) -> None:
    assert window._stack.currentWidget() is window._welcome
    window.command_actions.run("NEW")
    window.command_actions.run("NEW")
    first, second = window.session.documents
    assert [window.tabs.tabText(i) for i in range(2)] == ["Drawing1", "Drawing2"]
    assert window._stack.currentWidget() is window.tabs
    assert window.tabs.currentIndex() == 1 and window.session.active == second

    window.tabs.setCurrentIndex(0)  # the user picks a tab
    assert window.session.active == first

    _, changes = second.add_circle((0, 0), 1)
    window.session.report(second, changes)
    ui.save_changes = "no"
    window.tabs.tabCloseRequested.emit(1)  # its close button: CLOSE, which asks
    assert ui.asked[-1] == "changes Drawing2"
    assert window.session.documents == [first] and window.tabs.count() == 1
    window.tabs.tabCloseRequested.emit(0)
    assert window.tabs.count() == 0 and window._stack.currentWidget() is window._welcome


def test_quitting_asks_about_unsaved_drawings(
    qtbot: QtBot, window: MainWindow, ui: QuietUi, tmp_path: Path
) -> None:
    window.command_actions.run("NEW")
    window.command_actions.run("NEW")
    kept, changed = window.session.documents
    _, changes = changed.add_line((0, 0), (1, 1))
    window.session.report(changed, changes)

    ui.save_changes = "cancel"
    assert not window.close() and window.isVisible(), "cancel keeps the window"
    assert ui.asked == ["changes Drawing2"], "only the modified drawing is asked about"

    ui.save_changes = "yes"
    ui.save_path = None  # ...but the file dialog is cancelled: stay
    assert not window.close()

    ui.save_path = tmp_path / "antes de salir.dxf"
    assert window.close()
    assert (tmp_path / "antes de salir.dxf").is_file() and not changed.is_modified
    assert kept.is_open, "closing the window does not close documents by itself"


def test_quitting_waits_for_background_work(
    qtbot: QtBot, window: MainWindow, ui: QuietUi, tmp_path: Path
) -> None:
    ui.open_path = big_dxf(tmp_path / "grande.dxf", 50_000)
    window.command_actions.run("OPEN")
    assert not window.close()
    assert history(window)[-1] == "Please wait: Opening grande.dxf…"
    wait_idle(qtbot, window)
    assert window.close()
