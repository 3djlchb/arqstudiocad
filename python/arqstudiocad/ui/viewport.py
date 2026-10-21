"""The viewport: a document drawn by the core and shown by Qt (ADR 0013, D5.3).

The core rasterizes the drawing into an image (``CadView``); this widget only
puts that image on screen, so no Python runs per entity. It renders at the
screen's physical resolution (high DPI) and again only when something
changed. Middle-button drag pans, the wheel zooms about the cursor, a
middle-button double click zooms to the extents, as in AutoCAD.

Selection, as AutoCAD (ADR 0016 D3, D4): a click adds the entity under the
cursor and Shift+click removes it; dragging left to right is a window (blue,
solid: what lies wholly inside), right to left a crossing (green, dashed:
whatever it touches); Esc, as on the command line, cancels the running command
and empties the selection. What is selected is drawn
blue and whatever the cursor rests on thicker, as an overlay painted over
the image: hovering and selecting never re-render the drawing. The viewport
only asks for selection changes (its signals); the session decides and
tells it what is selected (:meth:`Viewport.show_selection`).

At a point prompt (ADR 0016 D5, D6) a click answers it instead: the point
under the cursor, taken to the object snap found there (its AutoCAD marker
and translated name are shown) or, with ORTHO and no snap, kept horizontal
or vertical from the last point, which a rubber band joins to the cursor.
An editing command's selection follows the cursor from that point, moved,
rotated, scaled or mirrored (its ``Drag``): the outlines are drawn through
a Qt transform, so the preview never re-renders the drawing either.

Selected entities show their grips (ADR 0016 D7), blue squares; a click
on one asks for a grip edit (:attr:`Viewport.grip_clicked`) instead of
selecting, and the hot grip turns red.

Keys typed in the viewport go to the command line (:attr:`Viewport.key_typed`),
as in AutoCAD, where typing anywhere answers the command.
"""

import math

from PySide6.QtCore import QEvent, QPointF, QRectF, Qt, Signal
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
    QTransform,
    QWheelEvent,
)
from PySide6.QtWidgets import QWidget

from arqstudiocad.core import (
    CadBounds2,
    CadChangeSet,
    CadDocument,
    CadGrip,
    CadSnap,
    CadView,
)
from arqstudiocad.drafting import Drafting
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr
from arqstudiocad.points import Point, ortho

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

#: Half the side of the snap aperture, in logical pixels (AutoCAD's
#: APERTURE is 10).
SNAP_PIXELS = 10
#: Half the size of a snap marker, in logical pixels.
MARKER_PIXELS = 6
#: Half the side of a grip, in logical pixels (AutoCAD's GRIPSIZE is 5).
GRIP_PIXELS = 5
#: The hot grip, the one being edited (AutoCAD's red).
HOT = QColor(230, 40, 40)

#: Snap markers and their names (AutoCAD's AutoSnap color).
MARKER = QColor(255, 200, 0)


def snap_name(kind: str) -> str:
    """A snap's name in the interface language."""
    names = {
        "END": tr("Endpoint"),
        "MID": tr("Midpoint"),
        "CEN": tr("Center"),
        "QUA": tr("Quadrant"),
        "INT": tr("Intersection"),
        "PER": tr("Perpendicular"),
        "TAN": tr("Tangent"),
        "NEA": tr("Nearest"),
    }
    return names.get(kind, kind)


