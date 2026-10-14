"""Main window: document tabs, menus, the command window, the layers,
properties and history panels, language switch and About."""

import platform
from functools import partial
from html import escape

from PySide6.QtCore import QSettings, Qt, qVersion
from PySide6.QtGui import QAction, QActionGroup, QCloseEvent, QKeySequence
from PySide6.QtWidgets import (
    QApplication,
    QDockWidget,
    QLabel,
    QMainWindow,
    QMessageBox,
    QProgressBar,
    QStackedWidget,
    QToolBar,
)

from arqstudiocad import _core
from arqstudiocad.commands import CommandContext, CommandRunner, UiServices, ViewControl, Worker
from arqstudiocad.commands.builtin import DEFAULT_SAVE_EXTENSION, builtin_registry
from arqstudiocad.core import (
    CadDocument,
    CadError,
    LossyConversionError,
    OverwritesDamagedError,
)
from arqstudiocad.credits import load_credits
from arqstudiocad.events import (
    ActiveDocumentChanged,
    DocumentChanged,
    DocumentClosed,
    DocumentOpened,
    Event,
    EventBus,
    LanguageChanged,
    WorkStarted,
)
from arqstudiocad.i18n import available_languages, current_language, set_language, tr
from arqstudiocad.product import APP_NAME
from arqstudiocad.session import Session
from arqstudiocad.ui.actions import CommandActions
from arqstudiocad.ui.auto_hide import AutoHide
from arqstudiocad.ui.background import QtWorker
from arqstudiocad.ui.command_window import CommandWindow
from arqstudiocad.ui.document_tabs import DocumentTabs
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.history_panel import HistoryPanel
from arqstudiocad.ui.layers_panel import LayersPanel
from arqstudiocad.ui.properties_panel import PropertiesPanel
from arqstudiocad.ui.qt_services import QtUiServices
from arqstudiocad.ui.qt_translations import QtTranslations
from arqstudiocad.ui.theme import THEME_SETTING, THEMES, Theme, apply_theme, as_theme, theme_title
from arqstudiocad.ui.viewport import Viewport
from arqstudiocad.ui.workspace_switcher import WorkspaceSwitcher
from arqstudiocad.workspaces import WorkspaceRegistry, builtin_workspaces

LANGUAGE_SETTING = "language"
#: Initial height of the command window, in pixels.
COMMAND_WINDOW_HEIGHT = 140


