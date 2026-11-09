"""The viewport: a document drawn by the core and shown by Qt (ADR 0013, D5.3).

The core rasterizes the drawing into an image (``CadView``); this widget only
puts that image on screen, so no Python runs per entity. It renders at the
screen's physical resolution (high DPI) and again only when something
changed. Middle-button drag pans, the wheel zooms about the cursor, a
middle-button double click zooms to the extents, as in AutoCAD; a left
double click on an entity asks to edit it (a block: BEDIT, ADR 0023 D3).

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

The viewport draws its own crosshair (:mod:`arqstudiocad.ui.crosshair`,
ADR 0029 D6) over the cached image, the system cursor hidden; a mouse move
repaints only the old and the new crosshair, never the drawing.

The model can be seen from AutoCAD's preset views (ADR 0030), in
wireframe: :meth:`Viewport.set_view`. In every view picking and selecting
work on what it shows, and a picked point falls on the XY plane, as
AutoCAD's with the World UCS; where the view sees that plane edge on, the
point is typed (:attr:`Viewport.point_refused`). Object snaps, ORTHO and
grips work in plan only, for now.

A right-click, the Menu key or Shift+F10 asks for the context menu
(:attr:`Viewport.context_requested`, ADR 0029 D1); the window decides what
it does: the menu, a repeat (a right-click, by the user's preference), or
Enter during a command.

Keys typed in the viewport go to the command line (:attr:`Viewport.key_typed`),
as in AutoCAD, where typing anywhere answers the command.
"""

import logging
import math
import os
import time

