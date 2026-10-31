"""Controls of the Options dialog's Display section (ADR 0029 D4, D6): a
color field, a swatch and a ``#RRGGBB`` field kept in step, and the live
crosshair preview.
"""

import re
from collections.abc import Callable

from PySide6.QtCore import QPointF, QRectF, QSize, Qt, Signal
from PySide6.QtGui import QColor, QPainter, QPaintEvent, QPalette, QPolygonF
from PySide6.QtWidgets import (
    QColorDialog,
    QFrame,
    QHBoxLayout,
    QLineEdit,
    QToolButton,
    QWidget,
)

from arqstudiocad.i18n import tr
from arqstudiocad.ui.crosshair import Crosshair, crosshair_color, paint_crosshair

#: A color as the fields take it.
HEX = re.compile(r"#[0-9a-fA-F]{6}")
#: The preview's size, in logical pixels (the brief's 150 by 100).
PREVIEW = QSize(150, 100)
#: The pick box's half side in the preview, as the viewport's.
PREVIEW_PICK = 4
#: An invalid field's border.
INVALID = "#d13438"


def color_value(text: str) -> str | None:
    """``text`` as a stored color: ``#rrggbb``, or empty; None if it is
    neither."""
    text = text.strip()
    if not text:
        return ""
    return text.lower() if HEX.fullmatch(text) else None


class ColorField(QWidget):
    """A swatch that opens Qt's color dialog and a ``#RRGGBB`` field. An
    optional field may be empty: the swatch then shows ``fallback()``, the
    color used instead. Text that is not a color marks the field and keeps
    the last valid value."""

    #: The value changed, by the swatch or by typing.
    changed = Signal()

    def __init__(
        self, fallback: Callable[[], QColor], required: bool = False, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self._fallback = fallback
        self.required = required
        self._value = ""
        self.invalid = False
        self.swatch = QToolButton()
        self.swatch.setFixedSize(30, 24)
        self.swatch.clicked.connect(self.choose)
        self.edit = QLineEdit()
        self.edit.setMaxLength(7)
        self.edit.setFixedWidth(170)
        self.edit.textEdited.connect(self._typed)
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(self.swatch)
        layout.addWidget(self.edit)
        layout.addStretch(1)
        self.retranslate()
        self.restyle()

    def value(self) -> str:
        """``#rrggbb``, or empty."""
        return self._value

    def set_value(self, value: str) -> None:
        self._value = value
        self.edit.setText(value.upper())
        self._mark(False)
        self.restyle()

    def shown(self) -> QColor:
        """The color the swatch shows."""
        return QColor(self._value) if self._value else self._fallback()

    def choose(self) -> None:
        """The swatch: Qt's color dialog, these being interface colors, not
        CAD colors (maintainer, 2026-10-30)."""
        color = QColorDialog.getColor(self.shown(), self, tr("Choose a color"))
        if color.isValid():
            self.set_value(color.name())
            self.changed.emit()

    def _typed(self, text: str) -> None:
        value = color_value(text)
        if value is None or (self.required and not value):
            self._mark(True)
            return
        self._value = value
        self._mark(False)
        self.restyle()
        self.changed.emit()

    def _mark(self, invalid: bool) -> None:
        self.invalid = invalid
        self.edit.setStyleSheet(f"border: 1px solid {INVALID};" if invalid else "")
        self.edit.setToolTip(self._hint() if invalid else "")

    def _hint(self) -> str:
        if self.required:
            return tr("Enter a color as #RRGGBB")
        return tr("Enter a color as #RRGGBB, or leave it empty")

    def retranslate(self) -> None:
        self.edit.setPlaceholderText(tr("#RRGGBB") if self.required else tr("#RRGGBB or empty"))
        self.swatch.setToolTip(tr("Choose a color"))
        self.swatch.setAccessibleName(tr("Choose a color"))
        if self.invalid:
            self.edit.setToolTip(self._hint())

    def restyle(self) -> None:
        line = self.palette().color(QPalette.ColorRole.Mid).name()
        self.swatch.setStyleSheet(
            f"QToolButton {{ background-color: {self.shown().name()}; border: 1px solid {line}; }}"
        )


class CrosshairPreview(QFrame):
    """The crosshair as the viewport draws it, on ``background``, live
    before Apply (ADR 0029 D6)."""

    def __init__(self, background: QColor, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.background = background
        self.crosshair = Crosshair()
        self.setFixedSize(PREVIEW)
        self.retranslate()

    def set_crosshair(self, style: Crosshair) -> None:
        if style != self.crosshair:
            self.crosshair = style
            self.retranslate()
            self.update()

    def retranslate(self) -> None:
        self.setAccessibleName(tr("Crosshair preview at {percent} %", percent=self.crosshair.size))

    def paintEvent(self, event: QPaintEvent) -> None:  # noqa: N802 (Qt API)
        painter = QPainter(self)
        area = QRectF(self.rect())
        painter.fillRect(area, self.background)
        center = area.center()
        if self.crosshair.type == "arrow":
            # The system arrow, drawn: the preview has no cursor of its own.
            painter.setRenderHint(QPainter.RenderHint.Antialiasing)
            painter.setPen(self.palette().color(QPalette.ColorRole.Shadow))
            painter.setBrush(crosshair_color(Crosshair(), self.background))
            x, y = center.x(), center.y() - 8
            arrow = [(0, 0), (0, 16), (4, 12), (7, 18), (9, 17), (6, 11), (11, 11)]
            painter.drawPolygon(QPolygonF([QPointF(x + dx, y + dy) for dx, dy in arrow]))
        else:
            paint_crosshair(painter, self.crosshair, center, area, PREVIEW_PICK, self.background)
        painter.setPen(self.palette().color(QPalette.ColorRole.Mid))
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawRect(area.adjusted(0, 0, -1, -1))
        painter.end()
