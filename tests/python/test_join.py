"""JOIN, lines and arcs (docs/design/join-command.md): a source and what
joins it, many at once, Close, what cannot join, the shared action."""

from math import pi

import pytest
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.core import CadArc, CadCircle
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget


def choose(h: Harness, ids: list[int]) -> None:
    """Selects ``ids`` at the current selection prompt, then Enter."""
    h.session.select(h.document, ids)
    h.run("")


def test_join_from_a_line_takes_the_collinear_lines_across_their_gaps() -> None:
    h = Harness()
    source = h.line((2.0, 0.0), (4.0, 0.0))
    ahead = h.line((6.0, 0.0), (9.0, 0.0))
    behind = h.line((-3.0, 0.0), (0.0, 0.0))
    across = h.line((3.0, -1.0), (3.0, 1.0))
    h.run("J")
    assert h.prompt().text == "Select source object or multiple objects to join at once:"
    choose(h, [source])
    assert h.prompt().text == "Select lines to join to source:"
    choose(h, [ahead, behind, across])
    assert h.runner.active is None
    assert h.lines() == [((-3.0, 0.0), (9.0, 0.0)), ((3.0, -1.0), (3.0, 1.0))]
    assert h.document.get_entity(source).kind == "LINE", "the source, its id kept"
    assert h.messages()[-2:] == ["Joined to the source: 2.", "Discarded from the operation: 1."]
    assert h.document.undo_history[-1] == "JOIN"
    h.run("UNDO")
    assert ((-3.0, 0.0), (0.0, 0.0)) in h.lines()


def test_join_from_an_arc_closes_it_into_its_circle() -> None:
    h = Harness()
    arc, _ = h.document.add_arc((0.0, 0.0), 5.0, 0.0, pi / 2)
    h.run("JOIN")
    choose(h, [arc])
    assert h.prompt().text == "Select arcs to join to source [Close(L)]:"
    h.run("L")
    assert h.runner.active is None
    circles = [g for e in h.document.get_entities() if isinstance(g := e.geometry, CadCircle)]
    assert len(circles) == 1 and circles[0].radius == 5.0
    assert h.messages()[-1] == "The arc is now a circle."
    assert h.document.undo_history[-1] == "JOIN"


def test_join_many_at_once_from_what_is_selected_already() -> None:
    h = Harness()
    a = h.line((0.0, 0.0), (1.0, 0.0))
    b, _ = h.document.add_arc((0.0, 5.0), 1.0, 0.0, 1.0)
    c = h.line((3.0, 0.0), (4.0, 0.0))
    d, _ = h.document.add_arc((0.0, 5.0), 1.0, 2.0, 3.0)
    lonely = h.line((0.0, 9.0), (1.0, 9.0))
    h.session.select(h.document, [a, b, c, d, lonely])
    h.run("JOIN")
    assert h.runner.active is None, "the selection answers at once"
    assert ((0.0, 0.0), (4.0, 0.0)) in h.lines()
    arc = h.document.get_entity(b).geometry
    assert isinstance(arc, CadArc) and (arc.start_angle, arc.end_angle) == (0.0, 3.0)
    assert h.messages()[-2:] == ["Joined: 2, into 2.", "Discarded from the operation: 1."]


def test_join_leaves_the_drawing_alone_when_nothing_can_join() -> None:
    h = Harness()
    circle, _ = h.document.add_circle((0.0, 0.0), 1.0)
    source = h.line((0.0, 0.0), (1.0, 0.0))
    other = h.line((0.0, 3.0), (1.0, 3.0))
    before = h.document.undo_history
    h.run("JOIN")
    choose(h, [circle])
    assert h.messages()[-1] == "JOIN currently joins lines and arcs only."
    h.run("JOIN")
    choose(h, [source])
    choose(h, [other])
    assert h.messages()[-2:] == ["Nothing joined.", "Discarded from the operation: 1."]
    assert h.document.undo_history == before


@pytest.mark.parametrize("stage", ["source", "others"])
def test_join_cancelled_at_any_prompt_changes_nothing(stage: str) -> None:
    h = Harness()
    source = h.line((0.0, 0.0), (1.0, 0.0))
    other = h.line((2.0, 0.0), (3.0, 0.0))
    before = h.document.undo_history
    h.run("JOIN")
    if stage == "others":
        choose(h, [source])
        h.session.select(h.document, [other])
    assert h.runner.active is not None
    h.runner.cancel()
    assert h.runner.active is None
    assert h.document.undo_history == before
    assert h.lines() == [((0.0, 0.0), (1.0, 0.0)), ((2.0, 0.0), (3.0, 0.0))]


def test_join_refuses_other_views_before_asking() -> None:
    from test_commands import FakeUi, FakeView

    h = Harness()
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    h.run("JOIN")
    assert h.runner.active is None
    assert h.messages()[-1] == "Use PLAN to switch to the Top view before using JOIN."


def test_join_action_is_shared_and_speaks_spanish(qtbot: QtBot) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    action = window.command_actions["JOIN"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    assert action in [button.defaultAction() for button in groups["modify"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    window.runner.execute("NEW")
    window.change_language("es")
    assert action.text() == "&Unir"
    found = window.runner.registry.find("UNIR")
    assert found is not None and found.name == "JOIN"
    window.runner.execute("UNIR")
    assert window.runner.prompt is not None
    assert (
        window.runner.prompt.message
        == "Seleccione el objeto de origen o varios objetos para unirlos a la vez"
    )
    window.runner.cancel()
