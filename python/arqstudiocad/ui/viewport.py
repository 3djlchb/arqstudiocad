"""The viewport: a document drawn by the core and shown by Qt (ADR 0013, D5.3).

The core rasterizes the drawing into an image (``CadView``); this widget only
puts that image on screen, so no Python runs per entity. It renders at the
screen's physical resolution (high DPI) and again only when something
changed. Middle-button drag pans, the wheel zooms about the cursor, a
middle-button double click zooms to the extents, as in AutoCAD.

Selection, as AutoCAD (ADR 0016 D3, D4): a click adds the entity under the
cursor and Shift+click removes it; dragging left to right is a window (blue,
solid: what lies wholly inside), right to left a crossing (green, dashed:
whatever it touches); Esc empties the selection. What is selected is drawn
blue and whatever the cursor rests on thicker, as an overlay painted over
the image: hovering and selecting never re-render the drawing. The viewport
only asks for selection changes (its signals); the session decides and
tells it what is selected (:meth:`Viewport.show_selection`).
"""

from PySide6.QtCore import QPointF, QRectF, Qt, Signal
from PySide6.QtGui import (
    QColor,
    QImage,
    QKeyEvent,
    QMouseEvent,
    QPainter,
    QPaintEvent,
    QPen,
    QPolygonF,
    QResizeEvent,
    QWheelEvent,
)
from PySide6.QtWidgets import QWidget

from arqstudiocad.core import CadBounds2, CadChangeSet, CadDocument, CadView

#: Zoom factor of one wheel notch (120 eighths of a degree).
WHEEL_STEP = 1.25

#: Half the side of the pick box, in logical pixels (AutoCAD's PICKBOX is 3).
PICK_PIXELS = 4

#: How far the mouse must move, in logical pixels, before a press becomes a
#: window instead of a click.
DRAG_PIXELS = 4

#: Selected entities and the window rectangle (OpenCADStudio's blue).
SELECTED = QColor(30, 144, 255)
#: The crossing rectangle.
CROSSING = QColor(60, 200, 90)
#: How opaque the window and crossing fills are, 0 to 255.
FILL_ALPHA = 40


