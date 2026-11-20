"""ID's read-only XYZ input and exact geometric capture."""

import math
from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPointF, Qt
from PySide6.QtGui import QMouseEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from test_point_input import Harness, history
from ui_fakes import QuietUi

from arqstudiocad.core import CadApplication, CadDocument, CadView
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import Viewport


@pytest.mark.parametrize("language", ["en", "es"])
@pytest.mark.parametrize(
    ("typed", "expected"),
    [
        ("3,4,5", "X = 3.0000     Y = 4.0000     Z = 5.0000"),
        ("-2,7,-12.5", "X = -2.0000     Y = 7.0000     Z = -12.5000"),
        ("3,4", "X = 3.0000     Y = 4.0000     Z = 0.0000"),
    ],
)
def test_id_reports_xyz_without_editing(language: str, typed: str, expected: str) -> None:
    set_language(language)
    h = Harness()
    document = h.session.active
    assert document is not None
    h.runner.execute("IDPUNTO" if language == "es" else "ID")
    assert h.prompts()[-1].spatial
    h.runner.execute(typed)
    assert h.messages()[-1] == expected
    assert not document.is_modified and not document.can_undo
    assert h.runner.active is None


@pytest.mark.parametrize("typed", ["1,2,nan", "1,2,inf", "1,2,-inf", "1,2,", "1,2,abc", "1,2,3,4"])
def test_invalid_z_keeps_id_waiting_without_an_edit(typed: str) -> None:
    h = Harness()
    h.runner.execute("ID")
    h.runner.execute(typed)
    assert h.runner.active == "IDPOINT"
    assert not h.messages()[-1].startswith("X =")
    h.runner.execute("1,2,3")
    assert h.messages()[-1].endswith("Z = 3.0000")


def drawing(tmp_path: Path) -> CadDocument:
    entities = [
        ["LINE", "10", "1", "20", "2", "30", "3", "11", "11", "21", "12", "31", "13"],
        ["CIRCLE", "10", "30", "20", "20", "30", "7", "40", "5"],
        ["ARC", "10", "50", "20", "20", "30", "9", "40", "4", "50", "0", "51", "90"],
        [
            "LWPOLYLINE",
            "90",
            "2",
            "38",
            "12",
            "10",
            "60",
            "20",
            "20",
            "42",
            "1",
            "10",
            "64",
            "20",
            "20",
        ],
    ]
    codes = ["0", "SECTION", "2", "ENTITIES"]
    for entity in entities:
        codes.extend(["0", entity[0], "8", "0", *entity[1:]])
    codes.extend(["0", "ENDSEC", "0", "EOF", ""])
    path = tmp_path / "synthetic-xyz.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    return CadApplication().open_document(path)


def screen3(view: CadView, point: tuple[float, float, float]) -> tuple[float, float]:
    x, y, z = point
    if view.view == "swiso":
        return view.to_screen((x - y) / math.sqrt(2), (x + y + 2 * z) / math.sqrt(6))
    if view.view == "front":
        return view.to_screen(x, z)
    return view.to_screen(x, y)


@pytest.mark.parametrize("name", ["top", "swiso", "front"])
@pytest.mark.parametrize(
    ("kind", "point"),
    [
        ("END", (11.0, 12.0, 13.0)),
        ("MID", (6.0, 7.0, 8.0)),
        ("CEN", (30.0, 20.0, 7.0)),
        ("MID", (50.0 + 2 * math.sqrt(2), 20.0 + 2 * math.sqrt(2), 9.0)),
        ("MID", (62.0, 18.0, 12.0)),
    ],
)
def test_capture_returns_exact_model_xyz_in_each_view(
    tmp_path: Path,
    name: str,
    kind: str,
    point: tuple[float, float, float],
) -> None:
    document = drawing(tmp_path)
    view = CadView(document, 800, 600)
    view.view = name
    screen = screen3(view, point)
    snap = view.snap_point3(*screen, 1, [kind])
    assert snap is not None
    assert snap[0] == kind
    assert snap[1] == pytest.approx(point)
    assert snap[2] == pytest.approx(screen)
    assert not document.is_modified and not document.can_undo


def test_solid_overlapping_vertices_prefer_frontmost_real_vertex() -> None:
    document = CadApplication().create_document("solid")
    document.add_box((80, 0, 5), (90, 10, 15))
    view = CadView(document, 800, 600)
    snap = view.snap_point3(*view.to_screen(90, 10), 1, ["END"])
    assert snap is not None and snap[1] == pytest.approx((90, 10, 15))
    assert view.snap_point3(*view.to_screen(90, 10), 1, []) is None
    for value in (float("nan"), float("inf"), -1.0):
        with pytest.raises(ValueError):
            view.snap_point3(100, 100, value, ["END"])


