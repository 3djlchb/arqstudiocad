"""Wireframe views (ADR 0030): the model seen from AutoCAD's preset views,
through the core's CadView, the -VIEW and PLAN commands, and the window's
menu, ribbon and status bar. Outside the plan, picking, selecting and
grips wait for it, for now."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QMouseEvent
from PySide6.QtWidgets import QApplication, QLabel, QMenu, QToolButton
from pytestqt.qtbot import QtBot
from test_commands import FakeUi, FakeView, harness, regression_drawing
from test_selection import plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadView
from arqstudiocad.events import CommandMessage
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget
from arqstudiocad.ui.viewport import Viewport

ATMOS = Path(__file__).resolve().parents[2] / "target/corpus/libredwg/2007/ATMOS-DC22S.dwg"


# The core's view


def test_solids_seen_from_the_front_stand_on_the_ground() -> None:
    document = CadApplication().open_document(regression_drawing(ATMOS))
    view = CadView(document, 400, 300)
    assert view.view == "top"
    above = view.bounds
    assert above is not None
    view.view = "front"
    front = view.bounds
    assert view.view == "front" and front is not None
    # Across is X, as in plan; up is Z, from the ground to the tallest
    # solid's top (MASSPROP's bounding box: Z 0 to 1179.515627).
    assert front.min[0] == pytest.approx(above.min[0])
    assert front.max[0] == pytest.approx(above.max[0])
    assert front.min[1] == pytest.approx(0.0, abs=1e-6)
    assert front.max[1] == pytest.approx(1179.515627, rel=1e-3)
    assert view.center == pytest.approx(((front.min[0] + front.max[0]) / 2, front.max[1] / 2))
    view.rebuild()
    assert view.view == "front", "rebuilt in its view"
    view.view = "swiso"
    assert view.shape_count > 0 and view.bounds != front
    with pytest.raises(ValueError, match="unknown view"):
        view.view = "plan"


def test_a_view_keeps_its_direction_through_changes() -> None:
    document = CadApplication().create_document("vista")
    view = CadView(document, 100, 100)
    view.view = "front"
    _, changes = document.add_line((1, 2), (5, 2))
    assert view.update(changes)
    bounds = view.bounds
    assert bounds is not None
    # Its Y runs away from the viewer: on the ground, from x 1 to 5.
    assert (bounds.min, bounds.max) == ((1.0, 0.0), (5.0, 0.0))


# The commands


def test_view_sees_the_model_from_a_preset_view() -> None:
    view = FakeView()
    h = harness(FakeUi(the_view=view))
    h.session.new_document()
    h.run("-VIEW")
    assert h.prompts()[-1] == (
        "Enter an option [Top/Bottom/Front/Back/Left/Right/SWiso/SEiso/NEiso/NWiso]:"
    )
    h.run("swiso")
    assert view.views == ["swiso"]
    assert h.messages()[-1] == "Object snaps, ORTHO and grips work in plan only, for now."
    h.run("-V", "nowhere", "front")
    assert h.messages()[-3:] == [
        "Invalid option.",
        "Type points in this view: it sees the XY plane edge on.",
        "Object snaps, ORTHO and grips work in plan only, for now.",
    ]
    h.run("PLAN")
    assert view.views == ["swiso", "front", "top"]


def test_vista_and_planta_in_spanish() -> None:
    view = FakeView()
    h = harness(FakeUi(the_view=view))
    h.session.new_document()
    set_language("es")
    try:
        h.run("-VISTA")
        assert h.prompts()[-1].startswith("Indique una opción [Superior(TOP)/Inferior(BOTTOM)/")
        h.run("isoSO", "PLANTA")
    finally:
        set_language("en")
    assert view.views == ["swiso", "top"]


def test_without_a_view_the_view_cannot_change() -> None:
    h = harness(FakeUi())
    h.session.new_document()
    h.run("PLAN")
    assert h.messages()[-1] == "There is no view to change."


# In the window


@pytest.fixture
def window(qtbot: QtBot) -> MainWindow:
    ui = QuietUi()
    w = MainWindow(ui=ui)
    ui.view_for = w.viewport_for
    qtbot.addWidget(w)
    w.resize(900, 600)
    w.show()
    qtbot.waitExposed(w)
    return w


def status(window: MainWindow) -> list[str]:
    return [label.text() for label in window.statusBar().findChildren(QLabel) if label.text()]


def test_the_views_are_in_the_view_menu_and_the_ribbon(window: MainWindow) -> None:
    action = window.command_actions["-VIEW"]
    assert action.text() == "3D &Views"
    views = action.menu()
    assert views is window.command_actions.views_menu
    titles = [a.text() for a in views.actions()]
    assert titles[:3] == ["&Top", "&Bottom", "&Front"] and len(titles) == 10
    menus = {m.title(): m for m in window.menuBar().findChildren(QMenu)}
    assert views.menuAction() in menus["&View"].actions()
    groups = {g.group.name: g for g in window.findChildren(RibbonGroupWidget)}
    (button,) = groups["views"].buttons
    assert button.defaultAction() is action
    assert button.popupMode() == QToolButton.ToolButtonPopupMode.MenuButtonPopup


def at_pixel(viewport: Viewport, kind: QEvent.Type, pixel: tuple[float, float]) -> None:
    """A mouse event at a pixel (the test screens have ratio 1)."""
    point = QPointF(*pixel)
    move = kind == QEvent.Type.MouseMove
    button = Qt.MouseButton.NoButton if move else Qt.MouseButton.LeftButton
    held = (
        Qt.MouseButton.LeftButton
        if kind == QEvent.Type.MouseButtonPress
        else Qt.MouseButton.NoButton
    )
    event = QMouseEvent(
        kind, point, viewport.mapToGlobal(point), button, held, Qt.KeyboardModifier.NoModifier
    )
    QApplication.sendEvent(viewport, event)


def click_at(viewport: Viewport, ground: tuple[float, float]) -> None:
    """A click where the point ``ground`` of the XY plane is seen."""
    pixel = viewport.view.plane_to_screen(*ground)
    at_pixel(viewport, QEvent.Type.MouseButtonPress, pixel)
    at_pixel(viewport, QEvent.Type.MouseButtonRelease, pixel)


def test_selecting_in_an_isometric_view(window: MainWindow, tmp_path: Path) -> None:
    document, viewport, ids = plan(window, tmp_path)
    window.command_actions.view_actions["swiso"].trigger()
    click_at(viewport, (50.0, 0.0))
    assert window.session.selection(document) == (ids[0],), "the line in front"
    window.session.clear_selection(document)
    click_at(viewport, (50.0, 35.0))
    assert window.session.selection(document) == (ids[2],), "the circle's far side"
    # A window around the whole view takes everything, in drawing order.
    corner = QPointF(viewport.width(), viewport.height())
    assert viewport.select_in(QPointF(0, 0), corner) == tuple(ids)
    assert viewport.select_in(QPointF(0, 0), QPointF(1, 1)) == ()


def test_a_picked_point_falls_on_the_xy_plane(window: MainWindow, tmp_path: Path) -> None:
    _, viewport, _ = plan(window, tmp_path)
    window.command_actions.view_actions["neiso"].trigger()
    window.runner.execute("LINE")
    click_at(viewport, (30.0, 50.0))
    prompt = window.runner.prompt
    assert prompt is not None and prompt.base == pytest.approx((30.0, 50.0))
    at_pixel(viewport, QEvent.Type.MouseMove, viewport.view.plane_to_screen(70.0, 20.0))
    assert viewport.target == pytest.approx((70.0, 20.0)) and viewport.snap is None
    # The rubber band runs from where the first point is seen.
    band = viewport.screen_point((30.0, 50.0))
    seen = viewport.view.plane_to_screen(30.0, 50.0)
    assert (band.x(), band.y()) == pytest.approx(seen)
    assert seen != pytest.approx(viewport.view.to_screen(30.0, 50.0))
    assert "70.0000, 20.0000" in status(window), "the cursor's point on the plane"
    window.runner.cancel()
    # From the front the plane is seen edge on: the point is typed.
    window.command_actions.view_actions["front"].trigger()
    said: list[str] = []
    window.session.bus.subscribe(CommandMessage, lambda e: said.append(e.text))
    window.runner.execute("LINE")
    at_pixel(viewport, QEvent.Type.MouseButtonPress, (300.0, 200.0))
    prompt = window.runner.prompt
    assert prompt is not None and prompt.message == "Specify first point"
    assert said == ["This view sees the XY plane edge on: type the point."]
    window.runner.cancel()


def test_the_drag_preview_follows_the_plane(window: MainWindow, tmp_path: Path) -> None:
    document, viewport, ids = plan(window, tmp_path)
    window.command_actions.view_actions["swiso"].trigger()
    window.session.select(document, [ids[0]])
    window.runner.execute("ROTATE")
    window.runner.execute("0,0")
    # A quarter turn: (100, 0) goes to (0, 100), where the view shows it.
    at_pixel(viewport, QEvent.Type.MouseMove, viewport.view.plane_to_screen(0.0, 5.0))
    preview = viewport.preview
    assert preview is not None
    turned = preview.map(QPointF(*viewport.view.plane_to_screen(100.0, 0.0)))
    expected = viewport.view.plane_to_screen(0.0, 100.0)
    assert (turned.x(), turned.y()) == pytest.approx(expected, abs=1e-6)
    window.runner.cancel()


def test_grips_and_snaps_wait_for_the_plan(window: MainWindow, tmp_path: Path) -> None:
    document, viewport, ids = plan(window, tmp_path)
    window.session.select(document, [ids[0]])
    QApplication.processEvents()
    grip = viewport.grips[0]
    window.command_actions.view_actions["seiso"].trigger()
    assert (viewport.view_name, viewport.in_plan) == ("seiso", False)
    assert "SE Isometric: snaps, ORTHO and grips in plan only" in status(window)
    assert viewport.grips, "kept for the plan"
    shown = viewport.view.plane_to_screen(*grip.point)
    assert viewport.grip_at(QPointF(*shown)) is None
    window.runner.execute("PLAN")
    assert viewport.in_plan and not any("plan only" in text for text in status(window))
    assert viewport.grip_at(QPointF(*viewport.view.to_screen(*grip.point))) == grip
