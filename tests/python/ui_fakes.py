"""Interface services for tests: answers decided by the test, never a
dialog (a modal dialog in a test would wait forever)."""

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

from arqstudiocad.commands import ViewControl
from arqstudiocad.core import CadDocument


@dataclass
class Zooms:
    """A stand-in view that counts zooms."""

    extents: int = 0
    factors: list[float] = field(default_factory=list)

    def zoom_extents(self) -> bool:
        self.extents += 1
        return True

    def zoom(self, factor: float) -> None:
        self.factors.append(factor)


@dataclass
class QuietUi:
    """File dialogs return ``open_path``/``save_path`` (None: cancelled); the
    save-changes question answers ``save_changes``; ``view_for`` gives the
    real views (e.g. ``window.viewport_for``), else ``zooms`` stands in."""

    open_path: Path | None = None
    save_path: Path | None = None
    save_changes: Literal["yes", "no", "cancel"] = "no"
    zooms: Zooms = field(default_factory=Zooms)
    view_for: Callable[[CadDocument], ViewControl | None] | None = None
    asked: list[str] = field(default_factory=list)

    def choose_open_path(self) -> Path | None:
        self.asked.append("open")
        return self.open_path

    def choose_save_path(self, suggested: str) -> Path | None:
        self.asked.append(f"save {suggested}")
        return self.save_path

    def ask_save_changes(self, name: str) -> Literal["yes", "no", "cancel"]:
        self.asked.append(f"changes {name}")
        return self.save_changes

    def view(self, document: CadDocument) -> ViewControl | None:
        return self.zooms if self.view_for is None else self.view_for(document)
