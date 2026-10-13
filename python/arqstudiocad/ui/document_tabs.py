"""The open drawings: a tab bar across the whole window and their views.

As in AutoCAD, the drawings' tabs run the full width of the window, under
the ribbon or the toolbars, and the panels start below them. So the tab bar
is not part of the central widget: the window puts :attr:`DocumentTabs.bar`
in a strip of its own above the dock areas, and this stack shows the view of
the chosen tab. Tabs and views keep the same order when tabs are dragged.
"""

from PySide6.QtCore import Signal
from PySide6.QtWidgets import QStackedWidget, QTabBar, QWidget


class DocumentTabs(QStackedWidget):
    """The views of the open drawings, chosen by :attr:`bar`."""

    tabCloseRequested = Signal(int)  # noqa: N815 (as QTabWidget's)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.bar = QTabBar()
        self.bar.setObjectName("document_tab_bar")
        self.bar.setDocumentMode(True)
        self.bar.setTabsClosable(True)
        self.bar.setMovable(True)
        self.bar.setExpanding(False)
        self.bar.setUsesScrollButtons(True)
        self.bar.currentChanged.connect(self.setCurrentIndex)
        self.bar.tabMoved.connect(self._on_tab_moved)
        self.bar.tabCloseRequested.connect(self.tabCloseRequested)
        self.currentChanged.connect(self.bar.setCurrentIndex)

    def addTab(self, widget: QWidget, text: str) -> int:  # noqa: N802 (as QTabWidget's)
        self.addWidget(widget)
        return self.bar.addTab(text)

    def removeTab(self, index: int) -> None:  # noqa: N802 (as QTabWidget's)
        widget = self.widget(index)
        if widget is not None:
            self.bar.removeTab(index)
            self.removeWidget(widget)

    def tabText(self, index: int) -> str:  # noqa: N802 (as QTabWidget's)
        return self.bar.tabText(index)

    def setTabText(self, index: int, text: str) -> None:  # noqa: N802 (as QTabWidget's)
        self.bar.setTabText(index, text)

    def tabToolTip(self, index: int) -> str:  # noqa: N802 (as QTabWidget's)
        return self.bar.tabToolTip(index)

    def setTabToolTip(self, index: int, tip: str) -> None:  # noqa: N802 (as QTabWidget's)
        self.bar.setTabToolTip(index, tip)

    def _on_tab_moved(self, origin: int, target: int) -> None:
        widget = self.widget(origin)
        if widget is None:
            return
        # Moving the shown view makes the stack show another for a moment:
        # it must not tell the bar, which picks the view itself.
        self.blockSignals(True)
        try:
            self.removeWidget(widget)
            self.insertWidget(target, widget)
        finally:
            self.blockSignals(False)
