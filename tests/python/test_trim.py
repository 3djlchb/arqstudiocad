"""TRIM and EXTEND (milestone 02.14, ADR 0035 D4): AutoCAD's Quick and
Standard modes, Shift for the other command, Undo and Erase, lines, arcs
and circles; their names in Spanish and their place in the interface.
Every expected shape is worked out by hand."""

from pathlib import Path

from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadArc, CadDocument, CadLine
from arqstudiocad.events import CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow

TRIM_PROMPT = (
    "Select object to trim or shift-select to extend "
    "[Cutting edges(T)/Fence(F)/Crossing(C)/Mode(O)/Erase(R)/Undo(U)]:"
)
EXTEND_PROMPT = (
    "Select object to extend or shift-select to trim "
    "[Boundary edges(B)/Fence(F)/Crossing(C)/Mode(O)/Undo(U)]:"
)


type Ends = tuple[tuple[float, float], tuple[float, float]]


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

    def line(self, a: tuple[float, float], b: tuple[float, float]) -> int:
        entity_id, _ = self.document.add_line(a, b)
        return entity_id

    def lines(self) -> list[Ends]:
        r = round
        found = []
        for e in self.document.get_entities():
            g = self.document.get_entity(e.id).geometry
            if isinstance(g, CadLine):
                found.append(
                    ((r(g.start[0], 9), r(g.start[1], 9)), (r(g.end[0], 9), r(g.end[1], 9)))
                )
        return sorted(found)


def crossed() -> tuple[Harness, int, int, int]:
    """A line along x from 0 to 10, crossed by uprights at x = 3 and 7."""
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    a = h.line((3.0, -1.0), (3.0, 1.0))
    b = h.line((7.0, -1.0), (7.0, 1.0))
    return h, line, a, b


UPRIGHTS = [((3.0, -1.0), (3.0, 1.0)), ((7.0, -1.0), (7.0, 1.0))]


def test_quick_trim_takes_away_the_part_between_the_nearest_edges() -> None:
    h, line, _, _ = crossed()
    h.run("TRIM")
    assert h.messages()[-1] == "Current settings: Mode=Quick"
    assert h.prompt().text == TRIM_PROMPT and h.prompt().entity
    h.runner.answer_entity(line, (5.0, 0.0))
    assert h.lines() == sorted([((0.0, 0.0), (3.0, 0.0)), ((7.0, 0.0), (10.0, 0.0)), *UPRIGHTS])
    assert h.document.undo_history[-1] == "TRIM"
    assert h.prompt().text == TRIM_PROMPT, "the next object"
    h.runner.answer_entity(line, (1.0, 0.0))
    assert ((0.0, 0.0), (3.0, 0.0)) not in h.lines(), "nothing bounds it on the left: erased"
    h.run("")
    assert h.runner.active is None


def test_quick_trim_erases_an_object_no_edge_bounds() -> None:
    h = Harness()
    alone = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("TR")
    h.runner.answer_entity(alone, (5.0, 0.0))
    assert h.lines() == []


def test_extend_reaches_the_nearest_boundary_and_shift_trims() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (5.0, 0.0))
    h.line((8.0, -1.0), (8.0, 1.0))
    h.run("EXTEND")
    assert h.prompt().text == EXTEND_PROMPT
    h.runner.answer_entity(line, (4.0, 0.0))
    assert ((0.0, 0.0), (8.0, 0.0)) in h.lines()
    assert h.document.undo_history[-1] == "EXTEND"
    # Shift-select trims instead: from 8 back, the end piece beyond nothing.
    h.runner.answer_entity(line, (1.0, 0.0), shift=True)
    assert ((0.0, 0.0), (8.0, 0.0)) not in h.lines()
    h.run("")


def test_shift_select_in_trim_extends() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (5.0, 0.0))
    h.line((8.0, -1.0), (8.0, 1.0))
    h.run("TRIM")
    h.runner.answer_entity(line, (4.0, 0.0), shift=True)
    assert ((0.0, 0.0), (8.0, 0.0)) in h.lines()


