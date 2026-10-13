import os
from collections.abc import Iterator

import pytest

# Qt tests run without a display, on every OS.
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from arqstudiocad.i18n import SOURCE_LANGUAGE, set_language


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
    from PySide6.QtWidgets import QFileDialog, QMessageBox

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
    ):
        monkeypatch.setattr(owner, name, staticmethod(refuse))
