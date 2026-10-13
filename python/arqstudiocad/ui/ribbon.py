"""The ribbon (ADR 0015): tabs holding groups of tool buttons.

Qt has no ribbon, so this is a tab widget whose pages are rows of groups,
each group a row of buttons with its title underneath. Buttons show the
commands' actions, as text until there are icons. Double-clicking a tab
folds the ribbon to its tab bar, and again unfolds it.
"""

from collections.abc import Mapping

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QAction
from PySide6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QSizePolicy,
    QStackedWidget,
    QTabWidget,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.workspaces import Discipline, RibbonGroup


class RibbonGroupWidget(QFrame):
    def __init__(self, group: RibbonGroup, actions: Mapping[str, QAction]) -> None:
        super().__init__()
        self.group = group
        self.setObjectName(f"ribbon_group_{group.name}")
        self.setFrameShape(QFrame.Shape.StyledPanel)
        self.buttons: list[QToolButton] = []
        row = QHBoxLayout()
        row.setSpacing(2)
        for name in group.commands:
            button = QToolButton()
            button.setDefaultAction(actions[name])
            button.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextOnly)
            button.setAutoRaise(True)
            row.addWidget(button)
            self.buttons.append(button)
        self.title = QLabel()
        self.title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        column = QVBoxLayout(self)
        column.setContentsMargins(4, 2, 4, 2)
        column.setSpacing(0)
        column.addLayout(row, 1)
        column.addWidget(self.title)
        self.retranslate()

    def retranslate(self) -> None:
        self.title.setText(self.group.title())


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
            self.addTab(page, "")
        self._folded = False
        self.tabBar().tabBarDoubleClicked.connect(self._on_double_click)
        self.retranslate()

    def retranslate(self) -> None:
        for index, tab in enumerate(self.discipline.ribbon):
            self.setTabText(index, tab.title())
        for group in self.groups:
            group.retranslate()

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
