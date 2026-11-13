"""OFFSET (milestone 02.14, ADR 0035 D4): AutoCAD's offset of lines,
circles and arcs, by a distance or through a point; Multiple, Undo, Erase
and Layer; one object picked at a time, in the viewport or by a typed
point; its name in Spanish and its place in the interface. Every expected
shape is worked out by hand."""

from pathlib import Path

import pytest
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadArc, CadCircle, CadDocument, CadLine
from arqstudiocad.events import CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow

DISTANCE = "Specify offset distance [Through(T)/Erase(E)/Layer(L)]"
SELECT = "Select object to offset [Exit(E)/Undo(U)] <E>:"
SIDE = "Specify point on side to offset [Exit(E)/Multiple(M)/Undo(U)]:"


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

    def shapes(self) -> list[CadLine | CadCircle | CadArc]:
        found = []
        for e in self.document.get_entities():
            g = self.document.get_entity(e.id).geometry
            assert g is not None
            found.append(g)
        return found

    def lines(self) -> list[tuple[tuple[float, float], tuple[float, float]]]:
        r = round
        return sorted(
            ((r(g.start[0], 9), r(g.start[1], 9)), (r(g.end[0], 9), r(g.end[1], 9)))
            for g in self.shapes()
            if isinstance(g, CadLine)
        )


def test_a_line_is_offset_to_the_side_shown_as_one_undo_step() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("OFFSET")
    assert h.messages()[-1] == "Current settings: Erase source=No  Layer=Source"
    assert h.prompt().text == f"{DISTANCE} <T>:", "Through until a distance is given"
    assert not h.prompt().point and not h.prompt().entity
    h.run("2")
    assert h.prompt().text == SELECT and h.prompt().entity
    h.runner.answer_entity(line, (5.0, 0.0))
    assert h.prompt().text == SIDE and h.prompt().point
    assert h.session.selection(h.document) == (line,), "the object is highlighted"
    h.run("5,3")
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0)), ((0.0, 2.0), (10.0, 2.0))]
    assert h.document.undo_history == ["LINE", "OFFSET"]
    assert h.session.selection(h.document) == ()
    assert h.prompt().text == SELECT, "the next object"
    h.run("")
    assert h.runner.active is None
    # The distance is remembered.
    h.run("OFFSET")
    assert h.prompt().text == f"{DISTANCE} <2>:"


def test_a_typed_point_on_the_object_picks_it() -> None:
    h = Harness()
    h.line((0.0, 0.0), (10.0, 0.0))
    h.run("OFFSET", "1", "50,50")
    assert h.messages()[-1] == "No object there."
    assert h.prompt().text == SELECT
    h.run("4,0", "4,-1")
    assert ((0.0, -1.0), (10.0, -1.0)) in h.lines()


def test_circles_and_arcs_grow_outside_and_shrink_inside() -> None:
    h = Harness()
    circle, _ = h.document.add_circle((0.0, 0.0), 3.0)
    arc, _ = h.document.add_geometry(CadArc((20.0, 0.0), 2.0, 0.0, 1.5))
    h.run("OFFSET", "1")
    h.runner.answer_entity(circle, (3.0, 0.0))
    h.run("10,0")
    h.runner.answer_entity(circle, (3.0, 0.0))
    h.run("0.5,0")
    h.runner.answer_entity(arc, (22.0, 0.0))
    h.run("20,0.5")
    radii = sorted(g.radius for g in h.shapes() if isinstance(g, CadCircle))
    assert radii == [2.0, 3.0, 4.0]
    arcs = [g for g in h.shapes() if isinstance(g, CadArc)]
    assert sorted(a.radius for a in arcs) == [1.0, 2.0]
    narrow = next(a for a in arcs if a.radius == 1.0)
    assert (narrow.start_angle, narrow.end_angle) == (0.0, 1.5), "between the same angles"
    # Inward by more than the radius: nothing left, and nothing made.
    h.run("")
    h.run("OFFSET", "5")
    h.runner.answer_entity(circle, (3.0, 0.0))
    h.run("0.5,0")
    assert h.messages()[-1] == "Cannot offset that object to that side."
    assert h.prompt().text == SIDE
    h.run("E")
    assert h.document.undo_history == ["CIRCLE", "ARC", "OFFSET", "OFFSET", "OFFSET"]


def test_through_passes_the_copy_through_the_point() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("OFFSET", "T")
    h.runner.answer_entity(line, (5.0, 0.0))
    assert h.prompt().text == "Specify through point [Exit(E)/Multiple(M)/Undo(U)]:"
    h.run("30,-4")
    assert ((0.0, -4.0), (10.0, -4.0)) in h.lines()
    h.run("", "OFFSET")
    assert h.prompt().text == f"{DISTANCE} <T>:", "Through is remembered"


def test_multiple_offsets_each_new_copy_again() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("OFFSET", "1")
    h.runner.answer_entity(line, (5.0, 0.0))
    h.run("M")
    assert h.prompt().text == "Specify point on side to offset [Exit(E)/Undo(U)]:"
    h.run("0,5", "0,5", "0,5")
    ys = sorted(a[1] for a, _ in h.lines())
    assert ys == [0.0, 1.0, 2.0, 3.0], "each from the last copy"
    h.run("U")
    assert sorted(a[1] for a, _ in h.lines()) == [0.0, 1.0, 2.0]
    h.run("")
    assert h.prompt().text == SELECT, "Enter in Multiple: the next object"
    h.run("U", "U", "U")
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))]
    assert h.messages()[-1] == "Nothing to undo."
    assert h.document.undo_history == ["LINE"], "the command's own offsets only"


