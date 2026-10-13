"""The working session: open documents, the active one, and the events that
tell everyone what happened to them (contract §5 and §7, ADR 0013).

This is the application layer between commands and the core: commands and
the interface go through it, never around it, so every change becomes an
event. It has no Qt.
"""

from pathlib import Path

from arqstudiocad.core import CadApplication, CadChangeSet, CadDocument
from arqstudiocad.events import (
    ActiveDocumentChanged,
    ActiveLayerChanged,
    DocumentChanged,
    DocumentClosed,
    DocumentOpened,
    DocumentSaved,
    EventBus,
)
from arqstudiocad.i18n import tr


class Session:
    """The documents open in one application window."""

    def __init__(self, bus: EventBus, app: CadApplication | None = None) -> None:
        self.bus = bus
        self.app = app if app is not None else CadApplication()
        self._documents: list[CadDocument] = []
        self._active: CadDocument | None = None
        self._new_count = 0

    @property
    def documents(self) -> list[CadDocument]:
        """The open documents, in opening order."""
        return list(self._documents)

    @property
    def active(self) -> CadDocument | None:
        """The document commands act on."""
        return self._active

    def activate(self, document: CadDocument | None) -> None:
        """Make ``document`` the one commands act on."""
        if document is not None and document not in self._documents:
            raise ValueError("not a document of this session")
        if document != self._active:
            self._active = document
            self.bus.publish(ActiveDocumentChanged(None if document is None else document.id))

    def new_document(self) -> CadDocument:
        """An empty drawing, named Drawing1, Drawing2… in the interface
        language, made active."""
        self._new_count += 1
        document = self.app.create_document(tr("Drawing{number}", number=self._new_count))
        return self._adopt(document)

    def open_document(self, path: Path) -> CadDocument:
        """Opens a DWG or DXF file and makes it active. Raises the core's
        errors (``CadIoError``, ``UnsupportedFormatError``) unchanged."""
        return self._adopt(self.app.open_document(path))

    def adopt(self, document: CadDocument) -> CadDocument:
        """Takes in a document opened elsewhere (e.g. on a worker thread)."""
        return self._adopt(document)

    def save(self, document: CadDocument, path: Path) -> None:
        """Saves as DWG or DXF, by the extension."""
        document.save(path)
        self.saved(document, path)

    def saved(self, document: CadDocument, path: Path) -> None:
        """Tells everyone ``document`` was saved to ``path`` (by work done
        elsewhere, e.g. on a worker thread)."""
        self.bus.publish(DocumentSaved(document.id, path))

    def close(self, document: CadDocument) -> None:
        """Closes it, discarding unsaved changes (ask first); the previous
        document becomes active."""
        index = self._documents.index(document)
        document.close()
        self._documents.remove(document)
        self.bus.publish(DocumentClosed(document.id))
        if self._active == document:
            self._active = None
            following = self._documents[max(0, index - 1)] if self._documents else None
            self.activate(following)
            if following is None:
                self.bus.publish(ActiveDocumentChanged(None))

    def report(self, document: CadDocument, changes: CadChangeSet) -> CadChangeSet:
        """Publishes what an operation on ``document`` did; returns it."""
        if not changes.is_empty:
            self.bus.publish(DocumentChanged(document.id, changes))
            if changes.document_changed:
                self.bus.publish(ActiveLayerChanged(document.id, document.current_layer.id))
        return changes

    def _adopt(self, document: CadDocument) -> CadDocument:
        self._documents.append(document)
        self.bus.publish(DocumentOpened(document.id, document.name, document.path))
        self.activate(document)
        return document
