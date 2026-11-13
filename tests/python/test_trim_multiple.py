"""AutoCAD 2021 multi-object TRIM/EXTEND, exercised without external CAD."""

import pytest
from PySide6.QtCore import QEvent, Qt
from pytestqt.qtbot import QtBot
from test_selection import click, mouse
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad.core import CadLine, InvalidGeometryError
from arqstudiocad.ui.main_window import MainWindow


def lines() -> tuple[Harness, list[int]]:
    h = Harness()
    ids = [h.line((0, y), (10, y)) for y in (0, 5, 10)]
    h.line((5, -5), (5, 15))
    return h, ids


def ends(h: Harness, entity_id: int) -> tuple[float, float]:
    g = h.document.get_entity(entity_id).geometry
    assert isinstance(g, CadLine)
    return g.end


def test_typed_fence_trims_multiple_entities_and_undo_stays_in_the_command() -> None:
    h, ids = lines()
    h.run("TRIM", "F", "8,-2", "8,12", "")
    assert [ends(h, i) for i in ids] == [(5, 0), (5, 5), (5, 10)]
    assert h.runner.active == "TRIM"
    h.run("U")
    assert ends(h, ids[-1]) == (10, 10)
    assert ends(h, ids[0]) == (5, 0)


def test_crossing_and_standard_mode_reuse_selected_cutting_edges() -> None:
    h, ids = lines()
    boundary = h.document.get_entities()[-1].id
    h.run("TRIM", "O", "S")
    h.session.select(h.document, [boundary])
    h.run("", "C", "7,-2", "9,12")
    assert [ends(h, i) for i in ids] == [(5, 0), (5, 5), (5, 10)]


def test_shift_fence_extends_and_extend_command_trims_with_shift() -> None:
    h = Harness()
    ids = [h.line((0, y), (3, y)) for y in (0, 5)]
    h.line((5, -5), (5, 10))
    h.run("TRIM")
    h.runner.answer_path(((2, -2), (2, 7)), shift=True)
    assert [ends(h, i) for i in ids] == [(5, 0), (5, 5)]
    h.run("")
    h.line((2, -5), (2, 10))
    h.run("EXTEND")
    h.runner.answer_path(((4, -2), (4, 7)), shift=True)
    assert [ends(h, i) for i in ids] == [(2, 0), (2, 5)]


def test_locked_and_nonintersecting_targets_do_not_stop_the_rest() -> None:
    h, ids = lines()
    layer, _ = h.document.create_layer("Locked")
    h.document.set_entity_layer([ids[1]], layer)
    h.document.set_layer_locked(layer, True)
    h.run("TRIM")
    h.runner.answer_path(((8, -2), (8, 12)))
    assert [ends(h, i) for i in ids] == [(5, 0), (10, 5), (5, 10)]
    assert "1 were on a locked layer." in h.messages()


def test_fence_query_uses_exact_curves_and_filters_hidden_layers() -> None:
    h = Harness()
    circle, _ = h.document.add_circle((0, 0), 2)
    assert h.document.fence_picks(((1.8, 1.8), (2.1, 2.1))) == []
    hit = h.document.fence_picks(((-3, 0), (3, 0)))
    assert hit == [(circle, (-2, 0))]
    layer, _ = h.document.create_layer("Hidden")
    h.document.set_entity_layer([circle], layer)
    h.document.set_layer_visible(layer, False)
    assert h.document.fence_picks(((-3, 0), (3, 0))) == []
    with pytest.raises(InvalidGeometryError):
        h.document.fence_picks(((float("nan"), 0), (1, 1)))


def test_fence_once_per_object_even_when_path_returns() -> None:
    h, ids = lines()
    h.run("TRIM")
    h.runner.answer_path(((8, -2), (8, 12), (8, -2)))
    assert [ends(h, i) for i in ids] == [(5, 0), (5, 5), (5, 10)]
    assert len(h.document.undo_history) == 7


@pytest.fixture
def drawing(qtbot: QtBot) -> tuple[MainWindow, list[int]]:
    w = MainWindow(ui=QuietUi())
    qtbot.addWidget(w)
    w.resize(1000, 700)
    w.show()
    qtbot.waitExposed(w)
    doc = w.session.new_document()
    ids = []
    for y in (0, 5, 10):
        entity_id, changes = doc.add_line((0, y), (10, y))
        w.session.report(doc, changes)
        ids.append(entity_id)
    _, changes = doc.add_line((5, -5), (5, 15))
    w.session.report(doc, changes)
    # Mutations are reported so the viewport updates its camera.
    w.viewports[doc.id].zoom_extents()
    w.session.drafting.set_osnap(False)
    return w, ids


