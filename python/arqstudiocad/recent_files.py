"""The drawings opened last, for File > Open Recent: the newest first,
without repeats, at most :data:`LIMIT`. No Qt: the window stores the list
in its settings and builds the menu from it."""

import os
from collections.abc import Iterable
from pathlib import Path

#: How many drawings the list keeps, as AutoCAD's default.
LIMIT = 10


def same_file(a: Path, b: Path) -> bool:
    """The same file, as the operating system compares names (Windows
    ignores case)."""
    return os.path.normcase(os.path.abspath(a)) == os.path.normcase(os.path.abspath(b))


class RecentFiles:
    """The recent drawings, newest first."""

    def __init__(self, paths: Iterable[str | Path] = (), limit: int = LIMIT) -> None:
        self.limit = limit
        self._paths: list[Path] = []
        for path in reversed(list(paths)):
            if str(path):
                self.add(Path(path))

    @property
    def paths(self) -> list[Path]:
        return list(self._paths)

    def add(self, path: Path) -> None:
        """``path`` was opened or saved: it goes first."""
        self._paths = [Path(os.path.abspath(path))] + [
            p for p in self._paths if not same_file(p, path)
        ]
        del self._paths[self.limit :]

    def remove(self, path: Path) -> None:
        self._paths = [p for p in self._paths if not same_file(p, path)]

    def clear(self) -> None:
        self._paths.clear()

    def to_strings(self) -> list[str]:
        """The list as settings store it."""
        return [str(p) for p in self._paths]
