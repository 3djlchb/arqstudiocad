"""The Options dialog (ADR 0029 D3, D4): the application's settings in
sections chosen from a sidebar, each subsection a titled group box, with
Reset section, OK, Apply and Cancel. Modal; it remembers its size and the
last section shown. Only settings whose feature exists are shown: this
version holds General (language and theme), Display (the crosshair, the
snap markers' and command line's colors, the block editor's background)
and User preferences (the wheel's zoom, what a right-click in model space
does).
"""

from collections.abc import Callable
from dataclasses import dataclass, fields, replace
from functools import partial
from typing import Any, Literal

from PySide6.QtCore import QEvent, QSettings, QSize, Qt
from PySide6.QtGui import QColor, QFont, QKeySequence, QPalette, QShortcut
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QSlider,
    QSpinBox,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.i18n import available_languages, tr
from arqstudiocad.ui.crosshair import (
    CROSSHAIR_TYPES,
    SIZES,
    Crosshair,
    CrosshairType,
    crosshair_color,
)
from arqstudiocad.ui.options_widgets import ColorField, CrosshairPreview, color_value
from arqstudiocad.ui.theme import THEMES, Theme, theme_title
from arqstudiocad.ui.viewport import (
    BLOCK_EDITOR_BACKGROUND,
    MARKER,
    MODEL_SPACE_BACKGROUND,
    WHEEL_PERCENT,
    WHEEL_PERCENTS,
)

#: The primary color: the current section and OK, white text on it at
#: about 5.5:1 (the brief's choice, as the Blocks palette's).
PRIMARY = "#0b6fa6"
#: The width of a row's label.
LABEL_WIDTH = 200
#: The dialog's first size.
SIZE = QSize(900, 640)
#: Where the dialog keeps its size and its last section.
SIZE_SETTING = "options/size"
SECTION_SETTING = "options/section"
#: What a right-click in model space does with nothing selected.
RIGHT_CLICK_SETTING = "options/right_click"

#: A right-click with nothing selected and no command running: the context
#: menu, or the last command repeated at once (ADR 0029 D4).
type RightClick = Literal["menu", "repeat"]
RIGHT_CLICKS: tuple[RightClick, ...] = ("menu", "repeat")


@dataclass(frozen=True)
class Options:
    """What the dialog edits. ``language`` is a language's code, or empty
    for the system's language."""

    language: str = ""
    theme: Theme = THEMES[0]
    right_click: RightClick = RIGHT_CLICKS[0]
    #: The wheel: forward zooms out when reversed (ZOOMWHEEL), and each
    #: notch zooms by ``zoom_factor`` percent.
    zoom_wheel_reversed: bool = False
    zoom_factor: int = WHEEL_PERCENT
    #: The crosshair (ADR 0029 D6): size in percent, type, ``#rrggbb`` or
    #: empty for automatic contrast.
    crosshair_size: int = 5
    crosshair_type: CrosshairType = CROSSHAIR_TYPES[0]
    crosshair_color: str = ""
    #: ``#rrggbb``, or empty: AutoCAD's yellow, the theme's text color.
    marker_color: str = ""
    command_line_color: str = ""
    #: Always a color.
    block_editor_background: str = QColor(*BLOCK_EDITOR_BACKGROUND).name()

    @property
    def crosshair(self) -> Crosshair:
        return Crosshair(self.crosshair_size, self.crosshair_type, self.crosshair_color)


#: Each section's defaults are these values for its settings.
DEFAULTS = Options()
#: The sections shown, in order, and the settings each one holds.
SECTIONS: dict[str, tuple[str, ...]] = {
    "general": ("language", "theme"),
    "display": (
        "crosshair_size",
        "crosshair_type",
        "crosshair_color",
        "marker_color",
        "command_line_color",
        "block_editor_background",
    ),
    "user_preferences": ("zoom_wheel_reversed", "zoom_factor", "right_click"),
}
#: The settings kept under ``options/``; language and theme keep their own
#: keys, shared with the Tools menu (ADR 0029 D5).
STORED = {
    name: f"options/{name}"
    for section in SECTIONS.values()
    for name in section
    if name not in ("language", "theme")
}


