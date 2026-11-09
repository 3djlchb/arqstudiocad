"""What the viewport draws over the drawing, described once (ADR 0034 D4).

The selection, the highlight, the grips, rubber bands, snap markers, the
drop frame, the window and the crosshair are an :class:`Overlay`: strokes,
fills and labels in logical pixels, in the order they are painted. The CPU
path paints it with ``QPainter`` over the rendered image (:func:`paint`);
the GPU path hands it to the view, which draws it with wgpu
(:func:`to_core`). One description, so the two cannot drift apart.
"""

import math
from dataclasses import dataclass, field

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QFont, QFontMetricsF, QImage, QPainter, QPen, QPolygonF

#: A color with straight alpha, each part 0 to 255.
type RGBA = tuple[int, int, int, int]
#: A point in logical pixels, origin top-left, y down.
type XY = tuple[float, float]
#: Segments of a circle drawn as a polygon: a marker is a few pixels wide.
CIRCLE_SEGMENTS = 24


@dataclass(frozen=True)
class Stroke:
    """A polyline. ``dashed`` is Qt's DashLine: four widths on, two off;
    ``crisp`` draws whole pixels, unsmoothed (the crosshair)."""

    points: tuple[XY, ...]
    color: RGBA
    width: float = 1.0
    dashed: bool = False
    closed: bool = False
    crisp: bool = False


@dataclass(frozen=True)
class Fill:
    """A filled rectangle between two corners."""

    min: XY
    max: XY
    color: RGBA


@dataclass(frozen=True)
class Label:
    """A text whose baseline starts at ``at``, as ``QPainter.drawText``."""

    at: XY
    text: str
    color: RGBA


@dataclass
class Overlay:
    """Strokes and fills in the order they are painted; labels over them."""

    items: list[Stroke | Fill] = field(default_factory=list)
    labels: list[Label] = field(default_factory=list)


def rgba(color: QColor) -> RGBA:
    """A ``QColor`` as an :data:`RGBA`."""
    return (color.red(), color.green(), color.blue(), color.alpha())


def circle(center: XY, r: float) -> tuple[XY, ...]:
    """A circle of radius ``r`` as a polygon, to draw closed."""
    x, y = center
    return tuple(
        (
            x + r * math.cos(2 * math.pi * i / CIRCLE_SEGMENTS),
            y + r * math.sin(2 * math.pi * i / CIRCLE_SEGMENTS),
        )
        for i in range(CIRCLE_SEGMENTS)
    )


def rectangle(a: XY, b: XY) -> tuple[XY, ...]:
    """The corners of the rectangle between ``a`` and ``b``, to draw closed."""
    (x0, y0), (x1, y1) = a, b
    return ((x0, y0), (x1, y0), (x1, y1), (x0, y1))


def marker(kind: str, at: XY, r: float, color: RGBA, width: float) -> list[Stroke]:
    """AutoCAD's AutoSnap glyph for a snap ``kind``, centred on ``at``."""
    x, y = at

    def line(*points: XY, closed: bool = False) -> Stroke:
        return Stroke(points, color, width, closed=closed)

    if kind == "END":
        return [line(*rectangle((x - r, y - r), (x + r, y + r)), closed=True)]
    if kind == "MID":
        return [line((x, y - r), (x + r, y + r), (x - r, y + r), closed=True)]
    if kind == "CEN":
        return [line(*circle(at, r), closed=True)]
    if kind == "QUA":
        return [line((x, y - r), (x + r, y), (x, y + r), (x - r, y), closed=True)]
    if kind == "INT":
        return [line((x - r, y - r), (x + r, y + r)), line((x - r, y + r), (x + r, y - r))]
    if kind == "PER":
        return [
            line((x - r, y + r), (x + r, y + r)),
            line((x - r, y + r), (x - r, y - r)),
            line((x - r, y), (x, y), (x, y + r)),
        ]
    if kind == "TAN":
        return [line(*circle(at, r), closed=True), line((x - r, y - r), (x + r, y - r))]
    # NEA: an hourglass
    return [line((x - r, y - r), (x + r, y - r), (x - r, y + r), (x + r, y + r), closed=True)]


def paint(painter: QPainter, overlay: Overlay) -> None:
    """Paints ``overlay`` with ``painter``, in logical pixels."""
    painter.save()
    for item in overlay.items:
        if isinstance(item, Fill):
            painter.setRenderHint(QPainter.RenderHint.Antialiasing, False)
            painter.fillRect(QRectF(QPointF(*item.min), QPointF(*item.max)), QColor(*item.color))
            continue
        if len(item.points) < 2:
            continue
        painter.setRenderHint(QPainter.RenderHint.Antialiasing, not item.crisp)
        pen = QPen(QColor(*item.color), item.width)
        if item.dashed:
            pen.setStyle(Qt.PenStyle.DashLine)
        painter.setPen(pen)
        painter.setBrush(Qt.BrushStyle.NoBrush)
        polygon = QPolygonF([QPointF(x, y) for x, y in item.points])
        if item.closed:
            painter.drawPolygon(polygon)
        else:
            painter.drawPolyline(polygon)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    for label in overlay.labels:
        painter.setPen(QColor(*label.color))
        painter.drawText(QPointF(*label.at), label.text)
    painter.restore()


def to_core(
    overlay: Overlay, font: QFont, ratio: float
) -> tuple[list[tuple[object, ...]], list[tuple[int, int, int, int, bytes]]]:
    """``overlay`` as ``CadView.present`` takes it: the items as tagged
    tuples, the labels painted by Qt into premultiplied images at physical
    pixels, so their text, font and direction stay Qt's."""
    items: list[tuple[object, ...]] = []
    for item in overlay.items:
        if isinstance(item, Fill):
            items.append(("fill", item.min, item.max, item.color))
        else:
            items.append(
                (
                    "stroke",
                    list(item.points),
                    item.color,
                    item.width,
                    item.dashed,
                    item.closed,
                    item.crisp,
                )
            )
    labels = [_label_image(label, font, ratio) for label in overlay.labels]
    return items, [label for label in labels if label is not None]


def _label_image(
    label: Label, font: QFont, ratio: float
) -> tuple[int, int, int, int, bytes] | None:
    metrics = QFontMetricsF(font)
    width = math.ceil(metrics.horizontalAdvance(label.text) * ratio) + 2
    height = math.ceil(metrics.height() * ratio)
    if width <= 2 or height <= 0:
        return None
    image = QImage(width, height, QImage.Format.Format_RGBA8888_Premultiplied)
    image.fill(Qt.GlobalColor.transparent)
    image.setDevicePixelRatio(ratio)
    painter = QPainter(image)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setFont(font)
    painter.setPen(QColor(*label.color))
    painter.drawText(QPointF(0, metrics.ascent()), label.text)
    painter.end()
    x, y = label.at
    return (
        round(x * ratio),
        round((y - metrics.ascent()) * ratio),
        width,
        height,
        bytes(image.constBits()),
    )
