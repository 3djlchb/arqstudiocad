"""One QAction per command (ADR 0013, D5.1).

Menus, toolbars and the ribbon all show these same actions, so a command is
written once and appears in every workspace. Triggering one starts its
command, cancelling any command already running, as AutoCAD's menus do.
"""

from collections.abc import Callable

from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import QWidget

from arqstudiocad.commands import CommandRunner
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
    }


class CommandActions:
    """The actions of every command in :data:`SHORTCUTS`."""

    def __init__(self, runner: CommandRunner, parent: QWidget) -> None:
        self._runner = runner
        self.by_command: dict[str, QAction] = {}
        self._closers: dict[str, Callable[[], None]] = {}
        for name, shortcut in SHORTCUTS.items():
            action = QAction(parent)
            action.setObjectName(f"action_{name.lower()}")
            if shortcut is not None:
                action.setShortcut(shortcut)
            action.setStatusTip(name)
            action.triggered.connect(lambda checked=False, n=name: self._triggered(n, checked))
            self.by_command[name] = action
        self.retranslate()

    def __getitem__(self, name: str) -> QAction:
        return self.by_command[name]

    def run(self, name: str) -> None:
        command = self._runner.registry.find(name)
        if command is not None:
            self._runner.start(command)

    def toggle(self, name: str, close: Callable[[], None]) -> None:
        """Makes ``name``'s action a toggle, as a palette's (ADR 0025 D2):
        checking it runs the command, unchecking it calls ``close``. Whoever
        owns what it toggles keeps its checked state up to date."""
        self.by_command[name].setCheckable(True)
        self._closers[name] = close

    def _triggered(self, name: str, checked: bool) -> None:
        close = self._closers.get(name)
        if close is not None and not checked:
            close()
        else:
            self.run(name)

    def set_theme(self, theme: Theme) -> None:
        """Refresh command icons for the active theme."""
        for name, action in self.by_command.items():
            action.setIcon(command_icon(name, theme))

    def retranslate(self) -> None:
        for name, text in labels().items():
            self.by_command[name].setText(text)
