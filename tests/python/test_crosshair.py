"""The crosshair the viewport draws (ADR 0029 D6): its shape by size and
type, its automatic color, the system cursor hidden under it, and a mouse
move repainting only the crosshair, never the drawing."""

import pytest
from PySide6.QtCore import QEvent, QLineF, QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QDragLeaveEvent, QMouseEvent, QRegion
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot

from arqstudiocad.events import EventBus
from arqstudiocad.session import Session
from arqstudiocad.ui.crosshair import (
    ON_DARK,
    ON_LIGHT,
    SMALL_ARM,
    Crosshair,
    crosshair_color,
    crosshair_lines,
    crosshair_rects,
)
from arqstudiocad.ui.viewport import BLOCK_EDITOR_BACKGROUND, PICK_PIXELS, Viewport

AREA = QRectF(0, 0, 1000, 600)
CENTER = QPointF(500, 300)


def arms(lines: list[QLineF]) -> list[QLineF]:
    """The arms: the lines before the pick box's four sides."""
    return lines[:-4]


def test_each_arm_is_the_size_of_the_half_area() -> None:
    lines = crosshair_lines(Crosshair(size=5), CENTER, AREA, PICK_PIXELS)
    left, right, up, down = arms(lines)
    # 5 % of the half-width (500) and the half-height (300).
    assert left == QLineF(475, 300, 500 - PICK_PIXELS, 300)
    assert right == QLineF(500 + PICK_PIXELS, 300, 525, 300)
    assert up == QLineF(500, 285, 500, 300 - PICK_PIXELS)
    assert down == QLineF(500, 300 + PICK_PIXELS, 500, 315)
    box = lines[-4:]
    assert QRectF(box[0].p1(), box[1].p2()) == QRectF(496, 296, 8, 8), "the pick box"
    bigger = arms(crosshair_lines(Crosshair(size=40), CENTER, AREA, PICK_PIXELS))
    assert bigger[0].p1().x() == 300 and bigger[2].p1().y() == 180


def test_at_100_percent_the_lines_cross_the_whole_area() -> None:
    corner = QPointF(20, 30)
    left, right, up, down = arms(crosshair_lines(Crosshair(size=100), corner, AREA, PICK_PIXELS))
    assert left.p1().x() <= AREA.left() and right.p2().x() >= AREA.right()
    assert up.p1().y() <= AREA.top() and down.p2().y() >= AREA.bottom()


def test_a_tiny_size_keeps_the_pick_box() -> None:
    lines = crosshair_lines(Crosshair(size=0), CENTER, QRectF(0, 0, 100, 100), PICK_PIXELS)
    assert arms(lines) == [], "no arm shorter than the pick box"
    assert len(lines) == 4


def test_the_small_cross_and_the_arrow() -> None:
    left, right, _, _ = arms(crosshair_lines(Crosshair(type="small"), CENTER, AREA, PICK_PIXELS))
    assert left.p1().x() == 500 - PICK_PIXELS - SMALL_ARM
    assert right.p2().x() == 500 + PICK_PIXELS + SMALL_ARM
    same = arms(crosshair_lines(Crosshair(size=90, type="small"), CENTER, AREA, PICK_PIXELS))
    assert same[0] == left, "the size does not change it"
    assert crosshair_lines(Crosshair(type="arrow"), CENTER, AREA, PICK_PIXELS) == []


def test_the_automatic_color_contrasts_with_the_background() -> None:
    auto = Crosshair()
    assert crosshair_color(auto, QColor(33, 40, 48)) == ON_DARK, "model space"
    assert crosshair_color(auto, QColor(*BLOCK_EDITOR_BACKGROUND)) == ON_DARK, "the block editor"
    assert crosshair_color(auto, QColor(255, 255, 255)) == ON_LIGHT
    # Luminance, not lightness: pure yellow is light, pure blue dark.
    assert crosshair_color(auto, QColor(255, 255, 0)) == ON_LIGHT
    assert crosshair_color(auto, QColor(0, 0, 255)) == ON_DARK
    assert crosshair_color(Crosshair(color="#ff8800"), QColor(0, 0, 0)) == QColor("#ff8800")
    assert crosshair_color(Crosshair(color="nonsense"), QColor(0, 0, 0)) == ON_DARK


