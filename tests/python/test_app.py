from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad import _core
from arqstudiocad.__main__ import main
from arqstudiocad.application import choose_language
from arqstudiocad.check import run_checks
from arqstudiocad.i18n import current_language
from arqstudiocad.ui.main_window import MainWindow


def test_check_passes_from_source_tree(qtbot: QtBot) -> None:
    results = run_checks()
    failed = [r for r in results if not r.ok]
    assert [r.name for r in results] == [
        "native core",
        "language packs",
        "credits (AUTHORS)",
        "Qt",
        "settings",
    ]
    assert not failed, failed


def test_check_command_exit_code(qtbot: QtBot) -> None:
    assert main(["--check"]) == 0


def test_choose_language_order() -> None:
    assert choose_language("es", "en", "en_US") == "es"
    assert choose_language(None, "es", "en_US") == "es"
    assert choose_language(None, None, "es_EC") == "es"
    assert choose_language("xx", 42, "zz_ZZ") == "en"


def test_window_switches_language(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    assert window.menuBar().actions()[0].text() == "&File"

    window.change_language("es")

    assert current_language() == "es"
    assert [a.text() for a in window.menuBar().actions()] == [
        "&Archivo",
        "&Edición",
        "&Ver",
        "&Herramientas",
        "&Dibujo",
        "Ay&uda",
    ]
    assert window.command_actions["SAVEAS"].text() == "Guardar &como…"
    assert window.command_window.prompt.text() == "Comando:"


def test_about_shows_versions_and_credits(qtbot: QtBot) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    text = window.about_text()
    assert _core.__version__ in text
    assert "GPL-3.0-or-later" in text
    assert "@3djlchb" in text
