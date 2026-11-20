"""Read-only native STRETCH outlines and the actual cursor overlay."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadBounds2, CadError, CadLine, CadView
from arqstudiocad.events import Drag
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.overlay import Overlay, Stroke
from arqstudiocad.ui.viewport import Viewport


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
    view.view = "Front"
    with pytest.raises(CadError, match="Top view"):
        view.stretch_outlines([source], (2, 3), crossing, polygons)


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


def _comb(xs: list[float]) -> list[tuple[float, float]]:
    """A crossing polygon with a 0.2-wide tooth around each x on y = 0,
    rising from a base below it."""
    points: list[tuple[float, float]] = []
    for x in xs:
        points += [(x - 0.1, -0.2), (x - 0.1, 0.2), (x + 0.1, 0.2), (x + 0.1, -0.2)]
    return [*points, (xs[-1] + 0.2, -0.4), (xs[0] - 0.2, -0.4)]


def test_a_crossing_polygon_shared_by_many_entities_stretches_each_by_its_own() -> None:
    app = CadApplication()
    doc = app.create_document("shared polygon")
    ids = [doc.add_line((3 * k, 0), (3 * k + 1, 0))[0] for k in range(12)]
    view = CadView(doc, 800, 600)
    view.zoom_extents()
    ends = _comb([3 * k + 1 for k in range(6)])  # the first six lines' ends
    starts = _comb([3 * k for k in range(6, 12)])  # the last six lines' starts
    polygons = [(e, ends) for e in ids[:6]] + [(e, starts) for e in ids[6:]]
    boxes = [
        (e, CadBounds2((x - 0.1, -0.1), (x + 0.1, 0.1)))
        for k, e in enumerate(ids)
        for x in [3 * k + 1 if k < 6 else 3 * k]
    ]
    history = doc.undo_history
    shown = view.stretch_outlines(ids, (2, 3), [], polygons)
    assert len(shown) == 12
    for got, want in zip(shown, view.stretch_outlines(ids, (2, 3), boxes, []), strict=True):
        assert got == pytest.approx(want)
    assert shown[0][-1] == pytest.approx(view.to_screen(3, 3)), "the first line's end"
    assert shown[11][0] == pytest.approx(view.to_screen(35, 3)), "the last line's start"
    crossed: list[tuple[float, float]] = [(0, 0), (2, 2), (0, 2), (2, 0)]
    bad = [(ids[-1], crossed)]  # crosses itself
    with pytest.raises(CadError):
        view.stretch_outlines(ids, (2, 3), [], polygons[:-1] + bad)
    with pytest.raises(CadError):
        doc.stretch_entities(ids, (2, 3), [], polygons[:-1] + bad)
    assert doc.undo_history == history
    doc.stretch_entities(ids, (2, 3), [], polygons)
    for k, e in enumerate(ids):
        line = doc.get_entity(e).geometry
        assert isinstance(line, CadLine)
        if k < 6:
            assert (line.start, line.end) == ((3 * k, 0), (3 * k + 3, 3))
        else:
            assert (line.start, line.end) == ((3 * k + 2, 3), (3 * k + 1, 0))
    assert len(doc.undo_history) == len(history) + 1


@pytest.mark.parametrize("ratio", [1.0, 2.0])
def test_outlines_and_previews_come_in_logical_pixels_at_any_display_ratio(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch, ratio: float
) -> None:
    document = CadApplication().create_document("ratio")
    source, _ = document.add_polyline([((0, 0), 0.5), ((5, 2), -0.5), ((10, 0), 0.0)])
    viewport = Viewport(document)
    qtbot.addWidget(viewport)
    viewport.view.resize(round(400 * ratio), round(300 * ratio))
    viewport.view.zoom_extents()
    monkeypatch.setattr(viewport, "devicePixelRatioF", lambda: ratio)
    physical = viewport.view.outlines([source])
    out = Overlay()
    viewport._outline_strokes(out, (source,), (0, 0, 0, 255), 1.0)
    assert [s.points for s in out.items if isinstance(s, Stroke)] == [
        tuple((x / ratio, y / ratio) for x, y in path) for path in physical
    ]
    area = [(source, CadBounds2((9, -1), (11, 1)))]
    viewport._dragged = Drag("stretch", (source,), crossing=tuple(area))
    viewport._base, viewport._target = (0.0, 0.0), (2.0, 3.0)
    preview = Overlay()
    viewport._stretch_items(preview, (0, 0, 0, 255))
    shown = viewport.view.stretch_outlines([source], (2, 3), area, [])
    assert [s.points for s in preview.items if isinstance(s, Stroke)] == [
        tuple((x / ratio, y / ratio) for x, y in path) for path in shown
    ]
    assert len(preview.items) == len(shown) > 0
