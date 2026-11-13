"""STRETCH's AutoCAD 2021 subset, verified locally without external CAD."""

import math
from pathlib import Path

import pytest
from pytestqt.qtbot import QtBot
from test_modify_commands import Harness
from test_selection import drag, plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadBounds2, CadLine, InvalidGeometryError, InvalidOperationError
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow


def area() -> CadBounds2:
    return CadBounds2((9, -1), (11, 1))


def test_typed_crossing_stretches_only_the_endpoint_and_undo_redo() -> None:
    h = Harness()
    h.run("S", "C", "9,-1", "11,1", "", "0,0", "@2,3")
    assert h.runner.active is None
    assert h.line(h.a) == ((0, 0), (12, 3))
    assert h.line(h.b) == ((0, 5), (10, 5))
    assert h.selection() == ()
    assert h.document.undo_history[-1] == "STRETCH"
    h.run("UNDO")
    assert h.line(h.a) == ((0, 0), (10, 0))
    h.run("REDO")
    assert h.line(h.a) == ((0, 0), (12, 3))


def test_individual_selection_moves_whole_and_enter_uses_first_point() -> None:
    h = Harness()
    h.select(h.a)
    h.run("STRETCH", "2,3", "")
    assert h.line(h.a) == ((2, 3), (12, 3))


def test_displacement_option_and_last_value() -> None:
    h = Harness()
    h.select(h.a)
    h.run("STRETCH", "D", "2,3")
    h.select(h.b)
    h.run("STRETCH", "", "")
    assert h.line(h.b) == ((2, 8), (12, 8))


def test_empty_selection_and_cancellation_change_nothing() -> None:
    h = Harness()
    before = h.document.undo_history
    h.run("STRETCH", "")
    assert h.runner.active is None
    h.run("STRETCH", "C", "9,-1", "11,1", "", "0,0")
    h.runner.cancel()
    assert h.document.undo_history == before
    assert h.line(h.a) == ((0, 0), (10, 0))


def test_selection_provenance_merges_and_is_cleared_by_removal() -> None:
    h = Harness()
    h.session.select(h.document, [h.a], crossing=area())
    left = CadBounds2((-1, -1), (1, 1))
    h.session.select(h.document, [h.a], crossing=left)
    assert len(h.session.crossing_selection(h.document)) == 2
    h.session.deselect(h.document, [h.a])
    h.select(h.a)
    assert h.session.crossing_selection(h.document) == []
    h.session.clear_selection(h.document)
    h.session.select(h.document, [h.a], crossing=area())
    h.select(h.a)  # an individual pick makes it a whole selection
    assert h.session.crossing_selection(h.document) == []


def test_locked_layers_are_skipped() -> None:
    h = Harness()
    layer, _ = h.document.create_layer("Locked")
    h.document.set_entity_layer([h.b], layer)
    h.document.set_layer_locked(layer, True)
    h.select(h.a, h.b)
    h.run("STRETCH", "0,0", "1,0")
    assert h.line(h.a) == ((1, 0), (11, 0))
    assert h.line(h.b) == ((0, 5), (10, 5))
    assert "1 were on a locked layer." in h.messages()


@pytest.mark.parametrize("bad", [math.nan, math.inf, -math.inf])
def test_binding_rejects_nonfinite_displacement(bad: float) -> None:
    h = Harness()
    with pytest.raises(InvalidGeometryError):
        h.document.stretch_entities([h.a], (bad, 0), [])
    assert h.line(h.a) == ((0, 0), (10, 0))


def test_multi_vertex_edit_is_simultaneous_and_keeps_bulges_and_style() -> None:
    h = Harness()
    poly, _ = h.document.add_polyline([((0, 0), 0.5), ((10, 0), 0), ((10, 10), 0)], False)
    before = h.document.get_entity(poly)
    box = CadBounds2((9, -1), (11, 11))
    h.document.stretch_entities([poly], (-10, 2), [(poly, box)])
    assert [g.point for g in h.document.grips([poly])] == [(0, 0), (0, 2), (0, 12)]
    assert h.document.get_entity(poly).layer == before.layer
    h.document.undo()
    assert [g.point for g in h.document.grips([poly])] == [(0, 0), (10, 0), (10, 10)]


