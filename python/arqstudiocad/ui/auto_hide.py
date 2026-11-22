"""Pinned and auto-hidden panels, as AutoCAD's palettes (ADR 0015).

Every panel's title bar has a pin. Pinned (the default), the panel stays
open where it is docked. Unpinned, it collapses to a tab with its name on
the side it was docked; resting the mouse on the tab, or clicking it, slides
the panel out over the drawing, and it slides back when the mouse leaves it
(unless something in it has the keyboard) or the drawing is clicked.
Pinning it again docks it back, open.

While a panel is auto-hidden its contents live in an overlay next to the
tab; the dock itself stays hidden.

Before the pin, a mode button shows how the panel behaves and changes it
(maintainer, 2026-11-19): an anchor while it is docked, a window while it
floats, as AutoCAD's palettes. Clicking it floats the panel, or docks it
back where it was; dragging and double-clicking the title still do too. A
floating panel keeps its pin: unpinned, it rolls up to its title bar when
the mouse leaves it and rolls down when the mouse comes back, as an
AutoCAD palette set to auto-hide.
"""

from collections.abc import Callable, Iterable, Mapping

from PySide6.QtCore import QEvent, QObject, QPoint, QRect, QSettings, QSize, Qt, QTimer
from PySide6.QtGui import QColor, QCursor, QIcon, QMouseEvent, QPainter, QPaintEvent, QPixmap
from PySide6.QtWidgets import (
    QAbstractButton,
    QApplication,
    QDockWidget,
    QFrame,
    QHBoxLayout,
    QLabel,
    QMainWindow,
    QStyle,
    QToolBar,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.i18n import tr

AUTO_HIDE_SETTING = "panels/auto_hidden"
#: How wide (or tall) each unpinned panel slides out, as the user left it.
OVERLAY_SIZE_SETTING = "panels/overlay_sizes"
#: The overlay's inner edge, in pixels, that drags to resize it.
GRIP = 6
#: The least an unpinned panel slides out to when it has no width of its own.
USEFUL_SIZE = 200
#: The floating panels set to roll up to their title bar.
ROLL_UP_SETTING = "panels/rolled_up"
#: How long the mouse rests on a side tab before its panel slides out
#: (maintainer, 2026-11-22): passing over the edge opens nothing.
HOVER_DELAY_MS = 250
#: How long the mouse may be away before an open overlay slides back.
LEAVE_DELAY_MS = 400

Side = Qt.DockWidgetArea
SIDES: dict[Side, Qt.ToolBarArea] = {
    Qt.DockWidgetArea.LeftDockWidgetArea: Qt.ToolBarArea.LeftToolBarArea,
    Qt.DockWidgetArea.RightDockWidgetArea: Qt.ToolBarArea.RightToolBarArea,
    Qt.DockWidgetArea.TopDockWidgetArea: Qt.ToolBarArea.TopToolBarArea,
    Qt.DockWidgetArea.BottomDockWidgetArea: Qt.ToolBarArea.BottomToolBarArea,
}
SIDE_NAMES = {
    Qt.DockWidgetArea.LeftDockWidgetArea: "left",
    Qt.DockWidgetArea.RightDockWidgetArea: "right",
    Qt.DockWidgetArea.TopDockWidgetArea: "top",
    Qt.DockWidgetArea.BottomDockWidgetArea: "bottom",
}


def pin_icon(pinned: bool, color: QColor) -> QIcon:
    """A drawn pin: upright when pinned, lying down when not."""
    pixmap = QPixmap(16, 16)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    if not pinned:
        painter.translate(16, 0)
        painter.rotate(90)
    painter.setPen(color)
    painter.setBrush(color)
    painter.drawRect(5, 1, 6, 6)  # the head
    painter.drawLine(3, 8, 13, 8)  # the collar
    painter.drawLine(8, 8, 8, 15)  # the needle
    painter.end()
    return QIcon(pixmap)


def mode_icon(floating: bool, color: QColor) -> QIcon:
    """A drawn mode: an anchor for a docked panel, a window for a floating
    one."""
    pixmap = QPixmap(16, 16)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    painter.setPen(color)
    if floating:
        painter.drawRect(1, 3, 13, 11)  # the window
        painter.fillRect(1, 3, 14, 3, color)  # its title bar
    else:
        painter.drawEllipse(6, 1, 4, 4)  # the ring
        painter.drawLine(8, 5, 8, 14)  # the shank
        painter.drawLine(5, 7, 11, 7)  # the stock
        painter.drawArc(2, 6, 12, 9, 180 * 16, 180 * 16)  # the arms
    painter.end()
    return QIcon(pixmap)


def is_vertical(side: Side) -> bool:
    return side in (Qt.DockWidgetArea.LeftDockWidgetArea, Qt.DockWidgetArea.RightDockWidgetArea)


class PanelTitleBar(QWidget):
    """A panel's title, as OpenCADStudio's: its name, the mode, the pin and
    close. Dragging and double-clicking still move and float the panel
    (QDockWidget handles the mouse events the bar leaves alone)."""

    def __init__(self, dock: QDockWidget, pinned: bool = True) -> None:
        super().__init__(dock)
        self.title = QLabel(dock.windowTitle())
        self.mode = QToolButton()
        self.mode.setAutoRaise(True)
        self.floating = dock.isFloating()
        self.pin = QToolButton()
        self.pin.setAutoRaise(True)
        self.close_button = QToolButton()
        self.close_button.setAutoRaise(True)
        style = self.style()
        self.close_button.setIcon(style.standardIcon(QStyle.StandardPixmap.SP_TitleBarCloseButton))
        row = QHBoxLayout(self)
        row.setContentsMargins(4, 1, 1, 1)
        row.setSpacing(0)
        row.addWidget(self.title, 1)
        for button in (self.mode, self.pin, self.close_button):
            button.setIconSize(QSize(12, 12))
            row.addWidget(button)
        dock.windowTitleChanged.connect(self.title.setText)
        self.set_pinned(pinned)
        self.set_floating(self.floating)

    def set_pinned(self, pinned: bool) -> None:
        self.pinned = pinned
        # The application's palette: the theme, even before it reaches this bar.
        self.pin.setIcon(pin_icon(pinned, QApplication.palette(self).windowText().color()))
        self.retranslate()

    def set_floating(self, floating: bool) -> None:
        self.floating = floating
        self.mode.setIcon(mode_icon(floating, QApplication.palette(self).windowText().color()))
        self.retranslate()

    def retranslate(self) -> None:
        self.mode.setToolTip(tr("Dock") if self.floating else tr("Float"))
        self.pin.setToolTip(tr("Auto-hide") if self.pinned else tr("Keep open"))
        self.close_button.setToolTip(tr("Close"))


class SideTab(QAbstractButton):
    """A collapsed panel's tab: its name, written along the side."""

    def __init__(self, dock: QDockWidget, side: Side) -> None:
        super().__init__()
        self.dock = dock
        self.side = side
        self.setText(dock.windowTitle())
        self.setObjectName(f"tab_{dock.objectName()}")
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setAttribute(Qt.WidgetAttribute.WA_Hover)
        dock.windowTitleChanged.connect(self._on_title)

    def _on_title(self, title: str) -> None:
        self.setText(title)
        self.updateGeometry()

    def sizeHint(self) -> QSize:  # noqa: N802 (Qt API)
        metrics = self.fontMetrics()
        along = metrics.horizontalAdvance(self.text()) + 16
        across = metrics.height() + 8
        return QSize(across, along) if is_vertical(self.side) else QSize(along, across)

    def paintEvent(self, event: QPaintEvent) -> None:  # noqa: N802 (Qt API)
        painter = QPainter(self)
        palette = self.palette()
        hovered = self.underMouse() or self.isDown()
        painter.fillRect(self.rect(), palette.highlight() if hovered else palette.button())
        painter.setPen(palette.mid().color())
        painter.drawRect(self.rect().adjusted(0, 0, -1, -1))
        painter.setPen(
            palette.highlightedText().color() if hovered else palette.buttonText().color()
        )
        if is_vertical(self.side):
            painter.translate(self.width(), 0)
            painter.rotate(90)
            area = QRect(0, 0, self.height(), self.width())
        else:
            area = self.rect()
        painter.drawText(area, Qt.AlignmentFlag.AlignCenter, self.text())


class EdgeGrip(QWidget):
    """An overlay's inner edge, the one facing the drawing: dragged, it
    resizes the overlay (maintainer, 2026-11-20). It reports where its own
    top-left corner goes, so the edge moves exactly as the mouse does,
    wherever on the grip it was taken."""

    def __init__(
        self,
        parent: QWidget,
        side: Side,
        dragged: Callable[[QPoint], None],
        ended: Callable[[], None],
    ) -> None:
        super().__init__(parent)
        self.setObjectName("overlay_edge")
        self.setCursor(
            Qt.CursorShape.SizeHorCursor if is_vertical(side) else Qt.CursorShape.SizeVerCursor
        )
        self._dragged, self._ended = dragged, ended
        self.dragging = False
        self._held = QPoint()

    def mousePressEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if event.button() == Qt.MouseButton.LeftButton:
            self.dragging = True
            self._held = event.position().toPoint()
            event.accept()

    def mouseMoveEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if self.dragging:
            self._dragged(event.globalPosition().toPoint() - self._held)
            event.accept()

    def mouseReleaseEvent(self, event: QMouseEvent) -> None:  # noqa: N802 (Qt API)
        if self.dragging:
            self.dragging = False
            self._ended()
            event.accept()


class Overlay(QFrame):
    """An unpinned panel slid out over the drawing: its title bar with the
    pin, then its contents; its edge facing the drawing resizes it."""

    def __init__(
        self,
        dock: QDockWidget,
        parent: QWidget,
        side: Side = Qt.DockWidgetArea.RightDockWidgetArea,
        dragged: Callable[[QPoint], None] | None = None,
        ended: Callable[[], None] | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName(f"overlay_{dock.objectName()}")
        self.setFrameShape(QFrame.Shape.StyledPanel)
        self.setAutoFillBackground(True)
        self.side = side
        self.title_bar = PanelTitleBar(dock, pinned=False)
        self.contents: QWidget | None = None
        self._column = QVBoxLayout(self)
        # Room for the grip on the side facing the drawing: it covers nothing.
        margins = {
            Qt.DockWidgetArea.LeftDockWidgetArea: (1, 1, GRIP, 1),
            Qt.DockWidgetArea.RightDockWidgetArea: (GRIP, 1, 1, 1),
            Qt.DockWidgetArea.TopDockWidgetArea: (1, 1, 1, GRIP),
            Qt.DockWidgetArea.BottomDockWidgetArea: (1, GRIP, 1, 1),
        }
        self._column.setContentsMargins(*margins.get(side, (1, 1, 1, 1)))
        self._column.setSpacing(0)
        self._column.addWidget(self.title_bar)
        self.grip = EdgeGrip(self, side, dragged or (lambda _point: None), ended or (lambda: None))
        self.hide()

    def resizeEvent(self, event: object) -> None:  # noqa: N802 (Qt API)
        w, h = self.width(), self.height()
        place = {
            Qt.DockWidgetArea.LeftDockWidgetArea: QRect(w - GRIP, 0, GRIP, h),
            Qt.DockWidgetArea.RightDockWidgetArea: QRect(0, 0, GRIP, h),
            Qt.DockWidgetArea.TopDockWidgetArea: QRect(0, h - GRIP, w, GRIP),
            Qt.DockWidgetArea.BottomDockWidgetArea: QRect(0, 0, w, GRIP),
        }
        self.grip.setGeometry(place.get(self.side, QRect(0, 0, GRIP, h)))
        self.grip.raise_()

    def hold(self, contents: QWidget) -> None:
        self.contents = contents
        self._column.addWidget(contents, 1)
        contents.show()

    def release(self) -> QWidget | None:
        contents, self.contents = self.contents, None
        if contents is not None:
            self._column.removeWidget(contents)
        return contents


class AutoHide(QObject):
    """The pins of ``window``'s panels and the tabs of the unpinned ones."""

    def __init__(
        self,
        window: QMainWindow,
        panels: Iterable[QDockWidget],
        settings: QSettings | None = None,
        useful: Mapping[QDockWidget, int] | None = None,
    ) -> None:
        """``useful``: the least each panel slides out to, so that what it
        holds is not cut (maintainer, 2026-11-20)."""
        super().__init__(window)
        self._useful = dict(useful or {})
        #: An overlay's edge is being dragged: it stays out meanwhile.
        self._resizing = False
        #: The tab group each unpinned panel was unpinned with: pinned and
        #: unpinned together, their side tabs side by side.
        self.groups: dict[QDockWidget, int] = {}
        self._next_group = 0
        #: The side tab the mouse rests on, until its panel slides out.
        self._hovered: SideTab | None = None
        self._window = window
        self._settings = settings
        self.panels = list(panels)
        self.title_bars: dict[QDockWidget, PanelTitleBar] = {}
        #: The unpinned panels: the side each was docked on, its tab, its overlay.
        self.sides: dict[QDockWidget, Side] = {}
        self.tabs: dict[QDockWidget, SideTab] = {}
        self.overlays: dict[QDockWidget, Overlay] = {}
        self._sizes: dict[QDockWidget, int] = {}
        #: The floating panels set to roll up, and the height each had when
        #: it was rolled up last.
        self.rolling: set[QDockWidget] = set()
        self.rolled: dict[QDockWidget, int] = {}
        #: A rolled-up panel's own minimum width, held at its width meanwhile.
        self._min_widths: dict[QDockWidget, int] = {}
        self.open: QDockWidget | None = None
        self._enabled = True
        self.bars: dict[Side, QToolBar] = {}
        for side, area in SIDES.items():
            bar = QToolBar(window)
            bar.setObjectName(f"auto_hide_{SIDE_NAMES[side]}")
            bar.setMovable(False)
            bar.setFloatable(False)
            bar.toggleViewAction().setVisible(False)
            bar.setOrientation(
                Qt.Orientation.Vertical if is_vertical(side) else Qt.Orientation.Horizontal
            )
            window.addToolBar(area, bar)
            bar.hide()
            self.bars[side] = bar
        for dock in self.panels:
            title = PanelTitleBar(dock)
            title.mode.clicked.connect(lambda _checked=False, d=dock: self.toggle_floating(d))
            title.pin.clicked.connect(lambda _checked=False, d=dock: self._pin_clicked(d))
            title.close_button.clicked.connect(dock.close)
            dock.topLevelChanged.connect(
                lambda floating, d=dock: self._top_level_changed(d, bool(floating))
            )
            dock.setTitleBarWidget(title)
            self.title_bars[dock] = title
        self._leave_timer = QTimer(self)
        self._leave_timer.setInterval(LEAVE_DELAY_MS)
        self._leave_timer.timeout.connect(self._check_leave)
        self._hover_timer = QTimer(self)
        self._hover_timer.setSingleShot(True)
        self._hover_timer.setInterval(HOVER_DELAY_MS)
        self._hover_timer.timeout.connect(self._hover_rested)
        self._roll_timer = QTimer(self)
        self._roll_timer.setInterval(LEAVE_DELAY_MS)
        self._roll_timer.timeout.connect(self._check_rolling)
        app = QApplication.instance()
        if app is not None:
            app.installEventFilter(self)

    # Pinning

    def is_pinned(self, dock: QDockWidget) -> bool:
        return dock not in self.sides

    def unpin(self, dock: QDockWidget, side: Side | None = None, group: int | None = None) -> bool:
        """Collapses ``dock`` to a tab on ``side`` (the side it is docked
        on), with ``group``'s tabs if given; False for a floating or already
        unpinned panel."""
        if not self.is_pinned(dock) or (side is None and dock.isFloating()):
            return False
        side = side if side is not None else self._window.dockWidgetArea(dock)
        if side not in SIDES:
            return False
        size = dock.width() if is_vertical(side) else dock.height()
        self._sizes[dock] = max(size, 180)
        contents = dock.widget()
        overlay = Overlay(
            dock,
            self._window,
            side,
            lambda point: self._drag_edge(dock, point),
            self._end_drag,
        )
        overlay.title_bar.pin.clicked.connect(lambda _checked=False: self.pin_group(dock))
        overlay.title_bar.mode.clicked.connect(lambda _checked=False: self.toggle_floating(dock))
        overlay.title_bar.close_button.clicked.connect(lambda _checked=False: self.close(dock))
        if contents is not None:
            overlay.hold(contents)
        if group is None:
            group = self._new_group()
        tab = SideTab(dock, side)
        tab.clicked.connect(lambda _checked=False: self.expand(dock))
        here = [d for d, t in self.tabs.items() if t.side == side]
        if here and not any(self.groups.get(d) == group for d in here):
            # Another group's tabs: a gap between them.
            self.bars[side].addSeparator()
        self.bars[side].addWidget(tab)
        self.groups[dock] = group
        self.bars[side].show()
        self.sides[dock], self.tabs[dock], self.overlays[dock] = side, tab, overlay
        dock.toggleViewAction().setEnabled(False)
        dock.hide()
        overlay.setEnabled(self._enabled)
        self._save()
        return True

    def pin(self, dock: QDockWidget) -> None:
        """Opens an unpinned panel again, docked where the arrangement keeps
        it (it never left the window's layout, only hid)."""
        if self.is_pinned(dock):
            return
        side = self.sides.pop(dock)
        self.groups.pop(dock, None)
        self._remove_tab(dock)
        overlay = self.overlays.pop(dock)
        contents = overlay.release()
        overlay.deleteLater()
        if self.open is dock:
            self.open = None
        if contents is not None:
            dock.setWidget(contents)
        dock.toggleViewAction().setEnabled(True)
        dock.show()
        # In front of its tabs: the panel just pinned is the one wanted.
        dock.raise_()
        # Docked at the size it slid out to, once its tab group has turned to
        # it: before that, some platforms (Windows) drop the request.
        orientation = Qt.Orientation.Horizontal if is_vertical(side) else Qt.Orientation.Vertical
        size = self.overlay_size(dock)
        self._window.resizeDocks([dock], [size], orientation)
        QTimer.singleShot(0, lambda: self._window.resizeDocks([dock], [size], orientation))
        self._save()

    # Tab groups (maintainer, 2026-11-22)

    def _new_group(self) -> int:
        self._next_group += 1
        return self._next_group

    def unpin_group(self, dock: QDockWidget) -> bool:
        """The pin of a docked panel: it and the panels sharing its tabs
        collapse together, their side tabs side by side, as an AutoCAD
        palette group. A panel alone collapses alone."""
        if not self.is_pinned(dock) or dock.isFloating():
            return False
        shared = set(self._window.tabifiedDockWidgets(dock))
        members = [d for d in self.panels if d is dock or d in shared]
        group = self._new_group()
        unpinned = False
        for member in members:
            if self.is_pinned(member) and not member.isFloating() and member.isVisible():
                unpinned = self.unpin(member, None, group) or unpinned
        if self.is_pinned(dock):
            unpinned = self.unpin(dock, None, group) or unpinned
        return unpinned

    def pin_group(self, dock: QDockWidget) -> None:
        """The pin of an unpinned panel: its whole group docks again, as
        tabs, the one pinned in front."""
        group = self.groups.get(dock)
        for member in [d for d in list(self.sides) if d is not dock]:
            if group is not None and self.groups.get(member) == group:
                self.pin(member)
        self.pin(dock)

    def close(self, dock: QDockWidget) -> None:
        """Closes an unpinned panel: docked back, then hidden, as closing a
        pinned one (the View menu opens it again); its group stays as it is."""
        self.pin(dock)
        dock.close()

    # Docked or floating (maintainer, 2026-11-19)

    def toggle_floating(self, dock: QDockWidget) -> None:
        """The mode button: a docked panel floats, a floating one docks back
        where it was. An unpinned panel is pinned first, docked, then floats."""
        self.pin(dock)
        dock.setFloating(not dock.isFloating())

    def _pin_clicked(self, dock: QDockWidget) -> None:
        if dock.isFloating():
            self.set_rolling(dock, dock not in self.rolling)
        else:
            self.unpin_group(dock)

    def _top_level_changed(self, dock: QDockWidget, floating: bool) -> None:
        self.title_bars[dock].set_floating(floating)
        if not floating:
            # Docked again: it stays open, as a pinned panel.
            self.set_rolling(dock, False)

    def set_rolling(self, dock: QDockWidget, rolling: bool) -> None:
        """Sets floating ``dock`` to roll up to its title bar when the mouse
        leaves it, or to stay open; a docked panel never rolls."""
        rolling = rolling and dock.isFloating()
        if not rolling:
            self.roll_down(dock)
            self.rolling.discard(dock)
        else:
            self.rolling.add(dock)
        self.title_bars[dock].set_pinned(not rolling)
        if self.rolling:
            self._roll_timer.start()
        else:
            self._roll_timer.stop()
        self._save()

    def roll_up(self, dock: QDockWidget) -> None:
        """Only the title bar of a floating panel stays: its contents hide."""
        contents = dock.widget()
        if dock in self.rolled or contents is None or not dock.isFloating():
            return
        width, self.rolled[dock] = dock.width(), dock.height()
        # Its width stays: only the height rolls up. Without its contents
        # the window would shrink to its title's width.
        self._min_widths[dock] = dock.minimumWidth()
        dock.setMinimumWidth(width)
        contents.hide()
        dock.resize(width, self.title_bars[dock].sizeHint().height() + 2)

    def roll_down(self, dock: QDockWidget) -> None:
        """The floating panel's contents again, at the height it had."""
        height = self.rolled.pop(dock, None)
        contents = dock.widget()
        if height is None or contents is None:
            return
        width = dock.width()
        dock.setMinimumWidth(self._min_widths.pop(dock, 0))
        contents.show()
        dock.resize(width, height)

    def _check_rolling(self) -> None:
        """Rolls up each rolling panel the mouse has left, unless something
        in it has the keyboard; rolls down the one it is over."""
        focus = QApplication.focusWidget()
        for dock in list(self.rolling):
            if not dock.isVisible():
                continue
            if self._under_mouse(dock):
                self.roll_down(dock)
            elif focus is None or not dock.isAncestorOf(focus):
                self.roll_up(dock)

    def pin_all(self) -> None:
        for dock in list(self.sides):
            self.pin(dock)

    def keep_hidden(self) -> None:
        """After a restored arrangement: unpinned panels stay hidden and the
        tab bars show only where there are tabs."""
        self.collapse()
        for dock in self.sides:
            dock.hide()
        for side, bar in self.bars.items():
            bar.setVisible(any(tab.side == side for tab in self.tabs.values()))

    def restore(self) -> None:
        """Unpins the panels that were unpinned last time, at the sizes they
        slid out to, and sets the floating ones that rolled up to roll up
        again."""
        # Read first: unpinning saves the sizes the panels have docked.
        sizes = None if self._settings is None else self._settings.value(OVERLAY_SIZE_SETTING)
        rolling = None if self._settings is None else self._settings.value(ROLL_UP_SETTING)
        if isinstance(rolling, str) and rolling:
            for name in rolling.split(","):
                dock = next((d for d in self.panels if d.objectName() == name), None)
                if dock is not None and dock.isFloating():
                    self.set_rolling(dock, True)
        saved = None if self._settings is None else self._settings.value(AUTO_HIDE_SETTING)
        by_name = {SIDE_NAMES[side]: side for side in SIDES}
        renumbered: dict[str, int] = {}
        for item in saved.split(",") if isinstance(saved, str) and saved else []:
            # name:side, or name:side:group since 2026-11-22.
            name, _, rest = item.partition(":")
            side_name, _, saved_group = rest.partition(":")
            dock = next((d for d in self.panels if d.objectName() == name), None)
            side = by_name.get(side_name)
            if dock is None or side is None:
                continue
            group = None
            if saved_group.isdigit() and saved_group != "0":
                group = renumbered.setdefault(saved_group, self._new_group())
            self.unpin(dock, side, group)
        if isinstance(sizes, str) and sizes:
            for item in sizes.split(","):
                name, _, value = item.partition(":")
                dock = next((d for d in self.panels if d.objectName() == name), None)
                if dock is not None and value.isdigit():
                    self._sizes[dock] = int(value)
            self._save()

    # Sliding out and back

    def expand(self, dock: QDockWidget) -> bool:
        """Slides an unpinned panel out over the drawing."""
        overlay = self.overlays.get(dock)
        if overlay is None:
            return False
        if self.open is not None and self.open is not dock:
            self.collapse()
        overlay.setGeometry(self._overlay_rect(dock))
        overlay.show()
        overlay.raise_()
        self.open = dock
        self._leave_timer.start()
        return True

    def collapse(self) -> None:
        if self.open is not None:
            self.overlays[self.open].hide()
            self.open = None
        self._leave_timer.stop()

    def set_enabled(self, enabled: bool) -> None:
        """Background work holds the drawing: open overlays wait too."""
        self._enabled = enabled
        for overlay in self.overlays.values():
            overlay.setEnabled(enabled)

    def restyle(self) -> None:
        """Redraws the pins in the theme's text color."""
        for bar in self.title_bars.values():
            bar.set_pinned(bar.pinned)
            bar.set_floating(bar.floating)
        for overlay in self.overlays.values():
            overlay.title_bar.set_pinned(overlay.title_bar.pinned)
            overlay.title_bar.set_floating(overlay.title_bar.floating)

    def retranslate(self) -> None:
        for bar in self.title_bars.values():
            bar.retranslate()
        for overlay in self.overlays.values():
            overlay.title_bar.retranslate()

    # Resizing (maintainer, 2026-11-20)

    def useful_size(self, dock: QDockWidget) -> int:
        """The least ``dock`` slides out to."""
        return self._useful.get(dock, USEFUL_SIZE)

    def overlay_size(self, dock: QDockWidget) -> int:
        """How wide (tall, at the top or bottom) ``dock`` slides out: as the
        user left it, never under its useful size."""
        return max(self._sizes.get(dock, 0), self.useful_size(dock))

    def _drawing_area(self) -> QRect:
        layout = self._window.layout()
        if layout is not None:
            layout.activate()  # docks just hidden or shown: place the drawing first
        central = self._window.centralWidget()
        return central.geometry() if central is not None else self._window.rect()

    def _drag_edge(self, dock: QDockWidget, corner: QPoint) -> None:
        """The overlay's inner edge follows its grip, whose top-left corner
        is at ``corner`` (global): not under the panel's useful size, nor
        past the drawing."""
        side = self.sides.get(dock)
        overlay = self.overlays.get(dock)
        if side is None or overlay is None:
            return
        self._resizing = True
        area = self._drawing_area()
        at = self._window.mapFromGlobal(corner)
        # The grip lies along the inside of the edge facing the drawing.
        if side == Qt.DockWidgetArea.RightDockWidgetArea:
            size, room = area.right() + 1 - at.x(), area.width()
        elif side == Qt.DockWidgetArea.LeftDockWidgetArea:
            size, room = at.x() + GRIP - area.left(), area.width()
        elif side == Qt.DockWidgetArea.TopDockWidgetArea:
            size, room = at.y() + GRIP - area.top(), area.height()
        else:
            size, room = area.bottom() + 1 - at.y(), area.height()
        self._sizes[dock] = max(self.useful_size(dock), min(size, room))
        overlay.setGeometry(self._overlay_rect(dock))

    def _end_drag(self) -> None:
        self._resizing = False
        self._save()

    def _overlay_rect(self, dock: QDockWidget) -> QRect:
        layout = self._window.layout()
        if layout is not None:
            layout.activate()  # docks just hidden or shown: place the drawing first
        central = self._window.centralWidget()
        area = central.geometry() if central is not None else self._window.rect()
        size = self.overlay_size(dock)
        side = self.sides[dock]
        if side == Qt.DockWidgetArea.LeftDockWidgetArea:
            return QRect(area.left(), area.top(), min(size, area.width()), area.height())
        if side == Qt.DockWidgetArea.RightDockWidgetArea:
            width = min(size, area.width())
            return QRect(area.right() - width + 1, area.top(), width, area.height())
        height = min(size, area.height())
        if side == Qt.DockWidgetArea.TopDockWidgetArea:
            return QRect(area.left(), area.top(), area.width(), height)
        return QRect(area.left(), area.bottom() - height + 1, area.width(), height)

    def _follow_window(self) -> None:
        if self.open is not None:
            self.overlays[self.open].setGeometry(self._overlay_rect(self.open))

    def _check_leave(self) -> None:
        """Slides the open panel back once the mouse has left it and its tab,
        unless something in it has the keyboard."""
        dock = self.open
        if dock is None:
            self._leave_timer.stop()
            return
        if self._resizing:
            return
        overlay = self.overlays[dock]
        focus = QApplication.focusWidget()
        if focus is not None and overlay.isAncestorOf(focus):
            return
        if not (self._under_mouse(overlay) or self._under_mouse(self.tabs[dock])):
            self.collapse()

    @staticmethod
    def _under_mouse(widget: QWidget) -> bool:
        return widget.isVisible() and widget.rect().contains(widget.mapFromGlobal(QCursor.pos()))

    def eventFilter(self, watched: QObject, event: QEvent) -> bool:  # noqa: N802 (Qt API)
        kind = event.type()
        if kind == QEvent.Type.Enter and isinstance(watched, SideTab):
            if self.open is not None:
                # Already looking through the panels: the next one at once.
                self.expand(watched.dock)
            else:
                self._hovered = watched
                self._hover_timer.start()
        elif kind == QEvent.Type.Leave and watched is self._hovered:
            self._hover_timer.stop()
            self._hovered = None
        elif kind == QEvent.Type.Enter and watched in self.rolled:
            # Back over a rolled-up panel: it rolls down at once.
            assert isinstance(watched, QDockWidget)
            self.roll_down(watched)
        elif kind == QEvent.Type.Resize and watched is self._window and self.open is not None:
            # The window has not placed the drawing yet: follow it after.
            QTimer.singleShot(0, self._follow_window)
        elif (
            kind == QEvent.Type.MouseButtonPress
            and self.open is not None
            and isinstance(event, QMouseEvent)
            and isinstance(watched, QWidget)
            and watched.window() is self._window
        ):
            overlay = self.overlays[self.open]
            tab = self.tabs[self.open]
            if not (overlay.isAncestorOf(watched) or watched in (overlay, tab)):
                self.collapse()
        return False

    def _hover_rested(self) -> None:
        tab, self._hovered = self._hovered, None
        if tab is not None and tab.dock in self.overlays:
            self.expand(tab.dock)

    def _remove_tab(self, dock: QDockWidget) -> None:
        tab = self.tabs.pop(dock)
        if self._hovered is tab:
            self._hover_timer.stop()
            self._hovered = None
        bar = self.bars[tab.side]
        for action in bar.actions():
            if bar.widgetForAction(action) is tab:
                bar.removeAction(action)
        tab.deleteLater()
        self._tidy(bar)
        if not any(t.side == tab.side for t in self.tabs.values()):
            bar.hide()

    @staticmethod
    def _tidy(bar: QToolBar) -> None:
        """No gap at either end of a side bar, nor two in a row."""
        previous_gap = True
        actions = bar.actions()
        for i, action in enumerate(actions):
            if action.isSeparator():
                last = all(a.isSeparator() for a in actions[i + 1 :])
                if previous_gap or last:
                    bar.removeAction(action)
                    continue
                previous_gap = True
            else:
                previous_gap = False

    def _save(self) -> None:
        if self._settings is not None:
            value = ",".join(
                f"{dock.objectName()}:{SIDE_NAMES[side]}:{self.groups.get(dock, 0)}"
                for dock, side in self.sides.items()
            )
            self._settings.setValue(AUTO_HIDE_SETTING, value)
            sizes = ",".join(
                f"{dock.objectName()}:{size}" for dock, size in self._sizes.items() if size > 0
            )
            self._settings.setValue(OVERLAY_SIZE_SETTING, sizes)
            rolling = sorted(dock.objectName() for dock in self.rolling)
            self._settings.setValue(ROLL_UP_SETTING, ",".join(rolling))
