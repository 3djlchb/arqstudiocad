"""Theme-aware icons for commands that have artwork."""

from pathlib import Path

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
_DRAW_ICONS = {"LINE": "line", "PLINE": "polyline", "CIRCLE": "circle", "ARC": "arc"}
#: The sizes drawn for each theme; the dark theme has 18 and 54 px too
#: (maintainer, 2026-10-25), the light one until its artwork arrives
#: scales its 48 px drawing.
_DRAW_SIZES = {"dark": (18, 24, 32, 48, 54), "light": (24, 32, 48)}
#: The drafting switches' icons, by switch; dark theme only for now.
_DRAFTING_ICONS = {"ORTHO": "orthogonal-18.svg"}
#: The side of the icons on ribbon buttons, one per row (maintainer,
#: 2026-10-25).
RIBBON_ICON = 54


def command_icon(command: str, theme: Theme) -> QIcon:
    """Load a command icon, with variants for the active theme and size."""
    root = icons_dir()
    theme_directory = "dark-themes" if theme == "dark" else "clear-themes"

    action_icon = _ACTION_ICONS.get(command)
    if action_icon is not None:
        return QIcon(str(root / "icons_actions" / theme_directory / action_icon))

    icon = QIcon()
    for size, path in draw_icon_files(command, theme).items():
        icon.addFile(str(path), QSize(size, size))
    return icon


def draw_icon_files(command: str, theme: Theme) -> dict[int, Path]:
    """A draw command's drawings for ``theme``, by size in pixels; none for
    a command without artwork."""
    draw_icon = _DRAW_ICONS.get(command)
    if draw_icon is None:
        return {}
    theme_directory = "dark-themes" if theme == "dark" else "clear-themes"
    folder = icons_dir() / "icons_draw" / theme_directory
    return {
        size: folder / f"{draw_icon}-{size}.svg"
        for size in _DRAW_SIZES["dark" if theme == "dark" else "light"]
    }


def drafting_icon(switch: str, theme: Theme) -> QIcon:
    """A drafting switch's icon (ORTHO); null where there is no artwork for
    ``theme`` yet, so the switch shows its name instead."""
    name = _DRAFTING_ICONS.get(switch)
    if name is None or theme != "dark":
        return QIcon()
    return QIcon(str(icons_dir() / "icons_snaps" / "dark-themes" / name))
