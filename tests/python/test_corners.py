"""FILLET and CHAMFER (milestone 02.14, ADR 0035 D4): AutoCAD's prompts
and options, Trim mode shared, Shift for a sharp corner, Multiple and
Undo; lines, and a line and an arc; their names in Spanish and their place
in the interface. Every expected shape is worked out by hand."""

import math

import pytest
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.core import CadArc, CadDocument, CadLine
from arqstudiocad.events import CommandMessage, CommandPrompt, Event, EventBus
from arqstudiocad.i18n import set_language
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow

FIRST = "Select first object [Undo(U)/Radius(R)/Trim(T)/Multiple(M)]:"
SECOND = "Select second object or shift-select to apply corner [Radius(R)]:"
FIRST_LINE = "Select first line [Undo(U)/Distance(D)/Angle(A)/Trim(T)/Method(E)/Multiple(M)]:"
SECOND_LINE = "Select second line or shift-select to apply corner [Distance(D)/Angle(A)/Method(E)]:"

type Ends = tuple[tuple[float, float], tuple[float, float]]


class Harness:
    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session))
        self.document: CadDocument = self.session.new_document()
        # Two lines crossing at the origin.
        self.a, _ = self.document.add_line((-10.0, 0.0), (10.0, 0.0))
        self.b, _ = self.document.add_line((0.0, -10.0), (0.0, 10.0))

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def prompt(self) -> CommandPrompt:
        return [e for e in self.events if isinstance(e, CommandPrompt)][-1]

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def ends(self, entity_id: int) -> Ends:
        g = self.document.get_entity(entity_id).geometry
        assert isinstance(g, CadLine)
        r = round
        return ((r(g.start[0], 9), r(g.start[1], 9)), (r(g.end[0], 9), r(g.end[1], 9)))

    def arcs(self) -> list[CadArc]:
        found = (self.document.get_entity(e.id).geometry for e in self.document.get_entities())
        return [g for g in found if isinstance(g, CadArc)]

    def corner(self) -> None:
        """Picks the two lines on their +x and +y sides."""
        self.runner.answer_entity(self.a, (5.0, 0.0))
        self.runner.answer_entity(self.b, (0.0, 5.0))


def test_fillet_rounds_the_corner_with_the_radius_set() -> None:
    h = Harness()
    h.run("FILLET")
    assert h.messages()[-1] == "Current settings: Mode=Trim, Radius=0"
    assert h.prompt().text == FIRST and h.prompt().entity
    h.run("R")
    assert h.prompt().text == "Specify fillet radius <0>:"
    h.run("2")
    assert h.prompt().text == FIRST
    h.runner.answer_entity(h.a, (5.0, 0.0))
    assert h.prompt().text == SECOND
    assert h.session.selection(h.document) == (h.a,), "the first is highlighted"
    h.runner.answer_entity(h.b, (0.0, 5.0))
    assert h.runner.active is None, "one corner, then done"
    assert h.ends(h.a) == ((2.0, 0.0), (10.0, 0.0))
    assert h.ends(h.b) == ((0.0, 2.0), (0.0, 10.0))
    [arc] = h.arcs()
    assert arc.center == pytest.approx((2.0, 2.0)) and arc.radius == pytest.approx(2.0)
    assert h.document.undo_history[-1] == "FILLET"
    h.run("FILLET")
    assert h.messages()[-1] == "Current settings: Mode=Trim, Radius=2", "remembered"


def test_shift_on_the_second_makes_a_sharp_corner() -> None:
    h = Harness()
    h.run("FILLET", "R", "2")
    h.runner.answer_entity(h.a, (5.0, 0.0))
    h.runner.answer_entity(h.b, (0.0, 5.0), shift=True)
    assert h.ends(h.a) == ((0.0, 0.0), (10.0, 0.0))
    assert h.arcs() == []


def test_no_trim_leaves_the_lines_and_is_shared_with_chamfer() -> None:
    h = Harness()
    h.run("FILLET", "T")
    assert h.prompt().text == "Enter Trim mode option [Trim(T)/No trim(N)] <T>:"
    h.run("N", "R", "2")
    h.corner()
    assert h.ends(h.a) == ((-10.0, 0.0), (10.0, 0.0)) and len(h.arcs()) == 1
    h.run("CHAMFER")
    assert h.messages()[-1].startswith("Current settings: Mode=No trim")


