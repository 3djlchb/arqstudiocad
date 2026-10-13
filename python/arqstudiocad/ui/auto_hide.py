"""Pinned and auto-hidden panels, as AutoCAD's palettes (ADR 0015).

Every panel's title bar has a pin. Pinned (the default), the panel stays
open where it is docked. Unpinned, it collapses to a tab with its name on
the side it was docked; resting the mouse on the tab, or clicking it, slides
the panel out over the drawing, and it slides back when the mouse leaves it
(unless something in it has the keyboard) or the drawing is clicked.
Pinning it again docks it back, open.

While a panel is auto-hidden its contents live in an overlay next to the
tab; the dock itself stays hidden.
"""

from collections.abc import Iterable

from PySide6.QtCore import QEvent, QObject, QRect, QSettings, QSize, Qt, QTimer
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


def is_vertical(side: Side) -> bool:
    return side in (Qt.DockWidgetArea.LeftDockWidgetArea, Qt.DockWidgetArea.RightDockWidgetArea)


class PanelTitleBar(QWidget):
    """A panel's title, as OpenCADStudio's: its name, the pin and close.
    Dragging and double-clicking still move and float the panel (QDockWidget
    handles the mouse events the bar leaves alone)."""

    def __init__(self, dock: QDockWidget, pinned: bool = True) -> None:
        super().__init__(dock)
        self.title = QLabel(dock.windowTitle())
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
        for button in (self.pin, self.close_button):
            button.setIconSize(QSize(12, 12))
            row.addWidget(button)
        dock.windowTitleChanged.connect(self.title.setText)
        self.set_pinned(pinned)
        self.retranslate()

    def set_pinned(self, pinned: bool) -> None:
        self.pinned = pinned
        # The application's palette: the theme, even before it reaches this bar.
        self.pin.setIcon(pin_icon(pinned, QApplication.palette(self).windowText().color()))
        self.retranslate()

    def retranslate(self) -> None:
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


class Overlay(QFrame):
    """An unpinned panel slid out over the drawing: its title bar with the
    pin, then its contents."""

    def __init__(self, dock: QDockWidget, parent: QWidget) -> None:
        super().__init__(parent)
        self.setObjectName(f"overlay_{dock.objectName()}")
        self.setFrameShape(QFrame.Shape.StyledPanel)
        self.setAutoFillBackground(True)
        self.title_bar = PanelTitleBar(dock, pinned=False)
        self.contents: QWidget | None = None
        self._column = QVBoxLayout(self)
        self._column.setContentsMargins(1, 1, 1, 1)
        self._column.setSpacing(0)
        self._column.addWidget(self.title_bar)
        self.hide()

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
    ) -> None:
        super().__init__(window)
        self._window = window
        self._settings = settings
        self.panels = list(panels)
        self.title_bars: dict[QDockWidget, PanelTitleBar] = {}
        #: The unpinned panels: the side each was docked on, its tab, its overlay.
        self.sides: dict[QDockWidget, Side] = {}
        self.tabs: dict[QDockWidget, SideTab] = {}
        self.overlays: dict[QDockWidget, Overlay] = {}
        self._sizes: dict[QDockWidget, int] = {}
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
            title.pin.clicked.connect(lambda _checked=False, d=dock: self.unpin(d))
            title.close_button.clicked.connect(dock.close)
            dock.topLevelChanged.connect(title.pin.setHidden)
            dock.setTitleBarWidget(title)
            self.title_bars[dock] = title
        self._leave_timer = QTimer(self)
        self._leave_timer.setInterval(LEAVE_DELAY_MS)
        self._leave_timer.timeout.connect(self._check_leave)
        app = QApplication.instance()
        if app is not None:
            app.installEventFilter(self)

    # Pinning

    def is_pinned(self, dock: QDockWidget) -> bool:
        return dock not in self.sides

    def unpin(self, dock: QDockWidget, side: Side | None = None) -> bool:
        """Collapses ``dock`` to a tab on ``side`` (the side it is docked
        on); False for a floating or already unpinned panel."""
        if not self.is_pinned(dock) or (side is None and dock.isFloating()):
            return False
        side = side if side is not None else self._window.dockWidgetArea(dock)
        if side not in SIDES:
            return False
        size = dock.width() if is_vertical(side) else dock.height()
        self._sizes[dock] = max(size, 180)
        contents = dock.widget()
        overlay = Overlay(dock, self._window)
        overlay.title_bar.pin.clicked.connect(lambda _checked=False: self.pin(dock))
        overlay.title_bar.close_button.clicked.connect(lambda _checked=False: self.close(dock))
        if contents is not None:
            overlay.hold(contents)
        tab = SideTab(dock, side)
        tab.clicked.connect(lambda _checked=False: self.expand(dock))
        self.bars[side].addWidget(tab)
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
        del self.sides[dock]
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
        self._save()

    def close(self, dock: QDockWidget) -> None:
        """Closes an unpinned panel: docked back, then hidden, as closing a
        pinned one (the View menu opens it again)."""
        self.pin(dock)
        dock.close()

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
        """Unpins the panels that were unpinned last time."""
        saved = None if self._settings is None else self._settings.value(AUTO_HIDE_SETTING)
        if not isinstance(saved, str) or not saved:
            return
        by_name = {SIDE_NAMES[side]: side for side in SIDES}
        for item in saved.split(","):
            name, _, side_name = item.partition(":")
            dock = next((d for d in self.panels if d.objectName() == name), None)
            side = by_name.get(side_name)
            if dock is not None and side is not None:
                self.unpin(dock, side)

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
        for overlay in self.overlays.values():
            overlay.title_bar.set_pinned(overlay.title_bar.pinned)

    def retranslate(self) -> None:
        for bar in self.title_bars.values():
            bar.retranslate()
        for overlay in self.overlays.values():
            overlay.title_bar.retranslate()

    def _overlay_rect(self, dock: QDockWidget) -> QRect:
        layout = self._window.layout()
        if layout is not None:
            layout.activate()  # docks just hidden or shown: place the drawing first
        central = self._window.centralWidget()
        area = central.geometry() if central is not None else self._window.rect()
        size = self._sizes.get(dock, 240)
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
            self.expand(watched.dock)
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

    def _remove_tab(self, dock: QDockWidget) -> None:
        tab = self.tabs.pop(dock)
        bar = self.bars[tab.side]
        for action in bar.actions():
            if bar.widgetForAction(action) is tab:
                bar.removeAction(action)
        tab.deleteLater()
        if not any(t.side == tab.side for t in self.tabs.values()):
            bar.hide()

    def _save(self) -> None:
        if self._settings is not None:
            value = ",".join(
                f"{dock.objectName()}:{SIDE_NAMES[side]}" for dock, side in self.sides.items()
            )
            self._settings.setValue(AUTO_HIDE_SETTING, value)
