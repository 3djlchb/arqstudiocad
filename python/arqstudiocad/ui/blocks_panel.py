"""The Blocks palette (ADR 0025): the block definitions of the active
drawing, filtered as the user types.

This is the palette's first step: the Drawing source as a list. Previews,
the other sources and inserting from the palette come in the next steps
(ADR 0025 D9).
"""

from PySide6.QtGui import QPalette
from PySide6.QtWidgets import (
    QDockWidget,
    QLabel,
    QLineEdit,
    QListWidget,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.events import Event
from arqstudiocad.i18n import tr
from arqstudiocad.session import Session
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.panel_style import apply_data_view_style


class BlocksPanel(QDockWidget):
    """The blocks of the active drawing, without anonymous or layout blocks."""

    def __init__(
        self, session: Session, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("blocks_panel")
        self._session = session
        self.search = QLineEdit()
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(self.refresh)
        self.list = QListWidget()
        apply_data_view_style(self.list)
        self.footer = QLabel()
        body = QWidget()
        column = QVBoxLayout(body)
        column.setContentsMargins(4, 4, 4, 4)
        column.addWidget(self.search)
        column.addWidget(self.list, 1)
        column.addWidget(self.footer)
        self.setWidget(body)
        for signal in (events.active_document_changed, events.document_changed):
            signal.connect(self.refresh)
        self.restyle()
        self.retranslate()

    def restyle(self) -> None:
        """Refresh palette-based colors after a theme change."""
        apply_data_view_style(self.list)
        muted = self.palette().color(QPalette.ColorRole.PlaceholderText).name()
        self.footer.setStyleSheet(f"color: {muted};")

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Blocks"))
        self.search.setPlaceholderText(tr("Search blocks"))
        self.refresh()

    def names(self) -> list[str]:
        """The active drawing's blocks that match the search, case-insensitively."""
        document = self._session.active
        if document is None or not document.is_open:
            return []
        wanted = self.search.text().strip().casefold()
        return [name for name in document.block_names if wanted in name.casefold()]

    def refresh(self, event: Event | str | None = None) -> None:
        document = self._session.active
        self.list.clear()
        if document is None or not document.is_open:
            self.list.setEnabled(False)
            self.footer.setText(tr("No drawing is open."))
            return
        self.list.setEnabled(True)
        shown = self.names()
        self.list.addItems(shown)
        if not document.block_names:
            self.footer.setText(tr("The drawing has no blocks."))
        elif not shown:
            self.footer.setText(tr("No block matches the search."))
        elif len(shown) == 1:
            self.footer.setText(tr("1 block"))
        else:
            self.footer.setText(tr("{count} blocks", count=len(shown)))