def test_the_repaint_covers_the_lines_only() -> None:
    style = Crosshair()
    rects = crosshair_rects(style, CENTER, AREA, PICK_PIXELS)
    region = QRegion()
    for rect in rects:
        region += rect
    for line in crosshair_lines(style, CENTER, AREA, PICK_PIXELS):
        for t in (0.0, 0.5, 1.0):
            assert region.contains(line.pointAt(t).toPoint())
    assert all(min(r.width(), r.height()) <= 5 for r in rects), "thin strips"
    assert region.boundingRect().width() < 60 and region.boundingRect().height() < 40


@pytest.fixture
def viewport(qtbot: QtBot) -> Viewport:
    v = Viewport(Session(EventBus()).new_document())
    qtbot.addWidget(v)
    v.resize(400, 300)
    v.show()
    qtbot.waitExposed(v)
    return v


def move(viewport: Viewport, at: QPointF, held: Qt.MouseButton = Qt.MouseButton.NoButton) -> None:
    event = QMouseEvent(
        QEvent.Type.MouseMove,
        at,
        viewport.mapToGlobal(at),
        Qt.MouseButton.NoButton,
        held,
        Qt.KeyboardModifier.NoModifier,
    )
    QApplication.sendEvent(viewport, event)


def press(viewport: Viewport, kind: QEvent.Type, at: QPointF, button: Qt.MouseButton) -> None:
    held = button if kind == QEvent.Type.MouseButtonPress else Qt.MouseButton.NoButton
    event = QMouseEvent(
        kind, at, viewport.mapToGlobal(at), button, held, Qt.KeyboardModifier.NoModifier
    )
    QApplication.sendEvent(viewport, event)


def lit(viewport: Viewport, x: int, y: int) -> bool:
    image = viewport.grab().toImage()
    background = QColor(*viewport.view.background)
    return abs(image.pixelColor(x, y).lightness() - background.lightness()) > 60


def test_the_viewport_draws_it_at_the_cursor(viewport: Viewport) -> None:
    assert viewport.cursor().shape() == Qt.CursorShape.BlankCursor, "the system cursor hidden"
    assert viewport.cursor_at is None and not lit(viewport, 200, 150)
    move(viewport, QPointF(200, 150))
    assert viewport.cursor_at == QPointF(200, 150)
    # 5 % of 200 and 150 px: arms out to 10 and 7.5 px; the box 4 px.
    assert lit(viewport, 192, 150) and lit(viewport, 208, 150)
    assert lit(viewport, 200, 143) and lit(viewport, 200, 157)
    assert lit(viewport, 196, 148), "the pick box"
    assert not lit(viewport, 200, 150), "the arms broken at the box"
    assert not lit(viewport, 180, 150), "beyond the arm"
    assert not lit(viewport, 192, 152) and not lit(viewport, 202, 143), "one pixel wide"
    viewport.leaveEvent(QEvent(QEvent.Type.Leave))
    assert viewport.cursor_at is None and not lit(viewport, 192, 150), "gone with the cursor"


