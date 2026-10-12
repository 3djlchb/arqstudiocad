"""Where the application's data lives, identically from source and frozen.

PyInstaller bundles keep data at the same relative paths as the repository,
so these functions are the only place that knows the difference.
"""

import sys
from pathlib import Path


def is_frozen() -> bool:
    """True when running from a PyInstaller bundle."""
    return bool(getattr(sys, "frozen", False))


def package_dir() -> Path:
    """The `arqstudiocad` package directory (inside the bundle when frozen)."""
    return Path(__file__).resolve().parent


def locales_dir() -> Path:
    """Folder holding one sub-folder per language pack."""
    return package_dir() / "locales"


PORTABLE_MARKER = "portable.txt"


def portable_dir() -> Path | None:
    """Folder of a portable installation, or None.

    A frozen build is portable when `portable.txt` sits next to its
    executable; settings then live in that folder instead of the system
    location (Windows registry, ~/.config...).
    """
    if not is_frozen():
        return None
    folder = Path(sys.executable).resolve().parent
    return folder if (folder / PORTABLE_MARKER).is_file() else None


def authors_file() -> Path | None:
    """The AUTHORS file (single source of the credits), or None if absent."""
    if is_frozen():
        # PyInstaller sets sys._MEIPASS to the bundle's data directory.
        candidate = Path(sys._MEIPASS) / "AUTHORS"  # type: ignore[attr-defined]
    else:
        # Source checkout: python/arqstudiocad -> repository root.
        candidate = package_dir().parents[1] / "AUTHORS"
    return candidate if candidate.is_file() else None
