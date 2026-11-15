"""STRETCH crossing polygons: real contour, provenance, prompts and clicks."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from pytestqt.qtbot import QtBot
from test_modify_commands import Harness
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadBounds2, CadLine
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.overlay import Stroke

POLYGON = ((9.0, -1.0), (11.0, -1.0), (11.0, 6.0), (10.5, 6.0), (10.5, 1.0), (9.0, 1.0))


def test_cp_stretches_the_actual_contour_and_undo_removes_one_step() -> None:
    h = Harness()
    h.run("STRETCH", "CP", *(f"{x},{y}" for x, y in POLYGON), "")
    assert h.runner.active == "STRETCH"
    assert h.session.crossing_polygons(h.document) == [(h.a, POLYGON)]
    h.run("", "0,0", "2,3")
    assert h.line(h.a) == ((0, 0), (12, 3))
    assert h.line(h.b) == ((0, 5), (10, 5))
    assert h.session.crossing_polygons(h.document) == []
    assert h.document.undo_history[-1] == "STRETCH"
    h.run("UNDO")
    assert h.line(h.a) == ((0, 0), (10, 0))
    h.run("REDO")
    assert h.line(h.a) == ((0, 0), (12, 3))


def test_polygon_and_rectangle_provenance_combines_but_whole_selection_wins() -> None:
    h = Harness()
    # Exercise the actual CP query before recording its validated provenance.
    assert h.document.select_crossing_polygon(POLYGON) == [h.a]
    h.session.select(h.document, (h.a,), polygon=POLYGON)
    h.session.select(h.document, (h.a,), crossing=CadBounds2((9, -1), (11, 1)))
    assert h.session.crossing_polygons(h.document) == [(h.a, POLYGON)]
    h.run("STRETCH", "0,0", "2,3")
    assert h.line(h.a) == ((0, 0), (12, 3)), "overlapping areas do not double the offset"
    h.run("UNDO")
    other = ((-1.0, -1.0), (1.0, -1.0), (0.0, 1.0))
    assert h.document.select_crossing_polygon(other) == [h.a]
    h.session.select(h.document, (h.a,), polygon=POLYGON)
    h.session.select(h.document, (h.a,), polygon=other)
    assert len(h.session.crossing_polygons(h.document)) == 2
    h.run("STRETCH", "0,0", "2,3")
    assert h.line(h.a) == ((2, 3), (12, 3))
    h.run("UNDO")
    h.session.select(h.document, (h.a,), polygon=POLYGON)
    h.session.select(h.document, (h.a,))
    assert h.session.crossing_polygons(h.document) == []
    h.session.select(h.document, (h.a,), polygon=POLYGON)
    assert h.session.crossing_polygons(h.document) == [], "a whole selection is not downgraded"
    h.run("STRETCH", "0,0", "2,3")
    assert h.line(h.a) == ((2, 3), (12, 3))
    h.run("UNDO")
    h.session.select(h.document, (h.a,), polygon=POLYGON)
    h.session.deselect(h.document, (h.a,))
    assert h.session.crossing_polygons(h.document) == []


@pytest.mark.parametrize("stage", ["first", "vertices", "base", "offset"])
def test_cp_can_be_cancelled_at_each_stage_without_an_edit(stage: str) -> None:
    h = Harness()
    before = h.document.undo_history
    h.run("S", "CP")
    assert h.runner.active == "STRETCH" and h.prompt().text == "Specify first polygon point:"
    if stage != "first":
        h.run("9,-1", "11,-1", "11,1", "9,1")
        assert h.prompt().text.startswith("Specify next polygon point")
    if stage in ("base", "offset"):
        h.run("", "")
        assert h.prompt().text.startswith("Specify base point")
    if stage == "offset":
        h.run("0,0")
        assert h.prompt().text.startswith("Specify second point")
    h.runner.cancel()
    assert h.line(h.a) == ((0, 0), (10, 0))
    assert h.document.undo_history == before
    h.session.clear_selection(h.document)
    assert h.session.crossing_polygons(h.document) == []


def test_invalid_polygon_changes_no_selection_and_undo_removes_last_vertex() -> None:
    h = Harness()
    h.run("S", "CP", "0,0", "2,2", "0,2", "2,0", "")
    assert h.selection() == ()
    assert h.messages()[-1].startswith("Invalid crossing polygon:")
    assert (
        h.runner.active == "STRETCH"
        and h.runner.prompt is not None
        and h.runner.prompt.kind == "select"
    )
    h.run("CP", "9,-1", "11,-1", "99,99", "U", "11,1", "9,1", "", "", "0,0", "2,3")
    assert h.line(h.a) == ((0, 0), (12, 3))
    assert h.line(h.b) == ((0, 5), (10, 5))


def test_cp_cursor_points_use_translated_prompts_and_existing_viewport_dispatch(
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
    window.session.drafting.set_osnap(False)
    window.change_language("es")
    window.runner.execute("ESTIRA")
    assert window.runner.prompt is not None
    assert "Polígono de cruce(CP)" in window.runner.prompt.shown()
    window.runner.execute("CP")
    assert window.runner.prompt is not None
    assert window.runner.prompt.message == "Precise el primer punto del polígono"
    for point in ((90, -10), (110, -10), (110, 10), (90, 10)):
        mouse(viewport, QEvent.Type.MouseButtonPress, point)
    outlines = [
        item
        for item in viewport.overlay().items
        if isinstance(item, Stroke) and item.closed and item.dashed
    ]
    assert any(len(item.points) >= 3 for item in outlines)
    for answer in ("", "", "0,0", "10,0"):
        window.runner.execute(answer)
    document = window.session.active
    assert document is not None and document.undo_history[-1] == "STRETCH"
    lines = [e.geometry for e in document.get_entities() if isinstance(e.geometry, CadLine)]
    assert len(lines) == 1
    assert lines[0].start == (0, 0) and lines[0].end == (110, 0)
    assert not any(
        isinstance(item, Stroke) and item.closed and item.dashed
        for item in viewport.overlay().items
    )
    set_language("en")
