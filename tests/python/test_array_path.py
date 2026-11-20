"""ARRAYPATH prompts, cancellation, shared actions and Spanish names."""

import pytest
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget


def chosen() -> tuple[Harness, int, int]:
    h = Harness()
    source = h.line((0, 0), (2, 0))
    guide, _ = h.document.add_polyline([((10, 0), 0), ((20, 0), 0), ((20, 10), 0)], False)
    h.session.select(h.document, [source])
    return h, source, guide


@pytest.mark.parametrize(
    "align,ends", [("Y", [(12, 0), (20, 2), (20, 12)]), ("N", [(12, 0), (22, 0), (22, 10)])]
)
def test_path_array_command_keeps_sources_and_guide_aligns_and_undoes_once(
    align: str, ends: list[tuple[int, int]]
) -> None:
    h, source, guide = chosen()
    h.run("ARRAYPATH", "0,0")
    h.runner.answer_entity(guide, (15, 0))
    h.run("3", align)
    assert h.runner.active is None
    assert h.lines() == sorted(
        [((0, 0), (2, 0)), *zip([(10, 0), (20, 0), (20, 10)], ends, strict=True)]
    )
    assert h.session.selection(h.document) == ()
    assert h.messages()[-1] == "Created 3 independent copies."
    assert h.document.get_entity(source).kind == "LINE"
    assert h.document.get_entity(guide).kind == "LWPOLYLINE"
    assert h.document.undo_history[-1] == "ARRAYPATH"
    h.run("UNDO")
    assert h.lines() == [((0, 0), (2, 0))]
    h.run("REDO")
    assert len(h.document.get_entities()) == 5


@pytest.mark.parametrize("stage", range(4))
def test_path_array_cancellation_at_each_prompt_preserves_geometry_and_defaults(stage: int) -> None:
    h, _, guide = chosen()
    before = h.document.undo_history
    h.run("ARRAYPATH")
    if stage >= 1:
        h.run("0,0")
    if stage >= 2:
        h.runner.answer_entity(guide, (15, 0))
    if stage >= 3:
        h.run("3")
    assert h.runner.active == "ARRAYPATH"
    h.runner.cancel()
    assert len(h.document.get_entities()) == 2
    assert h.document.undo_history == before
    assert not any(key.startswith("PATH_") for key in h.runner.context.remembered)


def test_path_array_invalid_count_defaults_and_previous_selection() -> None:
    h, _, guide = chosen()
    h.run("ARRAYPATH", "0,0")
    circle, _ = h.document.add_circle((0, 10), 2)
    h.runner.answer_entity(circle, (0, 12))
    assert "currently supports" in h.messages()[-1]
    h.runner.answer_entity(guide, (15, 0))
    h.run("1.5")
    assert h.runner.active == "ARRAYPATH"
    h.run("1")
    assert "between 2 and 100000" in h.messages()[-1]
    h.run("3", "N")
    assert h.runner.context.remembered["PATH_COUNT"] == 3
    assert h.runner.context.remembered["PATH_ALIGN_ITEMS"] is False
    h.run("ARRAYPATH", "P", "", "0,0")
    h.runner.answer_entity(guide, (15, 0))
    h.run("", "")
    assert h.runner.active is None
    assert len(h.lines()) == 7


def test_path_array_verb_noun_empty_selection_and_top_guard() -> None:
    from test_commands import FakeUi, FakeView

    h, source, guide = chosen()
    h.session.clear_selection(h.document)
    h.run("ARRAYPATH")
    assert h.runner.prompt is not None and h.runner.prompt.kind == "select"
    h.session.select(h.document, [source])
    h.run("", "0,0")
    h.runner.answer_entity(guide, (15, 0))
    h.run("2", "Y")
    assert len(h.document.get_entities()) == 4
    h.run("ARRAYPATH", "")
    assert h.runner.active is None and len(h.document.get_entities()) == 4
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    h.run("ARRAYPATH")
    assert h.messages()[-1] == "Use PLAN to switch to the Top view before using ARRAYPATH."


def test_path_array_failure_preserves_defaults_selection_and_history() -> None:
    h = Harness()
    source = h.line((1e308, 0), (1.1e308, 0))
    guide = h.line((0, 0), (1e308, 0))
    h.session.select(h.document, [source])
    h.runner.context.remembered["PATH_COUNT"] = 4
    before = h.document.undo_history
    h.run("ARRAYPATH", "0,0")
    h.runner.answer_entity(guide, (0, 0))
    h.run("3", "N")
    assert h.messages()[-1].startswith("Error: ")
    assert h.runner.context.remembered["PATH_COUNT"] == 4
    assert h.session.selection(h.document) == (source,)
    assert h.document.undo_history == before
    assert len(h.document.get_entities()) == 2


def test_path_array_shared_action_and_spanish_catalog(qtbot: QtBot) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    action = window.command_actions["ARRAYPATH"]
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Modify"].actions()
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    assert action in [b.defaultAction() for b in groups["modify"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    window.change_language("es")
    assert action.text() == "Matriz de &camino"
    found = window.runner.registry.find("MATRIZCAMINO")
    assert found is not None and found.name == "ARRAYPATH"
