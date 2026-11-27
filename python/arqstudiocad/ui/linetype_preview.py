"""Small previews for simple A-aligned linetype patterns."""

from functools import cache

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPen, QPixmap


@cache
def linetype_preview(pattern: tuple[float, ...]) -> QIcon:
    """Returns a compact preview for a dash/gap/dot pattern."""
    pixmap = QPixmap(64, 16)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setPen(QPen(QColor(190, 190, 190), 1.4))
    if not pattern:
        painter.drawLine(2, 8, 62, 8)
    else:
        period = sum(abs(value) for value in pattern)
        scale = 24.0 / period if period > 1e-12 else 1.0
        x = 2.0
        while x < 62.0:
            for value in pattern:
                length = abs(value) * scale
                if value > 0:
                    painter.drawLine(round(x), 8, round(min(x + length, 62.0)), 8)
                elif value == 0:
                    painter.drawPoint(round(x), 8)
                    length = 1.0
                x += max(length, 0.5)
                if x >= 62.0:
                    break
    painter.end()
    return QIcon(pixmap)
