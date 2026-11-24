"""ELLIPSE (docs/design/ellipse-command.md): an axis and the other's
distance, Center, Rotation, the elliptical Arc with angles, Included angle
or Parameters; cancelling; the preview following the cursor; the shared
action in Spanish."""

import math
from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad import ellipses
from arqstudiocad.core import CadEllipse
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget


def drawn(h: Harness) -> tuple[tuple[float, float], tuple[float, float]]:
    """The box of the one ellipse drawn, rounded."""
    found = [e for e in h.document.get_entities() if e.kind == "ELLIPSE"]
    assert len(found) == 1, found
    bounds = found[0].bounds
    assert bounds is not None
    r = round
    return (r(bounds.min[0], 6), r(bounds.min[1], 6)), (r(bounds.max[0], 6), r(bounds.max[1], 6))


def test_an_axis_and_the_distance_to_the_other() -> None:
    h = Harness()
    h.run("EL")
    assert h.prompt().text == "Specify axis endpoint of ellipse [Arc(A)/Center(C)]:"
    h.run("0,0", "10,0")
    assert h.prompt().text == "Specify distance to other axis [Rotation(R)]:"
    h.run("3")
    assert h.runner.active is None
    assert drawn(h) == ((0.0, -3.0), (10.0, 3.0))
    assert h.document.undo_history[-1] == "ELLIPSE"
    h.run("UNDO")
    assert not [e for e in h.document.get_entities() if e.kind == "ELLIPSE"]


def test_by_its_center_and_the_longer_axis_made_the_major() -> None:
    h = Harness()
    h.run("ELLIPSE", "C", "5,0", "7,0", "4")
    assert drawn(h) == ((3.0, -4.0), (7.0, 4.0))


def test_by_a_rotation_about_the_major_axis() -> None:
    h = Harness()
    h.run("ELLIPSE", "0,0", "10,0", "R", "60")
    assert drawn(h) == ((0.0, -2.5), (10.0, 2.5)), "cos 60° of 5"


def test_an_elliptical_arc_from_the_first_axis_drawn() -> None:
    # The first axis drawn points from the center (5,0) to (0,0): angle 0
    # there, counterclockwise. To 90°: down to (5,-3).
    h = Harness()
    h.run("ELLIPSE", "A", "0,0", "10,0", "3", "0", "90")
    assert drawn(h) == ((0.0, -3.0), (5.0, 0.0))
    # An included angle of 180° from 0: the lower half.
    h = Harness()
    h.run("ELLIPSE", "A", "0,0", "10,0", "3", "0", "I", "180")
    assert drawn(h) == ((0.0, -3.0), (10.0, 0.0))
    # By parameters: a quarter of the parameter is the minor axis's end too.
    h = Harness()
    h.run("ELLIPSE", "A", "0,0", "10,0", "3", "P", "0", "90")
    assert drawn(h) == ((0.0, -3.0), (5.0, 0.0))


def test_cancelling_or_no_ellipse_draws_nothing() -> None:
    h = Harness()
    before = h.document.undo_history
    h.run("ELLIPSE", "0,0", "10,0")
    h.runner.cancel()
    h.run("ELLIPSE", "2,2", "2,2", "3")
    assert h.messages()[-1] == "Those points and distances define no ellipse."
    assert h.document.undo_history == before


def test_the_preview_is_the_ellipse_the_command_will_draw() -> None:
    center, first = (5.0, 0.0), (0.0, 0.0)
    shown = ellipses.candidate(center, first, None, None, False, (5.0, 3.0))
    assert shown == CadEllipse.from_axis(center, first, 3.0)
    arc = ellipses.candidate(center, first, 3.0, 0.0, False, (5.0, -7.0))
    assert arc is not None and not arc.is_full
    expected = CadEllipse.from_axis(center, first, 3.0).arc(first, 0.0, math.pi / 2)
    flat = [c for point in arc.points() for c in point]
    assert flat == pytest.approx([c for point in expected.points() for c in point])
    assert ellipses.candidate(center, first, None, None, False, center) is None


