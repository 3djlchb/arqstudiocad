"""Theme-aware icons for commands that have artwork."""

from PySide6.QtCore import QSize
from PySide6.QtGui import QIcon

from arqstudiocad.paths import icons_dir
from arqstudiocad.ui.theme import Theme

_ACTION_ICONS = {
    "NEW": "new.svg",
    "OPEN": "open.svg",
    "SAVE": "save.svg",
    "SAVEAS": "save-as.svg",
    "UNDO": "undo.svg",
    "REDO": "redo.svg",
}
_DRAW_ICONS = {"LINE": "line", "CIRCLE": "circle", "ARC": "arc"}
_DRAW_SIZES = (24, 32, 48)


def command_icon(command: str, theme: Theme) -> QIcon:
    """Load a command icon, with variants for the active theme and size."""
    root = icons_dir()
    theme_directory = "dark-themes" if theme == "dark" else "clear-themes"

    action_icon = _ACTION_ICONS.get(command)
    if action_icon is not None:
        return QIcon(str(root / "icons_actions" / theme_directory / action_icon))

    draw_icon = _DRAW_ICONS.get(command)
    if draw_icon is None:
        return QIcon()

    icon = QIcon()
    for size in _DRAW_SIZES:
        path = root / "icons_draw" / theme_directory / f"{draw_icon}-{size}.svg"
        icon.addFile(str(path), QSize(size, size))
    return icon
