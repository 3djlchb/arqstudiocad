"""The model-space context menu (ADR 0029 D1): a right-click on the drawing
with no command running and nothing selected opens Repeat {command}, which
starts what an empty Enter starts, and Options…. Before any command it reads
"Repeat last command", disabled, so the menu keeps its shape.
"""

from collections.abc import Callable

from PySide6.QtCore import QPoint, QPointF, QRect, QRectF, Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPainterPath, QPalette, QPen, QPixmap
from PySide6.QtWidgets import QMenu, QWidget

from arqstudiocad.commands import Command
from arqstudiocad.i18n import command_display_name, tr

#: The icons' side, in logical pixels.
ICON = 16


def _canvas(size: int, ratio: float) -> tuple[QPixmap, QPainter]:
    pixmap = QPixmap(round(size * ratio), round(size * ratio))
    pixmap.setDevicePixelRatio(ratio)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    return pixmap, painter


def repeat_icon(color: QColor, ratio: float = 1.0) -> QIcon:
    """A circular arrow, as the brief's Repeat item shows."""
    pixmap, painter = _canvas(ICON, ratio)
    pen = QPen(color, 1.6)
    pen.setCapStyle(Qt.PenCapStyle.RoundCap)
    painter.setPen(pen)
    ring = QRectF(3, 3, 10, 10)
    # Three quarters of a circle, open at the top right.
    painter.drawArc(ring, 60 * 16, 290 * 16)
    head = QPainterPath(QPointF(13.6, 2.2))
    head.lineTo(13.4, 6.6)
    head.lineTo(9.2, 5.6)
    painter.setBrush(color)
    painter.drawPath(head)
    painter.end()
    return QIcon(pixmap)


def gear_icon(color: QColor, ratio: float = 1.0) -> QIcon:
    """A small gear, as the brief's Options… item shows."""
    pixmap, painter = _canvas(ICON, ratio)
    painter.setPen(QPen(color, 1.3))
    painter.translate(8, 8)
    for _ in range(8):
        painter.drawLine(QPointF(0, -5), QPointF(0, -7))
        painter.rotate(45)
    painter.drawEllipse(QPointF(0, 0), 4.6, 4.6)
    painter.drawEllipse(QPointF(0, 0), 1.8, 1.8)
    painter.end()
    return QIcon(pixmap)


def model_space_menu(
    last: Command | None,
    parent: QWidget,
    repeat: Callable[[], None],
    options: Callable[[], None],
) -> QMenu:
    """The menu: Repeat ``last`` (disabled without one), a separator and
    Options…; ``repeat`` and ``options`` do what they say."""
    menu = QMenu(parent)
    colors = parent.palette()
    ratio = parent.devicePixelRatioF()
    if last is None:
        item = menu.addAction(tr("Repeat last command"))
        item.setEnabled(False)
    else:
        label = tr("&Repeat {command}", command=command_display_name(last.name))
        # After a tab, Qt aligns the text right, as a shortcut's.
        item = menu.addAction(f"{label}\t{tr('Enter')}")
        item.triggered.connect(repeat)
        menu.setDefaultAction(item)
        menu.setActiveAction(item)
    item.setObjectName("repeat")
    item.setIcon(repeat_icon(colors.color(QPalette.ColorRole.Highlight), ratio))
    menu.addSeparator()
    settings = menu.addAction(
        gear_icon(colors.color(QPalette.ColorRole.Text), ratio), tr("&Options…")
    )
    settings.setObjectName("options")
    settings.triggered.connect(options)
    return menu


def inside(menu: QMenu, at: QPoint, area: QRect) -> QPoint:
    """Where ``menu`` opens with its top-left corner at ``at``, moved left
    or up as little as needed to stay in ``area`` (screen coordinates)."""
    size = menu.sizeHint()
    x = max(area.left(), min(at.x(), area.right() + 1 - size.width()))
    y = max(area.top(), min(at.y(), area.bottom() + 1 - size.height()))
    return QPoint(x, y)
