"""Shared CAD color selection for entity properties and layers."""

from typing import cast

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QColor, QPalette
from PySide6.QtWidgets import (
    QApplication,
    QColorDialog,
    QDialog,
    QDialogButtonBox,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QScrollArea,
    QTabWidget,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.core import CadColor
from arqstudiocad.i18n import tr

_RECENT_LIMIT = 8
_recent_colors: list[CadColor] = []


def color_name(color: CadColor) -> str:
    """Return a concise CAD label for an indexed or true color."""
    names = {
        1: tr("Red"),
        2: tr("Yellow"),
        3: tr("Green"),
        4: tr("Cyan"),
        5: tr("Blue"),
        6: tr("Magenta"),
        7: tr("White"),
        8: tr("Dark gray"),
        9: tr("Light gray"),
    }
    if color.index is not None:
        return names.get(color.index, str(color.index))
    if color.rgb is not None:
        return "{},{},{}".format(*color.rgb)
    if color.kind == "bylayer":
        return tr("ByLayer")
    if color.kind == "byblock":
        return tr("ByBlock")
    if color.kind == "none":
        return tr("None")
    return ""


def remember_color(color: CadColor) -> None:
    """Keep recently selected explicit colors for this application session."""
    if color.index is None and color.rgb is None:
        return
    if color in _recent_colors:
        _recent_colors.remove(color)
    _recent_colors.insert(0, color)
    del _recent_colors[_RECENT_LIMIT:]


def recent_colors() -> tuple[CadColor, ...]:
    """The most recently selected explicit colors, newest first."""
    return tuple(_recent_colors)


def matching_index_colors(query: str) -> tuple[int, ...]:
    """ACI indices matching a name, index, or displayed RGB value."""
    needle = query.strip().casefold()
    if not needle:
        return tuple(range(1, 256))
    if needle.isdecimal() and 1 <= int(needle) <= 255:
        return (int(needle),)
    rgb_needle = needle.replace(" ", "")
    return tuple(
        index
        for index in range(1, 256)
        if needle in str(index).casefold()
        or needle in color_name(CadColor.from_index(index)).casefold()
        or needle
        in ",".join(str(channel) for channel in CadColor.from_index(index).display_rgb()).casefold()
        or rgb_needle
        in ",".join(str(channel) for channel in CadColor.from_index(index).display_rgb()).casefold()
    )


def _display_rgb(color: CadColor, background: tuple[int, int, int]) -> tuple[int, int, int]:
    return color.display_rgb(background)


def _background_rgb(widget: QWidget) -> tuple[int, int, int]:
    rgba = cast(
        tuple[int, int, int, int],
        widget.palette().color(QPalette.ColorRole.Window).getRgb(),
    )
    return rgba[:3]


def _color_button(color: CadColor, parent: QWidget, *, compact: bool = False) -> QToolButton:
    button = QToolButton(parent)
    button.setAutoRaise(False)
    button.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
    button.setFixedSize(22 if compact else 28, 20 if compact else 26)
    button.setCheckable(True)
    button.setProperty("cadColor", color)
    rgb = _display_rgb(color, _background_rgb(parent))
    button.setStyleSheet(
        f"QToolButton {{ background-color: rgb({rgb[0]},{rgb[1]},{rgb[2]}); "
        "border: 1px solid #777; }"
        "QToolButton:checked { border: 2px solid palette(highlight); }"
    )
    label = color_name(color)
    index = color.index
    rgb_text = ", ".join(str(channel) for channel in color.display_rgb())
    button.setToolTip(
        tr("ACI {index} · RGB: {rgb}", index=index, rgb=rgb_text)
        if index is not None
        else f"RGB: {rgb_text} ({label})"
    )
    button.setAccessibleName(label if index is None else f"{index} {label}")
    return button


class ColorPickerDialog(QDialog):
    """A searchable ACI palette and a true-color picker in one dialog."""

    color_picked = Signal(object)

    def __init__(
        self,
        current: CadColor | None = None,
        *,
        allow_special: bool = True,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setWindowTitle(tr("Select color"))
        self._allow_special = allow_special
        self._selected = current or CadColor.from_index(7)
        self._index_buttons: dict[int, QToolButton] = {}
        self._special_buttons: dict[CadColor, QPushButton] = {}
        self._recent_buttons: list[QToolButton] = []
        self._true_color = QColorDialog(self)
        self._true_color.setOption(QColorDialog.ColorDialogOption.NoButtons, True)
        self._true_color.setOption(QColorDialog.ColorDialogOption.ShowAlphaChannel, False)
        self._true_color.setWindowFlags(Qt.WindowType.Widget)
        self._true_color.currentColorChanged.connect(self._on_true_color_changed)

        self.tabs = QTabWidget()
        self.index_page = self._make_index_page()
        self.tabs.addTab(self.index_page, tr("Index Color"))
        self.tabs.addTab(self._true_color, tr("True Color"))
        self.tabs.currentChanged.connect(self._sync_true_color)

        self.preview = QLabel()
        self.preview.setFixedSize(30, 24)
        self.selection_label = QLabel()
        self.recent_group = QGroupBox(tr("Recent colors"))
        self.recent_layout = QHBoxLayout(self.recent_group)
        self.recent_layout.setContentsMargins(6, 4, 6, 4)
        self._refresh_recent()

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        footer = QHBoxLayout()
        footer.addWidget(self.preview)
        footer.addWidget(self.selection_label, 1)
        footer.addWidget(buttons)
        layout = QVBoxLayout(self)
        layout.addWidget(self.tabs, 1)
        layout.addWidget(self.recent_group)
        layout.addLayout(footer)
        self._fit_to_screen(parent)
        self._select(self._selected)

    def _fit_to_screen(self, parent: QWidget | None) -> None:
        anchor = parent.window() if parent is not None else None
        screen = anchor.screen() if anchor is not None else QApplication.primaryScreen()
        if screen is None:
            self.resize(900, 720)
            self.setMinimumSize(650, 500)
            return

        available = screen.availableGeometry()
        width = min(900, max(1, available.width() - 32))
        height = min(720, max(1, available.height() - 48))
        self.setMinimumSize(min(650, width), min(500, height))
        self.resize(width, height)

        center = anchor.frameGeometry().center() if anchor is not None else available.center()
        x = min(max(center.x() - width // 2, available.left()), available.right() - width + 1)
        y = min(max(center.y() - height // 2, available.top()), available.bottom() - height + 1)
        self.move(x, y)

    def selected_color(self) -> CadColor:
        """The color the user chose, available after the dialog is accepted."""
        return self._selected

    def _make_index_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setContentsMargins(8, 8, 8, 8)
        self.search = QLineEdit()
        self.search.setClearButtonEnabled(True)
        self.search.setPlaceholderText(tr("Search colors…"))
        self.search.textChanged.connect(self._filter_index_colors)
        layout.addWidget(self.search)
        self._no_results = QLabel(tr("No colors match your search."))
        self._no_results.hide()
        layout.addWidget(self._no_results)

        if self._allow_special:
            special = QHBoxLayout()
            for label, color in (
                (tr("ByLayer"), CadColor.by_layer()),
                (tr("ByBlock"), CadColor.by_block()),
            ):
                button = QPushButton(label)
                button.setCheckable(True)
                button.clicked.connect(lambda _checked=False, c=color: self._select(c))
                special.addWidget(button)
                self._special_buttons[color] = button
            special.addStretch(1)
            layout.addLayout(special)

        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        content = QWidget()
        content_layout = QVBoxLayout(content)
        standard = QGroupBox(tr("Standard colors"))
        self._standard_group = standard
        standard_layout = QHBoxLayout(standard)
        for index in range(1, 10):
            standard_layout.addWidget(self._make_index_button(index, compact=False))
        content_layout.addWidget(standard)

        palette = QGroupBox(tr("ACI 10-249 · color families and shades"))
        self._palette_group = palette
        palette_layout = QGridLayout(palette)
        palette_layout.setHorizontalSpacing(2)
        palette_layout.setVerticalSpacing(2)
        for family in range(24):
            for shade in range(10):
                index = 10 + family * 10 + shade
                palette_layout.addWidget(
                    self._make_index_button(index, compact=True), shade, family
                )
        content_layout.addWidget(palette)

        grayscale = QGroupBox(tr("Grayscale colors"))
        self._grayscale_group = grayscale
        grayscale_layout = QHBoxLayout(grayscale)
        for index in range(250, 256):
            grayscale_layout.addWidget(self._make_index_button(index))
        content_layout.addWidget(grayscale)
        content_layout.addStretch(1)
        scroll.setWidget(content)
        layout.addWidget(scroll, 1)
        return page

    def _make_index_button(self, index: int, *, compact: bool = False) -> QToolButton:
        color = CadColor.from_index(index)
        button = _color_button(color, self, compact=compact)
        button.clicked.connect(lambda _checked=False, c=color: self._select(c))
        self._index_buttons[index] = button
        return button

    def _filter_index_colors(self, query: str) -> None:
        matching = set(matching_index_colors(query))
        needle = query.strip().casefold()
        for index, button in self._index_buttons.items():
            button.setVisible(index in matching)
        for color, special_button in self._special_buttons.items():
            special_button.setVisible(not needle or needle in color_name(color).casefold())
        self._standard_group.setVisible(any(index in matching for index in range(1, 10)))
        self._palette_group.setVisible(any(index in matching for index in range(10, 250)))
        self._grayscale_group.setVisible(any(index in matching for index in range(250, 256)))
        special_match = any(button.isVisible() for button in self._special_buttons.values())
        self._no_results.setVisible(not matching and not special_match)

    def _select(self, color: CadColor) -> None:
        self._selected = color
        for button in self._index_buttons.values():
            button.setChecked(button.property("cadColor") == color)
        for choice, special_button in self._special_buttons.items():
            special_button.setChecked(color == choice)
        for recent_button in self._recent_buttons:
            recent_button.setChecked(recent_button.property("cadColor") == color)
        self._update_selection()

    def _on_true_color_changed(self, color: QColor) -> None:
        if color.isValid():
            self._selected = CadColor.from_rgb(color.red(), color.green(), color.blue())
            self._update_selection()

    def _sync_true_color(self, index: int) -> None:
        if index == 1 and self._selected.rgb is not None:
            r, g, b = self._selected.rgb
            self._true_color.setCurrentColor(QColor(r, g, b))

    def _update_selection(self) -> None:
        if self._selected.index is not None:
            rgb = self._selected.display_rgb()
            details = tr("ACI {index} · RGB: {rgb}", index=self._selected.index, rgb=rgb)
        elif self._selected.rgb is not None:
            details = tr("RGB: {rgb}", rgb=self._selected.rgb)
        else:
            details = color_name(self._selected)
        self.selection_label.setText(details)
        rgb = self._display_rgb(self._selected, _background_rgb(self))
        self.preview.setStyleSheet(
            f"background-color: rgb({rgb[0]},{rgb[1]},{rgb[2]}); border: 1px solid palette(mid);"
        )
        if self._selected.rgb is not None:
            r, g, b = self._selected.rgb
            self._true_color.setCurrentColor(QColor(r, g, b))

    @staticmethod
    def _display_rgb(color: CadColor, background: tuple[int, int, int]) -> tuple[int, int, int]:
        return _display_rgb(color, background)

    def _refresh_recent(self) -> None:
        while self.recent_layout.count():
            item = self.recent_layout.takeAt(0)
            if item is None:
                continue
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()
        self._recent_buttons.clear()
        for color in recent_colors():
            button = _color_button(color, self, compact=True)
            button.clicked.connect(lambda _checked=False, c=color: self._select(c))
            self.recent_layout.addWidget(button)
            self._recent_buttons.append(button)
        if not self._recent_buttons:
            self.recent_layout.addWidget(QLabel(tr("No recent colors")))
        self.recent_layout.addStretch(1)
        if hasattr(self, "_index_buttons"):
            self._select(self._selected)

    def accept(self) -> None:
        remember_color(self._selected)
        self.color_picked.emit(self._selected)
        super().accept()
