"""Main window: menus, the command window, language switch and About."""

import platform
from functools import partial
from html import escape

from PySide6.QtCore import QSettings, Qt, qVersion
from PySide6.QtGui import QAction, QActionGroup, QKeySequence
from PySide6.QtWidgets import QLabel, QMainWindow, QMessageBox

from arqstudiocad import _core
from arqstudiocad.commands import CommandContext, CommandRunner, UiServices
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.credits import load_credits
from arqstudiocad.events import Event, EventBus, LanguageChanged
from arqstudiocad.i18n import available_languages, current_language, set_language, tr
from arqstudiocad.product import APP_NAME
from arqstudiocad.session import Session
from arqstudiocad.ui.actions import CommandActions
from arqstudiocad.ui.command_window import CommandWindow
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.qt_services import QtUiServices
from arqstudiocad.ui.qt_translations import QtTranslations

LANGUAGE_SETTING = "language"


class MainWindow(QMainWindow):
    def __init__(
        self,
        settings: QSettings | None = None,
        bus: EventBus | None = None,
        ui: UiServices | None = None,
    ) -> None:
        super().__init__()
        self._settings = settings
        self.bus = bus if bus is not None else EventBus()
        self.events = QtEventBridge(self.bus, self)
        self.events.language_changed.connect(self._on_language_changed)
        self.session = Session(self.bus)
        self.runner = CommandRunner(
            builtin_registry(),
            CommandContext(self.session, ui if ui is not None else QtUiServices(self)),
        )
        self.command_actions = CommandActions(self.runner, self)
        self.command_window = CommandWindow(self.runner, self.events, self)
        self.addDockWidget(Qt.DockWidgetArea.BottomDockWidgetArea, self.command_window)
        for signal in (
            self.events.active_document_changed,
            self.events.document_saved,
            self.events.document_changed,
        ):
            signal.connect(self._update_title)
        self._qt_translations = QtTranslations()
        self._qt_translations.apply(current_language())

        self._welcome = QLabel()
        self._welcome.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.setCentralWidget(self._welcome)

        menu_bar = self.menuBar()
        self._file_menu = menu_bar.addMenu("")
        for name in ("NEW", "OPEN", "SAVE", "SAVEAS", "CLOSE"):
            self._file_menu.addAction(self.command_actions[name])
        self._file_menu.addSeparator()
        self._exit_action = self._file_menu.addAction("")
        self._exit_action.setShortcut(QKeySequence.StandardKey.Quit)
        self._exit_action.triggered.connect(self.close)

        self._edit_menu = menu_bar.addMenu("")
        for name in ("UNDO", "REDO"):
            self._edit_menu.addAction(self.command_actions[name])

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
        self._update_title()
        self._file_menu.setTitle(tr("&File"))
        self._exit_action.setText(tr("E&xit"))
        self._edit_menu.setTitle(tr("&Edit"))
        self.command_actions.retranslate()
        self.command_window.retranslate()
        self._tools_menu.setTitle(tr("&Tools"))
        self._language_menu.setTitle(tr("&Language"))
        self._help_menu.setTitle(tr("&Help"))
        self._about_action.setText(tr("&About {app}"))
        self._welcome.setText(
            tr(
                "{app} {version} — CAD core {core}",
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
        """Switch the interface language, remember it, and tell everyone."""
        if not set_language(code):
            return
        if self._settings is not None:
            self._settings.setValue(LANGUAGE_SETTING, code)
        self.bus.publish(LanguageChanged(code))

    def _on_language_changed(self, event: LanguageChanged) -> None:
        self._qt_translations.apply(event.code)
        self.retranslate()

    def _update_title(self, event: Event | None = None) -> None:
        """``name* — App``: the active drawing, starred while modified."""
        document = self.session.active
        if document is None or not document.is_open:
            self.setWindowTitle(APP_NAME)
            return
        star = "*" if document.is_modified else ""
        self.setWindowTitle(f"{document.name}{star} — {APP_NAME}")

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
        QMessageBox.about(self, tr("About {app}"), self.about_text())
