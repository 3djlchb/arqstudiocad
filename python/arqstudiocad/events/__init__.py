"""Application events and the event bus (contract §7, ADR 0013 D5.5).

Events are frozen dataclasses: facts about what happened, never requests.
The core never calls Python; the application turns each change set it gets
from the core into a :class:`DocumentChanged`, and so on. This module has no
Qt: :mod:`arqstudiocad.ui.event_bridge` re-emits events as Qt signals.

Handlers run synchronously, in subscription order, on the publishing thread.
A handler that raises is logged and does not stop the others. An event
published from inside a handler is delivered after the current one has
reached every handler, so every handler sees events in the same order.
"""

import logging
import threading
from collections import deque
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

from arqstudiocad.core import CadChangeSet

__all__ = [
    "ActiveDocumentChanged",
    "ActiveLayerChanged",
    "BlockSaved",
    "CommandCancelled",
    "CommandFinished",
    "CommandMessage",
    "CommandPrompt",
    "CommandStarted",
    "DocumentChanged",
    "DocumentClosed",
    "DocumentOpened",
    "DocumentSaved",
    "DraftingChanged",
    "Drag",
    "Event",
    "EventBus",
    "LanguageChanged",
    "SelectionChanged",
    "Subscription",
    "WorkFinished",
    "WorkStarted",
]

log = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class Event:
    """Base of every event; subscribing to it receives them all."""


@dataclass(frozen=True, slots=True)
class DocumentOpened(Event):
    """A document was created (``path`` is None) or opened from a file."""

    document_id: int
    name: str
    path: Path | None


@dataclass(frozen=True, slots=True)
class DocumentSaved(Event):
    """A document was written to ``path``."""

    document_id: int
    path: Path


@dataclass(frozen=True, slots=True)
class BlockSaved(Event):
    """Block editor ``editor_id`` was saved into block ``name`` of document
    ``document_id`` (ADR 0023 D3); the editor is unmodified now."""

    editor_id: int
    document_id: int
    name: str


@dataclass(frozen=True, slots=True)
class DocumentClosed(Event):
    """A document was closed."""

    document_id: int


@dataclass(frozen=True, slots=True)
class ActiveDocumentChanged(Event):
    """Another document became the one commands act on; None when none is open."""

    document_id: int | None


@dataclass(frozen=True, slots=True)
class DocumentChanged(Event):
    """A document changed; ``changes`` names what, so views update only that."""

    document_id: int
    changes: CadChangeSet


@dataclass(frozen=True, slots=True)
class SelectionChanged(Event):
    """The selected entities of a document."""

    document_id: int
    entity_ids: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class ActiveLayerChanged(Event):
    """The layer new entities go to."""

    document_id: int
    layer_id: int


@dataclass(frozen=True, slots=True)
class CommandStarted(Event):
    """A command began; ``name`` is its English name, e.g. ``"LINE"``."""

    name: str


@dataclass(frozen=True, slots=True)
class CommandFinished(Event):
    """A command ended normally."""

    name: str


@dataclass(frozen=True, slots=True)
class CommandCancelled(Event):
    """A command was cancelled (Esc) or failed; ``reason`` is technical English."""

    name: str
    reason: str = ""


@dataclass(frozen=True, slots=True)
class Drag:
    """What a point prompt shows following the cursor (ADR 0016 D3): the
    entities, moved, rotated, scaled or mirrored as the point picked at
    that moment would leave them, from the prompt's base point."""

    kind: Literal["move", "rotate", "scale", "mirror"]
    entity_ids: tuple[int, ...]
    #: Whether a rubber band joins the base point to the cursor.
    band: bool = True


@dataclass(frozen=True, slots=True)
class CommandPrompt(Event):
    """A running command asks for input; ``text`` is ready to show,
    translated, e.g. ``"Zoom [Extents(E)/In(I)/Out(O)] <E>:"``. With
    ``point``, it asks for a point the viewport may pick; ``base`` is the
    last point, for ORTHO and the rubber band, and ``drag`` what follows the
    cursor."""

    name: str
    text: str
    point: bool = False
    base: tuple[float, float] | None = None
    drag: Drag | None = None


@dataclass(frozen=True, slots=True)
class DraftingChanged(Event):
    """The running object snaps (on and which, by AutoCAD's key) or ORTHO
    changed (ADR 0016 D5)."""

    osnap: bool
    snaps: tuple[str, ...]
    ortho: bool


@dataclass(frozen=True, slots=True)
class CommandMessage(Event):
    """A line for the command window's history, translated."""

    text: str


@dataclass(frozen=True, slots=True)
class WorkStarted(Event):
    """Long work began on a worker thread (opening, saving); ``message`` is
    translated, e.g. ``"Opening plano.dwg…"``."""

    message: str


@dataclass(frozen=True, slots=True)
class WorkFinished(Event):
    """The long work ended, well or not."""


@dataclass(frozen=True, slots=True)
class LanguageChanged(Event):
    """The interface language changed to ``code`` (e.g. ``"es"``)."""

    code: str


type Handler[E: Event] = Callable[[E], None]


class Subscription:
    """What :meth:`EventBus.subscribe` returns: cancel it to stop receiving."""

    def __init__(self, bus: "EventBus", event_type: type[Event], handler: Handler[Event]) -> None:
        self._bus = bus
        self._entry = (event_type, handler)

    def cancel(self) -> None:
        """Stop receiving; cancelling twice is harmless."""
        self._bus._remove(self._entry)


class EventBus:
    """Delivers events to the handlers subscribed to their type."""

    def __init__(self) -> None:
        self._handlers: list[tuple[type[Event], Handler[Event]]] = []
        self._queue: deque[Event] = deque()
        self._delivering = False
        self._lock = threading.RLock()

    def subscribe[E: Event](self, event_type: type[E], handler: Handler[E]) -> Subscription:
        """Call ``handler`` with every event of ``event_type`` or a subclass."""
        entry = (event_type, handler)
        with self._lock:
            self._handlers.append(entry)  # type: ignore[arg-type]
        return Subscription(self, event_type, handler)  # type: ignore[arg-type]

    def publish(self, event: Event) -> None:
        """Deliver ``event`` to its handlers, now (or right after the event
        being delivered, if called from a handler)."""
        with self._lock:
            self._queue.append(event)
            if self._delivering:
                return
            self._delivering = True
        try:
            while True:
                with self._lock:
                    if not self._queue:
                        return
                    current = self._queue.popleft()
                    handlers = [h for t, h in self._handlers if isinstance(current, t)]
                for handler in handlers:
                    try:
                        handler(current)
                    except Exception:
                        log.exception("event handler failed on %r", current)
        finally:
            with self._lock:
                self._delivering = False

    def _remove(self, entry: tuple[type[Event], Handler[Event]]) -> None:
        with self._lock:
            if entry in self._handlers:
                self._handlers.remove(entry)
