"""The ribbon (ADR 0015): tabs holding groups of tool buttons.

Qt has no ribbon, so this is a tab widget whose pages are rows of groups,
each group its buttons with its title underneath. Buttons show command icons
when available, :data:`RIBBON_ICON` pixels, and their names underneath,
in columns of :data:`ROWS` buttons: one for now, so they stand side by side
as AutoCAD's large buttons (maintainer, 2026-10-25). A tab wider than the
window scrolls sideways rather than widen the window. Double-clicking a tab
folds the ribbon to its tab bar, and again unfolds it.
"""

from collections.abc import Mapping

from PySide6.QtCore import QSize, Qt, Signal
from PySide6.QtGui import QAction, QResizeEvent
from PySide6.QtWidgets import (
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QScrollArea,
    QSizePolicy,
    QStackedWidget,
    QTabWidget,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.ui.command_icons import RIBBON_ICON
from arqstudiocad.ui.panel_style import toggle_focus_style
from arqstudiocad.workspaces import Discipline, RibbonGroup

#: Buttons stacked in one column of a group: one, for now (maintainer,
#: 2026-10-25).
ROWS = 1


class RibbonGroupWidget(QFrame):
    def __init__(self, group: RibbonGroup, actions: Mapping[str, QAction]) -> None:
        super().__init__()
        self.group = group
        self.setObjectName(f"ribbon_group_{group.name}")
        self.setFrameShape(QFrame.Shape.StyledPanel)
        self.buttons: list[QToolButton] = []
        grid = QGridLayout()
        grid.setSpacing(2)
        for index, name in enumerate(group.commands):
            button = QToolButton()
            button.setDefaultAction(actions[name])
            if not actions[name].icon().isNull():
                button.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextUnderIcon)
                button.setIconSize(QSize(RIBBON_ICON, RIBBON_ICON))
            else:
                button.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextOnly)
            button.setAutoRaise(True)
            if actions[name].isCheckable():
                # Pressed while what it toggles is open (ADR 0025 D2).
                button.setStyleSheet(toggle_focus_style())
            grid.addWidget(button, index % ROWS, index // ROWS)
            self.buttons.append(button)
        self.title = QLabel()
        self.title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        column = QVBoxLayout(self)
        column.setContentsMargins(4, 2, 4, 2)
        column.setSpacing(0)
        column.addLayout(grid, 1)
        column.addWidget(self.title)
        self.retranslate()

    def retranslate(self) -> None:
        self.title.setText(self.group.title())


class RibbonPage(QScrollArea):
    """A tab's groups, scrolled sideways when the window is narrower than
    they are, so the ribbon never sets the window's width (small screens,
    ADR 0013 D5.8)."""

    def __init__(self, page: QWidget) -> None:
        super().__init__()
        self.setFrameShape(QFrame.Shape.NoFrame)
        self.setWidgetResizable(True)
        self.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        self.setWidget(page)

    def _height(self) -> int:
        """The groups' height, and the scroll bar's while it shows."""
        page = self.widget()
        height = page.sizeHint().height() if page is not None else 0
        if page is not None and page.sizeHint().width() > self.viewport().width():
            height += self.horizontalScrollBar().sizeHint().height()
        return height

    def sizeHint(self) -> QSize:  # noqa: N802 (Qt API)
        page = self.widget()
        return QSize(page.sizeHint().width() if page is not None else 0, self._height())

    def minimumSizeHint(self) -> QSize:  # noqa: N802 (Qt API)
        return QSize(0, self._height())

    def resizeEvent(self, event: QResizeEvent) -> None:  # noqa: N802 (Qt API)
        super().resizeEvent(event)
        # The scroll bar comes or goes: the ribbon's height follows.
        self.updateGeometry()


class Ribbon(QTabWidget):
    """The ribbon of one discipline."""

    folded_changed = Signal(bool)

    def __init__(
        self, discipline: Discipline, actions: Mapping[str, QAction], parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.discipline = discipline
        self.setObjectName(f"ribbon_{discipline.name}")
        self.setDocumentMode(True)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Maximum)
        self.groups: list[RibbonGroupWidget] = []
        for tab in discipline.ribbon:
            page = QWidget()
            page.setObjectName(f"ribbon_tab_{tab.name}")
            row = QHBoxLayout(page)
            row.setContentsMargins(2, 2, 2, 2)
            for group in tab.groups:
                widget = RibbonGroupWidget(group, actions)
                row.addWidget(widget)
                self.groups.append(widget)
            row.addStretch(1)
            self.addTab(RibbonPage(page), "")
        self._folded = False
        self.tabBar().tabBarDoubleClicked.connect(self._on_double_click)
        self.set_context(None)
        self.retranslate()

    def retranslate(self) -> None:
        for index, tab in enumerate(self.discipline.ribbon):
            self.setTabText(index, tab.title())
        for group in self.groups:
            group.retranslate()

    def set_context(self, context: str | None) -> None:
        """Shows the contextual tabs of ``context`` only, and brings the
        first of them forward when it starts; leaving it, the first tab."""
        current = self.discipline.ribbon[self.currentIndex()] if self.count() else None
        entering = None
        for index, tab in enumerate(self.discipline.ribbon):
            shown = tab.context is None or tab.context == context
            self.setTabVisible(index, shown)
            if tab.context is not None and shown and entering is None:
                entering = index
        if entering is not None and current is not None and current.context != context:
            self.setCurrentIndex(entering)
        elif current is not None and current.context not in (None, context):
            self.setCurrentIndex(0)

    @property
    def folded(self) -> bool:
        return self._folded

    def set_folded(self, folded: bool) -> None:
        """Folded, only the tab bar shows; unfolded, the current tab's tools."""
        if folded == self._folded:
            return
        self._folded = folded
        stack = self.findChild(QStackedWidget)
        if stack is not None:
            stack.setVisible(not folded)
        self.setMaximumHeight(self.tabBar().sizeHint().height() if folded else 16_777_215)
        self.folded_changed.emit(folded)

    def _on_double_click(self, index: int) -> None:
        if index >= 0:
            self.set_folded(not self._folded)
