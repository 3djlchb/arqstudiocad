"""BREAK command flows: known coordinates, cancellation, events and UI input."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.core import CadArc, CadLine
from arqstudiocad.events import DocumentChanged
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget


def test_break_pick_first_override_relative_input_and_atomic_undo() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.events.clear()
    h.run("BR")
    assert h.prompt().entity
    h.runner.answer_entity(line, (1.0, 0.0))
    assert h.prompt().text == "Specify second break point [First point(F)]:"
    h.run("F", "3,0", "@4,0")
    assert h.lines() == [((0.0, 0.0), (3.0, 0.0)), ((7.0, 0.0), (10.0, 0.0))]
    assert h.runner.active is None
    assert h.document.undo_history[-1] == "BREAK"
    assert any(isinstance(event, DocumentChanged) for event in h.events)
    h.run("UNDO")
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))]
    h.run("REDO")
    assert h.lines() == [((0.0, 0.0), (3.0, 0.0)), ((7.0, 0.0), (10.0, 0.0))]


def test_break_same_point_and_circle_direction() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    h.run("BREAK")
    h.runner.answer_entity(line, (4.0, 0.0))
    h.run("@")
    assert h.lines() == [((0.0, 0.0), (4.0, 0.0)), ((4.0, 0.0), (10.0, 0.0))]
    circle, _ = h.document.add_circle((20.0, 0.0), 5.0)
    h.run("BREAK")
    h.runner.answer_entity(circle, (25.0, 0.0))
    h.run("20,5")
    arcs = [e.geometry for e in h.document.get_entities() if isinstance(e.geometry, CadArc)]
    assert len(arcs) == 1
    assert arcs[0].start_angle == pytest.approx(1.5707963267948966)
    assert arcs[0].end_angle == pytest.approx(0.0)


@pytest.mark.parametrize("stage", ["selection", "second", "first", "replacement-second"])
def test_break_cancellation_never_changes_the_drawing(stage: str) -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    before = h.document.undo_history
    h.run("BREAK")
    assert h.prompt().entity and h.runner.active is not None
    if stage != "selection":
        h.runner.answer_entity(line, (2.0, 0.0))
    if stage in ("first", "replacement-second"):
        h.run("F")
        assert h.prompt().text == "Specify first break point:"
    if stage == "replacement-second":
        h.run("3,0")
        assert h.prompt().text == "Specify second break point:"
    assert h.runner.active is not None
    h.runner.cancel()
    assert h.document.undo_history == before
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))]
    assert h.runner.active is None


def test_break_unsupported_entities_and_invalid_points_leave_no_edit() -> None:
    h = Harness()
    unsupported, _ = h.document.add_text("unsupported", (0, 0), 1, 0)
    circle, _ = h.document.add_circle((20, 0), 5)
    before = h.document.undo_history
    h.run("BREAK")
    h.runner.answer_entity(unsupported, (3, 0))
    assert (
        h.messages()[-1] == "BREAK currently supports lines, arcs, circles, ellipses, splines and "
        "lightweight polylines."
    )
    assert h.prompt().entity
    h.runner.answer_entity(circle, (25, 0))
    h.run("@")
    assert h.messages()[-1] == "Those points do not define a valid break."
    assert h.document.undo_history == before
    assert h.document.get_entity(circle).kind == "CIRCLE"


def test_break_action_is_shared_and_viewport_clicks_answer_both_points(
    qtbot: QtBot, tmp_path: Path
) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)
    action = window.command_actions["BREAK"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    assert action in [button.defaultAction() for button in groups["modify"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    action.trigger()
    mouse(viewport, QEvent.Type.MouseButtonPress, (30.0, 0.0))
    assert window.runner.prompt is not None and window.runner.prompt.kind == "point"
    mouse(viewport, QEvent.Type.MouseButtonPress, (70.0, 0.0))
    document = window.session.active
    assert document is not None and document.undo_history[-1] == "BREAK"
    lines = [e.geometry for e in document.get_entities() if isinstance(e.geometry, CadLine)]
    assert len(lines) == 2
    assert sorted(g.end[0] for g in lines) == pytest.approx([30, 100], abs=1)
    window.change_language("es")
    assert action.text() == "&Partir"
    found = window.runner.registry.find("PARTE")
    assert found is not None and found.name == "BREAK"
    window.runner.execute("PARTE")
    assert window.runner.prompt is not None
    assert window.runner.prompt.message == "Seleccione el objeto a partir"
    window.runner.cancel()


def test_break_refuses_non_top_views_before_asking_for_an_object() -> None:
    from test_commands import FakeUi, FakeView

    h = Harness()
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    before = h.document.undo_history
    h.run("BREAK")
    assert h.runner.active is None
    assert h.document.undo_history == before
    assert h.messages()[-1] == "Use PLAN to switch to the Top view before using BREAK."


def test_break_on_a_locked_layer_says_so_and_asks_again() -> None:
    h = Harness()
    line = h.line((0.0, 0.0), (10.0, 0.0))
    layer, _ = h.document.create_layer("LOCKED")
    h.document.set_entity_layer([line], layer)
    h.document.set_layer_locked(layer, True)
    before = h.document.undo_history
    h.run("BREAK")
    h.runner.answer_entity(line, (3.0, 0.0))
    assert h.messages()[-1] == "1 were on a locked layer."
    assert h.prompt().entity, "another object is asked for"
    h.runner.cancel()
    assert h.lines() == [((0.0, 0.0), (10.0, 0.0))]
    assert h.document.undo_history == before