def test_multiple_and_undo() -> None:
    h = Harness()
    c, _ = h.document.add_line((20.0, 0.0), (30.0, 0.0))
    d, _ = h.document.add_line((25.0, -5.0), (25.0, 5.0))
    h.run("FILLET", "R", "1", "M")
    h.corner()
    assert h.prompt().text == FIRST, "Multiple: the next pair"
    h.runner.answer_entity(c, (28.0, 0.0))
    h.runner.answer_entity(d, (25.0, 3.0))
    assert len(h.arcs()) == 2
    h.run("U")
    assert len(h.arcs()) == 1
    h.run("U", "U")
    assert h.arcs() == [] and h.messages()[-1] == "Nothing to undo."
    h.run("")
    assert h.runner.active is None


def test_a_fillet_too_large_or_of_the_same_object_is_refused() -> None:
    h = Harness()
    h.run("FILLET", "R", "50")
    h.corner()
    assert h.messages()[-1] == "Cannot fillet those objects with that radius."
    assert h.ends(h.a) == ((-10.0, 0.0), (10.0, 0.0))
    h.run("FILLET")
    h.runner.answer_entity(h.a, (5.0, 0.0))
    h.runner.answer_entity(h.a, (6.0, 0.0))
    assert h.messages()[-1] == "Select another object."
    assert h.prompt().text == SECOND


def test_a_line_and_an_arc_are_filleted() -> None:
    h = Harness()
    line, _ = h.document.add_line((-10.0, 20.0), (10.0, 20.0))
    arc, _ = h.document.add_geometry(CadArc((0.0, 25.0), 3.0, math.pi, 2.0 * math.pi))
    h.run("FILLET", "R", "2")
    h.runner.answer_entity(line, (5.0, 20.0))
    h.runner.answer_entity(arc, (2.6, 23.5))
    fillets = [a for a in h.arcs() if a.radius == pytest.approx(2.0)]
    assert len(fillets) == 1 and fillets[0].center == pytest.approx((4.0, 22.0))


def test_chamfer_by_distances_and_by_angle() -> None:
    h = Harness()
    h.run("CHAMFER")
    assert h.messages()[-1] == "Current settings: Mode=Trim, Distance 1=0, Distance 2=0"
    assert h.prompt().text == FIRST_LINE
    h.run("D")
    assert h.prompt().text == "Specify first chamfer distance <0>:"
    h.run("2")
    assert h.prompt().text == "Specify second chamfer distance <2>:", "the first by default"
    h.run("3")
    h.runner.answer_entity(h.a, (5.0, 0.0))
    assert h.prompt().text == SECOND_LINE
    h.runner.answer_entity(h.b, (0.0, 5.0))
    assert h.ends(h.a) == ((2.0, 0.0), (10.0, 0.0))
    assert h.ends(h.b) == ((0.0, 3.0), (0.0, 10.0))
    assert h.document.undo_history[-1] == "CHAMFER"
    # By length and angle: 2 along the first, at 60°: 2√3 along the second.
    h2 = Harness()
    h2.run("CHAMFER", "A")
    assert h2.prompt().text == "Specify chamfer length on the first line <0>:"
    h2.run("2", "60")
    h2.corner()
    assert h2.ends(h2.b)[0] == (0.0, round(2.0 * math.sqrt(3.0), 9))
    h2.run("CHAMFER")
    assert h2.messages()[-1] == "Current settings: Mode=Trim, Length=2, Angle=60"


def test_chamfer_method_switches_back_and_lines_only() -> None:
    h = Harness()
    h.run("CHAMFER", "A", "2", "60", "CHAMFER", "E")
    assert h.prompt().text == "Enter trim method [Distance(D)/Angle(A)] <A>:"
    h.run("D")
    circle, _ = h.document.add_circle((30.0, 0.0), 1.0)
    h.runner.answer_entity(circle, (31.0, 0.0))
    assert h.messages()[-1] == "Cannot chamfer that object."


def test_fillet_and_chamfer_in_spanish() -> None:
    h = Harness()
    set_language("es")
    for typed, name in (("EMPALME", "FILLET"), ("CHAFLAN", "CHAMFER"), ("F", "FILLET")):
        found = h.runner.registry.find(typed)
        assert found is not None and found.name == name


def test_fillet_and_chamfer_are_in_the_modify_group_menu_and_toolbar(qtbot: QtBot) -> None:
    from PySide6.QtWidgets import QMenu, QToolBar

    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    for name, text in (("FILLET", "&Fillet"), ("CHAMFER", "C&hamfer")):
        action = window.command_actions[name]
        assert action.text() == text
        assert action in [b.defaultAction() for b in groups["modify"].buttons]
        assert action in menus["&Modify"].actions()
        assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