from PySide6.QtCore import QEvent, QPoint, QPointF, QRectF, Qt, Signal
from PySide6.QtGui import (
    QColor,
    QContextMenuEvent,
    QCursor,
    QDragEnterEvent,
    QDragLeaveEvent,
    QDragMoveEvent,
    QDropEvent,
    QGuiApplication,
    QImage,
    QKeyEvent,
    QMouseEvent,
    QNativeInterface,
    QPaintEngine,
    QPainter,
    QPaintEvent,
    QRegion,
    QResizeEvent,
    QShowEvent,
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
from arqstudiocad.ui.block_drag import dragged_block
from arqstudiocad.ui.crosshair import Crosshair, crosshair_rects, crosshair_strokes
from arqstudiocad.ui.overlay import (
    RGBA,
    Fill,
    Label,
    Overlay,
    Stroke,
    marker,
    rectangle,
    rgba,
    to_core,
)
from arqstudiocad.ui.overlay import paint as paint_overlay

#: How much one wheel notch (120 eighths of a degree) zooms, in percent:
#: the default of Options > User preferences > Zoom, and its bounds.
WHEEL_PERCENT = 25
WHEEL_PERCENTS = (5, 100)
#: The zoom factor of one notch by default.
WHEEL_STEP = 1 + WHEEL_PERCENT / 100

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

#: Until the renderer is chosen at start (ADR 0034 D6), naming a GPU backend
#: here (``opengl``, ``vulkan``) makes viewports present with the GPU, on
#: X11 only.
GPU_VIEWPORT_ENV = "ARQ_GPU_VIEWPORT"

_log = logging.getLogger(__name__)


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


#: Seconds within which a keyboard context-menu event follows the Shift+F10
#: that already asked for the menu.
MENU_KEY_ECHO = 0.5

#: Model space's background, the core's default (``CadView.background``).
MODEL_SPACE_BACKGROUND = (33, 40, 48)

#: The block editor's background, so it is never mistaken for the drawing:
#: a mid grey, as AutoCAD's and progeCAD's, the same in both themes; ACI 7
#: stays white on it (maintainer, 2026-10-25). The default of Options >
#: Display > Block editor.
BLOCK_EDITOR_BACKGROUND = (96, 96, 96)


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
    #: A block dragged from the Blocks palette came over the drawing (its
    #: name): INSERT should start, so it follows the cursor (ADR 0025 D5).
    block_drag_entered = Signal(str)
    #: That drag ended: dropped (True), its point already picked, or left
    #: the drawing (False), so the INSERT it started should be cancelled.
    block_drag_ended = Signal(bool)
    #: That drag was dropped with no INSERT running yet (its clashes are
    #: asked about first): where, found as a click finds a point.
    block_dropped = Signal(object)
    #: An entity was double-clicked with the left button (its id), with no
    #: point asked for: it should be edited.
    entity_double_clicked = Signal(int)
    #: The view changed (its name, ``"top"`` for the plan; ADR 0030).
    view_changed = Signal(str)
    #: A click at a point prompt in a view that sees the XY plane edge on:
    #: the point must be typed (ADR 0030 D4).
    point_refused = Signal()
    #: A right-click, the Menu key or Shift+F10 (ADR 0029 D1): where the
    #: context menu would open, a ``QPoint`` on the screen, and whether the
    #: keyboard asked (True) or the right button (False).
    context_requested = Signal(object, bool)

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
        #: When Shift+F10 last asked for the context menu (``time.monotonic``).
        self._menu_key_at = -MENU_KEY_ECHO
        self._base: Point | None = None
        self._target: Point | None = None
        self._snap: CadSnap | None = None
        self._dragged: Drag | None = None  # what follows the cursor from the base
        # Grips of the selection, and the one being edited.
        self._grips: tuple[CadGrip, ...] = ()
        self._hot: CadGrip | None = None
        self.setMouseTracking(True)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        #: How the crosshair looks, and where it is (logical pixels; None
        #: while the cursor is away or the view is being panned).
        self._crosshair = Crosshair()
        #: The snap markers' color (Options > Display).
        self._marker = MARKER
        #: One wheel notch's zoom factor, and whether forward zooms out
        #: (Options > User preferences > Zoom).
        self._wheel_step = WHEEL_STEP
        self._wheel_reversed = False
        self._cursor_at: QPointF | None = None
        self.setCursor(self._rest_cursor())
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent)
        self.setMinimumSize(64, 64)
        # A block dragged from the palette over the drawing, by name.
        self._drop_name: str | None = None
        self.setAcceptDrops(True)
        # The GPU, opted into (GPU_VIEWPORT_ENV): the widget becomes a native
        # window wgpu presents to, and Qt never paints it (ADR 0034 D1).
        self._gpu_backend = _gpu_backend()
        self._gpu = False
        if self._gpu_backend is not None:
            self._set_native(True)
            app = QGuiApplication.instance()
            if app is not None:
                app.aboutToQuit.connect(self._detach_gpu)

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

    @property
    def view_name(self) -> str:
        """Which way the model is seen: ``"top"`` (the plan), ``"swiso"``..."""
        return self.view.view

    @property
    def in_plan(self) -> bool:
        """Whether the model is seen in plan, where snaps, ORTHO and grips
        work."""
        return self.view.view == "top"

    def set_view(self, name: str) -> None:
        """Sees the model from view ``name`` (ADR 0030), all of it."""
        if name == self.view.view:
            return
        self.view.view = name
        self._auto_fit = False
        self._hover = self._target = self._snap = None
        self.view_changed.emit(name)
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
        (bx, by), (tx, ty) = self._base, self._target
        world = _world_transform(self._dragged.kind, (0.0, 0.0), (tx - bx, ty - by))
        if world is None:
            return None
        # From the screen to the XY plane about the base point, the drag
        # there, and back: in plan only a scale, a flip and a shift; in a
        # view, its projection. About the base, survey coordinates stay
        # small.
        plane = self._plane_transform(self._base)
        to_plane, invertible = plane.inverted()
        return to_plane * world * plane if invertible else None

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
        for grip in self._shown_grips():
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

    # Qt documents returning no engine for WA_PaintOnScreen; PySide6's stub
    # says the engine is never None.
    def paintEngine(self) -> QPaintEngine | None:  # type: ignore[override]  # noqa: N802
        # None while the GPU paints: Qt must not paint over its window.
        return None if self._gpu else super().paintEngine()

    def showEvent(self, event: QShowEvent) -> None:  # noqa: N802 (Qt API)
        super().showEvent(event)
        if self._gpu_backend is not None and not self._gpu:
            self._attach_gpu()

    def event(self, event: QEvent) -> bool:
        kind = event.type()
        if kind == QEvent.Type.DeferredDelete:
            # Before Qt destroys the native window the GPU presents to.
            self._detach_gpu()
        elif kind == QEvent.Type.WinIdChange and self._gpu:
            self._detach_gpu()
            self._attach_gpu()
        return super().event(event)

    def paintEvent(self, event: QPaintEvent) -> None:  # noqa: N802 (Qt API)
        if self._gpu:
            ratio = self.devicePixelRatioF()
            items, labels = to_core(self.overlay(), self.font(), ratio)
            try:
                self.view.present(items, labels, ratio)
            except RuntimeError as error:
                self._gpu_failed(str(error))
                self.update()
            return
        if self._dirty or self._image is None:
            self._render()
        painter = QPainter(self)
        if self._image is not None:
            painter.drawImage(0, 0, self._image)
        paint_overlay(painter, self.overlay())
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
            if self._wheel_reversed:
                notches = -notches
            self.view.zoom_at(x, y, self._wheel_step**notches)
            self._redraw()
        event.accept()

    def mousePressEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self._drag = event.position()
            self.setCursor(Qt.CursorShape.ClosedHandCursor)
            # The hand alone while panning.
            self._move_crosshair(None)
            event.accept()
            return
        if event.button() == Qt.MouseButton.LeftButton:
            if self._asking:
                self._aim(event.position())
                if self._target is not None:
                    self.point_picked.emit(self._target)
                elif not self.in_plan:
                    self.point_refused.emit()
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
        if self._drag is None:
            self._move_crosshair(position)
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
        self._follow_cursor(position)

    def mouseReleaseEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.MiddleButton:
            self._drag = None
            self.setCursor(self._rest_cursor())
            self._move_crosshair(event.position())
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
        if event.button() == Qt.MouseButton.LeftButton and not self._asking:
            picked = self.pick_at(event.position())
            if picked is not None:
                self.entity_double_clicked.emit(picked)
            event.accept()
            return
        super().mouseDoubleClickEvent(event)

    def contextMenuEvent(self, event: QContextMenuEvent) -> None:  # noqa: N802 (Qt API)
        event.accept()
        if event.reason() == QContextMenuEvent.Reason.Mouse:
            self.context_requested.emit(event.globalPos(), False)
        elif time.monotonic() - self._menu_key_at > MENU_KEY_ECHO:
            self.context_requested.emit(self.menu_point(), True)
        # Else Shift+F10, already asked for when it was pressed.

    def menu_point(self) -> QPoint:
        """Where a menu opened from the keyboard goes: at the cursor, or at
        the center when the cursor is outside (ADR 0029 D1)."""
        at = self.mapFromGlobal(QCursor.pos())
        if not self.rect().contains(at):
            at = self.rect().center()
        return self.mapToGlobal(at)

    def keyPressEvent(self, event: QKeyEvent) -> None:  # noqa: N802 (Qt API)
        if event.key() == Qt.Key.Key_F10 and event.modifiers() == Qt.KeyboardModifier.ShiftModifier:
            # Not every platform turns Shift+F10 into a context-menu event;
            # where one does, it is not asked for twice.
            self._menu_key_at = time.monotonic()
            self.context_requested.emit(self.menu_point(), True)
            event.accept()
            return
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

    # Dropping a block from the palette (ADR 0025 D5)

    def dragEnterEvent(self, event: QDragEnterEvent) -> None:  # noqa: N802 (Qt API)
        found = dragged_block(event.mimeData())
        if found is None or found[0] != self.document.id:
            event.ignore()  # another drawing's block, or not a block
            return
        event.acceptProposedAction()
        self._drop_name = found[1]
        self.block_drag_entered.emit(found[1])
        self._follow_drop(event.position())

    def dragMoveEvent(self, event: QDragMoveEvent) -> None:  # noqa: N802 (Qt API)
        if self._drop_name is None:
            event.ignore()
            return
        event.acceptProposedAction()
        self._follow_drop(event.position())

    def dragLeaveEvent(self, event: QDragLeaveEvent) -> None:  # noqa: N802 (Qt API)
        self._move_crosshair(None)
        if self._drop_name is not None:
            self._drop_name = None
            self.update()
            self.block_drag_ended.emit(False)

    def dropEvent(self, event: QDropEvent) -> None:  # noqa: N802 (Qt API)
        if self._drop_name is None:
            event.ignore()
            return
        event.acceptProposedAction()
        self._follow_drop(event.position())
        target = self._target if self._asking else None
        dropped = None
        if not self._asking:
            self._aim(event.position())
            dropped, self._target, self._snap = self._target, None, None
        self._drop_name = None
        self.update()
        if target is not None:
            self.point_picked.emit(target)  # where INSERT puts the block
        elif dropped is not None:
            self.block_dropped.emit(dropped)
        self.block_drag_ended.emit(True)

    def _follow_drop(self, position: QPointF) -> None:
        """The dragged block follows the cursor, snapped, as at a click."""
        self._move_crosshair(position)
        if self._asking:
            self._aim(position)
        self._follow_cursor(position)
        self.update()

    def leaveEvent(self, event: object) -> None:  # noqa: N802 (Qt API)
        self._set_hover(None)
        self._move_crosshair(None)

    # The crosshair (ADR 0029 D6)

    @property
    def crosshair(self) -> Crosshair:
        """How the crosshair looks."""
        return self._crosshair

    def set_crosshair(self, style: Crosshair) -> None:
        if style != self._crosshair:
            self._crosshair = style
            if self._drag is None:
                self.setCursor(self._rest_cursor())
            self.update()

    @property
    def wheel(self) -> tuple[float, bool]:
        """One wheel notch's zoom factor, and whether it is reversed."""
        return self._wheel_step, self._wheel_reversed

    def set_wheel(self, percent: int, reverse: bool = False) -> None:
        """Each notch zooms by ``percent``; forward zooms in, or out when
        ``reversed`` (AutoCAD's ZOOMWHEEL 1)."""
        self._wheel_step = 1 + percent / 100
        self._wheel_reversed = reverse

    @property
    def marker_color(self) -> QColor:
        """The snap markers' color."""
        return self._marker

    def set_marker_color(self, color: str) -> None:
        """The snap markers in ``color`` (``#RRGGBB``), or AutoCAD's
        yellow when it is empty (maintainer, 2026-10-30)."""
        chosen = QColor(color) if color else QColor()
        marker = chosen if chosen.isValid() else MARKER
        if marker != self._marker:
            self._marker = marker
            self.update()

    def set_background(self, rgb: tuple[int, int, int]) -> None:
        """The drawing's background, drawn again."""
        if tuple(self.view.background) != rgb:
            self.view.background = rgb
            self._redraw()

    @property
    def cursor_at(self) -> QPointF | None:
        """Where the crosshair is drawn, or None when it is not."""
        return self._cursor_at

    def _rest_cursor(self) -> Qt.CursorShape:
        """The system cursor over the drawing: hidden under the crosshair,
        or the arrow when that is the type chosen."""
        if self._crosshair.type == "arrow":
            return Qt.CursorShape.ArrowCursor
        return Qt.CursorShape.BlankCursor

    def _move_crosshair(self, position: QPointF | None) -> None:
        """Moves the crosshair, repainting where it was and where it goes,
        not the drawing."""
        if position == self._cursor_at:
            return
        area = QRectF(self.rect())
        region = QRegion()
        for at in (self._cursor_at, position):
            if at is not None:
                for rect in crosshair_rects(self._crosshair, at, area, PICK_PIXELS):
                    region += rect
        self._cursor_at = position
        if not region.isEmpty():
            self.update(region)

    # Picking, in logical (widget) coordinates

    def pick_at(self, position: QPointF) -> int | None:
        """The entity under ``position``, within the pick box, in any
        view."""
        x, y = self._physical(position)
        if not self.in_plan:
            return self.view.pick(x, y, PICK_PIXELS * self.devicePixelRatioF())
        aperture = PICK_PIXELS * self.devicePixelRatioF() / self.view.scale
        return self.document.pick(self.view.to_world(x, y), aperture)

    def select_in(self, start: QPointF, end: QPointF) -> tuple[int, ...]:
        """What a window from ``start`` to ``end`` selects: left to right a
        window, right to left a crossing, in any view."""
        crossing = end.x() < start.x()
        if not self.in_plan:
            corners = (*self._physical(start), *self._physical(end))
            return tuple(self.view.select_window(*corners, crossing))
        a = self.view.to_world(*self._physical(start))
        b = self.view.to_world(*self._physical(end))
        return tuple(self.document.select_window(CadBounds2(a, b), crossing=crossing))

    def _aim(self, position: QPointF) -> None:
        """Where a click at ``position`` puts the point: the snap found
        there, else the cursor kept by ORTHO, else the cursor. In another
        view, the point of the XY plane under the cursor, without snaps or
        ORTHO for now; none where the plane is seen edge on."""
        if not self.in_plan:
            point = self.view.plane_point(*self._physical(position))
            if (point, None) != (self._target, self._snap):
                self._target, self._snap = point, None
                self.update()
            return
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

    def overlay(self) -> Overlay:
        """What is drawn over the drawing now, in logical pixels and in the
        order it is painted (ADR 0034 D4): the selection, the highlight, the
        dragged outlines, the grips, the rubber band and the snap, the drop
        frame, the window, and the crosshair. Painted by ``QPainter`` over
        the CPU image, or drawn by the GPU."""
        out = Overlay()
        text = rgba(self.palette().text().color())
        if self._selected:
            self._outline_strokes(out, self._selected, rgba(SELECTED), 2.0)
        if self._hover is not None:
            hover = rgba(SELECTED) if self._hover in self._selected else text
            self._outline_strokes(out, (self._hover,), hover, 3.0)
        preview = self.preview
        if preview is not None and self._dragged is not None:
            self._outline_strokes(out, self._dragged.entity_ids, text, 1.0, preview)
        self._grip_items(out)
        if self._asking:
            self._point_items(out, text)
        if self._drop_name is not None:
            highlight = rgba(self.palette().highlight().color())
            frame = QRectF(self.rect()).adjusted(2, 2, -2, -2)
            corners = rectangle((frame.left(), frame.top()), (frame.right(), frame.bottom()))
            out.items.append(Stroke(corners, highlight, 2.0, dashed=True, closed=True))
            out.labels.append(
                Label((12.0, 22.0), tr("Drop to insert {name}", name=self._drop_name), text)
            )
        if self._press is not None and self._band is not None:
            crossing = self._band.x() < self._press.x()
            color = QColor(CROSSING if crossing else SELECTED)
            box = QRectF(self._press, self._band).normalized()
            a, b = (box.left(), box.top()), (box.right(), box.bottom())
            fill = QColor(color)
            fill.setAlpha(FILL_ALPHA)
            out.items.append(Fill(a, b, rgba(fill)))
            out.items.append(
                Stroke(rectangle(a, b), rgba(color), 1.0, dashed=crossing, closed=True)
            )
        if self._cursor_at is not None:
            out.items.extend(
                crosshair_strokes(
                    self._crosshair,
                    self._cursor_at,
                    QRectF(self.rect()),
                    PICK_PIXELS,
                    QColor(*self.view.background),
                )
            )
        return out

    def _outline_strokes(
        self,
        out: Overlay,
        entity_ids: tuple[int, ...],
        color: RGBA,
        width: float,
        transform: QTransform | None = None,
    ) -> None:
        """The entities' outlines, which come in physical pixels, moved by
        ``transform`` when they are being dragged."""
        ratio = self.devicePixelRatioF()
        for polyline in self.view.outlines(entity_ids):
            points = [QPointF(x, y) for x, y in polyline]
            if transform is not None:
                points = [transform.map(p) for p in points]
            out.items.append(
                Stroke(tuple((p.x() / ratio, p.y() / ratio) for p in points), color, width)
            )

    def _grip_items(self, out: Overlay) -> None:
        """Grips as filled squares, the hot one red, edged in the base color."""
        ratio = self.devicePixelRatioF()
        r = GRIP_PIXELS
        edge = rgba(self.palette().base().color())
        for grip in self._shown_grips():
            x, y = self.view.to_screen(*grip.point)
            a, b = (x / ratio - r, y / ratio - r), (x / ratio + r, y / ratio + r)
            out.items.append(Fill(a, b, rgba(HOT if grip == self._hot else SELECTED)))
            out.items.append(Stroke(rectangle(a, b), edge, 1.0, closed=True))

    def _point_items(self, out: Overlay, text: RGBA) -> None:
        """The rubber band from the last point and the snap marker."""
        if self._target is None:
            return
        at = self.screen_point(self._target)
        to = (at.x(), at.y())
        if self._base is not None and (self._dragged is None or self._dragged.band):
            base = self.screen_point(self._base)
            out.items.append(Stroke(((base.x(), base.y()), to), text, 1.0, dashed=True))
        if self._snap is not None:
            color = rgba(self._marker)
            out.items.extend(marker(self._snap.kind, to, MARKER_PIXELS, color, 2.0))
            label = (at.x() + MARKER_PIXELS + 4, at.y() - MARKER_PIXELS - 4)
            out.labels.append(Label(label, snap_name(self._snap.kind), color))

    def screen_point(self, point: Point) -> QPointF:
        """Where a point of the XY plane is seen, in logical pixels, in any
        view: the rubber band's and the snap marker's place."""
        ratio = self.devicePixelRatioF()
        x, y = self.view.plane_to_screen(*point)
        return QPointF(x / ratio, y / ratio)

    def _follow_cursor(self, position: QPointF) -> None:
        """The point of the XY plane under the cursor, for the status bar
        and typed distances; none where the view sees the plane edge on."""
        point = self.view.plane_point(*self._physical(position))
        if point is None:
            return
        if self.drafting is not None:
            self.drafting.cursor = point
        self.cursor_moved.emit(*point)

    def _plane_transform(self, origin: Point) -> QTransform:
        """The XY plane, from ``origin``, to physical pixels: affine in
        every view."""
        x, y = origin
        ox, oy = self.view.plane_to_screen(x, y)
        xx, xy = self.view.plane_to_screen(x + 1.0, y)
        yx, yy = self.view.plane_to_screen(x, y + 1.0)
        return QTransform(xx - ox, xy - oy, yx - ox, yy - oy, ox, oy)

    def _shown_grips(self) -> tuple[CadGrip, ...]:
        """The grips drawn and clicked: none outside the plan, for now."""
        return self._grips if self.in_plan else ()

    def _physical(self, point: QPointF) -> tuple[float, float]:
        ratio = self.devicePixelRatioF()
        return point.x() * ratio, point.y() * ratio

    def _redraw(self) -> None:
        self._dirty = True
        self.update()

    # The GPU (ADR 0034)

    @property
    def gpu(self) -> tuple[str, str, str] | None:
        """``(backend, adapter, device_type)`` while the GPU presents."""
        return self.view.gpu if self._gpu else None

    def _attach_gpu(self) -> None:
        backend = self._gpu_backend
        if backend is None:
            return
        display = _x11_display()
        if display is None:
            self._gpu_failed("the GPU viewport needs Qt on X11 (xcb) for now")
            return
        try:
            adapter = self.view.attach_window(backend, display, int(self.winId()))
        except (RuntimeError, ValueError) as error:
            self._gpu_failed(str(error))
            return
        self._gpu = True
        _log.info("viewport presented by the GPU: %s", adapter)
        self._redraw()

    def _detach_gpu(self) -> None:
        if self._gpu:
            self.view.detach_window()
            self._gpu = False

    def _gpu_failed(self, reason: str) -> None:
        """Back to the CPU rasterizer in this widget, for this session."""
        _log.warning("the GPU viewport stopped, drawing on the CPU: %s", reason)
        self._detach_gpu()
        self._gpu_backend = None
        self._set_native(False)
        self._redraw()

    def _set_native(self, on: bool) -> None:
        for attribute in (
            Qt.WidgetAttribute.WA_NativeWindow,
            Qt.WidgetAttribute.WA_PaintOnScreen,
            Qt.WidgetAttribute.WA_NoSystemBackground,
        ):
            if attribute != Qt.WidgetAttribute.WA_NativeWindow or on:
                self.setAttribute(attribute, on)

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


def _world_transform(kind: str, base: Point, target: Point) -> QTransform | None:
    """The transform of the XY plane a drag from ``base`` to ``target``
    applies: moving, rotating, scaling or mirroring about the line between
    them. None where they coincide and only a move has a meaning."""
    (bx, by), (tx, ty) = base, target
    if kind == "move":
        return QTransform.fromTranslate(tx - bx, ty - by)
    distance = math.dist(base, target)
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


def _x11_display() -> int | None:
    """Qt's ``Display*`` when it runs on X11 (xcb), else None. PySide6 has
    the X11 interface only in its Linux build, so it is looked up, never
    named: on Windows and macOS there is nothing to find."""
    app = QGuiApplication.instance()
    if not isinstance(app, QGuiApplication) or app.platformName() != "xcb":
        return None
    x11 = getattr(QNativeInterface, "QX11Application", None)
    native = app.nativeInterface()
    if x11 is None or not isinstance(native, x11):
        return None
    display = int(native.display())
    return display or None


def _gpu_backend() -> str | None:
    """The backend opted into with ``GPU_VIEWPORT_ENV``, if any."""
    name = os.environ.get(GPU_VIEWPORT_ENV, "").strip()
    return name or None