class Viewport(QWidget):
    """Shows one document; also the :class:`~arqstudiocad.commands.ViewControl`
    the ZOOM command acts on."""

    #: World coordinates under the cursor, for the status bar.
    cursor_moved = Signal(float, float)
    #: Entities to add to the selection (a click, a window, a crossing).
    select_requested = Signal(object)
    #: Entities to remove from the selection (Shift+click or Shift+window).
    deselect_requested = Signal(object)
    #: The selection should be emptied (Esc).
    clear_requested = Signal()

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
        # Selection: what the session says is selected, what the cursor
        # rests on, and where a left-button press started.
        self._selected: tuple[int, ...] = ()
        self._hover: int | None = None
        self._press: QPointF | None = None
        self._band: QPointF | None = None  # the other corner while dragging a window
        self.setMouseTracking(True)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
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
        if self._hover is not None and self._hover in changes.deleted:
            self._hover = None
            self.update()

    # Selection

    @property
    def selected(self) -> tuple[int, ...]:
        """What the viewport shows as selected."""
        return self._selected

    @property
    def hovered(self) -> int | None:
        """The entity under the cursor, highlighted."""
        return self._hover

    def show_selection(self, entity_ids: tuple[int, ...]) -> None:
        """Shows these entities as selected (the session's selection)."""
        if entity_ids != self._selected:
            self._selected = entity_ids
            self.update()

    # Qt

    def paintEvent(self, event: QPaintEvent) -> None:  # noqa: N802 (Qt API)
        if self._dirty or self._image is None:
            self._render()
        painter = QPainter(self)
        if self._image is not None:
            painter.drawImage(0, 0, self._image)
        self._paint_overlay(painter)
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
        if event.button() == Qt.MouseButton.LeftButton:
            self._press = event.position()
            self._band = None
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
        elif self._press is not None and event.buttons() & Qt.MouseButton.LeftButton:
            if self._band is not None or _distance(position, self._press) > DRAG_PIXELS:
                self._band = position
                self.update()
        else:
            self._set_hover(self.pick_at(position))
        self.cursor_moved.emit(*self.view.to_world(*self._physical(position)))

    def mouseReleaseEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self._drag = None
            self.setCursor(Qt.CursorShape.CrossCursor)
            event.accept()
            return
        if event.button() == Qt.MouseButton.LeftButton and self._press is not None:
            remove = bool(event.modifiers() & Qt.KeyboardModifier.ShiftModifier)
            if self._band is not None:
                found = self.select_in(self._press, self._band)
            else:
                picked = self.pick_at(event.position())
                found = () if picked is None else (picked,)
            self._press = self._band = None
            self.update()
            if found:
                (self.deselect_requested if remove else self.select_requested).emit(found)
            event.accept()
            return
        super().mouseReleaseEvent(event)

    def mouseDoubleClickEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self.zoom_extents()
            event.accept()
            return
        super().mouseDoubleClickEvent(event)

    def keyPressEvent(self, event: QKeyEvent) -> None:  # noqa: N802 (Qt API)
        if event.key() == Qt.Key.Key_Escape:
            self._press = self._band = None
            self.update()
            self.clear_requested.emit()
            event.accept()
            return
        super().keyPressEvent(event)

    def leaveEvent(self, event: object) -> None:  # noqa: N802 (Qt API)
        self._set_hover(None)

    # Picking, in logical (widget) coordinates

    def pick_at(self, position: QPointF) -> int | None:
        """The entity under ``position``, within the pick box."""
        x, y = self._physical(position)
        aperture = PICK_PIXELS * self.devicePixelRatioF() / self.view.scale
        return self.document.pick(self.view.to_world(x, y), aperture)

    def select_in(self, start: QPointF, end: QPointF) -> tuple[int, ...]:
        """What a window from ``start`` to ``end`` selects: left to right a
        window, right to left a crossing."""
        a = self.view.to_world(*self._physical(start))
        b = self.view.to_world(*self._physical(end))
        crossing = end.x() < start.x()
        return tuple(self.document.select_window(CadBounds2(a, b), crossing=crossing))

    # Helpers

    def _set_hover(self, entity_id: int | None) -> None:
        if entity_id != self._hover:
            self._hover = entity_id
            self.update()

    def _paint_overlay(self, painter: QPainter) -> None:
        """The selection, the highlight and the window, over the image."""
        ratio = self.devicePixelRatioF()
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.save()
        painter.scale(1 / ratio, 1 / ratio)  # outlines come in physical pixels
        if self._selected:
            painter.setPen(QPen(SELECTED, 2 * ratio))
            self._draw_outlines(painter, self._selected)
        if self._hover is not None:
            color = SELECTED if self._hover in self._selected else self.palette().text().color()
            painter.setPen(QPen(color, 3 * ratio))
            self._draw_outlines(painter, (self._hover,))
        painter.restore()
        if self._press is not None and self._band is not None:
            crossing = self._band.x() < self._press.x()
            color = CROSSING if crossing else SELECTED
            pen = QPen(color, 1)
            if crossing:
                pen.setStyle(Qt.PenStyle.DashLine)
            fill = QColor(color)
            fill.setAlpha(FILL_ALPHA)
            painter.setPen(pen)
            painter.setBrush(fill)
            painter.drawRect(QRectF(self._press, self._band).normalized())

    def _draw_outlines(self, painter: QPainter, entity_ids: tuple[int, ...]) -> None:
        for polyline in self.view.outlines(entity_ids):
            painter.drawPolyline(QPolygonF([QPointF(x, y) for x, y in polyline]))

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


def _distance(a: QPointF, b: QPointF) -> float:
    return float(((a.x() - b.x()) ** 2 + (a.y() - b.y()) ** 2) ** 0.5)
