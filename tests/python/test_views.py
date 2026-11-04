"""Wireframe views (ADR 0030): the model seen from AutoCAD's preset views,
through the core's CadView, the -VIEW and PLAN commands, and the window's
menu, ribbon and status bar. Outside the plan, picking, selecting and
grips wait for it, for now."""

from pathlib import Path

import pytest
from PySide6.QtCore import QPointF
from PySide6.QtWidgets import QApplication, QLabel, QMenu, QToolButton
from pytestqt.qtbot import QtBot
from test_commands import FakeUi, FakeView, harness, regression_drawing
from test_selection import click, plan
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadView
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.ribbon import RibbonGroupWidget

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
    assert h.messages()[-1] == (
        "Type points in this view: object snaps, ORTHO and grips work in plan for now."
    )
    h.run("-V", "nowhere", "front")
    assert h.messages()[-2] == "Invalid option."
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


def test_outside_the_plan_picking_and_grips_wait(window: MainWindow, tmp_path: Path) -> None:
    document, viewport, ids = plan(window, tmp_path)
    window.session.select(document, [ids[0]])
    QApplication.processEvents()
    grip = viewport.grips[0]
    at = viewport.view.to_screen(*grip.point)
    window.command_actions.view_actions["front"].trigger()
    assert (viewport.view_name, viewport.in_plan) == ("front", False)
    assert "Front: snaps, ORTHO and grips in plan only" in status(window)
    assert viewport.grips, "kept for the plan"
    # Neither where it was nor where its plan point falls now.
    assert viewport.grip_at(QPointF(*at)) is None
    assert viewport.grip_at(QPointF(*viewport.view.to_screen(*grip.point))) is None
    window.session.clear_selection(document)
    click(viewport, (50.0, 0.0))
    assert window.session.selection(document) == (), "nothing picked"
    assert viewport.select_in(QPointF(0, 0), QPointF(800, 500)) == ()
    # A point prompt is answered by typing: a click gives no point.
    window.runner.execute("LINE")
    click(viewport, (50.0, 0.0))
    assert viewport.target is None and window.runner.prompt is not None
    assert window.runner.prompt.message == "Specify first point"
    window.runner.cancel()
    window.runner.execute("PLAN")
    assert viewport.in_plan and viewport.view_name == "top"
    assert not any("plan only" in text for text in status(window))
    click(viewport, (50.0, 0.0))
    assert window.session.selection(document) == (ids[0],)
