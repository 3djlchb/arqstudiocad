"""Re-emits application events as Qt signals (contract §7, ADR 0013 D5.5).

Widgets connect to these signals instead of subscribing to the bus. The
bridge lives on the GUI thread, so an event published from a worker thread
(opening or saving in the background) reaches its slots on the GUI thread,
through Qt's queued connections.
"""

from PySide6.QtCore import QObject, Signal, SignalInstance

from arqstudiocad.events import (
    ActiveDocumentChanged,
    ActiveLayerChanged,
    BlockRenamed,
    BlockSaved,
    BlockUsed,
    CommandCancelled,
    CommandFinished,
    CommandMessage,
    CommandPrompt,
    CommandStarted,
    DocumentChanged,
    DocumentClosed,
    DocumentOpened,
    DocumentSaved,
    DraftingChanged,
    Event,
    EventBus,
    LanguageChanged,
    OptionsChanged,
    SelectionChanged,
    WorkFinished,
    WorkStarted,
)


class QtEventBridge(QObject):
    """One signal per event type, plus ``event_published`` for every event."""

    event_published = Signal(object)
    document_opened = Signal(object)
    document_saved = Signal(object)
    block_saved = Signal(object)
    block_renamed = Signal(object)
    block_used = Signal(object)
    document_closed = Signal(object)
    active_document_changed = Signal(object)
    document_changed = Signal(object)
    selection_changed = Signal(object)
    active_layer_changed = Signal(object)
    command_started = Signal(object)
    command_finished = Signal(object)
    command_cancelled = Signal(object)
    command_prompt = Signal(object)
    drafting_changed = Signal(object)
    command_message = Signal(object)
    language_changed = Signal(object)
    options_changed = Signal(object)
    work_started = Signal(object)
    work_finished = Signal(object)

    def __init__(self, bus: EventBus, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._signals: dict[type[Event], SignalInstance] = {
            DocumentOpened: self.document_opened,
            DocumentSaved: self.document_saved,
            BlockSaved: self.block_saved,
            BlockRenamed: self.block_renamed,
            BlockUsed: self.block_used,
            DocumentClosed: self.document_closed,
            ActiveDocumentChanged: self.active_document_changed,
            DocumentChanged: self.document_changed,
            SelectionChanged: self.selection_changed,
            ActiveLayerChanged: self.active_layer_changed,
            CommandStarted: self.command_started,
            CommandFinished: self.command_finished,
            CommandCancelled: self.command_cancelled,
            CommandPrompt: self.command_prompt,
            DraftingChanged: self.drafting_changed,
            CommandMessage: self.command_message,
            LanguageChanged: self.language_changed,
            OptionsChanged: self.options_changed,
            WorkStarted: self.work_started,
            WorkFinished: self.work_finished,
        }
        self._subscription = bus.subscribe(Event, self._emit)
        self.destroyed.connect(self._subscription.cancel)

    def _emit(self, event: Event) -> None:
        self.event_published.emit(event)
        signal = self._signals.get(type(event))
        if signal is not None:
            signal.emit(event)
