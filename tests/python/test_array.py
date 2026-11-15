"""Rectangular ARRAY command input, cancellation, localization and shared action."""

import pytest
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget


def chosen() -> tuple[Harness, int]:
    h = Harness()
    source = h.line((0, 0), (2, 0))
    h.session.select(h.document, [source])
    return h, source


def test_array_signed_grid_degrees_selection_events_and_undo() -> None:
    h, source = chosen()
    h.run("AR", "2", "3", "-5", "10", "90")
    assert h.runner.active is None
    assert h.lines() == [((x, y), (x + 2, y)) for x in (0, 5) for y in (0, 10, 20)]
    assert h.session.selection(h.document) == ()
    assert h.messages()[-1] == "Created 5 independent copies."
    assert h.document.undo_history[-1] == "ARRAYRECT"
    h.run("UNDO")
    assert h.lines() == [((0, 0), (2, 0))]
    assert h.document.get_entity(source).kind == "LINE"
    h.run("REDO")
    assert len(h.lines()) == 6


def test_array_validates_counts_spacing_and_remembers_only_completed_parameters() -> None:
    h, _ = chosen()
    h.run("ARRAY", "1.5")
    assert h.messages()[-1] == "Enter a whole number from 1 to 100000."
    h.run("1", "1")
    assert h.messages()[-1] == "The array must contain between 2 and 100000 cells."
    h.run("2", "0")
    assert h.messages()[-1] == "Spacing must be nonzero."
    h.run("-3", "0")
    assert h.lines() == [((-3, 0), (-1, 0)), ((0, 0), (2, 0))]
    assert h.runner.context.remembered["ARRAY_ROWS"] == 1
    h.run("-ARRAY", "P", "", "", "", "", "")
    assert len(h.lines()) == 3


@pytest.mark.parametrize("answers", [[], ["2"], ["2", "3"], ["2", "3", "5"], ["2", "3", "5", "10"]])
def test_array_cancellation_at_every_parameter_is_non_mutating(answers: list[str]) -> None:
    h, _ = chosen()
    before = h.document.undo_history
    h.run("ARRAYRECT", *answers)
    assert h.runner.active == "ARRAYRECT"
    h.runner.cancel()
    assert h.lines() == [((0, 0), (2, 0))]
    assert h.document.undo_history == before
    assert "ARRAY_ROWS" not in h.runner.context.remembered


def test_array_verb_noun_selection_and_empty_selection() -> None:
    h = Harness()
    source = h.line((0, 0), (2, 0))
    h.run("ARRAY")
    assert h.runner.prompt is not None and h.runner.prompt.kind == "select"
    h.session.select(h.document, [source])
    h.run("", "2", "1", "4", "0")
    assert h.lines() == [((0, 0), (2, 0)), ((0, 4), (2, 4))]
    h.run("ARRAY", "")
    assert h.runner.active is None
    assert len(h.lines()) == 2


def test_array_refuses_non_plan_view() -> None:
    from test_commands import FakeUi, FakeView

    h, _ = chosen()
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    h.run("ARRAY")
    assert h.runner.active is None
    assert h.messages()[-1] == "Use PLAN to switch to the Top view before using ARRAYRECT."


def test_array_action_is_shared_and_translated(qtbot: QtBot) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    action = window.command_actions["ARRAYRECT"]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    assert action in [b.defaultAction() for b in groups["modify"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    window.change_language("es")
    assert action.text() == "&Matriz rectangular"
    found = window.runner.registry.find("MATRIZ")
    assert found is not None and found.name == "ARRAYRECT"
    window.runner.execute("NEW")
    action.trigger()
    assert window.runner.prompt is not None
    assert window.runner.prompt.message == "Designe objetos"
    window.runner.cancel()
