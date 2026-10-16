"""Shared compact styling for panel tables and action buttons.

The colors come from the active Qt palette so the panels follow both themes.
"""

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QFrame,
    QHeaderView,
    QPushButton,
    QTableView,
    QTreeView,
)


def apply_data_view_style(view: QAbstractItemView, *, row_height: int = 26) -> None:
    """Give tables and property grids the shared compact panel appearance."""
    colors = QApplication.palette(view)
    base = colors.base().color().name()
    alternate = colors.alternateBase().color().name()
    text = colors.text().color().name()
    button = colors.button().color().name()
    button_text = colors.buttonText().color().name()
    mid = colors.mid().color().name()
    highlight = colors.highlight().color().name()
    highlighted_text = colors.highlightedText().color().name()

    view.setFrameShape(QFrame.Shape.NoFrame)
    view.setAlternatingRowColors(True)
    view.setVerticalScrollMode(QAbstractItemView.ScrollMode.ScrollPerPixel)
    view.setHorizontalScrollMode(QAbstractItemView.ScrollMode.ScrollPerPixel)
    if isinstance(view, QTreeView):
        view.setUniformRowHeights(True)
        header = view.header()
    elif isinstance(view, QTableView):
        view.setShowGrid(False)
        header = view.horizontalHeader()
        vertical = view.verticalHeader()
        vertical.setDefaultSectionSize(row_height)
        vertical.setMinimumSectionSize(row_height)
        vertical.setSectionResizeMode(QHeaderView.ResizeMode.Fixed)
    else:
        return

    header.setMinimumHeight(row_height)
    header.setDefaultAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter)
    header.setHighlightSections(False)
    view.setStyleSheet(
        f"""
        QTreeView, QTableView {{
            background-color: {base};
            alternate-background-color: {alternate};
            color: {text};
            border: none;
            selection-background-color: {highlight};
            selection-color: {highlighted_text};
            outline: none;
        }}
        QTreeView::item, QTableView::item {{
            padding: 3px 6px;
            border-bottom: 1px solid {mid};
        }}
        QHeaderView {{ border: none; background-color: {button}; }}
        QHeaderView::section {{
            background-color: {button};
            color: {button_text};
            border: none;
            border-bottom: 1px solid {mid};
            padding: 4px 6px;
        }}
        QComboBox {{
            background-color: {base};
            color: {text};
            border: 1px solid {mid};
            border-radius: 2px;
            padding: 2px 5px;
        }}
        QComboBox:hover {{ border-color: {highlight}; }}
        """
    )


def apply_property_grid_style(view: QTreeView) -> None:
    """Style the properties grid like the reference palette, for either theme."""
    colors = QApplication.palette(view)
    base = colors.base().color().name()
    text = colors.text().color().name()
    muted_text = colors.placeholderText().color().name()
    border = colors.mid().color().name()
    highlight = colors.highlight().color().name()
    highlighted_text = colors.highlightedText().color().name()
    button = colors.button().color().name()
    button_text = colors.buttonText().color().name()

    view.setFrameShape(QFrame.Shape.NoFrame)
    view.setAlternatingRowColors(False)
    view.setStyleSheet(
        f"""
        QTreeView {{
            background-color: {colors.window().color().name()};
            color: {text};
            border: none;
            outline: none;
        }}
        QTreeView::item {{ padding: 2px 5px; border: none; }}
        QLineEdit {{
            background-color: {base};
            color: {text};
            border: 1px solid {border};
            border-radius: 1px;
            padding: 2px 5px;
            selection-background-color: {highlight};
            selection-color: {highlighted_text};
        }}
        QLineEdit:hover {{ border-color: {highlight}; }}
        QComboBox {{
            background-color: {base};
            color: {text};
            border: 1px solid {border};
            border-radius: 1px;
            padding: 2px 5px;
        }}
        QComboBox#propertyAccentEditor {{
            background-color: {highlight};
            color: {highlighted_text};
            border-color: {highlight};
        }}
        QComboBox QAbstractItemView {{
            background-color: {base};
            color: {text};
            selection-background-color: {highlight};
            selection-color: {highlighted_text};
            border: 1px solid {border};
        }}
        """
    )
    view.setProperty("propertyLabelColor", muted_text)
    view.setProperty("propertyBaseColor", base)
    view.setProperty("propertyTextColor", text)
    view.setProperty("propertySectionColor", button)
    view.setProperty("propertySectionTextColor", button_text)
    view.setProperty("propertyBorderColor", border)


def apply_panel_action_style(button: QPushButton) -> None:
    """Style a compact panel action using the current theme palette."""
    colors = QApplication.palette(button)
    background = colors.button().color().name()
    text = colors.buttonText().color().name()
    border = colors.mid().color().name()
    highlight = colors.highlight().color().name()
    highlighted_text = colors.highlightedText().color().name()
    button.setMinimumHeight(26)
    button.setStyleSheet(
        f"""
        QPushButton {{
            background-color: {background};
            color: {text};
            border: 1px solid {border};
            border-radius: 2px;
            padding: 3px 9px;
        }}
        QPushButton:hover {{ border-color: {highlight}; }}
        QPushButton:pressed {{
            background-color: {highlight};
            color: {highlighted_text};
        }}
        """
    )
