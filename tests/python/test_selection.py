"""Selection, as AutoCAD (ADR 0016 D3, D4): the session's model, and the
viewport's clicks, windows, crossings, Esc and highlight."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QKeyEvent, QMouseEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.core import CadDocument
from arqstudiocad.events import Event, EventBus, SelectionChanged
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import Viewport

# The session's model, no Qt


@pytest.fixture
def session() -> tuple[Session, list[Event]]:
    bus = EventBus()
    events: list[Event] = []
    bus.subscribe(SelectionChanged, events.append)
    return Session(bus), events


def test_the_selection_adds_removes_and_empties(session: tuple[Session, list[Event]]) -> None:
    s, events = session
    doc = s.new_document()
    a, b, c = (doc.add_line((i, 0), (i, 1))[0] for i in range(3))
    s.select(doc, [a, b])
    s.select(doc, [b, c, c])
    assert s.selection(doc) == (a, b, c), "in the order selected, once each"
    s.deselect(doc, [b])
    assert s.selection(doc) == (a, c)
    s.clear_selection(doc)
    assert s.selection(doc) == ()
    assert events == [
        SelectionChanged(doc.id, (a, b)),
        SelectionChanged(doc.id, (a, b, c)),
        SelectionChanged(doc.id, (a, c)),
        SelectionChanged(doc.id, ()),
    ]
    s.clear_selection(doc)
    s.select(doc, [])
    s.deselect(doc, [b])
    assert len(events) == 4, "no event when nothing changes"


def test_each_document_has_its_own_selection(session: tuple[Session, list[Event]]) -> None:
    s, _ = session
    first, second = s.new_document(), s.new_document()
    line = first.add_line((0, 0), (1, 1))[0]
    s.select(first, [line])
    assert s.selection(second) == ()
    s.close(first)
    assert s.selection(first) == (), "a closed document keeps no selection"


def test_what_is_deleted_or_undone_leaves_the_selection(
    session: tuple[Session, list[Event]],
) -> None:
    s, events = session
    doc = s.new_document()
    kept = s.edit(doc, lambda d: d.add_line((0, 0), (1, 0))[1]).created[0]
    gone = s.edit(doc, lambda d: d.add_line((0, 1), (1, 1))[1]).created[0]
    s.select(doc, [kept, gone])
    s.report(doc, doc.undo())  # undoes the second line: it no longer exists
    assert s.selection(doc) == (kept,)
    assert events[-1] == SelectionChanged(doc.id, (kept,))
    s.edit(doc, lambda d: d.delete_entities([kept]))
    assert s.selection(doc) == ()


# The viewport, in the main window


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    ui.view_for = w.viewport_for
    qtbot.addWidget(w)
    w.resize(900, 600)
    w.show()
    qtbot.waitExposed(w)
    return w


def plan(window: MainWindow, tmp_path: Path) -> tuple[CadDocument, Viewport, list[int]]:
    """A line (0,0)-(100,0), a line (0,50)-(100,50) and a circle at (50,25),
    radius 10, shown whole."""
    codes = ["0", "SECTION", "2", "ENTITIES"]
    for y in (0, 50):
        codes += ["0", "LINE", "8", "0", "10", "0", "20", str(y), "30", "0"]
        codes += ["11", "100", "21", str(y), "31", "0"]
    codes += ["0", "CIRCLE", "8", "0", "10", "50", "20", "25", "30", "0", "40", "10"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "plano.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    document = window.session.open_document(path)
    viewport = window.viewports[document.id]
    QApplication.processEvents()
    ids = [e.id for e in document.get_entities()]
    return document, viewport, ids


def mouse(
    viewport: Viewport,
    kind: QEvent.Type,
    world: tuple[float, float],
    held: Qt.MouseButton = Qt.MouseButton.LeftButton,
    modifiers: Qt.KeyboardModifier = Qt.KeyboardModifier.NoModifier,
) -> None:
    """A mouse event at a world point (the test screens have ratio 1)."""
    point = QPointF(*viewport.view.to_screen(*world))
    button = Qt.MouseButton.NoButton if kind == QEvent.Type.MouseMove else Qt.MouseButton.LeftButton
    event = QMouseEvent(kind, point, viewport.mapToGlobal(point), button, held, modifiers)
    QApplication.sendEvent(viewport, event)


def click(viewport: Viewport, world: tuple[float, float], shift: bool = False) -> None:
    modifiers = Qt.KeyboardModifier.ShiftModifier if shift else Qt.KeyboardModifier.NoModifier
    mouse(viewport, QEvent.Type.MouseButtonPress, world, modifiers=modifiers)
    mouse(
        viewport,
        QEvent.Type.MouseButtonRelease,
        world,
        held=Qt.MouseButton.NoButton,
        modifiers=modifiers,
    )


def drag(viewport: Viewport, start: tuple[float, float], end: tuple[float, float]) -> None:
    mouse(viewport, QEvent.Type.MouseButtonPress, start)
    mouse(viewport, QEvent.Type.MouseMove, end)
    mouse(viewport, QEvent.Type.MouseButtonRelease, end, held=Qt.MouseButton.NoButton)


def test_a_click_adds_and_shift_click_removes(window: MainWindow, tmp_path: Path) -> None:
    document, viewport, (bottom, _, circle) = plan(window, tmp_path)
    click(viewport, (30, 0))
    click(viewport, (60, 25))  # on the circle
    assert window.session.selection(document) == (bottom, circle)
    assert viewport.selected == (bottom, circle), "the session tells the viewport"
    click(viewport, (30, 0), shift=True)
    assert window.session.selection(document) == (circle,)
    click(viewport, (50, 25))  # inside the circle, on nothing
    assert window.session.selection(document) == (circle,)


def test_left_to_right_is_a_window_right_to_left_a_crossing(
    window: MainWindow, tmp_path: Path
) -> None:
    document, viewport, (bottom, _, circle) = plan(window, tmp_path)
    drag(viewport, (35, 10), (70, 40))  # the circle wholly inside, the lines not
    assert window.session.selection(document) == (circle,)
    window.session.clear_selection(document)
    drag(viewport, (70, 40), (35, -5))  # touches the bottom line and the circle
    assert window.session.selection(document) == (bottom, circle)


def test_esc_empties_the_selection(window: MainWindow, tmp_path: Path) -> None:
    document, viewport, (bottom, top, _) = plan(window, tmp_path)
    window.session.select(document, [bottom, top])
    escape = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_Escape, Qt.KeyboardModifier.NoModifier)
    QApplication.sendEvent(viewport, escape)
    assert window.session.selection(document) == ()
    window.session.select(document, [bottom])
    QApplication.sendEvent(window.command_window.input, escape)
    assert window.session.selection(document) == (), "from the command line too"


def test_the_cursor_highlights_what_it_rests_on(window: MainWindow, tmp_path: Path) -> None:
    _, viewport, (bottom, _, circle) = plan(window, tmp_path)
    mouse(viewport, QEvent.Type.MouseMove, (30, 0), held=Qt.MouseButton.NoButton)
    assert viewport.hovered == bottom
    mouse(viewport, QEvent.Type.MouseMove, (50, 15), held=Qt.MouseButton.NoButton)
    assert viewport.hovered == circle
    mouse(viewport, QEvent.Type.MouseMove, (50, 25), held=Qt.MouseButton.NoButton)
    assert viewport.hovered is None


def test_selected_and_highlighted_entities_are_painted_over_the_image(
    window: MainWindow, tmp_path: Path
) -> None:
    document, viewport, (bottom, _, _) = plan(window, tmp_path)
    sx, sy = viewport.view.to_screen(30, 0)
    before = viewport.grab().toImage().pixelColor(round(sx), round(sy))
    window.session.select(document, [bottom])
    after = viewport.grab().toImage().pixelColor(round(sx), round(sy))
    assert (after.red(), after.green(), after.blue()) != (
        before.red(),
        before.green(),
        before.blue(),
    )
    assert after.blue() > after.red(), "drawn blue"
    window.session.clear_selection(document)
    cleared = viewport.grab().toImage().pixelColor(round(sx), round(sy))
    assert cleared == before, "the overlay goes, the drawing stays"
