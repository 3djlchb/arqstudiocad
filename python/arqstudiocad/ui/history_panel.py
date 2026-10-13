"""The history panel (ADR 0013, D5.6): the active drawing's undo steps.

The list starts at "(start)", then every step that can be undone, oldest
first, then the steps that can be redone, in gray. The current state is the
selected row; choosing another row undoes or redoes until the drawing is in
that state.
"""

from PySide6.QtGui import QBrush, QPalette
from PySide6.QtWidgets import QDockWidget, QListWidget, QListWidgetItem, QWidget

from arqstudiocad.core import CadError
from arqstudiocad.events import CommandMessage, Event
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui.event_bridge import QtEventBridge


class HistoryPanel(QDockWidget):
    """The undo history of the active drawing."""

    def __init__(
        self, session: Session, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("history_panel")
        self._session = session
        self._filling = False
        self.list = QListWidget()
        self.list.currentRowChanged.connect(self._on_row_chosen)
        self.setWidget(self.list)
        for signal in (events.active_document_changed, events.document_changed):
            signal.connect(self.refresh)
        self.retranslate()

    def retranslate(self) -> None:
        self.setWindowTitle(tr("History"))
        self.refresh()

    def position(self) -> int:
        """The row of the current state: the number of steps that can be undone."""
        document = self._session.active
        return 0 if document is None or not document.is_open else len(document.undo_history)

    def refresh(self, event: Event | None = None) -> None:
        document = self._session.active
        self._filling = True
        try:
            self.list.clear()
            if document is None or not document.is_open:
                self.list.setEnabled(False)
                return
            self.list.setEnabled(True)
            self.list.addItem(tr("(start)"))
            for name in document.undo_history:
                self.list.addItem(name)
            redo_brush = QBrush(self.palette().color(QPalette.ColorRole.PlaceholderText))
            for name in document.redo_history:
                item = QListWidgetItem(name)
                item.setForeground(redo_brush)
                self.list.addItem(item)
            self.list.setCurrentRow(self.position())
        finally:
            self._filling = False

    def go_to(self, row: int) -> None:
        """Undoes or redoes until the drawing is in the state of ``row``."""
        document = self._session.active
        if document is None or not document.is_open:
            return
        try:
            while len(document.undo_history) > row and document.can_undo:
                self._session.edit(document, lambda d: d.undo())
            while len(document.undo_history) < row and document.can_redo:
                self._session.edit(document, lambda d: d.redo())
        except CadError as error:
            self._session.bus.publish(CommandMessage(tr("Error: {message}", message=str(error))))
        self.refresh()

    def _on_row_chosen(self, row: int) -> None:
        if not self._filling and row >= 0:
            self.go_to(row)
