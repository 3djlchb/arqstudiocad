"""The viewport: a document drawn by the core and shown by Qt (ADR 0013, D5.3).

The core rasterizes the drawing into an image (``CadView``); this widget only
puts that image on screen, so no Python runs per entity. It renders at the
screen's physical resolution (high DPI) and again only when something
changed. Middle-button drag pans, the wheel zooms about the cursor, a
middle-button double click zooms to the extents, as in AutoCAD.
"""

from PySide6.QtCore import QPointF, Qt, Signal
from PySide6.QtGui import QImage, QMouseEvent, QPainter, QPaintEvent, QResizeEvent, QWheelEvent
from PySide6.QtWidgets import QWidget

from arqstudiocad.core import CadChangeSet, CadDocument, CadView

#: Zoom factor of one wheel notch (120 eighths of a degree).
WHEEL_STEP = 1.25


class Viewport(QWidget):
    """Shows one document; also the :class:`~arqstudiocad.commands.ViewControl`
    the ZOOM command acts on."""

    #: World coordinates under the cursor, for the status bar.
    cursor_moved = Signal(float, float)

    def __init__(self, document: CadDocument, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.document = document
        self.view = CadView(document, 1, 1)
        self._image: QImage | None = None
        self._pixels: bytes = b""  # keeps the image's memory alive
        self._dirty = True
        # Show the whole drawing at whatever size the widget ends up, until
        # the user zooms or pans (a new tab starts tiny, then grows).
        self._auto_fit = True
        self._drag: QPointF | None = None
        self.setMouseTracking(True)
        self.setCursor(Qt.CursorShape.CrossCursor)
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent)
        self.setMinimumSize(64, 64)

    # The view, for commands (ViewControl)

    def zoom_extents(self) -> bool:
        changed = self.view.zoom_extents()
        self._redraw()
        return changed

    def zoom(self, factor: float) -> None:
        """Zooms about the middle of the view."""
        self._auto_fit = False
        self.view.zoom_at(self.view.width / 2, self.view.height / 2, factor)
        self._redraw()

    def apply(self, changes: CadChangeSet) -> None:
        """Follows a change of the document."""
        if self.view.update(changes):
            self._redraw()

    # Qt

    def paintEvent(self, event: QPaintEvent) -> None:  # noqa: N802 (Qt API)
        if self._dirty or self._image is None:
            self._render()
        painter = QPainter(self)
        if self._image is not None:
            painter.drawImage(0, 0, self._image)
        painter.end()

    def resizeEvent(self, event: QResizeEvent) -> None:  # noqa: N802 (Qt API)
        ratio = self.devicePixelRatioF()
        self.view.resize(max(1, round(self.width() * ratio)), max(1, round(self.height() * ratio)))
        if self._auto_fit:
            self.view.zoom_extents()
        self._redraw()

    def wheelEvent(self, event: QWheelEvent) -> None:  # noqa: N802 (Qt API)
        notches = event.angleDelta().y() / 120
        if notches:
            x, y = self._physical(event.position())
            self._auto_fit = False
            self.view.zoom_at(x, y, WHEEL_STEP**notches)
            self._redraw()
        event.accept()

    def mousePressEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self._drag = event.position()
            self.setCursor(Qt.CursorShape.ClosedHandCursor)
            event.accept()
            return
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        position = event.position()
        if self._drag is not None and event.buttons() & Qt.MouseButton.MiddleButton:
            ratio = self.devicePixelRatioF()
            delta = position - self._drag
            self._drag = position
            self._auto_fit = False
            self.view.pan(delta.x() * ratio, delta.y() * ratio)
            self._redraw()
        self.cursor_moved.emit(*self.view.to_world(*self._physical(position)))

    def mouseReleaseEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self._drag = None
            self.setCursor(Qt.CursorShape.CrossCursor)
            event.accept()
            return
        super().mouseReleaseEvent(event)

    def mouseDoubleClickEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self.zoom_extents()
            event.accept()
            return
        super().mouseDoubleClickEvent(event)

    # Helpers

    def _physical(self, point: QPointF) -> tuple[float, float]:
        ratio = self.devicePixelRatioF()
        return point.x() * ratio, point.y() * ratio

    def _redraw(self) -> None:
        self._dirty = True
        self.update()

    def _render(self) -> None:
        self._pixels = self.view.render()
        image = QImage(
            self._pixels,
            self.view.width,
            self.view.height,
            QImage.Format.Format_RGBA8888_Premultiplied,
        )
        image.setDevicePixelRatio(self.devicePixelRatioF())
        self._image = image
        self._dirty = False