def test_erase_takes_the_source_away_in_the_same_step() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("OFFSET", "E")
    assert h.prompt().text == "Erase source object after offsetting? [Yes(Y)/No(N)] <N>:"
    h.run("Y")
    assert h.prompt().text == f"{DISTANCE} <T>:"
    h.run("2")
    h.runner.answer_entity(line, (5.0, 0.0))
    h.run("5,5", "")
    assert h.lines() == [((0.0, 2.0), (10.0, 2.0))]
    assert h.document.undo_history == ["LINE", "OFFSET"], "one step: the copy and the erase"
    h.document.undo()
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))]
    h.run("OFFSET")
    assert h.messages()[-1] == "Current settings: Erase source=Yes  Layer=Source"


def test_layer_current_puts_the_copy_on_the_current_layer() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    walls, _ = h.document.create_layer("WALLS")
    h.document.set_current_layer(walls)
    h.run("OFFSET", "L")
    assert h.prompt().text == ("Enter layer option for offset objects [Current(C)/Source(S)] <S>:")
    h.run("C", "1")
    h.runner.answer_entity(line, (5.0, 0.0))
    h.run("5,5", "")
    layers = sorted(e.layer for e in h.document.get_entities())
    assert layers == ["0", "WALLS"]
    # Source: the copy keeps the source's layer.
    h.run("OFFSET", "L", "S", "1")
    h.runner.answer_entity(line, (5.0, 0.0))
    h.run("5,-5", "")
    assert sorted(e.layer for e in h.document.get_entities()) == ["0", "0", "WALLS"]


def test_what_is_not_a_line_circle_or_arc_cannot_be_offset_yet() -> None:
    h = Harness()
    square = [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0), ((4.0, 4.0), 0.0)]
    polyline, _ = h.document.add_polyline(square, True)
    h.run("OFFSET", "1")
    h.runner.answer_entity(polyline, (2.0, 0.0))
    assert h.messages()[-1] == "Cannot offset that object."
    assert h.prompt().text == SELECT


def test_an_entity_answer_is_ignored_at_other_prompts() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("OFFSET")
    h.runner.answer_entity(line, (5.0, 0.0))
    assert h.prompt().text == f"{DISTANCE} <T>:"


def test_offset_in_spanish() -> None:
    h = Harness()
    set_language("es")
    for typed in ("EQDIST", "O", "OFFSET"):
        found = h.runner.registry.find(typed)
        assert found is not None and found.name == "OFFSET"


def test_offset_is_in_the_modify_group_menu_and_toolbar(qtbot: QtBot) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    action = window.command_actions["OFFSET"]
    assert action.text() == "&Offset"
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    assert action in [b.defaultAction() for b in groups["modify"].buttons]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))


def test_a_click_in_the_viewport_picks_the_object(qtbot: QtBot, tmp_path: Path) -> None:
    from PySide6.QtCore import QEvent
    from test_point_input import mouse, plan

    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)  # a line (0,0)-(100,0), a circle at (50,40)
    window.runner.execute("OFFSET")
    window.runner.execute("5")
    # Nowhere near an object: still asking.
    mouse(viewport, QEvent.Type.MouseButtonPress, (30.0, 20.0))
    assert window.runner.prompt is not None
    assert window.runner.prompt.shown() == SELECT
    mouse(viewport, QEvent.Type.MouseButtonPress, (50.0, 0.0))
    assert window.runner.prompt is not None
    assert window.runner.prompt.shown() == SIDE, "the line was picked"
    mouse(viewport, QEvent.Type.MouseButtonPress, (50.0, -20.0))
    document = window.session.active
    assert document is not None
    ends = sorted(
        (g.start[1], g.end[1])
        for g in (document.get_entity(e.id).geometry for e in document.get_entities())
        if isinstance(g, CadLine)
    )
    assert ends == [(-5.0, -5.0), (0.0, 0.0)]
    window.runner.cancel()


@pytest.mark.parametrize("typed", ["0", "-1", "abc"])
def test_the_distance_is_a_number_greater_than_zero(typed: str) -> None:
    h = Harness()
    h.run("OFFSET", typed)
    assert h.messages()[-1] == "Type a number greater than zero."
    assert h.prompt().text == f"{DISTANCE} <T>:"


def test_an_offset_the_core_refuses_leaves_nothing_behind() -> None:
    h = Harness()
    locked, _ = h.document.create_layer("LOCKED")
    h.document.set_current_layer(locked)
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.document.set_layer_locked(locked, True)
    history = list(h.document.undo_history)
    h.run("OFFSET", "1")
    h.runner.answer_entity(line, (5.0, 0.0))
    h.run("5,5")
    assert h.messages()[-1].startswith("Error: "), h.messages()[-1]
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))], "no copy left half made"
    assert h.document.undo_history == history
    assert h.prompt().text == SIDE, "the side is asked again"
