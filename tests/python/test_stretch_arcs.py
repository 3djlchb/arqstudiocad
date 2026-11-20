"""Partial ARC STRETCH: measured behavior, command errors and transient contours."""

import math
from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from pytestqt.qtbot import QtBot
from test_modify_commands import Harness
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.core import (
    CadApplication,
    CadArc,
    CadBounds2,
    CadColor,
    CadView,
    InvalidGeometryError,
)
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.overlay import Stroke


@pytest.mark.parametrize(
    "sweep,start,cx,cy,radius",
    [
        (90, False, 0.3542858741, 2.4583673926, 5.2560660172),
        (90, True, 2.0692751777, -1.6485995261, 6.9631727984),
        (180, False, 1.8227973253, 2.1297906385, 3.825),
        (180, True, 1.7775043613, -0.8037783482, 6.825),
        (270, False, 0.6074094784, 0.7617270144, 4.4581475452),
        (270, True, 2.2793799644, 0.4664228978, 5.9226136393),
    ],
)
def test_native_measured_arc_binding_preview_history_and_safe_save(
    tmp_path: Path, sweep: int, start: bool, cx: float, cy: float, radius: float
) -> None:
    app = CadApplication()
    doc = app.create_document("arc stretch")
    source, _ = doc.add_arc((0, 0), 5, 0, math.radians(sweep))
    doc.set_entity_color([source], CadColor.from_index(3))
    original = doc.get_entity(source)
    geometry = original.geometry
    assert isinstance(geometry, CadArc)
    point = geometry.start_point if start else geometry.end_point
    box = CadBounds2((point[0] - 1, point[1] - 1), (point[0] + 1, point[1] + 1))
    polygon = [
        (point[0] - 1, point[1] - 1),
        (point[0] + 1, point[1] - 1),
        (point[0] + 1, point[1] + 1),
        (point[0] - 1, point[1] + 1),
    ]
    view = CadView(doc, 800, 600)
    history = doc.undo_history
    shown = view.stretch_outlines([source], (3, 2), [(source, box)], [(source, polygon)])
    assert len(shown) == 1
    assert doc.undo_history == history
    assert doc.get_entity(source).geometry == geometry
    changes = doc.stretch_entities([source], (3, 2), [(source, box)], [(source, polygon)])
    assert changes.modified == (source,)
    after = doc.get_entity(source)
    result = after.geometry
    assert isinstance(result, CadArc)
    assert result.center == pytest.approx((cx, cy), rel=0, abs=1e-10)
    assert result.radius == pytest.approx(radius, rel=0, abs=1e-10)
    assert after.elevation == 0 and after.color == original.color and after.layer == original.layer
    assert len(doc.undo_history) == len(history) + 1 and doc.undo_history[-1] == "STRETCH"
    view.update(changes)
    outlines = view.outlines([source])
    assert len(outlines) == len(shown)
    for actual, expected in zip(outlines[0], shown[0], strict=True):
        assert actual == pytest.approx(expected)
    doc.undo()
    assert doc.get_entity(source).geometry == geometry
    doc.redo()
    assert doc.get_entity(source).geometry == result
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"arc.{suffix}"
        app.save_document(doc.id, path)
        back = app.open_document(path).get_entity(source)
        reopened = back.geometry
        assert isinstance(reopened, CadArc)
        assert reopened.center == pytest.approx(result.center, rel=0, abs=1e-9)
        assert reopened.radius == pytest.approx(result.radius, rel=0, abs=1e-9)
        assert reopened.start_point == pytest.approx(result.start_point, rel=0, abs=1e-9)
        assert reopened.end_point == pytest.approx(result.end_point, rel=0, abs=1e-9)
        assert reopened.start_angle == pytest.approx(result.start_angle, rel=0, abs=1e-12)
        assert reopened.end_angle == pytest.approx(result.end_angle, rel=0, abs=1e-12)
        assert back.elevation == 0 and back.color == original.color


def test_partial_arc_collapse_refuses_preview_and_rolls_back_command_in_spanish() -> None:
    h = Harness()
    source, _ = h.document.add_arc((0, 0), 5, 0, math.pi / 2)
    original = h.document.get_entity(source).geometry
    assert isinstance(original, CadArc)
    area = CadBounds2((-1, 4), (1, 6))
    before = h.document.undo_history
    view = CadView(h.document, 800, 600)
    with pytest.raises(InvalidGeometryError):
        view.stretch_outlines([source], (5, -5), [(source, area)], [])
    with pytest.raises(InvalidGeometryError):
        h.document.stretch_entities([h.b, source], (5, -5), [(source, area)])
    assert h.document.undo_history == before
    assert h.line(h.b) == ((0, 5), (10, 5))
    h.session.select(h.document, [source], crossing=area)
    h.select(h.b)
    set_language("es")
    h.run("ESTIRA", "0,0", "5,-5")
    assert h.runner.active is None
    assert h.document.undo_history == before
    assert h.document.get_entity(source).geometry == original
    assert h.line(h.b) == ((0, 5), (10, 5))
    assert (
        h.messages()[-1] == "ESTIRA no puede hacer coincidir extremos ni crear geometría inválida. "
        "No se modificó ningún objeto."
    )


def test_partial_arc_command_success_and_whole_translation_controls() -> None:
    h = Harness()
    source, _ = h.document.add_arc((0, 0), 5, 0, math.pi / 2)
    original = h.document.get_entity(source).geometry
    h.run("STRETCH", "C", "-1,4", "1,6", "", "0,0", "3,2")
    result = h.document.get_entity(source).geometry
    assert isinstance(result, CadArc)
    assert result.center == pytest.approx((0.3542858741, 2.4583673926), rel=0, abs=1e-10)
    h.run("UNDO")
    assert h.document.get_entity(source).geometry == original
    for area in (None, CadBounds2((-1, -1), (6, 6))):
        h.session.select(h.document, [source], crossing=area)
        h.run("STRETCH", "0,0", "3,2")
        moved = h.document.get_entity(source).geometry
        assert isinstance(moved, CadArc)
        assert moved.center == pytest.approx((3, 2)) and moved.radius == 5
        assert moved.start_angle == pytest.approx(0) and moved.end_angle == pytest.approx(
            math.pi / 2
        )
        h.run("UNDO")


def test_partial_arc_cursor_overlay_is_read_only_and_clears_on_cancel(
    qtbot: QtBot, tmp_path: Path
) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(1000, 700)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)
    doc = window.session.active
    assert doc is not None
    source, changes = doc.add_arc((0, 0), 50, 0, math.pi / 2)
    window.session.report(doc, changes)
    viewport.view.zoom_extents()
    original = doc.get_entity(source).geometry
    window.session.drafting.set_osnap(False)
    area = CadBounds2((-1, 49), (1, 51))
    window.session.select(doc, [source], crossing=area)
    history = doc.undo_history
    window.runner.execute("STRETCH")
    window.runner.execute("0,0")
    mouse(viewport, QEvent.Type.MouseMove, (5, 3))
    target = viewport.target
    assert target is not None
    contour = viewport.view.stretch_outlines([source], target, [(source, area)], [])[0]
    ratio = viewport.devicePixelRatioF()
    expected = tuple((x / ratio, y / ratio) for x, y in contour)
    assert len(expected) > 2
    assert any(
        isinstance(item, Stroke) and item.points == expected for item in viewport.overlay().items
    )
    assert doc.undo_history == history and doc.get_entity(source).geometry == original
    window.runner.cancel()
    assert viewport.preview is None
    assert not any(
        isinstance(item, Stroke) and item.points == expected for item in viewport.overlay().items
    )
    assert doc.undo_history == history