@pytest.mark.parametrize("name", ["top", "swiso", "front"])
def test_id_click_carries_xyz_and_projects_the_marker_at_its_true_height(
    qtbot: QtBot,
    tmp_path: Path,
    name: str,
) -> None:
    document = drawing(tmp_path)
    # The UI owns its session document; the synthetic path is reused only as input.
    assert document.path is not None
    window = MainWindow(ui=QuietUi())
    qtbot.addWidget(window)
    window.resize(1000, 700)
    window.show()
    doc = window.session.open_document(Path(document.path))
    QApplication.processEvents()
    viewport = window.viewports[doc.id]
    viewport.set_view(name)
    window.session.drafting.set_snaps({"MID"})
    window.runner.execute("ID")
    point = (6.0, 7.0, 8.0)
    sx, sy = screen3(viewport.view, point)
    at = QPointF(sx / viewport.devicePixelRatioF(), sy / viewport.devicePixelRatioF())
    for kind in (QEvent.Type.MouseMove, QEvent.Type.MouseButtonPress):
        button = (
            Qt.MouseButton.LeftButton
            if kind == QEvent.Type.MouseButtonPress
            else Qt.MouseButton.NoButton
        )
        event = QMouseEvent(
            kind,
            at,
            viewport.mapToGlobal(at),
            button,
            Qt.MouseButton.NoButton,
            Qt.KeyboardModifier.NoModifier,
        )
        QApplication.sendEvent(viewport, event)
        if kind == QEvent.Type.MouseMove:
            assert viewport.snap is not None and viewport.snap.kind == "MID"
            label = next(label for label in viewport.overlay().labels if label.text == "Midpoint")
            assert label.at == pytest.approx((at.x() + 10, at.y() - 10))
    assert history(window)[-1] == "X = 6.0000     Y = 7.0000     Z = 8.0000"
    assert not viewport._spatial and viewport._snap3 is None and viewport._target3 is None
    assert not doc.is_modified and not doc.can_undo


def test_free_click_uses_xy_plane_and_cancel_restores_2d_input(qtbot: QtBot) -> None:
    document = CadApplication().create_document("empty")
    viewport = Viewport(document)
    qtbot.addWidget(viewport)
    viewport.view.resize(800, 600)
    viewport.ask_point(None, spatial=True)
    viewport._aim(QPointF(450, 350))
    assert viewport._target3 == pytest.approx((*viewport.view.to_world(450, 350), 0.0))
    viewport.end_point()
    assert not viewport._spatial and viewport._target3 is None
    viewport.ask_point(None)
    viewport._aim(QPointF(450, 350))
    assert viewport.target == pytest.approx(viewport.view.to_world(450, 350))
    assert viewport._target3 is None


def test_high_zoom_curved_midpoint_is_not_lost_to_render_tessellation(tmp_path: Path) -> None:
    codes = [
        "0",
        "SECTION",
        "2",
        "ENTITIES",
        "0",
        "ARC",
        "8",
        "0",
        "10",
        "50",
        "20",
        "20",
        "30",
        "9",
        "40",
        "4",
        "50",
        "28",
        "51",
        "62",
        "0",
        "ENDSEC",
        "0",
        "EOF",
        "",
    ]
    path = tmp_path / "high-zoom-arc.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    document = CadApplication().open_document(path)
    view = CadView(document, 800, 600)
    view.view = "swiso"
    point = (50 + 2 * math.sqrt(2), 20 + 2 * math.sqrt(2), 9.0)
    at = screen3(view, point)
    view.zoom_at(*at, 1e6 / view.scale)
    at = screen3(view, point)
    snap = view.snap_point3(*at, 1, ["MID"])
    assert snap is not None and snap[1] == pytest.approx(point)


def test_id_rejects_unimplemented_snap_override_and_can_be_cancelled() -> None:
    h = Harness()
    h.runner.execute("ID")
    h.runner.execute("INT")
    assert h.messages()[-1] == "ID supports END, MID, CEN and QUA object snaps."
    assert h.runner.active == "IDPOINT"
    assert h.session.drafting.override is None
    h.runner.cancel()
    assert h.runner.active is None
