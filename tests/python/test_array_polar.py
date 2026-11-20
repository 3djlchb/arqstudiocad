"""Polar ARRAY command workflow, cancellation and shared localized action."""

import pytest
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget


def chosen() -> Harness:
    h = Harness()
    source = h.line((10, 0), (12, 0))
    h.session.select(h.document, [source])
    return h


@pytest.mark.parametrize("fill,tip", [("180", (-12, 0)), ("-180", (-12, 0)), ("360", (0, -12))])
def test_polar_command_degrees_orientation_selection_and_undo(
    fill: str, tip: tuple[int, int]
) -> None:
    h = chosen()
    h.run("ARRAYPOLAR", "0,0", "4" if fill == "360" else "3", fill, "Y")
    assert h.runner.active is None
    assert any(end == tip for _, end in h.lines())
    assert ((10, 0), (12, 0)) in h.lines()
    assert h.session.selection(h.document) == ()
    assert h.document.undo_history[-1] == "ARRAYPOLAR"
    assert h.runner.context.remembered["POLAR_ROTATE_ITEMS"] is True
    h.run("UNDO")
    assert h.lines() == [((10, 0), (12, 0))]
    h.run("REDO")
    assert len(h.lines()) == (4 if fill == "360" else 3)


def test_polar_no_rotation_requests_base_and_preserves_group() -> None:
    h = chosen()
    other = h.line((10, 2), (12, 2))
    h.session.select(h.document, [other, *h.session.selection(h.document)])
    h.run("ARRAYPOLAR", "0,0", "3", "180", "N")
    assert h.runner.prompt is not None
    assert h.runner.prompt.message == "Specify array base point"
    assert len(h.lines()) == 2
    h.run("10,0")
    assert h.lines() == sorted(
        [
            ((10, 0), (12, 0)),
            ((10, 2), (12, 2)),
            ((0, 10), (2, 10)),
            ((0, 12), (2, 12)),
            ((-10, 0), (-8, 0)),
            ((-10, 2), (-8, 2)),
        ]
    )
    assert h.runner.context.remembered["POLAR_ROTATE_ITEMS"] is False


@pytest.mark.parametrize(
    "answers", [[], ["0,0"], ["0,0", "3"], ["0,0", "3", "180"], ["0,0", "3", "180", "N"]]
)
def test_polar_cancel_each_parameter_leaves_geometry_history_and_defaults(
    answers: list[str],
) -> None:
    h = chosen()
    before = h.document.undo_history
    h.run("ARRAYPOLAR", *answers)
    assert h.runner.active == "ARRAYPOLAR"
    h.runner.cancel()
    assert h.lines() == [((10, 0), (12, 0))]
    assert h.document.undo_history == before
    assert not any(key.startswith("POLAR_") for key in h.runner.context.remembered)


def test_polar_invalid_parameters_then_defaults_and_previous_selection() -> None:
    h = chosen()
    h.run("ARRAYPOLAR", "0,0", "1")
    assert h.runner.active == "ARRAYPOLAR"
    assert len(h.lines()) == 1
    h.run("3", "0")
    assert h.runner.active == "ARRAYPOLAR"
    h.run("361")
    assert h.runner.active == "ARRAYPOLAR"
    h.run("180", "Y")
    assert len(h.lines()) == 3
    h.run("ARRAYPOLAR", "P", "", "0,0", "", "", "")
    assert h.runner.active is None
    assert len(h.lines()) == 5


def test_polar_verb_noun_empty_selection_and_top_view_guard() -> None:
    from test_commands import FakeUi, FakeView

    h = Harness()
    source = h.line((10, 0), (12, 0))
    h.run("ARRAYPOLAR")
    assert h.runner.prompt is not None and h.runner.prompt.kind == "select"
    h.session.select(h.document, [source])
    h.run("", "0,0", "2", "180", "Y")
    assert len(h.lines()) == 2
    h.run("ARRAYPOLAR", "")
    assert h.runner.active is None and len(h.lines()) == 2
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    h.run("ARRAYPOLAR")
    assert h.messages()[-1] == "Use PLAN to switch to the Top view before using ARRAYPOLAR."


def test_polar_action_shared_in_menu_ribbon_toolbar_and_spanish_catalog(qtbot: QtBot) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    action = window.command_actions["ARRAYPOLAR"]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    assert action in [b.defaultAction() for b in groups["modify"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    window.change_language("es")
    assert action.text() == "Matriz &polar"
    found = window.runner.registry.find("MATRIZPOLAR")
    assert found is not None and found.name == "ARRAYPOLAR"


def test_polar_native_failure_preserves_defaults_selection_and_geometry() -> None:
    h = chosen()
    h.run("ARRAYPOLAR", "0,0", "3", "180", "Y")
    original = h.document.get_entities()[0].id
    h.session.select(h.document, [original])
    before = h.document.undo_history
    defaults = dict(h.runner.context.remembered)
    geometry = h.lines()
    h.run("ARRAYPOLAR", "-1e308,0", "4", "360", "Y")
    assert h.runner.active is None
    assert h.messages()[-1].startswith("Error: ")
    assert h.document.undo_history == before
    assert h.runner.context.remembered == defaults
    assert h.lines() == geometry
    assert h.session.selection(h.document) == (original,)
