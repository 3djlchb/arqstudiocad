"""The crosshair the viewport draws (ADR 0029 D6), in place of the system's
cross cursor, so it has a size, a type and a color.

**Crosshair** is two lines crossing at the cursor. Each arm is the size's
percent of the drawing area's half-width or half-height; at 100 % the lines
cross the whole area (AutoCAD's CURSORSIZE). **Small cross** is a short
fixed cross. Both have the pick box at the center, the arms broken around
it. **Arrow** is the system arrow: nothing is drawn.

An empty color means automatic contrast: light on a dark background, dark
on a light one. Painting needs no widget, so the Options dialog's preview
paints the same crosshair.
"""

from dataclasses import dataclass
from typing import Literal

from PySide6.QtCore import QLineF, QPointF, QRect, QRectF
from PySide6.QtGui import QColor, QPainter, QPen

#: The crosshair's shape (the Options dialog's Type).
type CrosshairType = Literal["crosshair", "arrow", "small"]
CROSSHAIR_TYPES: tuple[CrosshairType, ...] = ("crosshair", "arrow", "small")
#: The size's bounds, in percent (AutoCAD's CURSORSIZE).
SIZES = (1, 100)
#: Each arm of the small cross beyond the pick box, in logical pixels.
SMALL_ARM = 8
#: The automatic colors: on a dark background, on a light one.
ON_DARK = QColor(255, 255, 255)
ON_LIGHT = QColor(0, 0, 0)


@dataclass(frozen=True)
class Crosshair:
    """How the crosshair looks: ``size`` in percent, ``type``, and
    ``color`` as ``#RRGGBB``, or empty for automatic contrast."""

    size: int = 5
    type: CrosshairType = CROSSHAIR_TYPES[0]
    color: str = ""


def crosshair_color(style: Crosshair, background: QColor) -> QColor:
    """The crosshair's color on ``background``: the chosen one, or the
    automatic contrast when it is empty or not a color."""
    chosen = QColor(style.color) if style.color else QColor()
    if chosen.isValid():
        return chosen
    # Relative luminance's midpoint, as the eye judges it, not lightness.
    light = 0.2126 * background.redF() + 0.7152 * background.greenF() + 0.0722 * background.blueF()
    return ON_LIGHT if light > 0.5 else ON_DARK


def crosshair_lines(style: Crosshair, at: QPointF, area: QRectF, pick: float) -> list[QLineF]:
    """The lines of the crosshair at ``at`` in ``area``, the pick box's
    half side ``pick`` (logical pixels): the arms, broken at the pick box,
    then the box's four sides. None for the arrow."""
    if style.type == "arrow":
        return []
    if style.type == "small":
        across = down = pick + SMALL_ARM
    elif style.size >= SIZES[1]:
        # The whole area, wherever the cursor is.
        across = down = max(area.width(), area.height())
    else:
        size = max(SIZES[0], style.size) / 100
        across = max(pick, size * area.width() / 2)
        down = max(pick, size * area.height() / 2)
    x, y = at.x(), at.y()
    arms = [
        QLineF(x - across, y, x - pick, y),
        QLineF(x + pick, y, x + across, y),
        QLineF(x, y - down, x, y - pick),
        QLineF(x, y + pick, x, y + down),
    ]
    box = QRectF(x - pick, y - pick, 2 * pick, 2 * pick)
    sides = [
        QLineF(box.topLeft(), box.topRight()),
        QLineF(box.topRight(), box.bottomRight()),
        QLineF(box.bottomRight(), box.bottomLeft()),
        QLineF(box.bottomLeft(), box.topLeft()),
    ]
    return [line for line in arms if line.length() > 0] + sides


def crosshair_rects(style: Crosshair, at: QPointF, area: QRectF, pick: float) -> list[QRect]:
    """What a repaint must cover to draw or erase the crosshair: one thin
    rectangle per line, never the whole area (ADR 0029 D6)."""
    return [
        QRectF(line.p1(), line.p2()).normalized().toAlignedRect().adjusted(-2, -2, 2, 2)
        for line in crosshair_lines(style, at, area, pick)
    ]


def paint_crosshair(
    painter: QPainter,
    style: Crosshair,
    at: QPointF,
    area: QRectF,
    pick: float,
    background: QColor,
) -> None:
    """Paints the crosshair: one crisp logical pixel wide."""
    lines = crosshair_lines(style, at, area, pick)
    if not lines:
        return
    painter.save()
    painter.setRenderHint(QPainter.RenderHint.Antialiasing, False)
    painter.setPen(QPen(crosshair_color(style, background), 1))
    painter.drawLines(lines)
    painter.restore()
