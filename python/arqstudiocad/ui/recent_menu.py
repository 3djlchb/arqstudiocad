"""File > Open Recent: the last drawings opened or saved, newest first,
remembered in the settings across sessions. A click asks the window to
open the drawing (:attr:`RecentFilesMenu.open_requested`)."""

from pathlib import Path

from PySide6.QtCore import QSettings, Signal
from PySide6.QtWidgets import QMenu, QWidget

from arqstudiocad.i18n import tr
from arqstudiocad.recent_files import RecentFiles

#: Where the list is kept in the settings.
RECENT_FILES_SETTING = "recent_files"


class RecentFilesMenu(QMenu):
    """One entry per recent drawing, ``&1 name.dwg``, with its full path as
    tooltip; then Clear Recent Files."""

    #: A recent drawing was chosen (its ``Path``).
    open_requested = Signal(object)

    def __init__(self, settings: QSettings | None, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("recent_files_menu")
        self.setToolTipsVisible(True)
        self._settings = settings
        stored = [] if settings is None else settings.value(RECENT_FILES_SETTING, [])
        self.recent = RecentFiles(stored if isinstance(stored, list) else [])
        self.retranslate()

    def add(self, path: Path) -> None:
        self.recent.add(path)
        self._changed()

    def remove(self, path: Path) -> None:
        self.recent.remove(path)
        self._changed()

    def clear_list(self) -> None:
        self.recent.clear()
        self._changed()

    def retranslate(self) -> None:
        self.setTitle(tr("Open &Recent"))
        self._rebuild()

    def _changed(self) -> None:
        if self._settings is not None:
            self._settings.setValue(RECENT_FILES_SETTING, self.recent.to_strings())
        self._rebuild()

    def _rebuild(self) -> None:
        self.clear()
        paths = self.recent.paths
        if not paths:
            self.addAction(tr("No recent files")).setEnabled(False)
            return
        for number, path in enumerate(paths, start=1):
            mnemonic = f"&{number}" if number < 10 else "1&0"
            action = self.addAction(f"{mnemonic} {path.name}")
            action.setToolTip(str(path))
            action.setStatusTip(str(path))
            action.triggered.connect(lambda _checked=False, p=path: self.open_requested.emit(p))
        self.addSeparator()
        self.addAction(tr("Clear Recent Files")).triggered.connect(self.clear_list)
