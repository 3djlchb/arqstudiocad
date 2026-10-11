"""Main window of the walking skeleton: menus, language switch and About."""

import platform
from functools import partial
from html import escape

from PySide6.QtCore import QSettings, Qt, qVersion
from PySide6.QtGui import QAction, QActionGroup, QKeySequence
from PySide6.QtWidgets import QLabel, QMainWindow, QMessageBox

from arqstudiocad import _core
from arqstudiocad.credits import load_credits
from arqstudiocad.i18n import available_languages, current_language, set_language, tr
from arqstudiocad.ui.qt_translations import QtTranslations

APP_NAME = "ArqStudioCAD"
LANGUAGE_SETTING = "language"


class MainWindow(QMainWindow):
    def __init__(self, settings: QSettings | None = None) -> None:
        super().__init__()
        self._settings = settings
        self._qt_translations = QtTranslations()
        self._qt_translations.apply(current_language())

        self._welcome = QLabel()
        self._welcome.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.setCentralWidget(self._welcome)

        menu_bar = self.menuBar()
        self._file_menu = menu_bar.addMenu("")
        self._exit_action = self._file_menu.addAction("")
        self._exit_action.setShortcut(QKeySequence.StandardKey.Quit)
        self._exit_action.triggered.connect(self.close)

        self._tools_menu = menu_bar.addMenu("")
        self._language_menu = self._tools_menu.addMenu("")
        self._language_group = QActionGroup(self)
        self._language_actions: dict[str, QAction] = {}
        for pack in available_languages():
            # A language is listed by its own name, so the menu stays readable
            # whichever language is active.
            action = self._language_menu.addAction(pack.name)
            action.setCheckable(True)
            action.triggered.connect(partial(self.change_language, pack.code))
            self._language_group.addAction(action)
            self._language_actions[pack.code] = action

        self._help_menu = menu_bar.addMenu("")
        self._about_action = self._help_menu.addAction("")
        self._about_action.triggered.connect(self.show_about)

        self.resize(1024, 700)
        self.retranslate()

    def retranslate(self) -> None:
        """Set every user-visible text from the active language."""
        self.setWindowTitle(APP_NAME)
        self._file_menu.setTitle(tr("&File"))
        self._exit_action.setText(tr("E&xit"))
        self._tools_menu.setTitle(tr("&Tools"))
        self._language_menu.setTitle(tr("&Language"))
        self._help_menu.setTitle(tr("&Help"))
        self._about_action.setText(tr("&About ArqStudioCAD"))
        self._welcome.setText(
            tr(
                "ArqStudioCAD {version} — CAD core {core}",
                version=_core.__version__,
                core=_core.__version__,
            )
            + "\n"
            + tr("Nothing to draw yet: this is the walking skeleton.")
        )
        action = self._language_actions.get(current_language())
        if action is not None:
            action.setChecked(True)

    def change_language(self, code: str) -> None:
        if not set_language(code):
            return
        if self._settings is not None:
            self._settings.setValue(LANGUAGE_SETTING, code)
        self._qt_translations.apply(code)
        self.retranslate()

    def about_text(self) -> str:
        """Rich text for Help > About, credits taken from AUTHORS."""
        parts = [
            f"<h3>{APP_NAME} {escape(_core.__version__)}</h3>",
            f"<p>{escape(tr('Open-source CAD for architecture and engineering.'))}</p>",
            "<p>"
            + escape(
                tr(
                    "CAD core {core} · Qt {qt} · Python {python}",
                    core=_core.__version__,
                    qt=qVersion(),
                    python=platform.python_version(),
                )
            )
            + "</p>",
            f"<p>{escape(tr('License: GPL-3.0-or-later'))}</p>",
        ]
        credits = load_credits()
        if credits is None:
            parts.append(f"<p>{escape(tr('Credits file not found.'))}</p>")
        else:
            for heading, people in (
                (tr("Maintainers"), credits.maintainers),
                (tr("Contributors"), credits.contributors),
            ):
                if people:
                    items = "".join(f"<li>{escape(person)}</li>" for person in people)
                    parts.append(f"<h4>{escape(heading)}</h4><ul>{items}</ul>")
        return "".join(parts)

    def show_about(self) -> None:
        QMessageBox.about(self, tr("About ArqStudioCAD"), self.about_text())
