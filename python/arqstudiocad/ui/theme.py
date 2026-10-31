"""Dark and light themes (ADR 0015; panels after OpenCADStudio's).

Both use Qt's Fusion style, which looks the same on every platform and
follows the palette fully. Each theme is a palette of its own, never the
system's: Fusion's standard palette follows the system's color scheme, so a
light theme built on it came out dark under a dark system. The windows'
title bars follow the theme too. Dark is the default, as in current CAD
tools; the drawing area stays dark in both, as AutoCAD's model space.
"""

from typing import Literal, get_args

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QPalette
from PySide6.QtWidgets import QApplication

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

#: The light palette: windows and buttons a warm light grey (#F2F1F0, the
#: maintainer's, 2026-10-30), lists and fields white, as Fusion's own light
#: palette. Text is about 15:1 on the grey, muted labels about 4.7:1, white
#: on the selection about 5.5:1.
LIGHT: tuple[tuple[QPalette.ColorRole, str], ...] = (
    (QPalette.ColorRole.Window, "#f2f1f0"),
    (QPalette.ColorRole.WindowText, "#1e1e1e"),
    (QPalette.ColorRole.Base, "#ffffff"),
    (QPalette.ColorRole.AlternateBase, "#f7f6f5"),
    (QPalette.ColorRole.Text, "#1e1e1e"),
    (QPalette.ColorRole.PlaceholderText, "#6b6b6b"),
    (QPalette.ColorRole.Button, "#f2f1f0"),
    (QPalette.ColorRole.ButtonText, "#1e1e1e"),
    (QPalette.ColorRole.BrightText, "#ffffff"),
    (QPalette.ColorRole.Light, "#ffffff"),
    (QPalette.ColorRole.Midlight, "#d9d8d7"),
    (QPalette.ColorRole.Mid, "#b8b7b6"),
    (QPalette.ColorRole.Dark, "#9f9e9d"),
    (QPalette.ColorRole.Shadow, "#767676"),
    (QPalette.ColorRole.Highlight, "#0b6fa6"),
    (QPalette.ColorRole.HighlightedText, "#ffffff"),
    (QPalette.ColorRole.Link, "#0b6fa6"),
    (QPalette.ColorRole.ToolTipBase, "#ffffdc"),
    (QPalette.ColorRole.ToolTipText, "#1e1e1e"),
)
#: Disabled text and controls, dimmed.
LIGHT_DISABLED: tuple[tuple[QPalette.ColorRole, str], ...] = (
    (QPalette.ColorRole.WindowText, "#a0a0a0"),
    (QPalette.ColorRole.Text, "#a0a0a0"),
    (QPalette.ColorRole.ButtonText, "#a0a0a0"),
    (QPalette.ColorRole.Highlight, "#b8b7b6"),
)


def theme_title(theme: Theme) -> str:
    return tr("Dark") if theme == "dark" else tr("Light")


def as_theme(value: object) -> Theme | None:
    """A theme read back from the settings; None if it is not one."""
    return next((theme for theme in THEMES if theme == value), None)


def palette(theme: Theme) -> QPalette:
    """The theme's palette, whatever the system's color scheme."""
    colors, disabled = (LIGHT, LIGHT_DISABLED) if theme == "light" else (DARK, DARK_DISABLED)
    result = QPalette()
    for role, color in colors:
        result.setColor(role, QColor(color))
    for role, color in disabled:
        result.setColor(QPalette.ColorGroup.Disabled, role, QColor(color))
    return result


def apply_theme(app: QApplication, theme: Theme) -> None:
    """Gives every window of ``app`` the theme."""
    # The title bars too, which the system draws (Qt 6.8 and later).
    app.styleHints().setColorScheme(
        Qt.ColorScheme.Light if theme == "light" else Qt.ColorScheme.Dark
    )
    app.setStyle("Fusion")
    app.setPalette(palette(theme))
