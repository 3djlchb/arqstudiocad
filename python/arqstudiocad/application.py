"""Application lifecycle: Qt application, language selection, main window."""

import sys
from typing import TYPE_CHECKING

from arqstudiocad.i18n import SOURCE_LANGUAGE, available_languages
from arqstudiocad.paths import portable_dir
from arqstudiocad.product import APP_ID, APP_NAME, ORGANIZATION

if TYPE_CHECKING:
    from PySide6.QtCore import QSettings

SETTINGS_FILE = "settings.ini"


def choose_language(requested: str | None, saved: object, system_locale: str) -> str:
    """First installed language among: command line, saved preference, the
    system's language, English."""
    installed = {pack.code for pack in available_languages()}
    candidates = [requested, saved if isinstance(saved, str) else None, system_locale[:2]]
    for code in candidates:
        if code and code in installed:
            return code
    return SOURCE_LANGUAGE


def open_settings() -> "QSettings":
    """Saved preferences: an INI file next to the executable in portable mode,
    otherwise the system location (registry on Windows, ~/.config on Linux).
    Requires the application and organization names to be set first."""
    from PySide6.QtCore import QSettings

    folder = portable_dir()
    if folder is not None:
        return QSettings(str(folder / SETTINGS_FILE), QSettings.Format.IniFormat)
    return QSettings()


def run(language: str | None = None) -> int:
    from PySide6.QtCore import QLocale
    from PySide6.QtWidgets import QApplication

    from arqstudiocad import _core
    from arqstudiocad.i18n import set_language
    from arqstudiocad.ui.main_window import LANGUAGE_SETTING, MainWindow
    from arqstudiocad.ui.theme import THEME_SETTING, THEMES, apply_theme, as_theme

    app = QApplication(sys.argv[:1])
    app.setApplicationName(APP_NAME)
    app.setOrganizationName(ORGANIZATION)
    app.setApplicationVersion(_core.__version__)
    app.setDesktopFileName(APP_ID)

    settings = open_settings()
    set_language(
        choose_language(language, settings.value(LANGUAGE_SETTING), QLocale.system().name())
    )

    apply_theme(app, as_theme(settings.value(THEME_SETTING)) or THEMES[0])
    window = MainWindow(settings)
    window.show()
    return app.exec()
