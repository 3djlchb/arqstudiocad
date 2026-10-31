"""The Options dialog (ADR 0029 D3, D4): its sections in a sidebar, each
subsection a titled group box, Reset section / OK / Apply / Cancel, its size
and last section remembered, Ctrl+Tab between sections; General holds the
language and the theme, which the window applies as the Tools menu does;
User preferences what a right-click in model space does. Every change is
announced in one OptionsChanged."""

from pathlib import Path

import pytest
from PySide6.QtCore import QSettings, QSize, Qt
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QDialog, QGroupBox, QWidget
from pytestqt.qtbot import QtBot

from arqstudiocad.events import OptionsChanged
from arqstudiocad.i18n import current_language, set_language
from arqstudiocad.ui.main_window import LANGUAGE_SETTING, MainWindow
from arqstudiocad.ui.options_dialog import (
    PRIMARY,
    RIGHT_CLICK_SETTING,
    SECTION_SETTING,
    SECTIONS,
    SIZE,
    SIZE_SETTING,
    Options,
    OptionsDialog,
    changed_keys,
    saved_right_click,
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


def titles(d: OptionsDialog) -> list[str]:
    """The group boxes' titles, section by section, top to bottom."""
    pages = [page for i in range(d.pages.count()) if isinstance(page := d.pages.widget(i), QWidget)]
    groups = sorted(
        d.findChildren(QGroupBox),
        key=lambda g: (
            next(i for i, page in enumerate(pages) if page.isAncestorOf(g)),
            g.y(),
        ),
    )
    return [g.title() for g in groups]


def test_general_holds_the_language_and_the_theme(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot)
    assert d.windowTitle() == "Options" and d.isModal()
    sections = [d.sidebar.item(r).text() for r in range(d.sidebar.count())]
    assert sections == ["General", "Display", "User preferences"]
    assert d.section() == "general" and d.size() == SIZE
    assert titles(d) == [
        "Language",
        "Theme",
        "Crosshair",
        "Block editor",
        "Right-click in model space",
    ]
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
        assert titles(d) == [
            "Idioma",
            "Tema",
            "Cursor en cruz",
            "Editor de bloques",
            "Clic derecho en el espacio modelo",
        ]
        assert d.sidebar.item(1).text() == "Visualización"
        assert d.sidebar.item(2).text() == "Preferencias de usuario"
        assert d.right_click.itemText(1) == "Repetir el último comando"
        assert d.during_command.itemText(0) == "Equivale a Intro"
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
    d, _ = dialog(qtbot, settings=settings)
    d.sidebar.setCurrentRow(2)
    d.ok_button.click()
    assert settings.value(SECTION_SETTING) == "user_preferences"
    assert dialog(qtbot, settings=settings)[0].section() == "user_preferences"
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
    assert window.options_values() == Options("", "dark", "menu")
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


def test_user_preferences_says_what_a_right_click_does(qtbot: QtBot) -> None:
    d, applied = dialog(qtbot)
    d.sidebar.setCurrentRow(2)
    assert d.section() == "user_preferences"
    page = d.pages.currentWidget()
    assert page is not None and page.isAncestorOf(d.right_click)
    assert d.right_click_label.text() == "With nothing selected"
    assert [d.right_click.itemText(i) for i in range(d.right_click.count())] == [
        "Show the context menu",
        "Repeat the last command",
    ]
    assert d.right_click.currentData() == "menu"
    assert d.during_command_label.text() == "During a command"
    # One choice until a command has a menu of its own (ADR 0029 D4).
    assert [d.during_command.itemText(i) for i in range(d.during_command.count())] == [
        "Acts as Enter"
    ]
    assert d.during_command.isEnabled()
    assert not d.right_click_label.font().bold() and d.right_click_group.font().bold()
    assert not d.during_command_label.font().bold(), "every row, not just the first"
    muted = d.language_label.styleSheet()
    assert muted.startswith("color:")
    assert d.right_click_label.styleSheet() == d.during_command_label.styleSheet() == muted
    d.right_click.setCurrentIndex(d.right_click.findData("repeat"))
    d.ok_button.click()
    assert applied.calls == [Options("", "dark", "repeat")]


def test_reset_section_resets_only_its_own_settings(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot, Options("es", "light", "repeat"))
    d.sidebar.setCurrentRow(2)
    assert d.right_click.currentData() == "repeat", "as saved"
    d.reset_button.click()
    assert d.pending == Options("es", "light", "menu")
    assert d.right_click.currentData() == "menu"
    d.sidebar.setCurrentRow(0)
    assert d.pending == Options("es", "light", "menu"), "kept across sections"
    d.reset_button.click()
    assert d.pending == Options("", "dark", "menu")


@pytest.mark.parametrize(
    ("key", "modifiers"),
    [
        (Qt.Key.Key_Tab, Qt.KeyboardModifier.ControlModifier),
        (Qt.Key.Key_PageDown, Qt.KeyboardModifier.ControlModifier),
        (
            Qt.Key.Key_Backtab,
            Qt.KeyboardModifier.ControlModifier | Qt.KeyboardModifier.ShiftModifier,
        ),
        (Qt.Key.Key_PageUp, Qt.KeyboardModifier.ControlModifier),
    ],
)
def test_the_keyboard_moves_between_sections(
    qtbot: QtBot, key: Qt.Key, modifiers: Qt.KeyboardModifier
) -> None:
    d, _ = dialog(qtbot)
    d.show()
    with qtbot.waitActive(d):
        d.activateWindow()
    d.language.setFocus()
    d.theme.setCurrentIndex(d.theme.findData("light"))
    forward = key in (Qt.Key.Key_Tab, Qt.Key.Key_PageDown)
    order = ["display", "user_preferences", "general"]
    if not forward:
        order = ["user_preferences", "display", "general"]
    for expected in order:
        QTest.keyClick(d.focusWidget() or d, key, modifiers)
        assert d.section() == expected, "around"
        assert d.pages.currentIndex() == list(SECTIONS).index(expected)
    assert d.pending.theme == "light", "nothing pending is lost"


def test_next_section_wraps_both_ways(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot)
    d.next_section(1)
    assert d.section() == "display"
    d.next_section(2)
    assert d.section() == "general"
    d.next_section(-1)
    assert d.section() == "user_preferences"


def test_the_right_click_setting_as_saved(tmp_path: Path) -> None:
    settings = QSettings(str(tmp_path / "r.ini"), QSettings.Format.IniFormat)
    assert saved_right_click(None) == "menu"
    assert saved_right_click(settings) == "menu"
    settings.setValue(RIGHT_CLICK_SETTING, "repeat")
    assert saved_right_click(settings) == "repeat"
    settings.setValue(RIGHT_CLICK_SETTING, "later")
    assert saved_right_click(settings) == "menu", "unknown: the default"


def test_changed_keys_names_each_setting() -> None:
    assert changed_keys(Options(), Options()) == ()
    assert changed_keys(Options(), Options("es", "light", "repeat")) == (
        "language",
        "theme",
        "right_click",
    )
    assert changed_keys(Options(), Options(right_click="repeat")) == ("right_click",)


def test_the_window_keeps_the_right_click_and_announces_changes(
    window: MainWindow, tmp_path: Path
) -> None:
    settings = window._settings
    assert settings is not None
    announced: list[OptionsChanged] = []
    window.bus.subscribe(OptionsChanged, announced.append)
    try:
        window.apply_options(Options("", "dark", "repeat"))
        assert window.right_click == "repeat"
        assert settings.value(RIGHT_CLICK_SETTING) == "repeat"
        assert window.options_values().right_click == "repeat"
        assert announced == [OptionsChanged(("right_click",))]
        window.apply_options(Options("", "dark", "repeat"))
        assert len(announced) == 1, "nothing changed: nothing announced"
        window.apply_options(Options("es", "light", "menu"))
        assert announced[1:] == [OptionsChanged(("language", "theme", "right_click"))], "once"
        window.change_theme("dark")
        window.change_language("en")
        assert announced[2:] == [OptionsChanged(("theme",)), OptionsChanged(("language",))]
    finally:
        window.change_language("en")
        window.change_theme("dark")
    settings.sync()
    again = MainWindow(settings)
    try:
        assert again.right_click == "menu"
        settings.setValue(RIGHT_CLICK_SETTING, "repeat")
        assert MainWindow(settings).right_click == "repeat", "kept across restarts"
    finally:
        again.deleteLater()