def test_dragging_in_empty_space_trims_all_crossed_entities(
    drawing: tuple[MainWindow, list[int]],
) -> None:
    w, ids = drawing
    doc = w.session.active
    assert doc is not None
    viewport = w.viewports[doc.id]
    w.runner.execute("TRIM")
    before = doc.undo_history
    mouse(viewport, QEvent.Type.MouseButtonPress, (8, -2))
    mouse(viewport, QEvent.Type.MouseMove, (8, 12))
    assert doc.undo_history == before
    assert viewport._entity_path
    assert viewport.overlay().items
    mouse(viewport, QEvent.Type.MouseButtonRelease, (8, 12), held=Qt.MouseButton.NoButton)
    for entity_id, y in zip(ids, (0, 5, 10), strict=True):
        assert doc.get_entity(entity_id).geometry == CadLine((0, y), (5, y))
    assert w.runner.active == "TRIM"
    assert not viewport._entity_path and viewport._fence_anchor is None


def test_two_empty_clicks_make_a_fence_and_escape_discards_it(
    drawing: tuple[MainWindow, list[int]],
) -> None:
    w, ids = drawing
    doc = w.session.active
    assert doc is not None
    viewport = w.viewports[doc.id]
    w.runner.execute("TRIM")
    click(viewport, (8, -2))
    assert viewport._fence_anchor is not None
    click(viewport, (8, 12))
    assert doc.get_entity(ids[0]).geometry == CadLine((0, 0), (5, 0))
    click(viewport, (3, -2))
    before = doc.undo_history
    w.runner.cancel()
    assert viewport._fence_anchor is None
    assert doc.undo_history == before


def test_shift_drag_extends_multiple_entities(drawing: tuple[MainWindow, list[int]]) -> None:
    w, ids = drawing
    doc = w.session.active
    assert doc is not None
    _, changes = doc.add_line((15, -5), (15, 15))
    w.session.report(doc, changes)
    viewport = w.viewports[doc.id]
    viewport.zoom_extents()
    w.runner.execute("TRIM")
    shift = Qt.KeyboardModifier.ShiftModifier
    mouse(viewport, QEvent.Type.MouseButtonPress, (8, -2), modifiers=shift)
    mouse(viewport, QEvent.Type.MouseMove, (8, 12), modifiers=shift)
    mouse(
        viewport,
        QEvent.Type.MouseButtonRelease,
        (8, 12),
        held=Qt.MouseButton.NoButton,
        modifiers=shift,
    )
    for entity_id, y in zip(ids, (0, 5, 10), strict=True):
        assert doc.get_entity(entity_id).geometry == CadLine((0, y), (15, y))


def test_fence_picks_the_curved_segment_of_a_polyline() -> None:
    h = Harness()
    poly, _ = h.document.add_polyline([((0, 0), 1), ((10, 0), 0)], False)
    hits = h.document.fence_picks(((5, -6), (5, -4)))
    assert len(hits) == 1 and hits[0][0] == poly
    assert hits[0][1] == pytest.approx((5, -5))


def test_crossing_resolves_the_first_hit_clockwise_from_the_first_corner() -> None:
    import math

    h = Harness()
    circle, _ = h.document.add_circle((0, 0), 2)
    hits = h.document.fence_picks(((-3, -1), (3, 1)), crossing=True)
    assert hits[0][0] == circle
    assert hits[0][1] == pytest.approx((-math.sqrt(3), 1))


def test_other_commands_do_not_accept_bulk_entity_input() -> None:
    h, ids = lines()
    before = h.document.undo_history
    h.run("OFFSET", "1")
    assert h.runner.prompt is not None and not h.runner.prompt.multiple
    h.runner.answer_path(((8, -2), (8, 12)))
    assert h.document.undo_history == before
    assert ends(h, ids[0]) == (10, 0)


def test_cancel_during_drag_leaves_every_entity_unchanged(
    drawing: tuple[MainWindow, list[int]],
) -> None:
    w, ids = drawing
    doc = w.session.active
    assert doc is not None
    viewport = w.viewports[doc.id]
    w.runner.execute("TRIM")
    before = doc.undo_history
    mouse(viewport, QEvent.Type.MouseButtonPress, (8, -2))
    mouse(viewport, QEvent.Type.MouseMove, (8, 12))
    w.runner.cancel()
    mouse(viewport, QEvent.Type.MouseButtonRelease, (8, 12), held=Qt.MouseButton.NoButton)
    assert doc.undo_history == before
    assert doc.get_entity(ids[0]).geometry == CadLine((0, 0), (10, 0))
    assert not viewport._entity_path and viewport._fence_anchor is None