def test_the_viewport_draws_the_ellipse_following_the_cursor(qtbot: QtBot, tmp_path: Path) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)
    action = window.command_actions["ELLIPSE"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Draw"].actions()
    assert action in [button.defaultAction() for button in groups["draw"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    action.trigger()
    mouse(viewport, QEvent.Type.MouseButtonPress, (20.0, 20.0))
    mouse(viewport, QEvent.Type.MouseButtonPress, (60.0, 20.0))
    mouse(viewport, QEvent.Type.MouseMove, (40.0, 30.0))
    shown = viewport.ellipse_preview()
    assert shown is not None and shown.center == pytest.approx((40.0, 20.0))
    items = len(viewport.overlay().items)
    assert items > 0
    mouse(viewport, QEvent.Type.MouseButtonPress, (40.0, 30.0))
    document = window.session.active
    assert document is not None and document.undo_history[-1] == "ELLIPSE"
    window.change_language("es")
    assert action.text() == "&Elipse"
    found = window.runner.registry.find("ELIPSE")
    assert found is not None and found.name == "ELLIPSE"


def test_angles_shown_by_points_count_from_the_first_axis_as_typed_ones() -> None:
    # The point (0,0) lies along the first axis: angle 0; (5,-3) a quarter
    # turn on: the same arc as typing 0 and 90.
    h = Harness()
    h.run("ELLIPSE", "A", "0,0", "10,0", "3", "0,0", "5,-3")
    assert drawn(h) == ((0.0, -3.0), (5.0, 0.0))


def test_an_included_angle_counts_from_the_start() -> None:
    # From 30° to 30° + 150° = 180° from the first axis, which points from
    # (5,0) to (0,0): the arc runs from the 210° direction down past the
    # bottom (5,-3) to (10,0).
    h = Harness()
    h.run("ELLIPSE", "A", "0,0", "10,0", "3", "30", "I", "150")
    (x0, y0), (x1, y1) = drawn(h)
    at = math.radians(210.0)
    r = 1.0 / math.sqrt(math.cos(at) ** 2 / 25.0 + math.sin(at) ** 2 / 9.0)
    assert x0 == pytest.approx(5.0 + r * math.cos(at))
    assert x1 == 10.0
    assert (y0, y1) == (-3.0, 0.0)


def test_an_ellipse_is_a_curve_to_snap_to_from_python() -> None:
    # About (0,0), semi-axes 5 and 3; a line along y = 1.8 crosses it at
    # x = ±4. From (10,0) the tangents touch at x = 2.5; from (0,7), the
    # perpendicular falls on the top.
    h = Harness()
    h.run("ELLIPSE", "C", "0,0", "5,0", "3")
    h.document.add_line((-9.0, 1.8), (9.0, 1.8))
    found = h.document.snap((4.1, 1.85), 0.3, ["INT"])
    assert found is not None and found.kind == "INT"
    assert found.point == pytest.approx((4.0, 1.8))
    touch = h.document.snap((2.6, 2.6), 0.3, ["TAN"], (10.0, 0.0))
    assert touch is not None and touch.kind == "TAN"
    assert touch.point == pytest.approx((2.5, 3.0 * math.sqrt(0.75)))
    foot = h.document.snap((0.2, 2.98), 0.3, ["PER"], (0.0, 7.0))
    assert foot is not None and foot.point == pytest.approx((0.0, 3.0))


def ellipse_ends(h: Harness) -> tuple[tuple[float, float], tuple[float, float]]:
    """The ends of the one elliptical arc drawn, rounded."""
    found = [e for e in h.document.get_entities() if e.kind == "ELLIPSE"]
    assert len(found) == 1, found
    curve = found[0].geometry
    assert isinstance(curve, CadEllipse) and not curve.is_full
    first, *_, last = curve.points()
    r = round
    return (r(first[0], 6), r(first[1], 6)), (r(last[0], 6), r(last[1], 6))


def test_trim_extend_and_break_edit_an_ellipse() -> None:
    # About (0,0), semi-axes 5 and 3; the upright x = 4 crosses it at
    # (4, ±1.8). Quick TRIM picked on its right leaves the long arc.
    h = Harness()
    h.run("ELLIPSE", "C", "0,0", "5,0", "3")
    (ellipse,) = [e.id for e in h.document.get_entities() if e.kind == "ELLIPSE"]
    h.line((4.0, -9.0), (4.0, 9.0))
    h.run("TRIM")
    h.runner.answer_entity(ellipse, (5.0, 0.0))
    h.run("")
    assert ellipse_ends(h) == ((4.0, 1.8), (4.0, -1.8))
    assert h.document.undo_history[-1] == "TRIM"
    # EXTEND it back at (4,-1.8), to the line x = 4.5 below: to (4.5, -√(9·0.19)).
    h.line((4.5, -9.0), (4.5, -1.0))
    h.run("EXTEND")
    h.runner.answer_entity(ellipse, (4.0, -1.8))
    h.run("")
    y = round(-3.0 * math.sqrt(1.0 - 0.81), 6)
    assert ellipse_ends(h) == ((4.0, 1.8), (4.5, y))
    # BREAK it at its top and at (-5,0): two arcs.
    h.run("BREAK")
    h.runner.answer_entity(ellipse, (0.0, 3.0))
    h.run("-5,0")
    arcs = [e for e in h.document.get_entities() if e.kind == "ELLIPSE"]
    assert len(arcs) == 2
    assert h.document.undo_history[-1] == "BREAK"


def test_a_whole_ellipse_is_not_extended_and_a_fence_trims_it() -> None:
    h = Harness()
    h.run("ELLIPSE", "C", "0,0", "5,0", "3")
    (ellipse,) = [e.id for e in h.document.get_entities() if e.kind == "ELLIPSE"]
    before = h.document.undo_history
    h.run("EXTEND")
    h.runner.answer_entity(ellipse, (5.0, 0.0))
    assert h.messages()[-1] == "Cannot extend that object."
    h.run("")
    assert h.document.undo_history == before
    # A fence across its right side, the upright x = 4 cutting it.
    h.line((4.0, -9.0), (4.0, 9.0))
    h.run("TRIM", "F", "4.8,-2", "4.8,2", "")
    assert ellipse_ends(h) == ((4.0, 1.8), (4.0, -1.8))


def test_an_ellipse_is_plan_geometry_and_offset_as_a_spline() -> None:
    h = Harness()
    h.run("ELLIPSE", "C", "0,0", "5,0", "3")
    (entity,) = [e for e in h.document.get_entities() if e.kind == "ELLIPSE"]
    assert isinstance(entity.geometry, CadEllipse) and entity.geometry.is_full
    assert entity.is_editable and entity.elevation == 0.0
    h.run("OFFSET", "1")
    h.runner.answer_entity(entity.id, (5.0, 0.0))
    h.run("9,0")
    kinds = sorted(e.kind for e in h.document.get_entities())
    assert kinds == ["ELLIPSE", "SPLINE"]
    assert h.document.undo_history[-1] == "OFFSET"