def section_title(key: str) -> str:
    return {
        "general": tr("General"),
        "display": tr("Display"),
        "user_preferences": tr("User preferences"),
    }[key]


def checked(name: str, saved: object) -> Any:
    """``saved`` as the value of the setting ``name``, or None when it is
    not one this version takes."""
    if name == "right_click":
        return next((value for value in RIGHT_CLICKS if value == saved), None)
    if name == "crosshair_type":
        return next((value for value in CROSSHAIR_TYPES if value == saved), None)
    if name in ("crosshair_size", "zoom_factor"):
        low, high = SIZES if name == "crosshair_size" else WHEEL_PERCENTS
        try:
            number = int(str(saved))
        except ValueError:
            return None
        return number if low <= number <= high else None
    if name == "zoom_wheel_reversed":
        # An INI file keeps it as text.
        if saved is True or saved == "true":
            return True
        if saved is False or saved == "false":
            return False
        return None
    color = color_value(saved) if isinstance(saved, str) else None
    if name == "block_editor_background" and not color:
        return None
    return color


def saved_options(
    settings: QSettings | None, language: str = "", theme: Theme = THEMES[0]
) -> Options:
    """The settings kept in ``settings``, each one this version does not
    take read as its default."""
    values: dict[str, Any] = {}
    for name, key in STORED.items():
        value = checked(name, settings.value(key)) if settings is not None else None
        if value is not None:
            values[name] = value
    return Options(language=language, theme=theme, **values)


def save_options(settings: QSettings, values: Options, keys: tuple[str, ...]) -> None:
    """Keeps the settings ``keys`` of ``values``."""
    for name in keys:
        if name in STORED:
            settings.setValue(STORED[name], getattr(values, name))


def saved_right_click(settings: QSettings | None) -> RightClick:
    """The right-click setting kept in ``settings``; the default when none
    is, or one this version does not know."""
    return saved_options(settings).right_click


def changed_keys(before: Options, after: Options) -> tuple[str, ...]:
    """The settings that differ, by field name."""
    return tuple(
        f.name for f in fields(Options) if getattr(before, f.name) != getattr(after, f.name)
    )


