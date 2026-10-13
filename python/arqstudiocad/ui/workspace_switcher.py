"""Shows a discipline in a layout and switches between workspaces (ADR 0015).

Each discipline's toolbars and ribbon are built once, from its data, when it
is first shown. Switching saves the window's dock and toolbar state for the
workspace being left and restores the one of the workspace entered, so each
keeps its own arrangement. The current discipline and layout are remembered.
"Reset layout" puts panels and toolbars back where they start.
"""

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field

from PySide6.QtCore import QByteArray, QSettings, Qt
from PySide6.QtGui import QAction, QActionGroup
from PySide6.QtWidgets import QMainWindow, QMenu, QToolBar, QToolButton

from arqstudiocad.i18n import tr
from arqstudiocad.ui.ribbon import Ribbon
from arqstudiocad.workspaces import LAYOUTS, Discipline, Layout, WorkspaceRegistry, layout_title

DISCIPLINE_SETTING = "workspace/discipline"


def state_setting(discipline: str, layout: Layout) -> str:
    return f"workspace/{discipline}/{layout}/state"


def as_layout(value: object) -> Layout | None:
    """A layout read back from the settings; None if it is not one."""
    return next((layout for layout in LAYOUTS if layout == value), None)


def layout_setting(discipline: str) -> str:
    """The layout last used for ``discipline``."""
    return f"workspace/{discipline}/layout"


@dataclass
class DisciplineWidgets:
    """What the window shows for one discipline."""

    toolbars: list[QToolBar] = field(default_factory=list)
    ribbon_bar: QToolBar | None = None
    ribbon: Ribbon | None = None


