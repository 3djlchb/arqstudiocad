"""The status bar's drafting switches (ADR 0016 D5): running object snaps
(F3, with a menu to choose which) and ORTHO (F8), shown and changed as the
session's :class:`~arqstudiocad.drafting.Drafting` says."""

from PySide6.QtCore import Qt
from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import QHBoxLayout, QMenu, QToolButton, QWidget

from arqstudiocad.drafting import Drafting
from arqstudiocad.events import DraftingChanged
from arqstudiocad.i18n import tr
from arqstudiocad.points import SNAP_KEYS
from arqstudiocad.ui.event_bridge import QtEventBridge
from arqstudiocad.ui.viewport import snap_name


class DraftingBar(QWidget):
    """OSNAP and ORTHO buttons; their actions carry F3 and F8, so the window
    that adds them gets the shortcuts."""

    def __init__(
        self, drafting: Drafting, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("drafting_bar")
        self._drafting = drafting

        self.osnap_action = QAction(self)
        self.osnap_action.setCheckable(True)
        self.osnap_action.setShortcut(QKeySequence(Qt.Key.Key_F3))
        self.osnap_action.setShortcutContext(Qt.ShortcutContext.WindowShortcut)
        self.osnap_action.toggled.connect(drafting.set_osnap)

        self.ortho_action = QAction(self)
        self.ortho_action.setCheckable(True)
        self.ortho_action.setShortcut(QKeySequence(Qt.Key.Key_F8))
        self.ortho_action.setShortcutContext(Qt.ShortcutContext.WindowShortcut)
        self.ortho_action.toggled.connect(drafting.set_ortho)

        # Which running snaps: a menu on the OSNAP button.
        self.snaps_menu = QMenu(self)
        self.snap_actions: dict[str, QAction] = {}
        for key in SNAP_KEYS:
            action = self.snaps_menu.addAction("")
            action.setCheckable(True)
            action.toggled.connect(self._choose_snaps)
            self.snap_actions[key] = action

        self.osnap_button = QToolButton()
        self.osnap_button.setDefaultAction(self.osnap_action)
        self.osnap_button.setMenu(self.snaps_menu)
        self.osnap_button.setPopupMode(QToolButton.ToolButtonPopupMode.MenuButtonPopup)
        self.ortho_button = QToolButton()
        self.ortho_button.setDefaultAction(self.ortho_action)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)
        layout.addWidget(self.osnap_button)
        layout.addWidget(self.ortho_button)

        self._show(DraftingChanged(drafting.osnap, drafting.snaps, drafting.ortho))
        events.drafting_changed.connect(self._show)
        self.retranslate()

    def retranslate(self) -> None:
        self.osnap_action.setText(tr("OSNAP"))
        self.osnap_action.setToolTip(tr("Object snaps (F3)"))
        self.ortho_action.setText(tr("ORTHO"))
        self.ortho_action.setToolTip(tr("Orthogonal mode (F8)"))
        for key, action in self.snap_actions.items():
            action.setText(snap_name(key))

    def _show(self, event: DraftingChanged) -> None:
        """Shows the drafting state without echoing it back."""
        for action, on in [(self.osnap_action, event.osnap), (self.ortho_action, event.ortho)]:
            action.blockSignals(True)
            action.setChecked(on)
            action.blockSignals(False)
        for key, action in self.snap_actions.items():
            action.blockSignals(True)
            action.setChecked(key in event.snaps)
            action.blockSignals(False)

    def _choose_snaps(self) -> None:
        self._drafting.set_snaps(
            frozenset(k for k, action in self.snap_actions.items() if action.isChecked())
        )