def test_standard_mode_cuts_at_the_edges_chosen_and_is_remembered() -> None:
    h, line, a, _ = crossed()
    h.run("TRIM", "O")
    assert h.prompt().text == "Enter a mode option [Quick(Q)/Standard(S)] <Q>:"
    h.run("S")
    assert h.messages()[-1] == "Select cutting edges..."
    assert h.prompt().text == "Select objects [All/Last(L)/Previous(P)]:"
    h.session.select(h.document, [a])
    h.run("")
    assert h.prompt().text == TRIM_PROMPT
    h.runner.answer_entity(line, (5.0, 0.0))
    assert ((0.0, 0.0), (3.0, 0.0)) in h.lines(), "cut at the edge chosen only"
    # With the edge at 3 alone, the part left of it has no edge: refused.
    h.runner.answer_entity(line, (1.0, 0.0))
    assert h.messages()[-1] == "No cutting edge crosses that part of the object."
    h.run("")
    h.run("TRIM")
    assert h.messages()[-2:] == ["Current settings: Mode=Standard", "Select cutting edges..."]
    h.run("")
    assert h.messages()[-1] == "Every object is an edge."


def test_cutting_edges_switch_to_standard_mode() -> None:
    h, line, _, b = crossed()
    h.run("TRIM", "T")
    h.session.select(h.document, [b])
    h.run("")
    h.runner.answer_entity(line, (5.0, 0.0))
    assert ((7.0, 0.0), (10.0, 0.0)) in h.lines(), "from its start up to the edge at 7"
    assert ((0.0, 0.0), (10.0, 0.0)) not in h.lines()
    h.run("", "EXTEND")
    assert h.messages()[-2] == "Current settings: Mode=Standard", "shared with EXTEND"


def test_undo_takes_back_the_last_change_of_the_run() -> None:
    h, line, _, _ = crossed()
    h.run("TRIM")
    h.runner.answer_entity(line, (5.0, 0.0))
    h.run("U")
    assert h.lines() == sorted([((0.0, 0.0), (10.0, 0.0)), *UPRIGHTS])
    h.run("U")
    assert h.messages()[-1] == "Nothing to undo."
    assert h.document.undo_history == ["LINE", "LINE", "LINE"]


def test_erase_removes_objects_without_leaving_trim() -> None:
    h, _, a, _ = crossed()
    h.run("TRIM", "R")
    h.session.select(h.document, [a])
    h.run("")
    assert ((3.0, -1.0), (3.0, 1.0)) not in h.lines()
    assert h.prompt().text == TRIM_PROMPT
    h.run("U")
    assert ((3.0, -1.0), (3.0, 1.0)) in h.lines()


def test_a_circle_cut_twice_becomes_an_arc() -> None:
    h = Harness()
    circle, _ = h.document.add_circle((0.0, 0.0), 5.0)
    h.line((-10.0, 0.0), (10.0, 0.0))
    h.run("TRIM")
    h.runner.answer_entity(circle, (0.0, 5.0))
    arcs = [
        g
        for g in (h.document.get_entity(e.id).geometry for e in h.document.get_entities())
        if isinstance(g, CadArc)
    ]
    assert len(arcs) == 1 and abs(arcs[0].start_angle - 3.141592653589793) < 1e-12


def test_what_cannot_be_trimmed_or_extended_is_said() -> None:
    h = Harness()
    square = [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0), ((4.0, 4.0), 0.0)]
    closed, _ = h.document.add_polyline(square, True)
    circle, _ = h.document.add_circle((20.0, 0.0), 1.0)
    alone = h.line((30.0, 0.0), (35.0, 0.0))
    text, _ = h.document.add_text("A", (50.0, 0.0), 2.5, 0.0)
    h.run("TRIM")
    h.runner.answer_entity(text, (50.5, 0.5))
    assert h.messages()[-1] == "Cannot trim that object."
    h.run("", "EXTEND")
    h.runner.answer_entity(circle, (21.0, 0.0))
    assert h.messages()[-1] == "Cannot extend that object."
    h.runner.answer_entity(closed, (2.0, 0.0))
    assert h.messages()[-1] == "Cannot extend that object.", "a closed polyline has no end"
    h.runner.answer_entity(alone, (34.0, 0.0))
    assert h.messages()[-1] == "Object does not intersect an edge."


def polyline_points(h: Harness, entity_id: int) -> list[tuple[float, float]]:
    grips = [g for g in h.document.grips([entity_id]) if g.vertex_index is not None]
    ordered = sorted(grips, key=lambda g: g.vertex_index or 0)
    return [(round(g.point[0], 9) + 0.0, round(g.point[1], 9) + 0.0) for g in ordered]


ELL = [((0.0, 0.0), 0.0), ((10.0, 0.0), 0.0), ((10.0, 10.0), 0.0)]