class Viewport(QWidget):
    """Shows one document; also the :class:`~arqstudiocad.commands.ViewControl`
    the ZOOM command acts on."""

    #: World coordinates under the cursor, for the status bar.
    cursor_moved = Signal(float, float)
    #: Entities to add to the selection (a click, a window, a crossing).
    select_requested = Signal(object)
    #: Entities to remove from the selection (Shift+click or Shift+window).
    deselect_requested = Signal(object)
    #: Esc: the running command should be cancelled and the selection
    #: emptied, as Esc does on the command line.
    escape_pressed = Signal()
    #: A point picked at a point prompt, snapped: ``(x, y)`` in the world.
    point_picked = Signal(object)
    #: A key typed in the viewport, for the command line (a ``QKeyEvent``).
    key_typed = Signal(object)
    #: A grip was clicked (a ``CadGrip``): it should be edited.
    grip_clicked = Signal(object)

    def __init__(
        self,
        document: CadDocument,
        parent: QWidget | None = None,
        drafting: Drafting | None = None,
    ) -> None:
        super().__init__(parent)
        self.document = document
        self.drafting = drafting
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
        # A point prompt: whether one is open, its last point, and where a
        # click would put the point now (snapped or ORTHO) with its snap.
        self._asking = False
        self._base: Point | None = None
        self._target: Point | None = None
        self._snap: CadSnap | None = None
        self._dragged: Drag | None = None  # what follows the cursor from the base
        # Grips of the selection, and the one being edited.
        self._grips: tuple[CadGrip, ...] = ()
        self._hot: CadGrip | None = None
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

    @property
    def target(self) -> Point | None:
        """Where a click would put the point, at a point prompt."""
        return self._target

    @property
    def snap(self) -> CadSnap | None:
        """The object snap under the cursor, at a point prompt."""
        return self._snap

    @property
    def preview(self) -> QTransform | None:
        """How the dragged entities are drawn now, in physical pixels: the
        transform the point under the cursor would apply to them."""
        if self._dragged is None or self._base is None or self._target is None:
            return None
        return _screen_transform(
            self._dragged.kind,
            self.view.to_screen(*self._base),
            self.view.to_screen(*self._target),
            math.dist(self._base, self._target),
        )

    def ask_point(self, base: Point | None, drag: Drag | None = None) -> None:
        """A command asks for a point, from ``base`` if it has one; ``drag``
        follows the cursor from it."""
        self._asking, self._base, self._dragged = True, base, drag
        self._target = self._snap = None
        self._set_hover(None)
        self.update()

    def end_point(self) -> None:
        """No point is asked for any more."""
        if self._asking:
            self._asking, self._base, self._target, self._snap = False, None, None, None
            self.update()

    @property
    def grips(self) -> tuple[CadGrip, ...]:
        """The grips shown."""
        return self._grips

    @property
    def hot(self) -> CadGrip | None:
        """The grip being edited, drawn red."""
        return self._hot

    def show_grips(self, grips: tuple[CadGrip, ...], hot: CadGrip | None = None) -> None:
        """Shows these grips, ``hot`` the one being edited."""
        if (grips, hot) != (self._grips, self._hot):
            self._grips, self._hot = grips, hot
            self.update()

    def grip_at(self, position: QPointF) -> CadGrip | None:
        """The grip under ``position`` (logical pixels), the closest."""
        ratio = self.devicePixelRatioF()
        best: tuple[float, CadGrip] | None = None
        for grip in self._grips:
            x, y = self.view.to_screen(*grip.point)
            d = max(abs(x / ratio - position.x()), abs(y / ratio - position.y()))
            if d <= GRIP_PIXELS and (best is None or d < best[0]):
                best = (d, grip)
        return None if best is None else best[1]

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
            if self._asking:
                self._aim(event.position())
                if self._target is not None:
                    self.point_picked.emit(self._target)
                event.accept()
                return
            grip = self.grip_at(event.position())
            if grip is not None:
                self.grip_clicked.emit(grip)
                event.accept()
                return
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
        elif self._asking:
            self._aim(position)
        else:
            self._set_hover(self.pick_at(position))
        world = self.view.to_world(*self._physical(position))
        if self.drafting is not None:
            self.drafting.cursor = world
        self.cursor_moved.emit(*world)

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
            self.escape_pressed.emit()
            event.accept()
            return
        if event.text() or event.key() in (Qt.Key.Key_Return, Qt.Key.Key_Enter):
            # A copy: Qt deletes the event when this handler returns.
            self.key_typed.emit(
                QKeyEvent(QEvent.Type.KeyPress, event.key(), event.modifiers(), event.text())
            )
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

    def _aim(self, position: QPointF) -> None:
        """Where a click at ``position`` puts the point: the snap found
        there, else the cursor kept by ORTHO, else the cursor."""
        world = self.view.to_world(*self._physical(position))
        snaps = self.drafting.active_snaps() if self.drafting is not None else ()
        found = None
        if snaps:
            aperture = SNAP_PIXELS * self.devicePixelRatioF() / self.view.scale
            found = self.document.snap(world, aperture, list(snaps), last_point=self._base)
        if found is not None:
            target = found.point
        elif self.drafting is not None and self.drafting.ortho and self._base is not None:
            target = ortho(self._base, world)
        else:
            target = world
        if (target, found) != (self._target, self._snap):
            self._target, self._snap = target, found
            self.update()

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
        preview = self.preview
        if preview is not None and self._dragged is not None:
            painter.setTransform(preview, combine=True)
            painter.setPen(QPen(self.palette().text().color(), 1 * ratio))
            self._draw_outlines(painter, self._dragged.entity_ids)
        painter.restore()
        self._paint_grips(painter)
        if self._asking:
            self._paint_point(painter)
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

    def _paint_grips(self, painter: QPainter) -> None:
        """Grips as filled squares, the hot one red."""
        ratio = self.devicePixelRatioF()
        r = GRIP_PIXELS
        painter.setPen(QPen(self.palette().base().color(), 1))
        for grip in self._grips:
            x, y = self.view.to_screen(*grip.point)
            painter.setBrush(HOT if grip == self._hot else SELECTED)
            painter.drawRect(QRectF(x / ratio - r, y / ratio - r, 2 * r, 2 * r))
        painter.setBrush(Qt.BrushStyle.NoBrush)

    def _paint_point(self, painter: QPainter) -> None:
        """The rubber band from the last point and the snap marker."""
        if self._target is None:
            return
        ratio = self.devicePixelRatioF()

        def logical(p: Point) -> QPointF:
            x, y = self.view.to_screen(*p)
            return QPointF(x / ratio, y / ratio)

        at = logical(self._target)
        if self._base is not None and (self._dragged is None or self._dragged.band):
            painter.setPen(QPen(self.palette().text().color(), 1, Qt.PenStyle.DashLine))
            painter.drawLine(logical(self._base), at)
        if self._snap is not None:
            painter.setPen(QPen(MARKER, 2))
            painter.setBrush(Qt.BrushStyle.NoBrush)
            draw_marker(painter, self._snap.kind, at, MARKER_PIXELS)
            label = at + QPointF(MARKER_PIXELS + 4, -MARKER_PIXELS - 4)
            painter.drawText(label, snap_name(self._snap.kind))

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


