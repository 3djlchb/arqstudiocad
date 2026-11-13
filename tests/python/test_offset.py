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


def test_what_cannot_be_offset_is_said() -> None:
    h = Harness()
    text, _ = h.document.add_text("A", (0.0, 0.0), 2.5, 0.0)
    h.run("OFFSET", "1")
    h.runner.answer_entity(text, (0.5, 0.5))
    assert h.messages()[-1] == "Cannot offset that object."
    assert h.prompt().text == SELECT


# Polylines: segments moved, their corners joined as AutoCAD's.

BOX = [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0), ((4.0, 2.0), 0.0), ((0.0, 2.0), 0.0)]


def vertices(h: Harness, entity_id: int) -> list[tuple[float, float]]:
    grips = [g for g in h.document.grips([entity_id]) if g.vertex_index is not None]
    ordered = sorted(grips, key=lambda g: g.vertex_index or 0)
    return [(round(g.point[0], 9) + 0.0, round(g.point[1], 9) + 0.0) for g in ordered]


def newest(h: Harness) -> int:
    return max(e.id for e in h.document.get_entities())


def test_a_closed_polyline_grows_outside_and_shrinks_inside() -> None:
    h = Harness()
    box, _ = h.document.add_polyline(BOX, True)
    h.run("OFFSET", "0.5")
    h.runner.answer_entity(box, (2.0, 0.0))
    h.run("2,-3")
    assert vertices(h, newest(h)) == [(-0.5, -0.5), (4.5, -0.5), (4.5, 2.5), (-0.5, 2.5)]
    assert dict(h.document.get_entity(newest(h)).details)["Closed"] == "true"
    h.runner.answer_entity(box, (2.0, 0.0))
    h.run("2,1")
    assert vertices(h, newest(h)) == [(0.5, 0.5), (3.5, 0.5), (3.5, 1.5), (0.5, 1.5)]
    assert h.document.undo_history[-2:] == ["OFFSET", "OFFSET"]


def test_a_polyline_offset_inward_too_far_leaves_nothing() -> None:
    h = Harness()
    box, _ = h.document.add_polyline(BOX, True)
    count = len(h.document.get_entities())
    h.run("OFFSET", "1.5")
    h.runner.answer_entity(box, (2.0, 0.0))
    h.run("2,1")
    assert h.messages()[-1] == "Cannot offset that object to that side."
    assert len(h.document.get_entities()) == count
    assert h.prompt().text == SIDE


def test_an_open_polyline_is_extended_outside_its_corner() -> None:
    h = Harness()
    ell, _ = h.document.add_polyline([((0.0, 0.0), 0.0), ((10.0, 0.0), 0.0), ((10.0, 10.0), 0.0)])
    h.run("OFFSET", "T")
    h.runner.answer_entity(ell, (5.0, 0.0))
    h.run("5,-1")
    assert vertices(h, newest(h)) == [(0.0, -1.0), (11.0, -1.0), (11.0, 10.0)]


def test_a_rectangle_keeps_its_fillets_width_and_layer() -> None:
    h = Harness()
    h.run("RECTANG", "F", "1", "W", "0.2", "0,0", "10,6")
    rect = newest(h)
    h.run("OFFSET", "0.5")
    h.runner.answer_entity(rect, (5.0, 0.0))
    h.run("5,-2", "")
    copy = h.document.get_entity(newest(h))
    shown = dict(copy.details)
    assert (shown["Vertices"], shown["Constant width"]) == ("8", "0.200000")
    assert copy.layer == h.document.get_entity(rect).layer
    b = copy.bounds
    assert b is not None
    corners = (b.min[0], b.min[1], b.max[0], b.max[1])
    assert corners == pytest.approx((-0.6, -0.6, 10.6, 6.6)), "0.5 out, and half its width"


def test_multiple_offsets_a_polyline_again_from_each_copy() -> None:
    h = Harness()
    box, _ = h.document.add_polyline(BOX, True)
    h.run("OFFSET", "1")
    h.runner.answer_entity(box, (2.0, 0.0))
    h.run("M", "2,-5", "2,-5", "")
    assert vertices(h, newest(h)) == [(-2.0, -2.0), (6.0, -2.0), (6.0, 4.0), (-2.0, 4.0)]


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
    count = len(h.events)
    h.run("5,5")
    said = h.messages()[-1]
    assert said.startswith("Error: ") and "group" not in said, said
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))], "no copy left half made"
    assert h.document.undo_history == history
    assert not h.document.in_group
    asked = [e for e in h.events[count:] if isinstance(e, CommandPrompt)]
    assert asked and asked[-1].text == SIDE, "the side is asked again"
    assert h.runner.active == "OFFSET", "the command goes on"
