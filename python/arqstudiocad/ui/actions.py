"""One QAction per command (ADR 0013, D5.1).

Menus, toolbars and the ribbon all show these same actions, so a command is
written once and appears in every workspace. Triggering one starts its
command, cancelling any command already running, as AutoCAD's menus do.
"""

from collections.abc import Callable

from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import QMenu, QWidget

from arqstudiocad.commands import CommandRunner
from arqstudiocad.commands.views import VIEWS, view_titles
from arqstudiocad.i18n import tr
from arqstudiocad.ui.command_icons import command_icon
from arqstudiocad.ui.theme import Theme

#: Commands with an action, and their standard shortcuts.
SHORTCUTS: dict[str, QKeySequence | QKeySequence.StandardKey | None] = {
    "NEW": QKeySequence.StandardKey.New,
    "OPEN": QKeySequence.StandardKey.Open,
    "SAVE": QKeySequence.StandardKey.Save,
    "SAVEAS": QKeySequence.StandardKey.SaveAs,
    "CLOSE": QKeySequence.StandardKey.Close,
    "UNDO": QKeySequence.StandardKey.Undo,
    "REDO": QKeySequence.StandardKey.Redo,
    "ZOOM": None,
    "LAYER": None,
    "BLOCKSPALETTE": QKeySequence("Ctrl+3"),
    "PROPERTIES": QKeySequence("Ctrl+1"),
    "BUDGET": None,
    "OPTIONS": None,
    "LINE": None,
    "PLINE": None,
    "CIRCLE": None,
    "ARC": None,
    "TEXT": None,
    "MTEXT": None,
    "INSERT": None,
    "BLOCK": None,
    "BEDIT": None,
    "BSAVE": None,
    "BCLOSE": None,
    "ERASE": QKeySequence.StandardKey.Delete,
    "MOVE": None,
    "COPY": None,
    "ROTATE": None,
    "SCALE": None,
    "MIRROR": None,
    "EXPLODE": None,
    "BOX": None,
    "WEDGE": None,
    "CYLINDER": None,
    "CONE": None,
    "SPHERE": None,
    "TORUS": None,
    "EXTRUDE": None,
    "MASSPROP": None,
    "-VIEW": None,
}


def labels() -> dict[str, str]:
    """Each action's text in the interface language. Written as literal
    tr() calls so the translation test sees every one."""
    return {
        "NEW": tr("&New"),
        "OPEN": tr("&Open…"),
        "SAVE": tr("&Save"),
        "SAVEAS": tr("Save &As…"),
        "CLOSE": tr("&Close"),
        "UNDO": tr("&Undo"),
        "REDO": tr("&Redo"),
        "ZOOM": tr("&Zoom"),
        "LAYER": tr("&Layers…"),
        "BLOCKSPALETTE": tr("&Blocks"),
        "PROPERTIES": tr("&Properties"),
        "BUDGET": tr("B&udget"),
        "OPTIONS": tr("&Options…"),
        "LINE": tr("&Line"),
        "PLINE": tr("&Polyline"),
        "CIRCLE": tr("&Circle"),
        "ARC": tr("&Arc"),
        "TEXT": tr("&Single Line Text"),
        "MTEXT": tr("&Multiline Text"),
        "INSERT": tr("&Insert Block…"),
        "BLOCK": tr("Create &Block…"),
        "BEDIT": tr("&Edit Block…"),
        "BSAVE": tr("&Save Block"),
        "BCLOSE": tr("C&lose Block Editor"),
        "ERASE": tr("&Erase"),
        "MOVE": tr("&Move"),
        "COPY": tr("C&opy"),
        "ROTATE": tr("&Rotate"),
        "SCALE": tr("&Scale"),
        "MIRROR": tr("M&irror"),
        "EXPLODE": tr("E&xplode"),
        "BOX": tr("&Box"),
        "WEDGE": tr("&Wedge"),
        "CYLINDER": tr("C&ylinder"),
        "CONE": tr("Co&ne"),
        "SPHERE": tr("Sp&here"),
        "TORUS": tr("&Torus"),
        "EXTRUDE": tr("E&xtrude"),
        "MASSPROP": tr("Mass &Properties"),
        "-VIEW": tr("3D &Views"),
    }


class CommandActions:
    """The actions of every command in :data:`SHORTCUTS`."""

    def __init__(self, runner: CommandRunner, parent: QWidget) -> None:
        self._runner = runner
        self.by_command: dict[str, QAction] = {}
        self._closers: dict[str, Callable[[], None]] = {}
        self._open: dict[str, Callable[[], bool]] = {}
        for name, shortcut in SHORTCUTS.items():
            action = QAction(parent)
            action.setObjectName(f"action_{name.lower()}")
            if shortcut is not None:
                action.setShortcut(shortcut)
            action.setStatusTip(name)
            action.triggered.connect(lambda checked=False, n=name: self._triggered(n, checked))
            self.by_command[name] = action
        #: AutoCAD's preset views (ADR 0030 D5): -VIEW's drop-down in the
        #: ribbon, View > 3D Views in the menu bar.
        self.views_menu = QMenu(parent)
        self.view_actions: dict[str, QAction] = {}
        for name in VIEWS:
            action = self.views_menu.addAction("")
            action.setObjectName(f"action_view_{name}")
            action.triggered.connect(lambda checked=False, n=name: self.show_view(n))
            self.view_actions[name] = action
        self.by_command["-VIEW"].setMenu(self.views_menu)
        self.retranslate()

    def __getitem__(self, name: str) -> QAction:
        return self.by_command[name]

    def run(self, name: str) -> None:
        command = self._runner.registry.find(name)
        if command is not None:
            self._runner.start(command)

    def show_view(self, name: str) -> None:
        """Sees the model from view ``name`` through -VIEW, as typing it."""
        self.run("-VIEW")
        if self._runner.active == "-VIEW":
            self._runner.execute(name.upper())

    def toggle(self, name: str, close: Callable[[], None], is_open: Callable[[], bool]) -> None:
        """Makes ``name``'s action a toggle, as a palette's (ADR 0025 D2):
        checking it runs the command, unchecking it calls ``close``. Whoever
        owns what it toggles keeps its checked state up to date; after the
        command runs it shows ``is_open``, in case the command refused."""
        self.by_command[name].setCheckable(True)
        self._closers[name] = close
        self._open[name] = is_open

    def _triggered(self, name: str, checked: bool) -> None:
        close = self._closers.get(name)
        if close is not None and not checked:
            close()
            return
        self.run(name)
        is_open = self._open.get(name)
        if is_open is not None:
            self.by_command[name].setChecked(is_open())

    def set_theme(self, theme: Theme) -> None:
        """Refresh command icons for the active theme."""
        for name, action in self.by_command.items():
            action.setIcon(command_icon(name, theme))

    def retranslate(self) -> None:
        for name, text in labels().items():
            self.by_command[name].setText(text)
        self.views_menu.setTitle(labels()["-VIEW"])
        for name, title in view_titles().items():
            self.view_actions[name].setText(title)