def draw_marker(painter: QPainter, kind: str, at: QPointF, r: float) -> None:
    """AutoCAD's AutoSnap glyph for a snap, centred on ``at``."""
    x, y = at.x(), at.y()

    def p(dx: float, dy: float) -> QPointF:
        return QPointF(x + dx, y + dy)

    if kind == "END":
        painter.drawRect(QRectF(x - r, y - r, 2 * r, 2 * r))
    elif kind == "MID":
        painter.drawPolygon(QPolygonF([p(0, -r), p(r, r), p(-r, r)]))
    elif kind == "CEN":
        painter.drawEllipse(at, r, r)
    elif kind == "QUA":
        painter.drawPolygon(QPolygonF([p(0, -r), p(r, 0), p(0, r), p(-r, 0)]))
    elif kind == "INT":
        painter.drawLine(p(-r, -r), p(r, r))
        painter.drawLine(p(-r, r), p(r, -r))
    elif kind == "PER":
        painter.drawLine(p(-r, r), p(r, r))
        painter.drawLine(p(-r, r), p(-r, -r))
        painter.drawLine(p(-r, 0), p(0, 0))
        painter.drawLine(p(0, 0), p(0, r))
    elif kind == "TAN":
        painter.drawEllipse(at, r, r)
        painter.drawLine(p(-r, -r), p(r, -r))
    else:  # NEA: an hourglass
        painter.drawPolygon(QPolygonF([p(-r, -r), p(r, -r), p(-r, r), p(r, r)]))


def _screen_transform(
    kind: str, base: tuple[float, float], target: tuple[float, float], distance: float
) -> QTransform | None:
    """The screen transform of a drag from ``base`` to ``target`` (screen
    points; ``distance`` between them in the world). The view only scales,
    flips and shifts, so moving, rotating, scaling and mirroring look on
    screen as they do in the world, about the same points."""
    (bx, by), (tx, ty) = base, target
    if kind == "move":
        return QTransform.fromTranslate(tx - bx, ty - by)
    if distance == 0.0:
        return None  # no direction, no factor
    t = QTransform()
    t.translate(bx, by)
    direction = math.atan2(ty - by, tx - bx)
    if kind == "rotate":
        t.rotateRadians(direction)
    elif kind == "scale":
        t.scale(distance, distance)
    else:  # mirror about the line from base to target
        t.rotateRadians(direction)
        t.scale(1.0, -1.0)
        t.rotateRadians(-direction)
    t.translate(-bx, -by)
    return t


def _distance(a: QPointF, b: QPointF) -> float:
    return float(((a.x() - b.x()) ** 2 + (a.y() - b.y()) ** 2) ** 0.5)
