"""Events, the event bus and its Qt bridge (contract §7, ADR 0013 D5.5)."""

import dataclasses
import logging
import threading
from pathlib import Path

import pytest
from PySide6.QtCore import QCoreApplication, QThread
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication
from arqstudiocad.events import (
    CommandCancelled,
    CommandStarted,
    DocumentChanged,
    DocumentOpened,
    Event,
    EventBus,
    LanguageChanged,
    SelectionChanged,
)
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.main_window import MainWindow


def test_events_are_frozen_values() -> None:
    opened = DocumentOpened(1, "Plano1", None)
    assert opened == DocumentOpened(1, "Plano1", None)
    assert hash(opened) == hash(DocumentOpened(1, "Plano1", None))
    with pytest.raises(dataclasses.FrozenInstanceError):
        opened.name = "otro"  # type: ignore[misc]
    assert CommandCancelled("LINE").reason == ""
    assert SelectionChanged(1, (5, 7)).entity_ids == (5, 7)
    assert DocumentOpened(2, "a.dwg", Path("a.dwg")).path == Path("a.dwg")


def test_a_change_set_travels_in_document_changed() -> None:
    doc = CadApplication().create_document("Plano1")
    _, changes = doc.add_line((0, 0), (1, 1))
    event = DocumentChanged(doc.id, changes)
    assert event.changes.created == changes.created


def test_handlers_get_their_type_in_subscription_order() -> None:
    bus = EventBus()
    seen: list[str] = []
    bus.subscribe(LanguageChanged, lambda e: seen.append(f"first {e.code}"))
    bus.subscribe(CommandStarted, lambda e: seen.append(f"command {e.name}"))
    bus.subscribe(Event, lambda e: seen.append(f"all {type(e).__name__}"))
    bus.subscribe(LanguageChanged, lambda e: seen.append(f"second {e.code}"))
    bus.publish(LanguageChanged("es"))
    bus.publish(CommandStarted("LINE"))
    assert seen == [
        "first es",
        "all LanguageChanged",
        "second es",
        "command LINE",
        "all CommandStarted",
    ]


def test_cancelled_subscriptions_stop_receiving() -> None:
    bus = EventBus()
    seen: list[Event] = []
    subscription = bus.subscribe(Event, seen.append)
    bus.publish(CommandStarted("A"))
    subscription.cancel()
    subscription.cancel()  # harmless
    bus.publish(CommandStarted("B"))
    assert seen == [CommandStarted("A")]


def test_a_failing_handler_does_not_stop_the_others(caplog: pytest.LogCaptureFixture) -> None:
    bus = EventBus()
    seen: list[str] = []

    def broken(event: CommandStarted) -> None:
        raise RuntimeError("a bug in a plugin")

    bus.subscribe(CommandStarted, broken)
    bus.subscribe(CommandStarted, lambda e: seen.append(e.name))
    with caplog.at_level(logging.ERROR, logger="arqstudiocad.events"):
        bus.publish(CommandStarted("LINE"))
    assert seen == ["LINE"]
    assert "event handler failed" in caplog.text and "a bug in a plugin" in caplog.text


def test_events_published_by_handlers_keep_the_order() -> None:
    # The first handler publishes B while A is being delivered: every
    # handler must still see A before B.
    bus = EventBus()
    seen: list[str] = []

    def chain(event: CommandStarted) -> None:
        seen.append(f"one {event.name}")
        if event.name == "A":
            bus.publish(CommandStarted("B"))

    bus.subscribe(CommandStarted, chain)
    bus.subscribe(CommandStarted, lambda e: seen.append(f"two {e.name}"))
    bus.publish(CommandStarted("A"))
    assert seen == ["one A", "two A", "one B", "two B"]


def test_the_bridge_re_emits_events_as_signals(qtbot: QtBot) -> None:
    bus = EventBus()
    bridge = QtEventBridge(bus)
    with (
        qtbot.waitSignal(bridge.language_changed) as typed,
        qtbot.waitSignal(bridge.event_published) as generic,
    ):
        bus.publish(LanguageChanged("es"))
    assert typed.args == [LanguageChanged("es")]
    assert generic.args == [LanguageChanged("es")]


def test_events_from_a_worker_thread_reach_slots_on_the_gui_thread(qtbot: QtBot) -> None:
    bus = EventBus()
    bridge = QtEventBridge(bus)
    gui = QCoreApplication.instance()
    assert gui is not None
    received: list[tuple[Event, bool]] = []
    bridge.document_opened.connect(
        lambda e: received.append((e, QThread.currentThread() is gui.thread()))
    )
    worker = threading.Thread(target=bus.publish, args=(DocumentOpened(3, "x.dwg", None),))
    with qtbot.waitSignal(bridge.document_opened, timeout=5000):
        worker.start()
        worker.join()
    qtbot.waitUntil(lambda: bool(received))
    assert received == [(DocumentOpened(3, "x.dwg", None), True)]


def test_the_window_retranslates_on_language_changed(qtbot: QtBot) -> None:
    bus = EventBus()
    window = MainWindow(bus=bus)
    qtbot.addWidget(window)
    seen: list[Event] = []
    bus.subscribe(LanguageChanged, seen.append)
    window.change_language("es")
    assert seen == [LanguageChanged("es")]
    assert window.menuBar().actions()[0].text() == "&Archivo"
    # An event is a fact: whoever changes the language (a command, later)
    # sets it and then publishes; the window follows.
    set_language("en")
    bus.publish(LanguageChanged("en"))
    assert window.menuBar().actions()[0].text() == "&File"


def test_a_destroyed_bridge_stops_listening(caplog: pytest.LogCaptureFixture) -> None:
    import shiboken6

    bus = EventBus()
    bridge = QtEventBridge(bus)
    shiboken6.delete(bridge)  # what Qt does when its parent window closes
    with caplog.at_level(logging.ERROR, logger="arqstudiocad.events"):
        bus.publish(LanguageChanged("es"))
    assert caplog.text == "", "nothing calls into the deleted object"
