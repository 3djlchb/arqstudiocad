"""One QAction per command (ADR 0013, D5.1).

Menus, toolbars and the ribbon all show these same actions, so a command is
written once and appears in every workspace. Triggering one starts its
command, cancelling any command already running, as AutoCAD's menus do.
"""

from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import QWidget

from arqstudiocad.commands import CommandRunner
from arqstudiocad.i18n import tr

#: Commands with an action, and their standard shortcuts.
SHORTCUTS: dict[str, QKeySequence.StandardKey | None] = {
    "NEW": QKeySequence.StandardKey.New,
    "OPEN": QKeySequence.StandardKey.Open,
    "SAVE": QKeySequence.StandardKey.Save,
    "SAVEAS": QKeySequence.StandardKey.SaveAs,
    "CLOSE": QKeySequence.StandardKey.Close,
    "UNDO": QKeySequence.StandardKey.Undo,
    "REDO": QKeySequence.StandardKey.Redo,
    "ZOOM": None,
    "LAYER": None,
    "LINE": None,
    "CIRCLE": None,
    "ARC": None,
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
        "LINE": tr("&Line"),
        "CIRCLE": tr("&Circle"),
        "ARC": tr("&Arc"),
    }


class CommandActions:
    """The actions of every command in :data:`SHORTCUTS`."""

    def __init__(self, runner: CommandRunner, parent: QWidget) -> None:
        self._runner = runner
        self.by_command: dict[str, QAction] = {}
        for name, shortcut in SHORTCUTS.items():
            action = QAction(parent)
            action.setObjectName(f"action_{name.lower()}")
            if shortcut is not None:
                action.setShortcut(shortcut)
            action.setStatusTip(name)
            action.triggered.connect(lambda _checked=False, n=name: self.run(n))
            self.by_command[name] = action
        self.retranslate()

    def __getitem__(self, name: str) -> QAction:
        return self.by_command[name]

    def run(self, name: str) -> None:
        command = self._runner.registry.find(name)
        if command is not None:
            self._runner.start(command)

    def retranslate(self) -> None:
        for name, text in labels().items():
            self.by_command[name].setText(text)
