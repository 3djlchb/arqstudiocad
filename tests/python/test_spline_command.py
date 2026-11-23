"""SPLINE (docs/design/spline-command.md, ADR 0042 D4): through fit points
with Knots, start and end Tangency, Undo and Close; by control vertices
with Degree, Undo and Close; the settings remembered; refusals; the
preview following the cursor; the shared action in Spanish."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent
from PySide6.QtWidgets import QMenu, QToolBar
from pytestqt.qtbot import QtBot
from test_point_input import mouse, plan
from test_trim import Harness
from ui_fakes import QuietUi

from arqstudiocad import splines
from arqstudiocad.core import CadSpline
from arqstudiocad.splines import SplineShape
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.overlay import Stroke
from arqstudiocad.ui.ribbon import RibbonGroupWidget

POINTS = [(0.0, 0.0), (3.0, 4.0), (7.0, 1.0), (9.0, 6.0)]
TYPED = [f"{x:g},{y:g}" for x, y in POINTS]

type Box = tuple[tuple[float, float], tuple[float, float]]


def box(points: list[tuple[float, float]]) -> Box:
    xs, ys = [p[0] for p in points], [p[1] for p in points]
    r = round
    return (r(min(xs), 6), r(min(ys), 6)), (r(max(xs), 6), r(max(ys), 6))


def drawn(h: Harness) -> Box:
    """The box of the one spline drawn, rounded: its control points'."""
    found = [e for e in h.document.get_entities() if e.kind == "SPLINE"]
    assert len(found) == 1, found
    bounds = found[0].bounds
    assert bounds is not None
    r = round
    return (r(bounds.min[0], 6), r(bounds.min[1], 6)), (r(bounds.max[0], 6), r(bounds.max[1], 6))


def expected(spline: CadSpline) -> Box:
    return box(spline.control_points)


def splines_drawn(h: Harness) -> int:
    return len([e for e in h.document.get_entities() if e.kind == "SPLINE"])


def test_through_fit_points_until_enter() -> None:
    h = Harness()
    h.run("SPL")
    assert h.messages()[-1] == "Current settings: Method=Fit  Knots=Chord"
    assert h.prompt().text == "Specify first point [Method(M)/Knots(K)]:"
    h.run(TYPED[0])
    assert h.prompt().text == "Enter next point [start Tangency(T)]:"
    h.run(TYPED[1])
    assert h.prompt().text == "Enter next point [end Tangency(T)/Undo(U)]:"
    h.run(TYPED[2])
    assert h.prompt().text == "Enter next point [end Tangency(T)/Undo(U)/Close(C)]:"
    h.run(TYPED[3], "")
    assert h.runner.active is None
    assert drawn(h) == expected(CadSpline.through(POINTS))
    assert h.document.undo_history[-1] == "SPLINE"
    h.run("UNDO")
    assert splines_drawn(h) == 0


def test_knots_are_chosen_and_remembered() -> None:
    h = Harness()
    h.run("SPLINE", "K", "S", *TYPED, "")
    assert drawn(h) == expected(CadSpline.through(POINTS, "square_root"))
    h.run("UNDO", "SPLINE")
    assert h.messages()[-1] == "Current settings: Method=Fit  Knots=Square root"
    h.run("K")
    assert (
        h.prompt().text == "Enter knot parameterization [Chord(C)/Square root(S)/Uniform(U)] <S>:"
    )
    h.run("U", *TYPED, "")
    assert drawn(h) == expected(CadSpline.through(POINTS, "uniform"))


def test_start_and_end_tangency() -> None:
    # The start tangent shown from the first point; the end tangent from
    # the last, which ends the command.
    h = Harness()
    h.run("SPLINE", TYPED[0], "T", "0,5", *TYPED[1:], "T", "12,6")
    assert h.runner.active is None
    made = CadSpline.through(POINTS, start_tangent=(0.0, 5.0), end_tangent=(3.0, 0.0))
    assert drawn(h) == expected(made)
    assert drawn(h) != expected(CadSpline.through(POINTS))


def test_undo_takes_back_the_last_point() -> None:
    h = Harness()
    h.run("SPLINE", *TYPED[:2], "20,20", "U", *TYPED[2:], "")
    assert drawn(h) == expected(CadSpline.through(POINTS))