def test_a_polyline_trimmed_in_the_middle_becomes_two() -> None:
    h = Harness()
    ell, _ = h.document.add_polyline(ELL)
    h.line((3.0, -1.0), (3.0, 1.0))
    h.line((9.0, 5.0), (11.0, 5.0))
    h.run("TRIM")
    h.runner.answer_entity(ell, (5.0, 0.0))
    pieces = sorted(
        polyline_points(h, e.id) for e in h.document.get_entities() if e.kind == "LWPOLYLINE"
    )
    assert pieces == [[(0.0, 0.0), (3.0, 0.0)], [(10.0, 5.0), (10.0, 10.0)]]
    assert h.document.undo_history[-1] == "TRIM"


def test_a_closed_polyline_trimmed_opens_and_then_extends() -> None:
    h = Harness()
    box, _ = h.document.add_polyline(
        [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0), ((4.0, 2.0), 0.0), ((0.0, 2.0), 0.0)], True
    )
    h.line((2.0, -1.0), (2.0, 3.0))
    h.line((5.0, -5.0), (5.0, 5.0))
    h.run("TRIM")
    h.runner.answer_entity(box, (4.0, 1.0))
    assert polyline_points(h, box) == [(2.0, 2.0), (0.0, 2.0), (0.0, 0.0), (2.0, 0.0)]
    assert dict(h.document.get_entity(box).details)["Closed"] == "false"
    # Open now: Shift-select extends its last segment, along y = 0, to x = 5.
    h.runner.answer_entity(box, (1.5, 0.0), shift=True)
    assert polyline_points(h, box) == [(2.0, 2.0), (0.0, 2.0), (0.0, 0.0), (5.0, 0.0)]
    assert h.document.undo_history[-2:] == ["TRIM", "EXTEND"]
    h.run("")


def test_an_open_polyline_extends_at_the_end_picked() -> None:
    h = Harness()
    ell, _ = h.document.add_polyline(ELL)
    h.line((5.0, 15.0), (15.0, 15.0))
    h.run("EXTEND")
    h.runner.answer_entity(ell, (10.0, 9.0))
    assert polyline_points(h, ell) == [(0.0, 0.0), (10.0, 0.0), (10.0, 15.0)]
    assert h.document.undo_history[-1] == "EXTEND"


def test_trim_and_extend_in_spanish() -> None:
    h = Harness()
    set_language("es")
    for typed, name in (
        ("RECORTA", "TRIM"),
        ("ALARGA", "EXTEND"),
        ("TR", "TRIM"),
        ("EX", "EXTEND"),
    ):
        found = h.runner.registry.find(typed)
        assert found is not None and found.name == name


def test_trim_and_extend_are_in_the_modify_group_menu_and_toolbar(qtbot: QtBot) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    for name, text in (("TRIM", "&Trim"), ("EXTEND", "E&xtend")):
        action = window.command_actions[name]
        assert action.text() == text
        assert action in [b.defaultAction() for b in groups["modify"].buttons]
        assert action in menus["&Modify"].actions()
        assert any(action in bar.actions() for bar in window.findChildren(QToolBar))


def test_a_shift_click_in_the_viewport_is_sent_with_shift(qtbot: QtBot, tmp_path: Path) -> None:
    from PySide6.QtCore import QEvent, QPointF, Qt
    from PySide6.QtGui import QMouseEvent
    from PySide6.QtWidgets import QApplication
    from test_point_input import plan

    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)  # a line (0,0)-(100,0), a circle at (50,40)
    sent: list[object] = []
    viewport.entity_picked.connect(sent.append)
    window.runner.execute("TRIM")

    def click(world: tuple[float, float], shift: bool) -> None:
        point = QPointF(*viewport.view.to_screen(*world))
        held = Qt.KeyboardModifier.ShiftModifier if shift else Qt.KeyboardModifier.NoModifier
        event = QMouseEvent(
            QEvent.Type.MouseButtonPress,
            point,
            viewport.mapToGlobal(point),
            Qt.MouseButton.LeftButton,
            Qt.MouseButton.NoButton,
            held,
        )
        QApplication.sendEvent(viewport, event)

    click((50.0, 30.0), shift=True)  # on the circle, with Shift: extend, refused
    assert sent and sent[-1][2] is True  # type: ignore[index]
    assert window.runner.prompt is not None
    assert window.runner.prompt.shown() == TRIM_PROMPT
    click((20.0, 0.0), shift=False)  # the line, no edge bounds it: erased
    assert sent[-1][2] is False  # type: ignore[index]
    document = window.session.active
    assert document is not None
    assert [e.kind for e in document.get_entities()] == ["CIRCLE"]
    window.runner.cancel()
