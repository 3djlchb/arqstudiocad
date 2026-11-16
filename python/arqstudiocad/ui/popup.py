"""Opening menus inside a window that holds native widgets.

The GPU viewport is a native window (ADR 0034), and Qt makes its siblings
native too: the panels, the status bar, the tabs. A menu takes its nearest
native ancestor's window as the window it belongs to, and when that is not
a top-level window Qt refuses it with "must be a top level window". A menu
opened through ``popup`` belongs to its owner's top-level window instead.
"""

from PySide6.QtCore import QPoint
from PySide6.QtWidgets import QMenu


def popup(menu: QMenu, at: QPoint) -> None:
    """Opens ``menu`` at ``at`` (screen coordinates), owned by the
    top-level window of the widget it was made for."""
    owner = menu.parentWidget()
    if owner is not None and owner is not owner.window():
        menu.setParent(owner.window(), menu.windowFlags())
    menu.popup(at)