def test_collapse_rolls_back_the_entire_edit() -> None:
    h = Harness()
    before = h.document.undo_history
    with pytest.raises(InvalidGeometryError):
        h.document.stretch_entities([h.b, h.a], (-10, 0), [(h.a, area())])
    assert h.document.undo_history == before
    assert h.line(h.b) == ((0, 5), (10, 5))
    poly, _ = h.document.add_polyline([((0, 0), 0), ((10, 0), 0)], False)
    with pytest.raises(InvalidGeometryError):
        h.document.stretch_entities([poly], (-10, 0), [(poly, area())])
    assert [g.point for g in h.document.grips([poly])] == [(0, 0), (10, 0)]


def test_solid_is_unchanged_and_partial_arc_is_explicitly_refused() -> None:
    h = Harness()
    box, _ = h.document.add_box((0, 0, 0), (1, 1, 1))
    before = h.document.undo_history
    changes = h.document.stretch_entities([box], (2, 0), [])
    assert not changes.modified and h.document.undo_history == before
    arc, _ = h.document.add_arc((0, 0), 10, 0, math.pi / 2)
    with pytest.raises(InvalidOperationError):
        h.document.stretch_entities([h.b, arc], (1, 0), [(arc, area())])
    assert h.line(h.b) == ((0, 5), (10, 5))


def test_spanish_and_forced_english_names() -> None:
    h = Harness()
    set_language("es")
    h.select(h.a)
    h.run("ESTIRA", "0,0", "1,0")
    assert h.line(h.a) == ((1, 0), (11, 0))
    assert h.runner.registry.find("_STRETCH") is not None


def test_mouse_crossing_reaches_the_command_and_the_ui(qtbot: QtBot, tmp_path: Path) -> None:
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    window.resize(1000, 700)
    window.show()
    qtbot.waitExposed(window)
    document, viewport, ids = plan(window, tmp_path)
    window.session.drafting.set_osnap(False)
    window.runner.execute("STRETCH")
    drag(viewport, (110, 10), (90, -10))
    assert window.session.crossing_selection(document)
    for line in ("", "0,0", "10,0"):
        window.runner.execute(line)
    assert document.get_entity(ids[0]).geometry == CadLine((0, 0), (110, 0))
    assert window.command_actions["STRETCH"].text() == "Stretc&h"
    from arqstudiocad.ui.ribbon import RibbonGroupWidget

    groups = [g for g in window.findChildren(RibbonGroupWidget) if g.group.name == "modify"]
    assert any("STRETCH" in g.group.commands for g in groups)


def test_stretch_refuses_other_views_before_selection() -> None:
    from test_commands import FakeUi, FakeView

    h = Harness()
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    before = h.document.undo_history
    h.run("STRETCH")
    assert h.runner.active is None
    assert h.document.undo_history == before
    assert h.messages()[-1] == "STRETCH currently requires the Top view. Use PLAN first."


def test_crossed_circle_moves_by_center_and_keeps_radius() -> None:
    from arqstudiocad.core import CadCircle

    h = Harness()
    circle, _ = h.document.add_circle((10, 0), 2)
    h.document.stretch_entities([circle], (3, 4), [(circle, area())])
    assert h.document.get_entity(circle).geometry == CadCircle((13, 4), 2)


def test_whole_window_and_all_move_the_whole_selection() -> None:
    h = Harness()
    h.run("STRETCH", "W", "-1,-1", "11,1", "", "0,0", "2,0")
    assert h.line(h.a) == ((2, 0), (12, 0))
    h.run("STRETCH", "ALL", "", "0,0", "0,2")
    assert h.line(h.a) == ((2, 2), (12, 2))
    assert h.line(h.b) == ((0, 7), (10, 7))


def test_deleted_selection_cannot_leave_stale_crossing_windows() -> None:
    h = Harness()
    h.session.select(h.document, [h.a], crossing=area())
    h.session.report(h.document, h.document.delete_entities([h.a]))
    assert h.session.crossing_selection(h.document) == []
    h.document.undo()
    h.select(h.a)
    assert h.session.crossing_selection(h.document) == []
