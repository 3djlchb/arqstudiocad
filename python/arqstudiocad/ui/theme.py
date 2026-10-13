"""Dark and light themes (ADR 0015; panels after OpenCADStudio's).

Both use Qt's Fusion style, which looks the same on every platform and
follows the palette fully. Dark is the default, as in current CAD tools; the
drawing area stays dark in both, as AutoCAD's model space.
"""

from typing import Literal, get_args

from PySide6.QtGui import QColor, QPalette
from PySide6.QtWidgets import QApplication, QStyleFactory

from arqstudiocad.i18n import tr

Theme = Literal["dark", "light"]
#: The themes, the first one the default.
THEMES: tuple[Theme, ...] = get_args(Theme)
THEME_SETTING = "theme"

#: The dark palette: (role, color) for every color group.
DARK: tuple[tuple[QPalette.ColorRole, str], ...] = (
    (QPalette.ColorRole.Window, "#2b2b2e"),
    (QPalette.ColorRole.WindowText, "#e0e0e0"),
    (QPalette.ColorRole.Base, "#1e1e20"),
    (QPalette.ColorRole.AlternateBase, "#303034"),
    (QPalette.ColorRole.Text, "#e0e0e0"),
    (QPalette.ColorRole.PlaceholderText, "#8a8a8a"),
    (QPalette.ColorRole.Button, "#3a3a3e"),
    (QPalette.ColorRole.ButtonText, "#e0e0e0"),
    (QPalette.ColorRole.BrightText, "#ffffff"),
    (QPalette.ColorRole.Light, "#4a4a50"),
    (QPalette.ColorRole.Midlight, "#404046"),
    (QPalette.ColorRole.Mid, "#46464c"),
    (QPalette.ColorRole.Dark, "#232326"),
    (QPalette.ColorRole.Shadow, "#141416"),
    (QPalette.ColorRole.Highlight, "#0a9be0"),
    (QPalette.ColorRole.HighlightedText, "#ffffff"),
    (QPalette.ColorRole.Link, "#4fb8f0"),
    (QPalette.ColorRole.ToolTipBase, "#3a3a3e"),
    (QPalette.ColorRole.ToolTipText, "#e0e0e0"),
)
#: Disabled text and controls, dimmed.
DARK_DISABLED: tuple[tuple[QPalette.ColorRole, str], ...] = (
    (QPalette.ColorRole.WindowText, "#7a7a7a"),
    (QPalette.ColorRole.Text, "#7a7a7a"),
    (QPalette.ColorRole.ButtonText, "#7a7a7a"),
    (QPalette.ColorRole.Highlight, "#46464c"),
)


def theme_title(theme: Theme) -> str:
    return tr("Dark") if theme == "dark" else tr("Light")


def as_theme(value: object) -> Theme | None:
    """A theme read back from the settings; None if it is not one."""
    return next((theme for theme in THEMES if theme == value), None)


def palette(theme: Theme) -> QPalette:
    style = QStyleFactory.create("Fusion")
    if theme == "light" or style is None:
        return QPalette() if style is None else style.standardPalette()
    result = QPalette()
    for role, color in DARK:
        result.setColor(role, QColor(color))
    for role, color in DARK_DISABLED:
        result.setColor(QPalette.ColorGroup.Disabled, role, QColor(color))
    return result


def apply_theme(app: QApplication, theme: Theme) -> None:
    """Gives every window of ``app`` the theme."""
    app.setStyle("Fusion")
    app.setPalette(palette(theme))
