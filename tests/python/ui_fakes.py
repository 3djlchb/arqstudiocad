"""Interface services for tests: answers decided by the test, never a
dialog (a modal dialog in a test would wait forever)."""

from collections.abc import Callable, Sequence
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Literal

from arqstudiocad.commands import (
    BlockClash,
    BlockDefinitionState,
    BlockDialogAction,
    ClashChoice,
    ViewControl,
)
from arqstudiocad.core import CadBlockData, CadDocument
from arqstudiocad.currencies import CurrencyRegister


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
    #: The clash dialog's choices (None: cancelled); the clashes it was shown.
    clash_answer: dict[str, ClashChoice] | None = None
    clashes: list[BlockClash] = field(default_factory=list)
    #: The Block Definition dialog's answers, in turn: why it closes and
    #: the fields it changes (None: cancelled); the states it showed.
    block_dialog: list[tuple[BlockDialogAction, dict[str, object]] | None] = field(
        default_factory=list
    )
    block_dialog_shown: list[BlockDefinitionState] = field(default_factory=list)
    #: Whether to redefine a block whose name is in use.
    redefine_answer: bool = True
    #: The Rename dialog's answers, in turn: the block and its new name
    #: (None: cancelled); the blocks it was opened on.
    rename_answers: list[tuple[str, str] | None] = field(default_factory=list)
    rename_chosen: list[str | None] = field(default_factory=list)
    #: The Purge dialog's answer (None: closed), whether it was shown, and
    #: the answer to the palette's "Purge block X?".
    purge_answer: tuple[list[str], bool] | None = None
    purge_shown: int = 0
    purge_confirm: bool = True
    #: The Currencies dialog's answer (None: cancelled), and the registers
    #: it was shown.
    currencies_answer: CurrencyRegister | None = None
    currencies_shown: list[CurrencyRegister] = field(default_factory=list)
    #: The Block Data dialog's answer (None: cancelled), and the blocks it
    #: was opened on.
    block_data_answer: CadBlockData | None = None
    block_data_shown: list[str] = field(default_factory=list)
    #: The answer to the palette's "Clear the budget data of block X?".
    clear_data_confirm: bool = True
    #: The block the Edit Block Definition dialog was opened on.
    edit_chosen: str | None = None
    #: BLOCK's answer when the loaded template has the name: go on, or
    #: ask for another name.
    template_name_answer: bool = True
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

    def define_block(
        self, document: CadDocument, state: BlockDefinitionState
    ) -> tuple[BlockDialogAction, BlockDefinitionState] | None:
        self.block_dialog_shown.append(state)
        answer = self.block_dialog.pop(0) if self.block_dialog else None
        if answer is None:
            return None
        action, changes = answer
        return action, replace(state, **changes)  # type: ignore[arg-type]

    def ask_redefine_block(self, name: str, references: int) -> bool:
        self.asked.append(f"redefine {name}: {references}")
        return self.redefine_answer

    def purge_blocks(self, document: CadDocument) -> tuple[list[str], bool] | None:
        self.purge_shown += 1
        return self.purge_answer

    def ask_purge_block(self, name: str) -> bool:
        self.asked.append(f"purge {name}")
        return self.purge_confirm

    def edit_currencies(self, document: CadDocument) -> CurrencyRegister | None:
        self.currencies_shown.append(
            CurrencyRegister(*document.currencies, *document.number_format)
        )
        return self.currencies_answer

    def edit_block_data(self, document: CadDocument, name: str) -> CadBlockData | None:
        self.block_data_shown.append(name)
        return self.block_data_answer

    def ask_clear_block_data(self, name: str) -> bool:
        self.asked.append(f"clear data {name}")
        return self.clear_data_confirm

    def rename_block(self, document: CadDocument, chosen: str | None) -> tuple[str, str] | None:
        self.rename_chosen.append(chosen)
        return self.rename_answers.pop(0) if self.rename_answers else None

    def choose_block_to_edit(self, document: CadDocument, chosen: str | None) -> str | None:
        # The Edit Block Definition dialog: what it lists and selects first.
        self.asked.append(f"Edit block: {', '.join(document.block_names)}")
        self.edit_chosen = chosen
        return self.block_answer

    def report_block_name_taken(self, name: str) -> None:
        self.asked.append(f"name taken {name}")

    def ask_block_name_in_template(self, name: str, template: str) -> bool:
        self.asked.append(f"name in template {name} {template}")
        return self.template_name_answer

    def resolve_block_clashes(self, clashes: Sequence[BlockClash]) -> dict[str, ClashChoice] | None:
        self.asked.append(f"clashes {', '.join(c.name for c in clashes)}")
        self.clashes = list(clashes)
        return self.clash_answer

    def edit_text(self, title: str, text: str = "") -> str | None:
        self.asked.append(f"text {title}: {text}")
        return self.text_answer

    def view(self, document: CadDocument) -> ViewControl | None:
        return self.zooms if self.view_for is None else self.view_for(document)

    def show_panel(self, name: str) -> bool:
        self.panels_shown.append(name)
        return True if self.panel_for is None else self.panel_for(name)