class MainWindow(QMainWindow):
    def __init__(
        self,
        settings: QSettings | None = None,
        bus: EventBus | None = None,
        ui: UiServices | None = None,
        worker: Worker | None = None,
        workspaces: WorkspaceRegistry | None = None,
    ) -> None:
        super().__init__()
        self._settings = settings
        self.bus = bus if bus is not None else EventBus()
        self.events = QtEventBridge(self.bus, self)
        self.events.language_changed.connect(self._on_language_changed)
        self.session = Session(self.bus)
        self.viewports: dict[int, Viewport] = {}
        self.ui = ui if ui is not None else QtUiServices(self, self.viewport_for, self.show_panel)
        self.worker = worker if worker is not None else QtWorker(self)
        self.runner = CommandRunner(
            builtin_registry(), CommandContext(self.session, self.ui, self.worker)
        )
        self.command_actions = CommandActions(self.runner, self)
        self.command_window = CommandWindow(self.runner, self.events, self)
        self.addDockWidget(Qt.DockWidgetArea.BottomDockWidgetArea, self.command_window)
        # A few lines, as in AutoCAD; the user can drag it taller.
        self.resizeDocks([self.command_window], [COMMAND_WINDOW_HEIGHT], Qt.Orientation.Vertical)
        # The panels, on the right: layers and properties as tabs, history below.
        self.layers_panel = LayersPanel(self.session, self.events, self)
        self.properties_panel = PropertiesPanel(self.session, self.events, self)
        self.history_panel = HistoryPanel(self.session, self.events, self)
        self.panels: dict[str, QDockWidget] = {
            "layers": self.layers_panel,
            "properties": self.properties_panel,
            "history": self.history_panel,
        }
        right = Qt.DockWidgetArea.RightDockWidgetArea
        # Panels move, float and dock on any side, side by side or as tabs,
        # and a tab group drags as one, as AutoCAD's palettes (ADR 0015).
        # Side panels take the full height, the command window between them.
        self.setDockOptions(
            QMainWindow.DockOption.AnimatedDocks
            | QMainWindow.DockOption.AllowNestedDocks
            | QMainWindow.DockOption.AllowTabbedDocks
            | QMainWindow.DockOption.GroupedDragging
        )
        for corner, side in (
            (Qt.Corner.TopLeftCorner, Qt.DockWidgetArea.LeftDockWidgetArea),
            (Qt.Corner.BottomLeftCorner, Qt.DockWidgetArea.LeftDockWidgetArea),
            (Qt.Corner.TopRightCorner, Qt.DockWidgetArea.RightDockWidgetArea),
            (Qt.Corner.BottomRightCorner, Qt.DockWidgetArea.RightDockWidgetArea),
        ):
            self.setCorner(corner, side)
        # Split before tabifying: splitting a tabified dock adds a tab.
        self.addDockWidget(right, self.layers_panel)
        self.splitDockWidget(self.layers_panel, self.history_panel, Qt.Orientation.Vertical)
        self.tabifyDockWidget(self.layers_panel, self.properties_panel)
        self.layers_panel.raise_()
        #: The pins: a panel stays open, or collapses to a tab on its side.
        self.auto_hide = AutoHide(self, self.panels.values(), settings)
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
        self.tabs = DocumentTabs()
        self.tabs.currentChanged.connect(self._on_tab_chosen)
        self.tabs.tabCloseRequested.connect(self._on_tab_close_requested)
        self._stack = QStackedWidget()
        self._stack.addWidget(self._welcome)
        self._stack.addWidget(self.tabs)
        # The drawings' tabs across the whole window, above the panels, as
        # AutoCAD's file tabs: a strip of their own, under the ribbon or the
        # toolbars, which the workspaces insert above it.
        self.tabs_strip = QToolBar(self)
        self.tabs_strip.setObjectName("document_tabs")
        self.tabs_strip.setMovable(False)
        self.tabs_strip.setFloatable(False)
        self.tabs_strip.toggleViewAction().setVisible(False)
        self.tabs_strip.addWidget(self.tabs.bar)
        self.addToolBar(Qt.ToolBarArea.TopToolBarArea, self.tabs_strip)
        self.tabs_strip.hide()
        self.setCentralWidget(self._stack)
        self._coordinates = QLabel()
        self._busy = QProgressBar()
        self._busy.setRange(0, 0)  # busy, without a percentage
        self._busy.setMaximumWidth(120)
        self._busy.hide()
        self.statusBar().addPermanentWidget(self._busy)
        self.statusBar().addPermanentWidget(self._coordinates)
        self.events.work_started.connect(self._on_work_started)
        self.events.work_finished.connect(self._on_work_finished)
        for signal in (self.events.document_changed, self.events.document_saved):
            signal.connect(self._update_tab_titles)
        self.events.document_opened.connect(self._on_document_opened)
        self.events.active_document_changed.connect(self._on_active_document_changed)
        self.events.document_closed.connect(self._on_document_closed)
        self.events.document_changed.connect(self._on_document_changed)

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

        self._view_menu = menu_bar.addMenu("")
        self._view_menu.addAction(self.command_actions["ZOOM"])
        self._view_menu.addSeparator()
        for panel in (*self.panels.values(), self.command_window):
            self._view_menu.addAction(panel.toggleViewAction())

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
        #: The theme in use: the application applies it on start (ADR 0015).
        self.theme: Theme = THEMES[0]
        if settings is not None:
            self.theme = as_theme(settings.value(THEME_SETTING)) or THEMES[0]
        self._theme_menu = self._tools_menu.addMenu("")
        self._theme_group = QActionGroup(self)
        self.theme_actions: dict[Theme, QAction] = {}
        for theme in THEMES:
            action = self._theme_menu.addAction("")
            action.setCheckable(True)
            action.triggered.connect(partial(self.change_theme, theme))
            self._theme_group.addAction(action)
            self.theme_actions[theme] = action
        self.theme_actions[self.theme].setChecked(True)

        self.workspaces = WorkspaceSwitcher(
            self,
            self.command_actions.by_command,
            workspaces if workspaces is not None else builtin_workspaces(),
            settings,
            above=self.tabs_strip,
        )
        self._view_menu.addSeparator()
        self._view_menu.addMenu(self.workspaces.menu)
        self.statusBar().addPermanentWidget(self.workspaces.button)

        self._help_menu = menu_bar.addMenu("")
        self._about_action = self._help_menu.addAction("")
        self._about_action.triggered.connect(self.show_about)

        self.resize(1024, 700)
        self.retranslate()
        self.workspaces.after_restore.append(self.auto_hide.keep_hidden)
        self.workspaces.after_restore.append(self._keep_tabs_strip_last)
        self.workspaces.before_reset.append(self.auto_hide.pin_all)
        self.workspaces.show_remembered()
        self.auto_hide.restore()

    def retranslate(self) -> None:
        """Set every user-visible text from the active language."""
        self._update_title()
        self._file_menu.setTitle(tr("&File"))
        self._exit_action.setText(tr("E&xit"))
        self._edit_menu.setTitle(tr("&Edit"))
        self._view_menu.setTitle(tr("&View"))
        self.command_actions.retranslate()
        self.command_window.retranslate()
        self.layers_panel.retranslate()
        self.properties_panel.retranslate()
        self.history_panel.retranslate()
        self.workspaces.retranslate()
        self.auto_hide.retranslate()
        self._tools_menu.setTitle(tr("&Tools"))
        self._language_menu.setTitle(tr("&Language"))
        self._theme_menu.setTitle(tr("T&heme"))
        for theme, theme_action in self.theme_actions.items():
            theme_action.setText(theme_title(theme))
        self._help_menu.setTitle(tr("&Help"))
        self._about_action.setText(tr("&About {app}"))
        self._welcome.setText(
            tr(
                "{app} {version} — CAD core {core}",
                version=_core.__version__,
                core=_core.__version__,
            )
            + "\n"
            + tr("Create or open a drawing: File menu, or type NEW or OPEN below.")
        )
        action = self._language_actions.get(current_language())
        if action is not None:
            action.setChecked(True)

    def change_theme(self, theme: Theme) -> None:
        """Switch between the dark and the light theme, and remember it."""
        app = QApplication.instance()
        if isinstance(app, QApplication):
            apply_theme(app, theme)
        # Colors drawn by hand follow the theme: the pins and the grid's sections.
        self.auto_hide.restyle()
        self.properties_panel.grid.restyle()
        self.theme = theme
        self.theme_actions[theme].setChecked(True)
        if self._settings is not None:
            self._settings.setValue(THEME_SETTING, theme)

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

    def show_panel(self, name: str) -> bool:
        """Shows and raises the panel called ``name``, for commands such as
        LAYER; False if there is no such panel."""
        panel = self.panels.get(name)
        if panel is None:
            return False
        if not self.auto_hide.is_pinned(panel):
            return self.auto_hide.expand(panel)
        panel.show()
        panel.raise_()
        return True

    # Viewports: one per open document; the active one is shown.

    def viewport_for(self, document: CadDocument) -> ViewControl | None:
        """The view of ``document``, for commands such as ZOOM."""
        return self.viewports.get(document.id)

    def _document(self, document_id: int) -> CadDocument | None:
        return next((d for d in self.session.documents if d.id == document_id), None)

    def _on_document_opened(self, event: DocumentOpened) -> None:
        document = self._document(event.document_id)
        if document is None or event.document_id in self.viewports:
            return
        viewport = Viewport(document)
        viewport.cursor_moved.connect(self._show_coordinates)
        self.viewports[event.document_id] = viewport
        self.tabs.addTab(viewport, document.name)
        self._show_tabs_strip()
        self._update_tab_titles()

    def _on_active_document_changed(self, event: ActiveDocumentChanged) -> None:
        viewport = None if event.document_id is None else self.viewports.get(event.document_id)
        if viewport is None:
            self._stack.setCurrentWidget(self._welcome)
            self._coordinates.clear()
            return
        self._stack.setCurrentWidget(self.tabs)
        self.tabs.setCurrentWidget(viewport)

    def _on_document_closed(self, event: DocumentClosed) -> None:
        viewport = self.viewports.pop(event.document_id, None)
        if viewport is not None:
            self.tabs.removeTab(self.tabs.indexOf(viewport))
            self._show_tabs_strip()
            viewport.deleteLater()

    def _keep_tabs_strip_last(self) -> None:
        """A restored arrangement may put the tab strip beside the ribbon
        (one saved before the strip existed): it goes back to a row of its
        own, the last at the top, above the panels."""
        self.removeToolBar(self.tabs_strip)
        self.addToolBarBreak(Qt.ToolBarArea.TopToolBarArea)
        self.addToolBar(Qt.ToolBarArea.TopToolBarArea, self.tabs_strip)
        self._show_tabs_strip()

    def _show_tabs_strip(self) -> None:
        """The tab strip shows while a drawing is open."""
        self.tabs_strip.setVisible(self.tabs.count() > 0)

    def _on_tab_chosen(self, index: int) -> None:
        viewport = self.tabs.widget(index)
        if isinstance(viewport, Viewport) and viewport.document.is_open:
            self.session.activate(viewport.document)

    def _on_tab_close_requested(self, index: int) -> None:
        viewport = self.tabs.widget(index)
        if isinstance(viewport, Viewport):
            self.session.activate(viewport.document)
            self.command_actions.run("CLOSE")

    def _update_tab_titles(self, event: Event | None = None) -> None:
        for viewport in self.viewports.values():
            document = viewport.document
            if not document.is_open:
                continue
            index = self.tabs.indexOf(viewport)
            self.tabs.setTabText(index, document.name + ("*" if document.is_modified else ""))
            path = document.path
            self.tabs.setTabToolTip(index, str(path) if path is not None else document.name)

    def _on_work_started(self, event: WorkStarted) -> None:
        self.statusBar().showMessage(event.message)
        self._busy.show()
        # The drawing may be locked by the background work (a save): the
        # panels wait until it is done instead of blocking the interface.
        for panel in self.panels.values():
            panel.setEnabled(False)
        self.auto_hide.set_enabled(False)

    def _on_work_finished(self, event: Event) -> None:
        self.statusBar().clearMessage()
        self._busy.hide()
        for panel in self.panels.values():
            panel.setEnabled(True)
        self.auto_hide.set_enabled(True)

    def closeEvent(self, event: QCloseEvent) -> None:  # noqa: N802 (Qt API)
        """Quitting: wait for background work, and ask about every drawing
        with unsaved changes (Save / Discard / Cancel)."""
        if self.runner.busy is not None:
            self.command_window.history.appendPlainText(
                tr("Please wait: {work}", work=self.runner.busy)
            )
            event.ignore()
            return
        for document in self.session.documents:
            if not document.is_modified:
                continue
            self.session.activate(document)
            answer = self.ui.ask_save_changes(document.name)
            if answer == "cancel":
                event.ignore()
                return
            if answer == "yes" and not self._save_on_quit(document):
                event.ignore()
                return
        self.workspaces.save_state()
        if isinstance(self.worker, QtWorker):
            self.worker.shutdown()
        event.accept()

    def _save_on_quit(self, document: CadDocument) -> bool:
        """Saves ``document`` while quitting; False if the user cancelled.
        Replacing the damaged file it was recovered from is asked first."""
        path = document.path or self.ui.choose_save_path(document.name + DEFAULT_SAVE_EXTENSION)
        while path is not None:
            try:
                self.session.save(document, path)
            except OverwritesDamagedError:
                answer = self.ui.ask_overwrite_damaged(path.name)
                if answer == "overwrite":
                    document.allow_overwriting_damaged()
                elif answer == "save_as":
                    path = self.ui.choose_save_path(path.name)
                else:
                    return False
            except LossyConversionError as error:
                if set(error.loss_kinds) <= set(document.accepted_losses):
                    # Refused although accepted: report it, never ask in circles.
                    self.command_window.history.appendPlainText(
                        tr("Error: {message}", message=str(error))
                    )
                    return False
                if not self.ui.ask_accept_losses(path.name, error.losses):
                    return False
                document.accept_losses(error.loss_kinds)
            except CadError as error:
                # Not saved (a failed check, a full disk…): stay open.
                self.command_window.history.appendPlainText(
                    tr("Error: {message}", message=str(error))
                )
                return False
            else:
                return True
        return False

    def _on_document_changed(self, event: DocumentChanged) -> None:
        viewport = self.viewports.get(event.document_id)
        if viewport is not None:
            viewport.apply(event.changes)

    def _show_coordinates(self, x: float, y: float) -> None:
        self._coordinates.setText(f"{x:.4f}, {y:.4f}")

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
