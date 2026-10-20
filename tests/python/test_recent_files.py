"""File > Open Recent: the last ten drawings opened or saved, remembered
across sessions, reopened with a click."""

import os
from pathlib import Path

from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QMenu
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.i18n import set_language
from arqstudiocad.recent_files import LIMIT, RecentFiles, same_file
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.recent_menu import RECENT_FILES_SETTING


def test_the_list_keeps_the_newest_first_without_repeats(tmp_path: Path) -> None:
    recent = RecentFiles()
    a, b = tmp_path / "a.dwg", tmp_path / "b.dxf"
    recent.add(a)
    recent.add(b)
    recent.add(a)
    assert recent.paths == [a, b]
    for i in range(LIMIT + 3):
        recent.add(tmp_path / f"{i}.dwg")
    assert len(recent.paths) == LIMIT == 10
    assert recent.paths[0] == tmp_path / f"{LIMIT + 2}.dwg"
    recent.remove(tmp_path / f"{LIMIT + 2}.dwg")
    assert recent.paths[0] == tmp_path / f"{LIMIT + 1}.dwg"
    again = RecentFiles(recent.to_strings())
    assert again.paths == recent.paths, "read back from the settings in order"
    recent.clear()
    assert recent.paths == []
    assert same_file(tmp_path / "X.dwg", tmp_path / "x.dwg") == (os.name == "nt")


def drawing(path: Path) -> Path:
    """A DXF with one line."""
    codes = ["0", "SECTION", "2", "ENTITIES", "0", "LINE", "8", "0", "10", "0", "20", "0"]
    codes += ["30", "0", "11", "1", "21", "0", "31", "0", "0", "ENDSEC", "0", "EOF", ""]
    path.write_text("\n".join(codes), encoding="ascii")
    return path


def entries(window: MainWindow) -> list[str]:
    return [a.text() for a in window.recent_menu.actions() if not a.isSeparator()]


def make(qtbot: QtBot, settings: QSettings, ui: QuietUi) -> MainWindow:
    window = MainWindow(settings, ui=ui)
    qtbot.addWidget(window)
    return window


def wait(qtbot: QtBot, window: MainWindow) -> None:
    qtbot.waitUntil(lambda: window.runner.active is None, timeout=30_000)


def test_opened_and_saved_drawings_join_the_list_and_are_remembered(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    ui = QuietUi()
    window = make(qtbot, settings, ui)
    file_menu = window.menuBar().actions()[0].menu()
    assert isinstance(file_menu, QMenu)
    texts = [a.text() for a in file_menu.actions()]
    assert texts[texts.index("&Open…") + 1] == "Open &Recent", "right after Open"
    assert entries(window) == ["No recent files"]
    assert not window.recent_menu.actions()[0].isEnabled()

    ui.open_path = drawing(tmp_path / "plano.dxf")
    window.runner.execute("OPEN")
    wait(qtbot, window)
    assert entries(window) == ["&1 plano.dxf", "Clear Recent Files"]
    assert window.recent_menu.actions()[0].toolTip() == str(tmp_path / "plano.dxf")

    ui.save_path = tmp_path / "copia.dxf"
    window.runner.execute("SAVEAS")
    wait(qtbot, window)
    assert entries(window)[:2] == ["&1 copia.dxf", "&2 plano.dxf"]
    settings.sync()

    later = make(
        qtbot, QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat), QuietUi()
    )
    assert entries(later)[:2] == ["&1 copia.dxf", "&2 plano.dxf"], "remembered"


def test_a_click_opens_the_drawing_or_shows_it_if_open(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    plano = drawing(tmp_path / "plano.dxf")
    settings.setValue(RECENT_FILES_SETTING, [str(plano)])
    window = make(qtbot, settings, QuietUi())
    window.recent_menu.actions()[0].trigger()
    wait(qtbot, window)
    assert [d.name for d in window.session.documents] == ["plano.dxf"]
    window.runner.execute("NEW")
    assert window.session.active is not None and window.session.active.name != "plano.dxf"
    window.recent_menu.actions()[0].trigger()
    wait(qtbot, window)
    assert len(window.session.documents) == 2, "not opened twice"
    assert window.session.active is not None and window.session.active.name == "plano.dxf"


def test_a_missing_drawing_leaves_the_list_and_the_list_can_be_cleared(
    qtbot: QtBot, tmp_path: Path
) -> None:
    settings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    kept = drawing(tmp_path / "plano.dxf")
    settings.setValue(RECENT_FILES_SETTING, [str(tmp_path / "borrado.dwg"), str(kept)])
    window = make(qtbot, settings, QuietUi())
    assert entries(window)[:2] == ["&1 borrado.dwg", "&2 plano.dxf"]
    window.recent_menu.actions()[0].trigger()
    history = window.command_window.history.toPlainText()
    assert "borrado.dwg was not found; it leaves the recent files." in history
    assert entries(window)[0] == "&1 plano.dxf" and window.session.documents == []
    clear = [a for a in window.recent_menu.actions() if a.text() == "Clear Recent Files"]
    clear[0].trigger()
    assert entries(window) == ["No recent files"]
    assert settings.value(RECENT_FILES_SETTING, []) in ([], None, "")


def test_open_recent_in_spanish(qtbot: QtBot, tmp_path: Path) -> None:
    window = make(qtbot, QSettings(str(tmp_path / "s.ini"), QSettings.Format.IniFormat), QuietUi())
    window.change_language("es")
    set_language("es")
    assert window.recent_menu.title() == "Abrir &recientes"
    assert entries(window) == ["No hay archivos recientes"]


def test_a_single_recent_drawing_reads_back_from_the_settings_file(
    qtbot: QtBot, tmp_path: Path
) -> None:
    # Written by one session, read by the next.
    written = QSettings(str(tmp_path / "s.ini"), QSettings.Format.IniFormat)
    written.setValue(RECENT_FILES_SETTING, [str(tmp_path / "solo.dwg")])
    written.sync()
    read = QSettings(str(tmp_path / "s.ini"), QSettings.Format.IniFormat)
    window = make(qtbot, read, QuietUi())
    assert entries(window)[0] == "&1 solo.dwg"
