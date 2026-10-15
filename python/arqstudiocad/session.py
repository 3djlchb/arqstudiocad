"""The working session: open documents, the active one, and the events that
tell everyone what happened to them (contract §5 and §7, ADR 0013).

This is the application layer between commands and the core: commands and
the interface go through it, never around it, so every change becomes an
event. It has no Qt.
"""

from collections.abc import Callable, Iterable
from pathlib import Path

from arqstudiocad.core import CadApplication, CadChangeSet, CadDocument
from arqstudiocad.drafting import Drafting
from arqstudiocad.events import (
    ActiveDocumentChanged,
    ActiveLayerChanged,
    DocumentChanged,
    DocumentClosed,
    DocumentOpened,
    DocumentSaved,
    EventBus,
    SelectionChanged,
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
        # The selected entities of each document, in the order selected
        # (ADR 0016 D3).
        self._selection: dict[int, tuple[int, ...]] = {}
        #: How points are picked: running snaps, ORTHO, the cursor.
        self.drafting = Drafting(bus)

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
        self._selection.pop(document.id, None)
        self.bus.publish(DocumentClosed(document.id))
        if self._active == document:
            self._active = None
            following = self._documents[max(0, index - 1)] if self._documents else None
            self.activate(following)
            if following is None:
                self.bus.publish(ActiveDocumentChanged(None))

    # Selection (ADR 0016 D3): per document, published as SelectionChanged.

    def selection(self, document: CadDocument) -> tuple[int, ...]:
        """The selected entities of ``document``, in the order selected."""
        return self._selection.get(document.id, ())

    def select(self, document: CadDocument, entity_ids: Iterable[int]) -> None:
        """Adds entities to the selection of ``document`` (a click, a window)."""
        current = self.selection(document)
        added = tuple(dict.fromkeys(i for i in entity_ids if i not in current))
        self._set_selection(document.id, current + added)

    def deselect(self, document: CadDocument, entity_ids: Iterable[int]) -> None:
        """Removes entities from the selection of ``document`` (Shift+click)."""
        removed = set(entity_ids)
        self._set_selection(
            document.id, tuple(i for i in self.selection(document) if i not in removed)
        )

    def clear_selection(self, document: CadDocument) -> None:
        """Empties the selection of ``document`` (Esc, the end of an editing
        command)."""
        self._set_selection(document.id, ())

    def _set_selection(self, document_id: int, entity_ids: tuple[int, ...]) -> None:
        if entity_ids != self._selection.get(document_id, ()):
            self._selection[document_id] = entity_ids
            self.bus.publish(SelectionChanged(document_id, entity_ids))

    def edit(
        self, document: CadDocument, operation: Callable[[CadDocument], CadChangeSet]
    ) -> CadChangeSet:
        """Runs one core operation on ``document`` and publishes what it did.
        The operation is one undo step; if it raises (a core error), nothing
        changed and nothing is published."""
        return self.report(document, operation(document))

    def report(self, document: CadDocument, changes: CadChangeSet) -> CadChangeSet:
        """Publishes what an operation on ``document`` did; returns it."""
        if not changes.is_empty:
            self.bus.publish(DocumentChanged(document.id, changes))
            if changes.document_changed:
                self.bus.publish(ActiveLayerChanged(document.id, document.current_layer.id))
            if changes.deleted:
                # What no longer exists is no longer selected (an erase, an
                # undo of a creation).
                self.deselect(document, changes.deleted)
        return changes

    def _adopt(self, document: CadDocument) -> CadDocument:
        self._documents.append(document)
        self.bus.publish(DocumentOpened(document.id, document.name, document.path))
        self.activate(document)
        return document