class WorkspaceSwitcher:
    """The workspace of ``window``, and the menu to choose it."""

    def __init__(
        self,
        window: QMainWindow,
        actions: Mapping[str, QAction],
        registry: WorkspaceRegistry,
        settings: QSettings | None = None,
    ) -> None:
        if len(registry) == 0:
            raise ValueError("no discipline to show")
        self._window = window
        self._actions = actions
        self._registry = registry
        self._settings = settings
        self._built: dict[str, DisciplineWidgets] = {}
        #: The panels as the window placed them, before any workspace.
        self._default_state = window.saveState()
        #: Called after an arrangement is restored, and before a reset.
        self.after_restore: list[Callable[[], None]] = []
        self.before_reset: list[Callable[[], None]] = []
        self.discipline: Discipline | None = None
        self.layout: Layout | None = None

        self.menu = QMenu(window)
        self._discipline_group = QActionGroup(window)
        self.discipline_actions: dict[str, QAction] = {}
        for discipline in registry:
            action = self.menu.addAction("")
            action.setCheckable(True)
            action.triggered.connect(
                lambda _checked=False, name=discipline.name: self.show(name, None)
            )
            self._discipline_group.addAction(action)
            self.discipline_actions[discipline.name] = action
        self.menu.addSeparator()
        self._layout_group = QActionGroup(window)
        self.layout_actions: dict[Layout, QAction] = {}
        for layout in LAYOUTS:
            action = self.menu.addAction("")
            action.setCheckable(True)
            action.triggered.connect(lambda _checked=False, chosen=layout: self.show(None, chosen))
            self._layout_group.addAction(action)
            self.layout_actions[layout] = action
        self.menu.addSeparator()
        self.reset_action = self.menu.addAction("")
        self.reset_action.triggered.connect(self.reset)

        #: The status-bar button that opens the same menu (AutoCAD's gear).
        self.button = QToolButton()
        self.button.setObjectName("workspace_button")
        self.button.setMenu(self.menu)
        self.button.setPopupMode(QToolButton.ToolButtonPopupMode.InstantPopup)
        self.button.setAutoRaise(True)

    def show_remembered(self) -> None:
        """The workspace used last time, each discipline in the layout it
        last had; the first discipline in a ribbon otherwise."""
        discipline = self._setting(DISCIPLINE_SETTING)
        self.show(discipline if isinstance(discipline, str) else None, None)

    def show(self, discipline: str | None, layout: Layout | None) -> None:
        """Shows ``discipline`` in ``layout``. None, or a name that is not
        registered, keeps the current discipline (or the first). Without a
        layout, or with one the discipline lacks, it gets the layout it last
        had, else the current one, else its default."""
        chosen = self._registry.get(discipline) if discipline is not None else None
        chosen = chosen or self.discipline or next(iter(self._registry))
        remembered = as_layout(self._setting(layout_setting(chosen.name)))
        new_layout = next(
            (c for c in (layout, remembered, self.layout) if c in chosen.layouts),
            chosen.layouts[0],
        )
        if chosen == self.discipline and new_layout == self.layout:
            return
        self.save_state()
        widgets = self._widgets(chosen)
        for name, built in self._built.items():
            here = name == chosen.name
            for toolbar in built.toolbars:
                toolbar.setVisible(here and new_layout == "classic")
            if built.ribbon_bar is not None:
                built.ribbon_bar.setVisible(here and new_layout == "ribbon")
        self.discipline, self.layout = chosen, new_layout
        saved = self._setting(state_setting(chosen.name, new_layout))
        if isinstance(saved, QByteArray):
            self._window.restoreState(saved)
            for hook in self.after_restore:
                hook()
        if widgets.ribbon_bar is not None:
            # The ribbon is the layout itself: it cannot be closed.
            widgets.ribbon_bar.setVisible(new_layout == "ribbon")
        if self._settings is not None:
            self._settings.setValue(DISCIPLINE_SETTING, chosen.name)
            self._settings.setValue(layout_setting(chosen.name), new_layout)
        self.discipline_actions[chosen.name].setChecked(True)
        for each, action in self.layout_actions.items():
            action.setEnabled(each in chosen.layouts)
        self.layout_actions[new_layout].setChecked(True)
        self._update_button()

    def reset(self) -> None:
        """Puts the panels back where the window first placed them and the
        current workspace's toolbars back at the top, docked."""
        for hook in self.before_reset:
            hook()
        self._window.restoreState(self._default_state)
        for hook in self.after_restore:
            hook()
        if self.discipline is not None:
            built = self._widgets(self.discipline)
            for toolbar in built.toolbars:
                self._window.addToolBar(Qt.ToolBarArea.TopToolBarArea, toolbar)
                toolbar.setVisible(self.layout == "classic")
            if built.ribbon_bar is not None:
                built.ribbon_bar.setVisible(self.layout == "ribbon")
        self.save_state()

    def save_state(self) -> None:
        """Remembers the window's arrangement for the current workspace."""
        if self._settings is not None and self.discipline is not None and self.layout:
            key = state_setting(self.discipline.name, self.layout)
            self._settings.setValue(key, self._window.saveState())

    def widgets(self, discipline: str) -> DisciplineWidgets | None:
        """What was built for ``discipline``, if it has been shown."""
        return self._built.get(discipline)

    def retranslate(self) -> None:
        self.menu.setTitle(tr("&Workspace"))
        self.reset_action.setText(tr("&Reset layout"))
        for discipline in self._registry:
            self.discipline_actions[discipline.name].setText(discipline.title())
        for layout, action in self.layout_actions.items():
            action.setText(layout_title(layout))
        for name, built in self._built.items():
            shown = self._registry.get(name)
            if shown is None:
                continue
            for toolbar, data in zip(built.toolbars, shown.toolbars, strict=True):
                toolbar.setWindowTitle(data.title())
            if built.ribbon is not None and built.ribbon_bar is not None:
                built.ribbon.retranslate()
                built.ribbon_bar.setWindowTitle(tr("Ribbon"))
        self._update_button()

    def _widgets(self, discipline: Discipline) -> DisciplineWidgets:
        built = self._built.get(discipline.name)
        if built is not None:
            return built
        built = DisciplineWidgets()
        for data in discipline.toolbars:
            toolbar = QToolBar(self._window)
            toolbar.setObjectName(f"toolbar_{discipline.name}_{data.name}")
            for name in data.commands:
                if name is None:
                    toolbar.addSeparator()
                else:
                    toolbar.addAction(self._actions[name])
            toolbar.hide()
            self._window.addToolBar(Qt.ToolBarArea.TopToolBarArea, toolbar)
            built.toolbars.append(toolbar)
        if discipline.ribbon:
            built.ribbon = Ribbon(discipline, self._actions)
            bar = QToolBar(self._window)
            bar.setObjectName(f"ribbon_bar_{discipline.name}")
            bar.setMovable(False)
            bar.setFloatable(False)
            bar.toggleViewAction().setVisible(False)
            bar.addWidget(built.ribbon)
            bar.hide()
            self._window.addToolBar(Qt.ToolBarArea.TopToolBarArea, bar)
            built.ribbon_bar = bar
        self._built[discipline.name] = built
        self.retranslate()
        return built

    def _update_button(self) -> None:
        if self.discipline is not None and self.layout is not None:
            self.button.setText(f"{self.discipline.title()} · {layout_title(self.layout)}")
            self.button.setToolTip(tr("Workspace"))

    def _setting(self, key: str) -> object:
        return None if self._settings is None else self._settings.value(key)