class OptionsDialog(QDialog):
    """Options: ``values`` shown, ``apply`` called with the values when
    applied (Apply or OK)."""

    def __init__(
        self,
        values: Options,
        apply: Callable[[Options], None],
        settings: QSettings | None = None,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("options_dialog")
        self.setModal(True)
        self._apply = apply
        self._settings = settings
        #: What is applied, and what the controls hold.
        self.applied = values
        self.pending = values
        self._filling = False
        #: The rows' labels and help, muted by :meth:`restyle`.
        self._labels: list[QLabel] = []
        self._helps: list[QLabel] = []

        self.sidebar = QListWidget()
        self.sidebar.setObjectName("options_sidebar")
        self.sidebar.setFixedWidth(190)
        self.pages = QStackedWidget()
        for key in SECTIONS:
            item = QListWidgetItem()
            item.setData(Qt.ItemDataRole.UserRole, key)
            item.setTextAlignment(Qt.AlignmentFlag.AlignCenter)
            item.setSizeHint(QSize(0, 46))
            self.sidebar.addItem(item)
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setWidget(self._page(key))
            self.pages.addWidget(scroll)
        self.sidebar.currentRowChanged.connect(self._show_section)
        for keys, step in (
            ("Ctrl+Tab", 1),
            ("Ctrl+PgDown", 1),
            ("Ctrl+Shift+Tab", -1),
            ("Ctrl+PgUp", -1),
        ):
            shortcut = QShortcut(QKeySequence(keys), self)
            shortcut.activated.connect(partial(self.next_section, step))

        self.reset_button = QPushButton()
        self.ok_button = QPushButton()
        self.apply_button = QPushButton()
        self.cancel_button = QPushButton()
        self.ok_button.setDefault(True)
        self.reset_button.clicked.connect(self.reset_section)
        self.ok_button.clicked.connect(self.ok)
        self.apply_button.clicked.connect(self.apply)
        self.cancel_button.clicked.connect(self.reject)
        actions = QHBoxLayout()
        actions.addWidget(self.reset_button)
        actions.addWidget(self.ok_button, 1)
        actions.addWidget(self.apply_button)
        actions.addWidget(self.cancel_button)

        content = QVBoxLayout()
        content.addWidget(self.pages, 1)
        content.addLayout(actions)
        body = QHBoxLayout(self)
        body.addWidget(self.sidebar)
        body.addLayout(content, 1)

        self.resize(self._saved_size())
        self.restyle()
        self.retranslate()
        self._fill()
        self.sidebar.setCurrentRow(self._saved_section())

    # Layout

    def _page(self, key: str) -> QWidget:
        page = QWidget()
        rows = QVBoxLayout(page)
        if key == "general":
            self.language_group = QGroupBox()
            self.language_label = QLabel()
            self.language = QComboBox()
            self.language.setObjectName("language")
            self.language.currentIndexChanged.connect(self._changed)
            self.theme_group = QGroupBox()
            self.theme_label = QLabel()
            self.theme = QComboBox()
            self.theme.setObjectName("theme")
            self.theme.currentIndexChanged.connect(self._changed)
            self._group(rows, self.language_group, [(self.language_label, self.language)])
            self._group(rows, self.theme_group, [(self.theme_label, self.theme)])
        elif key == "display":
            self._display_page(rows)
        elif key == "user_preferences":
            self.zoom_group = QGroupBox()
            self.zoom_wheel_reversed = QCheckBox()
            self.zoom_wheel_reversed.toggled.connect(self._changed)
            self.zoom_factor_label = QLabel()
            self.zoom_slider = QSlider(Qt.Orientation.Horizontal)
            self.zoom_spin = QSpinBox()
            for control in (self.zoom_slider, self.zoom_spin):
                control.setRange(*WHEEL_PERCENTS)
            self.zoom_slider.valueChanged.connect(self.zoom_spin.setValue)
            self.zoom_spin.valueChanged.connect(self.zoom_slider.setValue)
            self.zoom_spin.valueChanged.connect(self._changed)
            self.zoom_unit = QLabel()
            zoom_box = QWidget()
            zoom_line = QHBoxLayout(zoom_box)
            zoom_line.setContentsMargins(0, 0, 0, 0)
            zoom_line.addWidget(self.zoom_slider, 1)
            zoom_line.addWidget(self.zoom_spin)
            zoom_line.addWidget(self.zoom_unit)
            self._group(
                rows,
                self.zoom_group,
                [(None, self.zoom_wheel_reversed), (self.zoom_factor_label, zoom_box)],
            )
            self.right_click_group = QGroupBox()
            self.right_click_label = QLabel()
            self.right_click = QComboBox()
            self.right_click.setObjectName("right_click")
            self.right_click.currentIndexChanged.connect(self._changed)
            # One choice until a command has a menu of its options (ADR 0029
            # D4); shown so the setting is where it will grow.
            self.during_command_label = QLabel()
            self.during_command = QComboBox()
            self.during_command.setObjectName("during_command")
            self._group(
                rows,
                self.right_click_group,
                [
                    (self.right_click_label, self.right_click),
                    (self.during_command_label, self.during_command),
                ],
            )
        rows.addStretch(1)
        return page

    def _display_page(self, rows: QVBoxLayout) -> None:
        """Display: the crosshair with its live preview, the colors, the
        block editor's background (ADR 0029 D4, D6)."""
        self.crosshair_group = QGroupBox()
        self.size_label = QLabel()
        self.size_slider = QSlider(Qt.Orientation.Horizontal)
        self.size_spin = QSpinBox()
        for control in (self.size_slider, self.size_spin):
            control.setRange(*SIZES)
        # The number is the value; the slider follows it, and moves it.
        self.size_slider.valueChanged.connect(self.size_spin.setValue)
        self.size_spin.valueChanged.connect(self.size_slider.setValue)
        self.size_spin.valueChanged.connect(self._changed)
        self.size_help = QLabel()
        self.size_help.setWordWrap(True)
        self.preview = CrosshairPreview(QColor(*MODEL_SPACE_BACKGROUND))
        self.preview_caption = QLabel()
        self.preview_caption.setAlignment(Qt.AlignmentFlag.AlignCenter)
        size_line = QHBoxLayout()
        size_line.addWidget(self.size_slider, 1)
        size_line.addWidget(self.size_spin)
        size_line.addWidget(QLabel("%"))
        size_column = QVBoxLayout()
        size_column.addLayout(size_line)
        size_column.addWidget(self.size_help)
        size_column.addStretch(1)
        preview_column = QVBoxLayout()
        preview_column.addWidget(self.preview)
        preview_column.addWidget(self.preview_caption)
        size_box = QWidget()
        size_row = QHBoxLayout(size_box)
        size_row.setContentsMargins(0, 0, 0, 0)
        size_row.addLayout(size_column, 1)
        size_row.addLayout(preview_column)
        self.type_label = QLabel()
        self.crosshair_type = QComboBox()
        self.crosshair_type.setObjectName("crosshair_type")
        self.crosshair_type.currentIndexChanged.connect(self._changed)
        self.crosshair_color_label = QLabel()
        self.crosshair_color = ColorField(
            lambda: crosshair_color(Crosshair(), self.preview.background)
        )
        self.marker_color_label = QLabel()
        self.marker_color = ColorField(lambda: QColor(MARKER))
        self.command_line_color_label = QLabel()
        self.command_line_color = ColorField(lambda: self.palette().color(QPalette.ColorRole.Text))
        self.colors_help = QLabel()
        self.colors_help.setWordWrap(True)
        self._group(
            rows,
            self.crosshair_group,
            [
                (self.size_label, size_box),
                (self.type_label, self.crosshair_type),
                (self.crosshair_color_label, self.crosshair_color),
                (self.marker_color_label, self.marker_color),
                (self.command_line_color_label, self.command_line_color),
            ],
            [self.colors_help],
        )
        self.block_editor_group = QGroupBox()
        self.block_editor_label = QLabel()
        self.block_editor_background = ColorField(
            lambda: QColor(*BLOCK_EDITOR_BACKGROUND), required=True
        )
        self._group(
            rows,
            self.block_editor_group,
            [(self.block_editor_label, self.block_editor_background)],
        )
        self._helps += [self.size_help, self.preview_caption]
        for field in self._color_fields():
            field.changed.connect(self._changed)

    def _color_fields(self) -> tuple[ColorField, ...]:
        return (
            self.crosshair_color,
            self.marker_color,
            self.command_line_color,
            self.block_editor_background,
        )

    def _group(
        self,
        rows: QVBoxLayout,
        group: QGroupBox,
        content: list[tuple[QLabel | None, QWidget]],
        notes: list[QLabel] | None = None,
    ) -> None:
        """``group`` added to ``rows``, holding a row per label and control:
        a muted label 200 px wide, wrapped when longer, so the controls of
        every group line up; then its control taking the free width (the
        brief's layout); a control without a label, a check box, across the
        group; then ``notes``, help across the group."""
        plain = QFont(group.font())
        # Set, not just copied, so the group's bold title is not inherited.
        plain.setBold(False)
        title = QFont(plain)
        title.setBold(True)
        # The title in bold, what the group holds not.
        group.setFont(title)
        form = QFormLayout(group)
        for label, control in content:
            control.setFont(plain)
            if label is None:
                form.addRow(control)
                continue
            label.setFont(plain)
            label.setFixedWidth(LABEL_WIDTH)
            label.setWordWrap(True)
            control.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
            form.addRow(label, control)
            self._labels.append(label)
        for note in notes or []:
            form.addRow(note)
            self._helps.append(note)
        small = QFont(plain)
        small.setPointSizeF(max(1.0, plain.pointSizeF() - 1))
        for note in self._helps:
            note.setFont(small)
        rows.addWidget(group)

    def restyle(self) -> None:
        """Colors from the palette, the primary one fixed (ADR 0029 D3)."""
        colors = self.palette()
        shade = colors.color(QPalette.ColorRole.Window).darker(115).name()
        text = colors.color(QPalette.ColorRole.Text).name()
        line = colors.color(QPalette.ColorRole.Mid).name()
        muted = colors.color(QPalette.ColorRole.PlaceholderText).name()
        self.sidebar.setStyleSheet(
            f"QListWidget {{ background-color: {shade}; border: 1px solid {line}; }}"
            f"QListWidget::item {{ color: {text}; border-bottom: 1px solid {line}; }}"
            f"QListWidget::item:selected {{ background-color: {PRIMARY}; color: white; }}"
        )
        self.ok_button.setStyleSheet(
            f"QPushButton {{ background-color: {PRIMARY}; color: white; font-weight: bold; "
            f"border: 1px solid {PRIMARY}; border-radius: 2px; padding: 6px 12px; }}"
        )
        for label in self._labels + self._helps:
            label.setStyleSheet(f"color: {muted};")
        for field in self._color_fields():
            field.restyle()

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Options"))
        for row in range(self.sidebar.count()):
            item = self.sidebar.item(row)
            item.setText(section_title(item.data(Qt.ItemDataRole.UserRole)))
        self.reset_button.setText(tr("Reset section"))
        self.ok_button.setText(tr("OK"))
        self.apply_button.setText(tr("Apply"))
        self.cancel_button.setText(tr("Cancel"))
        self.language_group.setTitle(tr("Language"))
        self.language_label.setText(tr("Interface language"))
        self.theme_group.setTitle(tr("Theme"))
        self.theme_label.setText(tr("Theme"))
        self.right_click_group.setTitle(tr("Right-click in model space"))
        self.right_click_label.setText(tr("With nothing selected"))
        self.during_command_label.setText(tr("During a command"))
        self.zoom_group.setTitle(tr("Zoom"))
        self.zoom_wheel_reversed.setText(tr("Reverse wheel zoom (ZOOMWHEEL)"))
        self.zoom_factor_label.setText(tr("Zoom factor"))
        self.zoom_spin.setAccessibleName(tr("Zoom factor"))
        self.zoom_slider.setAccessibleName(tr("Zoom factor"))
        self.zoom_unit.setText(tr("% per notch"))
        self.crosshair_group.setTitle(tr("Crosshair"))
        self.size_label.setText(tr("Size"))
        self.size_spin.setAccessibleName(tr("Crosshair size, in percent"))
        self.size_slider.setAccessibleName(tr("Crosshair size, in percent"))
        self.size_help.setText(
            tr("Percent of the drawing area. At 100 % the lines cross the whole screen.")
        )
        self.preview_caption.setText(tr("Preview"))
        self.preview.retranslate()
        self.type_label.setText(tr("Type"))
        self.crosshair_color_label.setText(tr("Crosshair color"))
        self.marker_color_label.setText(tr("Object-snap marker color"))
        self.command_line_color_label.setText(tr("Command-line text color"))
        self.colors_help.setText(
            tr("Leave the color empty to keep the viewport's automatic contrast.")
        )
        self.block_editor_group.setTitle(tr("Block editor"))
        self.block_editor_label.setText(tr("Background color"))
        for field in self._color_fields():
            field.retranslate()
        self._fill()

    def _fill(self) -> None:
        """The controls' items and values, from ``pending``."""
        self._filling = True
        try:
            self.language.clear()
            self.language.addItem(tr("System language"), "")
            # Each language by its own name, readable whichever is active.
            for pack in available_languages():
                self.language.addItem(pack.name, pack.code)
            self.language.setCurrentIndex(max(0, self.language.findData(self.pending.language)))
            self.theme.clear()
            for theme in THEMES:
                self.theme.addItem(theme_title(theme), theme)
            self.theme.setCurrentIndex(max(0, self.theme.findData(self.pending.theme)))
            self.right_click.clear()
            self.right_click.addItem(tr("Show the context menu"), "menu")
            self.right_click.addItem(tr("Repeat the last command"), "repeat")
            self.right_click.setCurrentIndex(
                max(0, self.right_click.findData(self.pending.right_click))
            )
            self.during_command.clear()
            self.during_command.addItem(tr("Acts as Enter"), "enter")
            self.zoom_wheel_reversed.setChecked(self.pending.zoom_wheel_reversed)
            self.zoom_spin.setValue(self.pending.zoom_factor)
            self.size_spin.setValue(self.pending.crosshair_size)
            self.crosshair_type.clear()
            for kind, title in zip(
                CROSSHAIR_TYPES, (tr("Crosshair"), tr("Arrow"), tr("Small cross")), strict=True
            ):
                self.crosshair_type.addItem(title, kind)
            self.crosshair_type.setCurrentIndex(
                max(0, self.crosshair_type.findData(self.pending.crosshair_type))
            )
            for field in self._color_fields():
                field.set_value(getattr(self.pending, self._field_name(field)))
        finally:
            self._filling = False
        self.apply_button.setEnabled(self.pending != self.applied)
        self.preview.set_crosshair(self.pending.crosshair)

    def _field_name(self, field: ColorField) -> str:
        """The setting a color field edits."""
        names = {
            id(self.crosshair_color): "crosshair_color",
            id(self.marker_color): "marker_color",
            id(self.command_line_color): "command_line_color",
            id(self.block_editor_background): "block_editor_background",
        }
        return names[id(field)]

    # Sections

    def section(self) -> str:
        """The section shown."""
        item = self.sidebar.currentItem()
        return str(item.data(Qt.ItemDataRole.UserRole)) if item is not None else ""

    def _show_section(self, row: int) -> None:
        if row >= 0:
            self.pages.setCurrentIndex(row)

    def next_section(self, step: int) -> None:
        """The section ``step`` rows down the sidebar (up when negative),
        wrapping around: Ctrl+Tab, Ctrl+PgDn and back (ADR 0029 D3)."""
        count = self.sidebar.count()
        self.sidebar.setCurrentRow((self.sidebar.currentRow() + step) % count)

    # Changes

    def _changed(self) -> None:
        if self._filling:
            return
        language = self.language.currentData()
        theme = self.theme.currentData()
        right_click = self.right_click.currentData()
        kind = self.crosshair_type.currentData()
        self.pending = replace(
            self.pending,
            language=str(language) if language is not None else "",
            theme=theme if theme in THEMES else self.pending.theme,
            right_click=right_click if right_click in RIGHT_CLICKS else self.pending.right_click,
            zoom_wheel_reversed=self.zoom_wheel_reversed.isChecked(),
            zoom_factor=self.zoom_spin.value(),
            crosshair_size=self.size_spin.value(),
            crosshair_type=kind if kind in CROSSHAIR_TYPES else self.pending.crosshair_type,
            **{self._field_name(field): field.value() for field in self._color_fields()},
        )
        self.apply_button.setEnabled(self.pending != self.applied)
        self.preview.set_crosshair(self.pending.crosshair)

    def reset_section(self) -> None:
        """The section's settings back to their defaults, pending until
        applied."""
        defaults = {key: getattr(DEFAULTS, key) for key in SECTIONS[self.section()]}
        self.pending = replace(self.pending, **defaults)
        self._fill()

    def apply(self) -> None:
        """Applies what is pending and stays open."""
        if self.pending == self.applied:
            return
        self._apply(self.pending)
        self.applied = self.pending
        # A language just applied names everything again.
        self.retranslate()

    def ok(self) -> None:
        self.apply()
        self.accept()

    def changeEvent(self, event: QEvent) -> None:  # noqa: N802 (Qt API)
        # A theme applied reaches the dialog as a new palette; one can come
        # while it is built, before its widgets exist.
        super().changeEvent(event)
        if event.type() == QEvent.Type.PaletteChange and hasattr(self, "language_label"):
            self.restyle()

    # Remembered between openings

    def _saved_size(self) -> QSize:
        saved = self._settings.value(SIZE_SETTING) if self._settings is not None else None
        return saved if isinstance(saved, QSize) and saved.isValid() else SIZE

    def _saved_section(self) -> int:
        saved = self._settings.value(SECTION_SETTING) if self._settings is not None else None
        keys = list(SECTIONS)
        return keys.index(saved) if saved in keys else 0

    def done(self, result: int) -> None:
        if self._settings is not None:
            self._settings.setValue(SIZE_SETTING, self.size())
            self._settings.setValue(SECTION_SETTING, self.section())
        super().done(result)
