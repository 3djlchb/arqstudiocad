"""Bounded native ARRAYRECT contours and the real shared cursor overlay."""

from math import atan2, pi
from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadError, CadView
from arqstudiocad.events import Drag
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.overlay import Stroke
from arqstudiocad.ui.viewport import Viewport


def paths(viewport: Viewport) -> list[tuple[tuple[float, float], ...]]:
    return [
        item.points
        for item in viewport.overlay().items
        if isinstance(item, Stroke) and item.width == 1 and not item.dashed
    ]


def test_array_preview_native_is_read_only_deduplicated_and_matches_block_polyline_commit() -> None:
    app = CadApplication()
    doc = app.create_document("preview")
    source, _ = doc.add_line((0, 0), (2, 0))
    block, _ = doc.create_block("cell", (0, 0), [source])
    poly, _ = doc.add_polyline([((0, 0), 1), ((2, 0), 0), ((2, 3), 0)], False)
    view = CadView(doc, 800, 600)
    before, originals = doc.undo_history, view.outlines([block, poly])
    shown, count, total = view.rectangular_array_outlines(
        [block, poly, block], 2, 2, -5, 10, pi / 2
    )
    assert (count, total) == (3, 3)
    assert len(doc.get_entities()) == 2 and doc.undo_history == before
    assert view.outlines([block, poly]) == originals
    copies, changes = doc.rectangular_array([block, poly], 2, 2, -5, 10, pi / 2)
    view.update(changes)
    committed = view.outlines(copies)
    assert len(shown) == len(committed) > 0
    for got, want in zip(shown, committed, strict=True):
        for actual, expected in zip(got, want, strict=True):
            assert actual == pytest.approx(expected, abs=1e-7)
    assert doc.undo_history[-1] == "ARRAYRECT"
    doc.undo()
    assert len(doc.get_entities()) == 2


def test_array_preview_native_rejects_errors_and_unseen_overflow_without_edits() -> None:
    doc = CadApplication().create_document("preview")
    source, _ = doc.add_line((0, 0), (2, 0))
    view = CadView(doc, 800, 600)
    before = doc.undo_history
    for ids, rows, cols, spacing, angle in [
        ([source], 0, 2, 1, 0),
        ([999999], 1, 2, 1, 0),
        ([source], 1, 100000, 1e308, 0),
        ([source], 1, 2, 1, float("nan")),
    ]:
        with pytest.raises(CadError):
            view.rectangular_array_outlines(ids, rows, cols, 0, spacing, angle)
    assert doc.undo_history == before and len(doc.get_entities()) == 1
    view.view = "Front"
    with pytest.raises(CadError, match="Top view"):
        view.rectangular_array_outlines([source], 1, 2, 0, 1, 0)


@pytest.mark.parametrize("finish", ["cancel", "click"])
def test_array_preview_default_cursor_cancel_or_commit_leaves_no_ghost(
    qtbot: QtBot, tmp_path: Path, finish: str
) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    viewport = plan(window, tmp_path)
    doc = window.session.active
    assert doc is not None
    source = doc.get_entities()[0].id
    window.session.select(doc, [source])
    window.session.drafting.set_osnap(False)
    window.runner.context.remembered["ARRAY_ANGLE"] = pi / 2
    before = doc.undo_history
    for answer in ("ARRAYRECT", "2", "2", "5", "10"):
        window.runner.execute(answer)
    assert viewport.preview is None
    # Initial preview uses the remembered angle before a mouse move.
    initial, shown, total = viewport.view.rectangular_array_outlines([source], 2, 2, 5, 10, pi / 2)
    assert shown == total == 3
    assert initial[0][0] == pytest.approx(viewport.view.to_screen(0, 10))
    assert initial[0][-1] == pytest.approx(viewport.view.to_screen(100, 10))
    assert tuple(
        (x / viewport.devicePixelRatioF(), y / viewport.devicePixelRatioF()) for x, y in initial[0]
    ) in paths(viewport)
    assert doc.undo_history == before and len(doc.get_entities()) == 2
    mouse(viewport, QEvent.Type.MouseMove, (15, 30))
    target = viewport.target
    assert target is not None
    angle = atan2(target[1], target[0])
    displayed, _, _ = viewport.view.rectangular_array_outlines([source], 2, 2, 5, 10, angle)
    assert tuple(
        (x / viewport.devicePixelRatioF(), y / viewport.devicePixelRatioF())
        for x, y in displayed[0]
    ) in paths(viewport)
    if finish == "cancel":
        window.runner.cancel()
        assert doc.undo_history == before and len(doc.get_entities()) == 2
        assert window.runner.context.remembered["ARRAY_ANGLE"] == pi / 2
    else:
        mouse(viewport, QEvent.Type.MouseButtonPress, (15, 30))
        mouse(viewport, QEvent.Type.MouseButtonRelease, (15, 30))
        assert window.runner.active is None
        assert len(doc.get_entities()) == 5
        assert len(doc.undo_history) == len(before) + 1
        copies = doc.get_entities()[2:]
        committed = viewport.view.outlines([entity.id for entity in copies])
        for got, want in zip(displayed, committed, strict=True):
            for actual, expected in zip(got, want, strict=True):
                assert actual == pytest.approx(expected, abs=1e-7)
    assert tuple(
        (x / viewport.devicePixelRatioF(), y / viewport.devicePixelRatioF())
        for x, y in displayed[0]
    ) not in paths(viewport)


@pytest.mark.parametrize("ratio", [1.0, 2.0])
def test_array_preview_uses_logical_pixels_at_any_display_ratio(
    qtbot: QtBot, monkeypatch: pytest.MonkeyPatch, ratio: float
) -> None:
    doc = CadApplication().create_document("ratio")
    source, _ = doc.add_line((0, 0), (2, 0))
    viewport = Viewport(doc)
    qtbot.addWidget(viewport)
    viewport.view.resize(round(400 * ratio), round(300 * ratio))
    viewport.view.zoom_extents()
    monkeypatch.setattr(viewport, "devicePixelRatioF", lambda: ratio)
    viewport.ask_point((0, 0), Drag("array_rect", (source,), rotation=pi / 2, array=(2, 2, 5, 10)))
    native, _, _ = viewport.view.rectangular_array_outlines([source], 2, 2, 5, 10, pi / 2)
    for path in native:
        assert tuple((x / ratio, y / ratio) for x, y in path) in paths(viewport)
    viewport.end_point()
    assert not any(
        tuple((x / ratio, y / ratio) for x, y in path) in paths(viewport) for path in native
    )


def test_array_preview_partial_status_is_visible_translated_and_does_not_limit_commit(
    qtbot: QtBot, tmp_path: Path
) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    viewport = plan(window, tmp_path)
    doc = window.session.active
    assert doc is not None
    source = doc.get_entities()[0].id
    window.session.select(doc, [source])
    before = doc.undo_history
    for answer in ("ARRAYRECT", "1", "1002", "1"):
        window.runner.execute(answer)
    assert "Partial preview: 1000 of 1001 copied cells." in [
        label.text for label in viewport.overlay().labels
    ]
    window.change_language("es")
    assert "Vista previa parcial: 1000 de 1001 celdas copiadas." in [
        label.text for label in viewport.overlay().labels
    ]
    assert doc.undo_history == before
    window.runner.execute("0")
    assert window.runner.active is None
    assert len(doc.get_entities()) == 1003
    assert not any("previa parcial" in label.text for label in viewport.overlay().labels)
