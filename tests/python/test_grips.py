"""Multifunction grips (ADR 0016 D7): which grips show, the grip edit's
modes and options, and grips in the viewport."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.commands import CommandContext, CommandRunner
from arqstudiocad.commands.builtin import builtin_registry
from arqstudiocad.commands.grips import GRIP_LIMIT, grip_edit, shown_grips
from arqstudiocad.core import CadArc, CadCircle, CadDocument, CadGrip, CadLine
from arqstudiocad.events import (
    CommandMessage,
    CommandPrompt,
    DocumentChanged,
    Drag,
    Event,
    EventBus,
)
from arqstudiocad.session import Session
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import HOT


class Harness:
    """A line (0,0)-(10,0), a circle at (20,0) radius 2 and an arc at
    (30,0) radius 2 from 0 to 180 degrees, all selected."""

    def __init__(self) -> None:
        self.bus = EventBus()
        self.events: list[Event] = []
        self.bus.subscribe(Event, self.events.append)
        self.session = Session(self.bus)
        self.runner = CommandRunner(builtin_registry(), CommandContext(self.session))
        self.document: CadDocument = self.session.new_document()
        self.line, _ = self.document.add_line((0.0, 0.0), (10.0, 0.0))
        self.circle, _ = self.document.add_circle((20.0, 0.0), 2.0)
        self.arc, _ = self.document.add_arc((30.0, 0.0), 2.0, 0.0, 3.141592653589793)
        self.session.select(self.document, [self.line, self.circle, self.arc])
        self.steps = len(self.document.undo_history)

    def grip(self, entity_id: int, kind: str, at: tuple[float, float] | None = None) -> CadGrip:
        for grip in shown_grips(self.session, self.document):
            same = grip.entity_id == entity_id and grip.kind == kind
            if same and (at is None or grip.point == pytest.approx(at)):
                return grip
        raise AssertionError(f"no {kind} grip of {entity_id}")

    def edit(self, grip: CadGrip, *answers: str) -> None:
        self.runner.start(grip_edit(grip))
        for answer in answers:
            self.runner.execute(answer)

    def geometry(self, entity_id: int) -> CadLine | CadCircle | CadArc | None:
        return self.document.get_entity(entity_id).geometry

    def prompt(self) -> str:
        return [e.text for e in self.events if isinstance(e, CommandPrompt)][-1]

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def new_steps(self) -> list[str]:
        return self.document.undo_history[self.steps :]


OPTIONS = "[Base point(B)/Copy(C)/Undo(U)/Exit(X)]:"


# Which grips show


def test_selected_entities_show_their_grips() -> None:
    h = Harness()
    kinds = [(g.entity_id, g.kind) for g in shown_grips(h.session, h.document)]
    assert kinds == [
        (h.line, "END"),
        (h.line, "MID"),
        (h.line, "END"),
        (h.circle, "CEN"),
        *[(h.circle, "QUA")] * 4,
        (h.arc, "END"),
        (h.arc, "MID"),
        (h.arc, "END"),
        (h.arc, "CEN"),
    ]


def test_locked_layers_and_large_selections_show_no_grips() -> None:
    h = Harness()
    layer_id, _ = h.document.create_layer("Locked")
    h.document.set_entity_layer([h.circle, h.arc], layer_id)
    h.document.set_layer_locked(layer_id, True)
    assert {g.entity_id for g in shown_grips(h.session, h.document)} == {h.line}
    many = [h.document.add_line((0.0, i + 1.0), (1.0, i + 1.0))[0] for i in range(GRIP_LIMIT)]
    h.session.clear_selection(h.document)
    h.session.select(h.document, many)
    assert len(shown_grips(h.session, h.document)) == 3 * GRIP_LIMIT, "up to the limit"
    h.session.select(h.document, [h.line])
    assert shown_grips(h.session, h.document) == [], "past AutoCAD's GRIPOBJLIMIT"


# Stretch


def test_stretching_a_line_end_moves_only_that_end() -> None:
    h = Harness()
    h.edit(h.grip(h.line, "END", (10.0, 0.0)))
    assert h.prompt() == f"** STRETCH ** Specify stretch point {OPTIONS}"
    h.runner.execute("20,5")
    assert h.geometry(h.line) == CadLine((0.0, 0.0), (20.0, 5.0))
    assert h.runner.active is None and h.new_steps() == ["GRIP"]
    assert set(h.session.selection(h.document)) == {h.line, h.circle, h.arc}, "kept"


def test_a_stretch_from_another_base_point_moves_the_grip_as_much() -> None:
    h = Harness()
    h.edit(h.grip(h.line, "END", (10.0, 0.0)), "B", "100,100", "100,105")
    assert h.geometry(h.line) == CadLine((0.0, 0.0), (10.0, 5.0))


def test_stretching_a_midpoint_or_a_center_moves_the_entity() -> None:
    h = Harness()
    grip = h.grip(h.line, "MID")
    h.runner.start(grip_edit(grip))
    prompt = [e for e in h.events if isinstance(e, CommandPrompt)][-1]
    assert prompt.drag == Drag("move", (h.line,)) and prompt.base == (5.0, 0.0)
    h.runner.execute("@0,3")
    assert h.geometry(h.line) == CadLine((0.0, 3.0), (10.0, 3.0))
    h.edit(h.grip(h.circle, "CEN"), "20,10")
    assert h.geometry(h.circle) == CadCircle((20.0, 10.0), 2.0)


def test_stretching_a_quadrant_changes_the_radius() -> None:
    h = Harness()
    h.edit(h.grip(h.circle, "QUA", (22.0, 0.0)), "25,0")
    assert h.geometry(h.circle) == CadCircle((20.0, 0.0), 5.0)
    h.edit(h.grip(h.circle, "QUA", (25.0, 0.0)), "20,0")
    assert h.messages()[-1] == "The object cannot take that shape."
    assert h.runner.active == "GRIP", "asked again"


def test_stretching_an_arc_end_keeps_its_midpoint_and_other_end() -> None:
    h = Harness()
    h.edit(h.grip(h.arc, "END", (32.0, 0.0)), "31,-1")
    arc = h.geometry(h.arc)
    assert isinstance(arc, CadArc)
    ends = {tuple(round(v, 9) for v in p) for p in (arc.start_point, arc.end_point)}
    assert ends == {(31.0, -1.0), (28.0, 0.0)}


def test_either_end_of_a_line_or_an_arc_stretches_alone() -> None:
    h = Harness()
    h.edit(h.grip(h.line, "END", (0.0, 0.0)), "-5,0")
    assert h.geometry(h.line) == CadLine((-5.0, 0.0), (10.0, 0.0)), "its direction kept"
    h.edit(h.grip(h.arc, "END", (28.0, 0.0)), "29,-1")
    arc = h.geometry(h.arc)
    assert isinstance(arc, CadArc)
    ends = {tuple(round(v, 9) for v in p) for p in (arc.start_point, arc.end_point)}
    assert ends == {(32.0, 0.0), (29.0, -1.0)}


def test_the_arc_passes_through_its_old_midpoint() -> None:
    h = Harness()
    h.edit(h.grip(h.arc, "END", (32.0, 0.0)), "31,-1")
    arc = h.geometry(h.arc)
    assert isinstance(arc, CadArc)
    cx, cy = arc.center
    assert ((30.0 - cx) ** 2 + (2.0 - cy) ** 2) ** 0.5 == pytest.approx(arc.radius)


# The other modes


def test_enter_cycles_the_modes_around_the_hot_grip() -> None:
    h = Harness()
    h.edit(h.grip(h.line, "END", (0.0, 0.0)))
    texts = []
    for _ in range(5):
        texts.append(h.prompt().split(" [")[0])
        h.runner.execute("")
    assert texts == [
        "** STRETCH ** Specify stretch point",
        "** MOVE ** Specify move point",
        "** ROTATE ** Specify rotation angle",
        "** SCALE ** Specify scale factor",
        "** MIRROR ** Specify second point",
    ]
    assert h.prompt().startswith("** STRETCH **"), "and round again"


def test_move_rotate_scale_and_mirror_apply_to_the_whole_selection() -> None:
    h = Harness()
    h.edit(h.grip(h.line, "END", (0.0, 0.0)), "", "0,1")  # MOVE
    assert h.geometry(h.line) == CadLine((0.0, 1.0), (10.0, 1.0))
    assert h.geometry(h.circle) == CadCircle((20.0, 1.0), 2.0)
    h.edit(h.grip(h.line, "END", (0.0, 1.0)), "", "", "90")  # ROTATE
    line = h.geometry(h.line)
    assert isinstance(line, CadLine) and line.end == pytest.approx((0.0, 11.0))
    h.edit(h.grip(h.line, "END", (0.0, 1.0)), "", "", "", "2")  # SCALE
    line = h.geometry(h.line)
    assert isinstance(line, CadLine) and line.end == pytest.approx((0.0, 21.0))
    h.edit(h.grip(h.line, "END", (0.0, 1.0)), "", "", "", "", "1,1")  # MIRROR
    line = h.geometry(h.line)
    assert isinstance(line, CadLine) and line.end == pytest.approx((0.0, -19.0))
    assert h.new_steps() == ["GRIP"] * 4


def test_copy_makes_a_copy_per_pick_until_enter_with_undo() -> None:
    h = Harness()
    before = len(h.document.get_entities())
    h.edit(h.grip(h.line, "END", (10.0, 0.0)), "C", "10,5", "10,10", "10,15", "U", "")
    assert h.geometry(h.line) == CadLine((0.0, 0.0), (10.0, 0.0)), "the original stays"
    entities = h.document.get_entities()
    assert len(entities) == before + 2
    assert [e.geometry for e in entities[-2:]] == [
        CadLine((0.0, 0.0), (10.0, 5.0)),
        CadLine((0.0, 0.0), (10.0, 10.0)),
    ]
    assert h.new_steps() == ["GRIP"], "one undo step"
    created = {i for e in h.events if isinstance(e, DocumentChanged) for i in e.changes.created}
    assert {e.id for e in entities[-2:]} <= created, "the viewport learns of each copy"
    h.document.undo()
    assert len(h.document.get_entities()) == before


def test_base_point_exit_and_refusals() -> None:
    h = Harness()
    h.edit(h.grip(h.line, "END", (0.0, 0.0)), "", "B", "100,100", "105,103")  # MOVE by (5,3)
    assert h.geometry(h.line) == CadLine((5.0, 3.0), (15.0, 3.0))
    said = len(h.messages())
    h.edit(h.grip(h.line, "END", (5.0, 3.0)), "X")
    assert h.runner.active is None and h.geometry(h.line) == CadLine((5.0, 3.0), (15.0, 3.0))
    assert len(h.messages()) == said, "eXit ends quietly"
    h.edit(h.grip(h.line, "END", (5.0, 3.0)), "U")
    assert h.messages()[-1] == "Nothing to undo."
    h.runner.execute("15,3")  # onto the other end
    assert h.messages()[-1] == "The object cannot take that shape."
    h.runner.cancel()
    h.edit(h.grip(h.line, "END", (5.0, 3.0)), "", "", "", "", "5,3")
    assert h.messages()[-1] == "The two points must differ."
    h.runner.cancel()
    assert h.new_steps() == ["GRIP"], "only the move made a step"


def test_a_grip_edit_is_not_repeated_by_enter() -> None:
    h = Harness()
    h.runner.execute("ID")
    h.runner.execute("1,1")
    h.edit(h.grip(h.line, "MID"), "5,1")
    h.runner.execute("")
    assert h.runner.active == "IDPOINT", "Enter repeats the last command typed"


# In the window


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


def test_clicking_a_grip_makes_it_hot_and_edits_it(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)  # a line (0,0)-(100,0), a circle at (50,40)
    document = window.session.active
    assert document is not None
    line_id = document.get_entities()[0].id
    window.session.drafting.set_osnap(False)
    window.session.select(document, [line_id])
    assert [g.kind for g in viewport.grips] == ["END", "MID", "END"]
    end = viewport.grips[2]
    x, y = viewport.view.to_screen(*end.point)
    ratio = viewport.devicePixelRatioF()
    assert viewport.grip_at(QPointF(x / ratio + 2, y / ratio - 2)) == end
    mouse(viewport, QEvent.Type.MouseButtonPress, end.point)
    assert window.runner.active == "GRIP" and viewport.hot == end
    image = viewport.grab().toImage()
    red = image.pixelColor(round(x / ratio), round(y / ratio))
    assert (red.red(), red.green(), red.blue()) == (HOT.red(), HOT.green(), HOT.blue())
    mouse(viewport, QEvent.Type.MouseMove, (100.0, 20.0))
    mouse(viewport, QEvent.Type.MouseButtonPress, (100.0, 20.0))
    QApplication.processEvents()
    line = document.get_entity(line_id).geometry
    assert isinstance(line, CadLine) and line.start == (0.0, 0.0)
    assert line.end == pytest.approx((100.0, 20.0), abs=0.5)
    assert viewport.hot is None and viewport.grips[2].point == line.end, "grips follow"


def test_grips_hide_while_another_command_runs(window: MainWindow, tmp_path: Path) -> None:
    viewport = plan(window, tmp_path)
    document = window.session.active
    assert document is not None
    window.session.select(document, [document.get_entities()[1].id])
    assert len(viewport.grips) == 5
    window.runner.execute("LINE")
    assert len(viewport.grips) == 0
    window.runner.cancel()
    window.session.select(document, [document.get_entities()[1].id])
    assert len(viewport.grips) == 5
