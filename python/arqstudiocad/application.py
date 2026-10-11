"""Application lifecycle: Qt application, language selection, main window."""

import sys

from arqstudiocad.i18n import SOURCE_LANGUAGE, available_languages
from arqstudiocad.product import APP_ID, APP_NAME, ORGANIZATION


def choose_language(requested: str | None, saved: object, system_locale: str) -> str:
    """First installed language among: command line, saved preference, the
    system's language, English."""
    installed = {pack.code for pack in available_languages()}
    candidates = [requested, saved if isinstance(saved, str) else None, system_locale[:2]]
    for code in candidates:
        if code and code in installed:
            return code
    return SOURCE_LANGUAGE


def run(language: str | None = None) -> int:
    from PySide6.QtCore import QLocale, QSettings
    from PySide6.QtWidgets import QApplication

    from arqstudiocad import _core
    from arqstudiocad.i18n import set_language
    from arqstudiocad.ui.main_window import LANGUAGE_SETTING, MainWindow

    app = QApplication(sys.argv[:1])
    app.setApplicationName(APP_NAME)
    app.setOrganizationName(ORGANIZATION)
    app.setApplicationVersion(_core.__version__)
    app.setDesktopFileName(APP_ID)

    settings = QSettings()
    set_language(
        choose_language(language, settings.value(LANGUAGE_SETTING), QLocale.system().name())
    )

    window = MainWindow(settings)
    window.show()
    return app.exec()
