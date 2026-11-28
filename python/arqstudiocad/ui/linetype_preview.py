"""Small previews for simple and complex A-aligned linetype patterns."""

from functools import cache
from itertools import pairwise

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPen, QPixmap

from arqstudiocad.ui.shx_shapes import shape_strokes


@cache
def linetype_preview(
    pattern: tuple[float, ...],
    complex_items: tuple[tuple[int, str, str, str, float, float, str, float, float], ...] = (),
) -> QIcon:
    """Returns a compact preview for a dash/gap/dot/text/shape pattern."""
    pixmap = QPixmap(64, 16)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    color = QColor(190, 190, 190)
    painter.setPen(QPen(color, 1.4))
    by_segment = {item[0]: item for item in complex_items}
    if not pattern:
        painter.drawLine(2, 8, 62, 8)
    else:
        period = sum(abs(value) for value in pattern)
        scale = 24.0 / period if period > 1e-12 else 1.0
        x = 2.0
        while x < 62.0:
            for index, value in enumerate(pattern):
                length = abs(value) * scale
                if value > 0:
                    painter.drawLine(round(x), 8, round(min(x + length, 62.0)), 8)
                elif value == 0:
                    painter.drawPoint(round(x), 8)
                    length = 1.0
                x += max(length, 0.5)
                embedded = by_segment.get(index)
                if embedded is not None and x < 62.0:
                    _segment, kind, content, _reference, _size, angle, _mode, offset_x, offset_y = (
                        embedded
                    )
                    painter.save()
                    painter.translate(x + offset_x * scale, 8 - offset_y * scale)
                    painter.rotate(angle * 180.0 / 3.141592653589793)
                    if kind == "text":
                        painter.drawText(-1, 4, content)
                    else:
                        glyph = shape_strokes(_reference, content)
                        if glyph is None or not any(glyph[2]):
                            painter.drawRect(-3, -3, 6, 6)
                            painter.drawLine(-2, -2, 2, 2)
                            painter.drawLine(-2, 2, 2, -2)
                        else:
                            points = [point for path in glyph[2] for point in path]
                            min_x = min(point[0] for point in points)
                            max_x = max(point[0] for point in points)
                            min_y = min(point[1] for point in points)
                            max_y = max(point[1] for point in points)
                            glyph_scale = 7.0 / max(max_x - min_x, max_y - min_y, 1.0)
                            painter.translate(0, 0)
                            painter.scale(glyph_scale, -glyph_scale)
                            painter.translate(-(min_x + max_x) / 2, -(min_y + max_y) / 2)
                            for path in glyph[2]:
                                for a, b in pairwise(path):
                                    painter.drawLine(QPointF(a[0], a[1]), QPointF(b[0], b[1]))
                    painter.restore()
                    x += 4.0
                if x >= 62.0:
                    break
    painter.end()
    return QIcon(pixmap)