def test_close_makes_it_periodic_and_drops_a_start_tangent() -> None:
    h = Harness()
    h.run("SPLINE", TYPED[0], "T", "1,0", *TYPED[1:], "C")
    assert h.runner.active is None
    made = CadSpline.through(POINTS, closed=True)
    assert made.is_closed
    assert drawn(h) == expected(made)


def test_by_control_vertices_with_a_degree_remembered() -> None:
    h = Harness()
    h.run("SPLINE", "M")
    assert h.prompt().text == "Enter spline creation method [Fit(F)/CV] <F>:"
    h.run("CV")
    assert h.messages()[-1] == "Current settings: Method=CV  Degree=3"
    assert h.prompt().text == "Specify first point [Method(M)/Degree(D)]:"
    h.run("D", "11")
    assert h.messages()[-1] == "Enter a whole number from 1 to 10."
    h.run("2.5")
    assert h.messages()[-1] == "Enter a whole number from 1 to 10."
    h.run("2", TYPED[0])
    assert h.prompt().text == "Enter next point:"
    h.run(TYPED[1])
    assert h.prompt().text == "Enter next point [Undo(U)]:"
    h.run(TYPED[2])
    assert h.prompt().text == "Enter next point [Close(C)/Undo(U)]:"
    h.run(TYPED[3], "")
    vertices = CadSpline.by_vertices(POINTS, 2)
    assert drawn(h) == expected(vertices) == box(POINTS)
    h.run("UNDO", "SPLINE")
    assert h.messages()[-1] == "Current settings: Method=CV  Degree=2"
    h.run(*TYPED, "C")
    closed = CadSpline.by_vertices(POINTS, 2, closed=True)
    assert closed.is_closed and drawn(h) == expected(closed)


def test_a_point_on_the_last_or_too_few_draws_nothing() -> None:
    h = Harness()
    before = h.document.undo_history
    h.run("SPLINE", TYPED[0], TYPED[0])
    assert h.messages()[-1] == "The two points must differ."
    said = len(h.messages())
    h.run("")
    assert h.runner.active is None
    assert len(h.messages()) == said, "one point: nothing drawn, nothing said"
    assert h.document.undo_history == before
    # A tangent shown by the point it starts from shows no direction.
    h.run("SPLINE", TYPED[0], "T", TYPED[0])
    assert h.messages()[-1] == "A point on the base point shows no direction."
    h.runner.cancel()
    assert h.document.undo_history == before


def test_the_preview_is_the_spline_the_command_will_draw() -> None:
    shape = SplineShape(tuple(POINTS[:3]), knots="square_root", start_tangent=(1.0, 0.0))
    shown = splines.candidate(shape, POINTS[3])
    made = CadSpline.through(POINTS, "square_root", start_tangent=(1.0, 0.0))
    assert shown is not None and shown.points() == made.points()
    end = SplineShape(tuple(POINTS), end=True)
    shown = splines.candidate(end, (12.0, 6.0))
    made = CadSpline.through(POINTS, end_tangent=(3.0, 0.0))
    assert shown is not None and shown.points() == made.points()
    cv = SplineShape(tuple(POINTS[:3]), "cv", degree=2)
    shown = splines.candidate(cv, POINTS[3])
    assert shown is not None and shown.points() == CadSpline.by_vertices(POINTS, 2).points()
    assert splines.candidate(shape, POINTS[2]) is None, "on the last point"
    assert splines.candidate(end, POINTS[3]) is None, "no direction"


