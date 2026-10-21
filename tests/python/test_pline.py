"""PLINE (milestone 02.8): straight segments and tangent arcs, Undo and
Close, one undo step; its name in Spanish and its place in the interface.
Every expected shape is worked out by hand."""

from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadDocument
from arqstudiocad.events import CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow


class Harness:
    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session))
        self.document: CadDocument = self.session.new_document()

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def prompt(self) -> CommandPrompt:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1]

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def polylines(self) -> list[int]:
        return [e.id for e in self.document.get_entities() if e.kind == "LWPOLYLINE"]

    def vertices(self) -> list[tuple[float, float]]:
        [only] = self.polylines()
        grips = self.document.grips([only])
        return [g.point for g in sorted(grips, key=lambda g: g.vertex_index or 0)]

    def box(self) -> tuple[tuple[float, float], tuple[float, float]]:
        [only] = self.polylines()
        b = self.document.get_entity(only).bounds
        assert b is not None
        r = round
        return (r(b.min[0], 6), r(b.min[1], 6)), (r(b.max[0], 6), r(b.max[1], 6))


def test_pline_draws_one_polyline_as_one_undo_step() -> None:
    h = Harness()
    h.run("PLINE")
    assert h.prompt().text == "Specify start point:"
    h.run("0,0")
    assert h.prompt().text == "Specify next point [Arc(A)/Undo(U)]:"
    h.run("10,0")
    assert h.prompt().text == "Specify next point [Arc(A)/Undo(U)]:", "Close needs two segments"
    h.run("A")
    assert h.prompt().text == "Specify end point of arc [Line(L)/Undo(U)/Close(C)]:", (
        "an arc closes after one segment"
    )
    h.run("L", "@0,5")
    assert h.prompt().base == (10.0, 5.0), "the rubber band starts at the last point"
    assert h.prompt().text == "Specify next point [Arc(A)/Undo(U)/Close(C)]:"
    h.run("")
    assert h.runner.active is None
    assert h.vertices() == [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0)]
    assert h.document.undo_history == ["PLINE"], "the whole run is one undo step"


def test_arcs_are_tangent_to_the_segment_before() -> None:
    # (0,0)-(10,0) East, then an arc to (10,10): it leaves East, so a half
    # circle around (10,5) bulging to x = 15; then back to (0,10) and
    # closed: the box (0,0)-(15,10).
    h = Harness()
    h.run("PL", "0,0", "10,0", "A")
    assert h.prompt().text == "Specify end point of arc [Line(L)/Undo(U)/Close(C)]:"
    h.run("10,10", "L", "0,10", "C")
    assert h.runner.active is None
    assert h.box() == ((0.0, 0.0), (15.0, 10.0))
    assert h.document.pick((0.0, 5.0), 0.1) == h.polylines()[0], "closed back to the start"
    # The first segment an arc: it leaves East. To (0,4), a half circle
    # around (0,2) bulging to x = 2.
    h = Harness()
    h.run("PLINE", "0,0", "A", "0,4", "")
    assert h.box() == ((0.0, 0.0), (2.0, 4.0))
    # An arc after an arc keeps tangent: from (0,4) the first arc ends
    # heading West; another half circle to (0,8) bulges to x = -2.
    h = Harness()
    h.run("PLINE", "0,0", "A", "0,4", "0,8", "")
    assert h.box() == ((-2.0, 0.0), (2.0, 8.0))


def test_undo_takes_back_the_last_segment_and_esc_keeps_the_rest() -> None:
    h = Harness()
    h.run("PLINE", "0,0", "10,0", "10,10", "U")
    assert h.vertices() == [(0.0, 0.0), (10.0, 0.0)]
    assert h.prompt().base == (10.0, 0.0)
    h.run("A", "10,10", "U", "U")
    assert h.polylines() == [], "back to the start point alone"
    h.run("U")
    assert h.messages()[-1] == "All segments already undone."
    h.run("5,5", "5,9")
    h.runner.cancel()
    assert h.vertices() == [(0.0, 0.0), (5.0, 5.0), (5.0, 9.0)], "Esc keeps what was drawn"
    assert len(h.document.undo_history) == 1


def test_points_no_segment_can_join_are_refused() -> None:
    h = Harness()
    h.run("PLINE", "0,0", "0,0")
    assert h.messages()[-1] == "The two points must differ."
    h.run("10,0", "A", "5,0")
    assert h.messages()[-1] == "No arc tangent to the last segment reaches that point."
    h.run("10,0")
    assert h.messages()[-1] == "The two points must differ."
    h.run("")
    assert h.vertices() == [(0.0, 0.0), (10.0, 0.0)]


def test_pline_in_spanish() -> None:
    h = Harness()
    set_language("es")
    found = h.runner.registry.find("POLILINEA")
    assert found is not None and found.name == "PLINE"


def test_pline_is_in_the_draw_group_menu_and_toolbar(qtbot: QtBot) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    pline = window.command_actions["PLINE"]
    assert pline.text() == "&Polyline"
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    draw = [b.defaultAction() for b in groups["draw"].buttons]
    assert draw[:2] == [window.command_actions["LINE"], pline], "right after Line"
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert pline in menus["&Draw"].actions()
    assert any(pline in bar.actions() for bar in window.findChildren(QToolBar))
