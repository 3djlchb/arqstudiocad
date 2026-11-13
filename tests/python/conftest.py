import os
from collections.abc import Iterator
from pathlib import Path

import pytest

# Qt tests run without a display, on every OS.
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from arqstudiocad.i18n import SOURCE_LANGUAGE, set_language


@pytest.fixture(autouse=True)
def _isolated_settings(tmp_path: Path) -> None:
    """Implicit settings use fresh INI files, including on Windows.

    Redirect both scopes so user settings cannot fall back to installed system
    preferences. Keep the temporary paths active through widget destruction and
    interpreter shutdown; restoring native defaults could expose the real profile.
    Explicit file-based settings (including portable mode) remain unchanged.
    """
    from PySide6.QtCore import QSettings

    QSettings.setDefaultFormat(QSettings.Format.IniFormat)
    for scope, name in (
        (QSettings.Scope.UserScope, "user"),
        (QSettings.Scope.SystemScope, "system"),
    ):
        QSettings.setPath(QSettings.Format.IniFormat, scope, str(tmp_path / name))


@pytest.fixture(autouse=True)
def _english_by_default() -> Iterator[None]:
    """Each test starts and ends in English; the active language is global state."""
    set_language(SOURCE_LANGUAGE)
    yield
    set_language(SOURCE_LANGUAGE)


@pytest.fixture(autouse=True)
def _no_modal_dialogs(monkeypatch: pytest.MonkeyPatch) -> None:
    """A real modal dialog would wait forever for a click: fail instead.
    Tests give the window ui_fakes.QuietUi."""
    from PySide6.QtWidgets import QFileDialog, QInputDialog, QMessageBox

    from arqstudiocad.ui import text_dialog

    def refuse(*args: object, **kwargs: object) -> object:
        raise AssertionError("a test opened a modal dialog; pass ui=QuietUi()")

    for owner, name in (
        (QMessageBox, "question"),
        (QMessageBox, "warning"),
        (QMessageBox, "information"),
        (QMessageBox, "critical"),
        (QMessageBox, "about"),
        (QFileDialog, "getOpenFileName"),
        (QFileDialog, "getSaveFileName"),
        (QInputDialog, "getItem"),
    ):
        monkeypatch.setattr(owner, name, staticmethod(refuse))
    monkeypatch.setattr(text_dialog, "ask_text", refuse)


@pytest.fixture(autouse=True)
def _no_windows_left(_isolated_settings: None) -> Iterator[None]:
    """Destroys the windows a test leaves: pytest-qt only closes them, and
    every live widget is restyled again by each later theme change."""
    yield
    from PySide6.QtCore import QCoreApplication, QEvent
    from PySide6.QtWidgets import QApplication

    if QApplication.instance() is None:
        return
    for widget in QApplication.topLevelWidgets():
        widget.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.Type.DeferredDelete)
