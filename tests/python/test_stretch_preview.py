"""Read-only native STRETCH outlines and the actual cursor overlay."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadBounds2, CadError, CadLine, CadView
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.overlay import Stroke


def test_stretch_outlines_are_exact_read_only_and_validate_inputs() -> None:
    app = CadApplication()
    doc = app.create_document("preview")
    source, _ = doc.add_line((0, 0), (10, 0))
    view = CadView(doc, 800, 600)
    before = doc.undo_history
    crossing = [(source, CadBounds2((9, -1), (11, 1)))]
    polygons = [(source, [(9, -1), (11, -1), (11, 1), (9, 1)])]
    shown = view.stretch_outlines([source], (2, 3), crossing, polygons)
    assert len(shown) == 1
    assert shown[0][0] == pytest.approx(view.to_screen(0, 0))
    assert shown[0][-1] == pytest.approx(view.to_screen(12, 3))
    assert doc.undo_history == before
    assert view.outlines([source])[0][-1] == pytest.approx(view.to_screen(10, 0))
    with pytest.raises(CadError):
        view.stretch_outlines([source], (float("nan"), 0), crossing, [])
    with pytest.raises(CadError):
        view.stretch_outlines([source], (2, 3), [], [(source, [(0, 0), (1, 1)])])
    doc.stretch_entities([source], (2, 3), crossing, polygons)
    geometry = doc.get_entity(source).geometry
    assert isinstance(geometry, CadLine)
    assert geometry.end == pytest.approx((12, 3))


@pytest.mark.parametrize("displacement", [False, True])
def test_stretch_cursor_previews_partial_geometry_and_clears_on_cancel(
    qtbot: QtBot, tmp_path: Path, displacement: bool
) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    viewport = plan(window, tmp_path)
    document = window.session.active
    assert document is not None
    source = document.get_entities()[0].id
    window.session.drafting.set_osnap(False)
    window.session.select(document, [source], crossing=CadBounds2((99, -1), (101, 1)))
    before = document.undo_history
    window.runner.execute("STRETCH")
    window.runner.execute("D" if displacement else "0,0")
    assert viewport.preview is None
    mouse(viewport, QEvent.Type.MouseMove, (5, 3))
    target = viewport.target
    assert target is not None
    shown = viewport.view.stretch_outlines(
        [source], target, [(source, CadBounds2((99, -1), (101, 1)))], []
    )[0]
    strokes = [
        item for item in viewport.overlay().items if isinstance(item, Stroke) and item.width == 1.0
    ]
    ratio = viewport.devicePixelRatioF()
    expected = tuple((x / ratio, y / ratio) for x, y in shown)
    assert any(s.points == expected for s in strokes)
    assert shown[0] == pytest.approx(viewport.view.to_screen(0, 0))
    assert shown[-1] == pytest.approx(viewport.view.to_screen(100 + target[0], target[1]))
    assert document.undo_history == before
    window.runner.cancel()
    assert viewport.preview is None
    assert not any(isinstance(s, Stroke) and s.points == expected for s in viewport.overlay().items)
    assert document.undo_history == before