def test_a_move_repaints_only_the_crosshair(
    viewport: Viewport, monkeypatch: pytest.MonkeyPatch
) -> None:
    move(viewport, QPointF(100, 100))
    QApplication.processEvents()
    repaints: list[tuple[object, ...]] = []
    monkeypatch.setattr(viewport, "update", lambda *area: repaints.append(area))
    move(viewport, QPointF(110, 105))
    assert len(repaints) == 1
    region = repaints[0][0]
    assert isinstance(region, QRegion)
    old = crosshair_rects(
        viewport.crosshair, QPointF(100, 100), QRectF(viewport.rect()), PICK_PIXELS
    )
    new = crosshair_rects(
        viewport.crosshair, QPointF(110, 105), QRectF(viewport.rect()), PICK_PIXELS
    )
    assert all(region.intersected(QRegion(r)) == QRegion(r) for r in old + new), (
        "where it was and where it goes"
    )
    bounds = region.boundingRect()
    assert bounds.width() < viewport.width() / 4, "never the whole drawing"
    assert bounds.height() < viewport.height() / 4
    move(viewport, QPointF(110, 105))
    assert len(repaints) == 1, "not moved, not repainted"


def test_panning_shows_the_hand_alone(viewport: Viewport) -> None:
    move(viewport, QPointF(100, 100))
    press(viewport, QEvent.Type.MouseButtonPress, QPointF(100, 100), Qt.MouseButton.MiddleButton)
    assert viewport.cursor().shape() == Qt.CursorShape.ClosedHandCursor
    assert viewport.cursor_at is None
    move(viewport, QPointF(130, 120), Qt.MouseButton.MiddleButton)
    assert viewport.cursor_at is None, "not while panning"
    press(viewport, QEvent.Type.MouseButtonRelease, QPointF(130, 120), Qt.MouseButton.MiddleButton)
    assert viewport.cursor().shape() == Qt.CursorShape.BlankCursor
    assert viewport.cursor_at == QPointF(130, 120)


def test_the_arrow_shows_the_system_arrow(viewport: Viewport) -> None:
    viewport.set_crosshair(Crosshair(type="arrow"))
    assert viewport.cursor().shape() == Qt.CursorShape.ArrowCursor
    move(viewport, QPointF(200, 150))
    assert not lit(viewport, 192, 150) and not lit(viewport, 196, 148), "nothing drawn"
    viewport.set_crosshair(Crosshair(size=40))
    assert viewport.cursor().shape() == Qt.CursorShape.BlankCursor
    assert lit(viewport, 130, 150), "the new size at once"


def test_the_color_follows_the_background(viewport: Viewport) -> None:
    move(viewport, QPointF(200, 150))
    white = viewport.grab().toImage().pixelColor(192, 150)
    assert white.lightness() > 200
    viewport.set_crosshair(Crosshair(color="#ff8800"))
    assert viewport.grab().toImage().pixelColor(192, 150) == QColor("#ff8800")
    viewport.set_crosshair(Crosshair())
    viewport.view.background = (240, 240, 240)
    viewport.update()
    assert viewport.grab().toImage().pixelColor(192, 150).lightness() < 60


def test_a_dragged_block_carries_the_crosshair(viewport: Viewport) -> None:
    from test_blocks_palette import drag_event

    from arqstudiocad.ui.block_drag import block_mime

    mime = block_mime(viewport.document.id, "Mesa")  # kept alive: the events point at it
    QApplication.sendEvent(viewport, drag_event("enter", viewport, mime, QPointF(60, 70)))
    assert viewport.cursor_at == QPointF(60, 70)
    QApplication.sendEvent(viewport, drag_event("move", viewport, mime, QPointF(90, 80)))
    assert viewport.cursor_at == QPointF(90, 80), "it follows the drag"
    QApplication.sendEvent(viewport, QDragLeaveEvent())
    assert viewport.cursor_at is None, "gone with the drag"


def test_a_new_style_is_repainted_at_once(
    viewport: Viewport, monkeypatch: pytest.MonkeyPatch
) -> None:
    repaints: list[tuple[object, ...]] = []
    monkeypatch.setattr(viewport, "update", lambda *area: repaints.append(area))
    viewport.set_crosshair(Crosshair(size=40))
    assert repaints == [()], "the whole viewport: the arms change length"
    viewport.set_crosshair(Crosshair(size=40))
    assert len(repaints) == 1, "the same style: nothing to repaint"