def test_the_viewport_draws_the_spline_following_the_cursor(qtbot: QtBot, tmp_path: Path) -> None:
    ui = QuietUi()
    window = MainWindow(ui=ui)
    ui.view_for = window.viewport_for
    qtbot.addWidget(window)
    window.resize(900, 600)
    window.show()
    qtbot.waitExposed(window)
    viewport = plan(window, tmp_path)
    action = window.command_actions["SPLINE"]
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert action in menus["&Draw"].actions()
    assert action in [button.defaultAction() for button in groups["draw"].buttons]
    assert any(action in bar.actions() for bar in window.findChildren(QToolBar))
    action.trigger()
    mouse(viewport, QEvent.Type.MouseButtonPress, (20.0, 20.0))
    mouse(viewport, QEvent.Type.MouseButtonPress, (60.0, 40.0))
    mouse(viewport, QEvent.Type.MouseMove, (100.0, 20.0))
    shown = viewport.spline_preview()
    assert shown is not None and len(shown.fit_points) == 3
    # The overlay draws it, a stroke along its points.
    strokes = [i for i in viewport.overlay().items if isinstance(i, Stroke)]
    assert any(len(i.points) == len(shown.points()) for i in strokes)
    mouse(viewport, QEvent.Type.MouseButtonPress, (100.0, 20.0))
    window.runner.execute("")
    document = window.session.active
    assert document is not None and document.undo_history[-1] == "SPLINE"
    window.change_language("es")
    assert action.text() == "&Spline"
    found = window.runner.registry.find("SPLINE")
    assert found is not None and found.name == "SPLINE"


@pytest.mark.parametrize("closed", [False, True])
def test_a_drawn_spline_snaps_to_its_fit_ends(closed: bool) -> None:
    h = Harness()
    h.run("SPLINE", *TYPED, "C" if closed else "")
    end = h.document.snap((9.1, 6.1), 0.3, ["END"])
    if closed:
        assert end is None, "a closed spline has no ends"
    else:
        assert end is not None and end.point == pytest.approx((9.0, 6.0))


def test_close_keeps_the_knots_chosen() -> None:
    h = Harness()
    h.run("SPLINE", "K", "U", *TYPED, "C")
    made = CadSpline.through(POINTS, "uniform", closed=True)
    assert drawn(h) == expected(made)
    assert drawn(h) != expected(CadSpline.through(POINTS, closed=True))


def test_spline_refuses_other_views_before_asking() -> None:
    from test_commands import FakeUi, FakeView

    h = Harness()
    h.runner.context.ui = FakeUi(the_view=FakeView(in_plan=False))
    h.run("SPLINE")
    assert h.runner.active is None
    assert h.messages()[-1] == "Use PLAN to switch to the Top view before using SPLINE."


def spline_ends(h: Harness, entity_id: int) -> list[tuple[float, float]]:
    """The ends of spline `entity_id`, rounded."""
    curve = h.document.spline_curve(entity_id)
    assert curve is not None
    first, *_, last = curve.points()
    return [(round(first[0], 9), round(first[1], 9)), (round(last[0], 9), round(last[1], 9))]


def test_trim_and_break_cut_a_spline() -> None:
    # The Bezier from (0,0) by (2,3) and (4,3) to (6,0), x = 6u, cut by
    # x = 2 and x = 4 where it is at height 2 (ADR 0043 D4).
    h = Harness()
    bezier = CadSpline.by_vertices([(0, 0), (2, 3), (4, 3), (6, 0)])
    spline, _ = h.document.add_spline(bezier)
    assert h.document.spline_curve(spline) is not None
    h.line((2.0, -9.0), (2.0, 9.0))
    h.line((4.0, -9.0), (4.0, 9.0))
    h.run("TRIM")
    h.runner.answer_entity(spline, (3.0, 2.25))
    h.run("")
    assert h.document.undo_history[-1] == "TRIM"
    pieces = [e for e in h.document.get_entities() if e.kind == "SPLINE"]
    assert len(pieces) == 2
    ends = sorted(end for e in pieces for end in spline_ends(h, e.id))
    assert ends == [(0.0, 0.0), (2.0, 2.0), (4.0, 2.0), (6.0, 0.0)]
    h.run("UNDO")
    assert len([e for e in h.document.get_entities() if e.kind == "SPLINE"]) == 1
    h.run("BREAK")
    h.runner.answer_entity(spline, (2.0, 2.0))
    h.run("4,2")
    assert h.document.undo_history[-1] == "BREAK"
    assert len([e for e in h.document.get_entities() if e.kind == "SPLINE"]) == 2
    # EXTEND still refuses it.
    h.run("EXTEND")
    h.runner.answer_entity(spline, (1.9, 1.9))
    assert h.messages()[-1] == "Cannot extend that object."
