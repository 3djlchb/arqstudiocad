"""The Options dialog (ADR 0029 D3, D4): the application's settings in
sections chosen from a sidebar, each subsection a titled group box, with
Reset section, OK, Apply and Cancel. Modal; it remembers its size and the
last section shown. Only settings whose feature exists are shown: this
version holds General (language and theme) and User preferences (what a
right-click in model space does).
"""

from collections.abc import Callable
from dataclasses import dataclass, fields, replace
from functools import partial
from typing import Literal

from PySide6.QtCore import QEvent, QSettings, QSize, Qt
from PySide6.QtGui import QFont, QKeySequence, QPalette, QShortcut
from PySide6.QtWidgets import (
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
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.i18n import available_languages, tr
from arqstudiocad.ui.theme import THEMES, Theme, theme_title

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


#: Each section's defaults are these values for its settings.
DEFAULTS = Options()
#: The sections shown, in order, and the settings each one holds.
SECTIONS: dict[str, tuple[str, ...]] = {
    "general": ("language", "theme"),
    "user_preferences": ("right_click",),
}


def section_title(key: str) -> str:
    return {"general": tr("General"), "user_preferences": tr("User preferences")}[key]


def saved_right_click(settings: QSettings | None) -> RightClick:
    """The right-click setting kept in ``settings``; the default when none
    is, or one this version does not know."""
    saved = settings.value(RIGHT_CLICK_SETTING) if settings is not None else None
    for value in RIGHT_CLICKS:
        if saved == value:
            return value
    return DEFAULTS.right_click


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
        #: The rows' labels, muted by :meth:`restyle`.
        self._labels: list[QLabel] = []

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
        elif key == "user_preferences":
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

    def _group(
        self, rows: QVBoxLayout, group: QGroupBox, content: list[tuple[QLabel, QWidget]]
    ) -> None:
        """``group`` added to ``rows``, holding a row per label and control:
        a muted label about 200 px wide, then its control taking the free
        width (the brief's layout)."""
        plain = QFont(group.font())
        # Set, not just copied, so the group's bold title is not inherited.
        plain.setBold(False)
        title = QFont(plain)
        title.setBold(True)
        # The title in bold, what the group holds not.
        group.setFont(title)
        form = QFormLayout(group)
        for label, control in content:
            label.setFont(plain)
            control.setFont(plain)
            label.setMinimumWidth(LABEL_WIDTH)
            control.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
            form.addRow(label, control)
            self._labels.append(label)
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
        for label in self._labels:
            label.setStyleSheet(f"color: {muted};")

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
        finally:
            self._filling = False
        self.apply_button.setEnabled(self.pending != self.applied)

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
        self.pending = Options(
            language=str(language) if language is not None else "",
            theme=theme if theme in THEMES else self.pending.theme,
            right_click=right_click if right_click in RIGHT_CLICKS else self.pending.right_click,
        )
        self.apply_button.setEnabled(self.pending != self.applied)

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
