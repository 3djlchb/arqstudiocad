"""The viewport in the main window (ADR 0013, D5.3)."""

from pathlib import Path

import pytest
from PySide6.QtCore import QEvent, QPoint, QPointF, Qt
from PySide6.QtGui import QMouseEvent, QWheelEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.core import CadDocument
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.viewport import WHEEL_STEP, Viewport

DARK = (33, 40, 48)


def write_dxf(path: Path) -> Path:
    """A 100 x 50 rectangle of lines and a diagonal."""
    codes = ["0", "SECTION", "2", "ENTITIES"]
    for (x0, y0), (x1, y1) in [
        ((0, 0), (100, 0)),
        ((100, 0), (100, 50)),
        ((100, 50), (0, 50)),
        ((0, 50), (0, 0)),
        ((0, 0), (100, 50)),
    ]:
        codes += ["0", "LINE", "8", "0", "10", str(x0), "20", str(y0), "30", "0"]
        codes += ["11", str(x1), "21", str(y1), "31", "0"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path.write_text("\n".join(codes), encoding="ascii")
    return path


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


def open_plan(window: MainWindow, tmp_path: Path) -> tuple[CadDocument, Viewport]:
    document = window.session.open_document(write_dxf(tmp_path / "plano.dxf"))
    viewport = window.viewports[document.id]
    QApplication.processEvents()
    return document, viewport


def lit(viewport: Viewport, wx: float, wy: float) -> bool:
    """Whether the viewport shows something at or next to world (wx, wy)."""
    image = viewport.grab().toImage()
    sx, sy = viewport.view.to_screen(wx, wy)
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            x, y = round(sx) + dx, round(sy) + dy
            if 0 <= x < image.width() and 0 <= y < image.height():
                c = image.pixelColor(x, y)
                if (c.red(), c.green(), c.blue()) != DARK:
                    return True
    return False


def send_mouse(
    viewport: Viewport,
    kind: QEvent.Type,
    at: tuple[float, float],
    button: Qt.MouseButton = Qt.MouseButton.MiddleButton,
    held: Qt.MouseButton = Qt.MouseButton.MiddleButton,
) -> None:
    point = QPointF(*at)
    event = QMouseEvent(
        kind, point, viewport.mapToGlobal(point), button, held, Qt.KeyboardModifier.NoModifier
    )
    QApplication.sendEvent(viewport, event)


def test_an_opened_drawing_is_shown_whole(window: MainWindow, tmp_path: Path) -> None:
    _, viewport = open_plan(window, tmp_path)
    assert window.tabs.currentWidget() is viewport
    assert viewport.view.shape_count == 5
    assert lit(viewport, 50, 0) and lit(viewport, 100, 25) and lit(viewport, 50, 25)
    assert not lit(viewport, 75, 10), "inside the rectangle, off the diagonal"
    left, top = viewport.view.to_screen(0, 50)
    right, bottom = viewport.view.to_screen(100, 0)
    assert 0 < left < right < viewport.width() and 0 < top < bottom < viewport.height()


def test_wheel_zooms_about_the_cursor(window: MainWindow, tmp_path: Path) -> None:
    _, viewport = open_plan(window, tmp_path)
    scale = viewport.view.scale
    at = QPointF(120, 80)
    under = viewport.view.to_world(at.x(), at.y())
    event = QWheelEvent(
        at,
        viewport.mapToGlobal(at),
        QPoint(),
        QPoint(0, 120),
        Qt.MouseButton.NoButton,
        Qt.KeyboardModifier.NoModifier,
        Qt.ScrollPhase.NoScrollPhase,
        False,
    )
    QApplication.sendEvent(viewport, event)
    assert viewport.view.scale == pytest.approx(scale * WHEEL_STEP)
    assert viewport.view.to_world(at.x(), at.y()) == pytest.approx(under)


def test_middle_drag_pans_and_double_click_zooms_extents(
    window: MainWindow, tmp_path: Path
) -> None:
    _, viewport = open_plan(window, tmp_path)
    fitted = (viewport.view.center, viewport.view.scale)
    grabbed = viewport.view.to_world(200, 150)
    send_mouse(viewport, QEvent.Type.MouseButtonPress, (200, 150))
    send_mouse(viewport, QEvent.Type.MouseMove, (230, 140))
    send_mouse(viewport, QEvent.Type.MouseButtonRelease, (230, 140), held=Qt.MouseButton.NoButton)
    assert viewport.view.to_world(230, 140) == pytest.approx(grabbed), "the drawing followed"
    send_mouse(viewport, QEvent.Type.MouseButtonDblClick, (10, 10))
    assert viewport.view.center == pytest.approx(fitted[0])
    assert viewport.view.scale == pytest.approx(fitted[1])


def test_the_status_bar_shows_the_cursor(window: MainWindow, tmp_path: Path) -> None:
    _, viewport = open_plan(window, tmp_path)
    sx, sy = viewport.view.to_screen(25, 12.5)
    send_mouse(
        viewport, QEvent.Type.MouseMove, (sx, sy), Qt.MouseButton.NoButton, Qt.MouseButton.NoButton
    )
    assert window._coordinates.text() == "25.0000, 12.5000"


def test_the_view_follows_changes_and_zoom(window: MainWindow, tmp_path: Path) -> None:
    document, viewport = open_plan(window, tmp_path)
    _, changes = document.add_circle((50, 25), 10)
    window.session.report(document, changes)
    QApplication.processEvents()
    assert viewport.view.shape_count == 6
    assert lit(viewport, 60, 25), "the new circle is drawn"
    scale = viewport.view.scale
    window.runner.execute("ZOOM")
    window.runner.execute("I")
    assert viewport.view.scale == pytest.approx(scale * 2)


def test_each_document_has_its_view(window: MainWindow, tmp_path: Path) -> None:
    first, first_view = open_plan(window, tmp_path)
    window.runner.execute("NEW")
    second = window.session.active
    assert second is not None and second != first
    assert window.tabs.currentWidget() is window.viewports[second.id]
    window.session.activate(first)
    assert window.tabs.currentWidget() is first_view
    window.runner.execute("CLOSE")
    window.runner.execute("CLOSE")
    assert window.viewports == {}
    assert window._stack.currentWidget() is window._welcome
