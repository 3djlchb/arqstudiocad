"""Interface services for tests: answers decided by the test, never a
dialog (a modal dialog in a test would wait forever)."""

from collections.abc import Callable, Sequence
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
    real views (e.g. ``window.viewport_for``), else ``zooms`` stands in;
    ``panel_for`` shows the real panels (``window.show_panel``)."""

    open_path: Path | None = None
    #: The template the Blocks palette's file dialog returns (None: cancelled).
    template_path: Path | None = None
    save_path: Path | None = None
    save_changes: Literal["yes", "no", "cancel"] = "no"
    overwrite_damaged: Literal["overwrite", "save_as", "cancel"] = "cancel"
    accept_losses: bool = False
    #: What the multiline text window returns (None: cancelled).
    text_answer: str | None = None
    #: The block INSERT's window chooses (None: cancelled).
    block_answer: str | None = None
    zooms: Zooms = field(default_factory=Zooms)
    view_for: Callable[[CadDocument], ViewControl | None] | None = None
    panel_for: Callable[[str], bool] | None = None
    asked: list[str] = field(default_factory=list)
    recovered: list[tuple[str, list[str]]] = field(default_factory=list)
    panels_shown: list[str] = field(default_factory=list)

    def choose_open_path(self) -> Path | None:
        self.asked.append("open")
        return self.open_path

    def choose_template_path(self) -> Path | None:
        self.asked.append("template")
        return self.template_path

    def choose_save_path(self, suggested: str) -> Path | None:
        self.asked.append(f"save {suggested}")
        return self.save_path

    def ask_save_changes(self, name: str) -> Literal["yes", "no", "cancel"]:
        self.asked.append(f"changes {name}")
        return self.save_changes

    def report_recovered(self, name: str, problems: Sequence[str]) -> None:
        self.recovered.append((name, list(problems)))

    def ask_overwrite_damaged(self, name: str) -> Literal["overwrite", "save_as", "cancel"]:
        self.asked.append(f"overwrite damaged {name}")
        return self.overwrite_damaged

    def ask_accept_losses(self, name: str, losses: Sequence[str]) -> bool:
        self.asked.append(f"losses {name}: {len(losses)}")
        # Asking again and again for the same save is a bug: fail, do not hang.
        assert len([a for a in self.asked if a.startswith("losses ")]) <= 3, "asked in circles"
        return self.accept_losses

    def choose_block(self, names: Sequence[str], title: str) -> str | None:
        self.asked.append(f"{title}: {', '.join(names)}")
        return self.block_answer

    def edit_text(self, title: str, text: str = "") -> str | None:
        self.asked.append(f"text {title}: {text}")
        return self.text_answer

    def view(self, document: CadDocument) -> ViewControl | None:
        return self.zooms if self.view_for is None else self.view_for(document)

    def show_panel(self, name: str) -> bool:
        self.panels_shown.append(name)
        return True if self.panel_for is None else self.panel_for(name)
