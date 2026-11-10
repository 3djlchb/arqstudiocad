"""Application lifecycle: Qt application, language selection, main window."""

import os
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING

from arqstudiocad.i18n import SOURCE_LANGUAGE, available_languages
from arqstudiocad.paths import portable_dir
from arqstudiocad.product import APP_ID, APP_NAME, ORGANIZATION
from arqstudiocad.renderer import CPU

if TYPE_CHECKING:
    from PySide6.QtCore import QSettings

SETTINGS_FILE = "settings.ini"
#: The crash sentinel's file (ADR 0034 D5), beside the settings.
GPU_SENTINEL_FILE = "gpu-starting"


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


def sentinel_path() -> Path:
    """Where the GPU's crash sentinel is kept: beside the INI file in
    portable mode, otherwise in the application's local data folder.
    Requires the application and organization names to be set first."""
    from PySide6.QtCore import QStandardPaths

    folder = portable_dir()
    if folder is None:
        location = QStandardPaths.StandardLocation.AppLocalDataLocation
        folder = Path(QStandardPaths.writableLocation(location))
    return folder / GPU_SENTINEL_FILE


def qt_platform_for_gpu(
    environ: Mapping[str, str], renderer: object, safe_mode: bool, platform: str = sys.platform
) -> str | None:
    """The platform Qt must run on so the graphics card can draw: ``"xcb"``
    in a Wayland session that has XWayland, since PySide6 does not give a
    window's ``wl_surface`` (ADR 0034 D1); None to leave Qt's own choice,
    as when the user chose a platform, the CPU draws, or there is no X
    server to run on."""
    if platform != "linux" or safe_mode or renderer == CPU:
        return None
    if environ.get("QT_QPA_PLATFORM"):
        return None
    if not environ.get("WAYLAND_DISPLAY") or not environ.get("DISPLAY"):
        return None
    return "xcb"


def run(language: str | None = None, safe_mode: bool = False) -> int:
    from PySide6.QtCore import QCoreApplication, QLocale
    from PySide6.QtWidgets import QApplication

    from arqstudiocad import _core
    from arqstudiocad.i18n import set_language
    from arqstudiocad.renderer import Sentinel
    from arqstudiocad.ui.main_window import LANGUAGE_SETTING, MainWindow
    from arqstudiocad.ui.options_dialog import STORED
    from arqstudiocad.ui.theme import THEME_SETTING, THEMES, apply_theme, as_theme

    # Named first: the settings are read before Qt picks its platform.
    QCoreApplication.setApplicationName(APP_NAME)
    QCoreApplication.setOrganizationName(ORGANIZATION)
    settings = open_settings()
    qt_platform = qt_platform_for_gpu(os.environ, settings.value(STORED["renderer"]), safe_mode)
    if qt_platform is not None:
        os.environ["QT_QPA_PLATFORM"] = qt_platform

    app = QApplication(sys.argv[:1])
    app.setApplicationVersion(_core.__version__)
    app.setDesktopFileName(APP_ID)

    set_language(
        choose_language(language, settings.value(LANGUAGE_SETTING), QLocale.system().name())
    )

    apply_theme(app, as_theme(settings.value(THEME_SETTING)) or THEMES[0])
    window = MainWindow(settings, safe_mode=safe_mode, sentinel=Sentinel(sentinel_path()))
    window.show()
    return app.exec()
