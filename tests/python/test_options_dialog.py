"""The Options dialog (ADR 0029 D3, D4): its sections in a sidebar, each
subsection a titled group box, Reset section / OK / Apply / Cancel, its size
and last section remembered; General holds the language and the theme, which
the window applies as the Tools menu does."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings, QSize
from PySide6.QtWidgets import QApplication, QDialog, QGroupBox
from pytestqt.qtbot import QtBot

from arqstudiocad.i18n import current_language, set_language
from arqstudiocad.ui.main_window import LANGUAGE_SETTING, MainWindow
from arqstudiocad.ui.options_dialog import (
    PRIMARY,
    SECTION_SETTING,
    SIZE,
    SIZE_SETTING,
    Options,
    OptionsDialog,
)
from arqstudiocad.ui.theme import THEME_SETTING, apply_theme

#: The dialog's values on a first start.
DEFAULT = Options()


class Applied:
    def __init__(self) -> None:
        self.calls: list[Options] = []

    def __call__(self, values: Options) -> None:
        self.calls.append(values)


def dialog(
    qtbot: QtBot, values: Options = DEFAULT, settings: QSettings | None = None
) -> tuple[OptionsDialog, Applied]:
    applied = Applied()
    d = OptionsDialog(values, applied, settings)
    qtbot.addWidget(d)
    return d, applied


def test_general_holds_the_language_and_the_theme(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot)
    assert d.windowTitle() == "Options" and d.isModal()
    assert [d.sidebar.item(r).text() for r in range(d.sidebar.count())] == ["General"]
    assert d.section() == "general" and d.size() == SIZE
    titles = [g.title() for g in d.findChildren(QGroupBox)]
    assert titles == ["Language", "Theme"]
    languages = [d.language.itemText(i) for i in range(d.language.count())]
    assert languages == ["System language", "English", "Español"], "each by its own name"
    assert d.language.currentData() == ""
    assert [d.theme.itemText(i) for i in range(d.theme.count())] == ["Dark", "Light"]
    assert [b.text() for b in (d.reset_button, d.ok_button, d.apply_button, d.cancel_button)] == [
        "Reset section",
        "OK",
        "Apply",
        "Cancel",
    ]
    assert d.ok_button.isDefault(), "Enter is OK"
    assert d.language_group.font().bold(), "the group's title"
    assert not d.language_label.font().bold() and not d.language.font().bold()
    assert PRIMARY in d.ok_button.styleSheet() and PRIMARY in d.sidebar.styleSheet()


def test_apply_applies_and_stays_open(qtbot: QtBot) -> None:
    d, applied = dialog(qtbot, Options("es", "dark"))
    assert d.language.currentData() == "es"
    assert not d.apply_button.isEnabled(), "nothing to apply"
    d.theme.setCurrentIndex(d.theme.findData("light"))
    assert d.apply_button.isEnabled()
    d.apply_button.click()
    assert applied.calls == [Options("es", "light")]
    assert not d.apply_button.isEnabled() and d.result() == 0
    d.theme.setCurrentIndex(d.theme.findData("dark"))
    d.theme.setCurrentIndex(d.theme.findData("light"))
    assert not d.apply_button.isEnabled(), "back as applied"


def test_ok_applies_and_closes_cancel_discards(qtbot: QtBot) -> None:
    d, applied = dialog(qtbot)
    d.language.setCurrentIndex(d.language.findData("es"))
    d.ok_button.click()
    assert applied.calls == [Options("es", "dark")]
    assert d.result() == QDialog.DialogCode.Accepted
    d, applied = dialog(qtbot)
    d.language.setCurrentIndex(d.language.findData("es"))
    d.cancel_button.click()
    assert applied.calls == [] and d.result() == QDialog.DialogCode.Rejected
    d, applied = dialog(qtbot)
    d.ok_button.click()
    assert applied.calls == [], "nothing changed: nothing applied"


def test_reset_section_restores_its_defaults_until_applied(qtbot: QtBot) -> None:
    d, applied = dialog(qtbot, Options("es", "light"))
    d.reset_button.click()
    assert (d.language.currentData(), d.theme.currentData()) == ("", "dark")
    assert applied.calls == [] and d.apply_button.isEnabled()
    d.apply_button.click()
    assert applied.calls == [Options("", "dark")]


def test_a_language_applied_names_the_dialog_again(qtbot: QtBot) -> None:
    def to_spanish(values: Options) -> None:
        set_language(values.language)

    d = OptionsDialog(Options(), to_spanish)
    qtbot.addWidget(d)
    try:
        d.language.setCurrentIndex(d.language.findData("es"))
        d.apply_button.click()
        assert d.windowTitle() == "Opciones"
        assert d.sidebar.item(0).text() == "General"
        assert d.apply_button.text() == "Aplicar" and d.reset_button.text() == "Restablecer sección"
        assert d.language.itemText(0) == "Idioma del sistema"
        assert d.language.currentData() == "es", "the choice is kept"
        assert [g.title() for g in d.findChildren(QGroupBox)] == ["Idioma", "Tema"]
    finally:
        set_language("en")


def test_a_theme_applied_colors_the_dialog_again(qtbot: QtBot) -> None:
    app = QApplication.instance()
    assert isinstance(app, QApplication)

    def themed(values: Options) -> None:
        apply_theme(app, values.theme)

    before = app.palette()
    try:
        apply_theme(app, "dark")
        d = OptionsDialog(Options(), themed)
        qtbot.addWidget(d)
        # Open, as it is while OK or Apply can be clicked.
        d.show()
        dark = d.sidebar.styleSheet()
        d.theme.setCurrentIndex(d.theme.findData("light"))
        d.apply_button.click()
        qtbot.waitUntil(lambda: d.sidebar.styleSheet() != dark)
    finally:
        app.setPalette(before)


def test_size_and_section_are_remembered(qtbot: QtBot, tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "s.ini"), QSettings.Format.IniFormat)
    d, _ = dialog(qtbot, settings=settings)
    d.resize(950, 700)
    d.cancel_button.click()
    assert settings.value(SIZE_SETTING) == QSize(950, 700)
    assert settings.value(SECTION_SETTING) == "general"
    again, _ = dialog(qtbot, settings=settings)
    assert again.size() == QSize(950, 700)
    settings.setValue(SECTION_SETTING, "gone")
    assert dialog(qtbot, settings=settings)[0].section() == "general"


@pytest.fixture
def window(qtbot: QtBot, tmp_path: Path) -> MainWindow:
    settings = QSettings(str(tmp_path / "w.ini"), QSettings.Format.IniFormat)
    w = MainWindow(settings)
    qtbot.addWidget(w)
    w.show()
    qtbot.waitExposed(w)
    return w


def test_the_window_applies_language_and_theme(window: MainWindow) -> None:
    settings = window._settings
    assert settings is not None
    assert window.options_values() == Options("", "dark")
    settings.setValue(LANGUAGE_SETTING, "xx")
    assert window.options_values().language == "", "not installed: the system's"
    try:
        window.apply_options(Options("es", "light"))
        assert current_language() == "es" and settings.value(LANGUAGE_SETTING) == "es"
        assert window.theme == "light" and settings.value(THEME_SETTING) == "light"
        assert window._tools_menu.title() == "&Herramientas", "retranslated"
        assert window.options_values() == Options("es", "light")
        # The system's language: nothing saved, as on first start.
        window.apply_options(Options("", "light"))
        assert settings.value(LANGUAGE_SETTING) is None
        assert window.options_values().language == ""
    finally:
        window.change_language("en")
        window.change_theme("dark")


def test_options_shows_the_dialog_until_it_closes(
    window: MainWindow, monkeypatch: pytest.MonkeyPatch
) -> None:
    shown: list[OptionsDialog] = []

    def exec_(self: OptionsDialog) -> int:
        shown.append(self)
        return 0

    monkeypatch.setattr(OptionsDialog, "exec", exec_)
    window.runner.execute("OPTIONS")
    assert len(shown) == 1 and shown[0].parent() is window
    assert window.runner.active is None
    window.command_actions["OPTIONS"].trigger()
    assert len(shown) == 2, "Tools, Options… too"
