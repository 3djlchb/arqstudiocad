"""The command system, the session and the phase 5 commands (contract §5,
ADR 0006, ADR 0013 D5.4). No Qt: commands run here as from a script."""

import logging
import os
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal, cast

import pytest
from damaged_files import LINES, recovered_dxf

from arqstudiocad.commands import (
    Background,
    Command,
    CommandCancelledError,
    CommandContext,
    CommandRegistry,
    CommandRunner,
    CommandScript,
    Option,
    Prompt,
    UiServices,
    ViewControl,
)
from arqstudiocad.commands.builtin import _write, builtin_registry
from arqstudiocad.core import CadDocument, LossyConversionError, SaveNotVerifiedError
from arqstudiocad.events import (
    ActiveDocumentChanged,
    CommandCancelled,
    CommandFinished,
    CommandMessage,
    CommandPrompt,
    CommandStarted,
    DocumentChanged,
    DocumentClosed,
    DocumentOpened,
    DocumentSaved,
    Event,
    EventBus,
)
from arqstudiocad.session import Session


@dataclass
class Harness:
    """A session, a runner on the built-in commands, and every event."""

    session: Session
    runner: CommandRunner
    events: list[Event]
    answers: list[str] = field(default_factory=list)

    def run(self, *lines: str) -> None:
        for line in lines:
            self.runner.execute(line)

    def messages(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandMessage)]

    def prompts(self) -> list[str]:
        return [e.text for e in self.events if isinstance(e, CommandPrompt)]

    def of[E: Event](self, kind: type[E]) -> list[E]:
        return [e for e in self.events if isinstance(e, kind)]


def harness(ui: UiServices | None = None, registry: CommandRegistry | None = None) -> Harness:
    bus = EventBus()
    events: list[Event] = []
    bus.subscribe(Event, events.append)
    session = Session(bus)
    runner = CommandRunner(registry or builtin_registry(), CommandContext(session, ui))
    return Harness(session, runner, events)


@dataclass
class FakeView:
    extents: int = 0
    zooms: list[float] = field(default_factory=list)

    def zoom_extents(self) -> bool:
        self.extents += 1
        return True

    def zoom(self, factor: float) -> None:
        self.zooms.append(factor)


@dataclass
class FakeUi:
    """Dialog answers decided by the test."""

    open_path: Path | None = None
    save_path: Path | None = None
    save_changes: Literal["yes", "no", "cancel"] = "no"
    overwrite_damaged: Literal["overwrite", "save_as", "cancel"] = "cancel"
    accept_losses: bool = False
    the_view: FakeView | None = None
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

    def report_recovered(self, name: str, problems: Sequence[str]) -> None:
        self.asked.append(f"recovered {name}: {len(problems)} problems")

    def ask_overwrite_damaged(self, name: str) -> Literal["overwrite", "save_as", "cancel"]:
        self.asked.append(f"overwrite damaged {name}")
        return self.overwrite_damaged

    def ask_accept_losses(self, name: str, losses: Sequence[str]) -> bool:
        self.asked.append(f"losses {name}: {list(losses)}")
        # Asking again and again for the same save is a bug: fail, do not hang.
        assert len([a for a in self.asked if a.startswith("losses ")]) <= 3, "asked in circles"
        return self.accept_losses

    def choose_block(self, names: Sequence[str]) -> str | None:
        return None

    def edit_text(self, title: str, text: str = "") -> str | None:
        return None

    def view(self, document: CadDocument) -> ViewControl | None:
        return self.the_view

    def show_panel(self, name: str) -> bool:
        self.asked.append(f"panel {name}")
        return True


# The registry


def test_commands_by_name_alias_any_case_and_forced_english() -> None:
    registry = builtin_registry()
    zoom = registry.find("ZOOM")
    assert zoom is not None
    assert registry.find("zoom") is zoom
    assert registry.find("z") is zoom, "alias"
    assert registry.find("_Zoom") is zoom, "leading _ forces English (ADR 0006)"
    assert registry.find("NOPE") is None
    assert "U" in registry.names() and "UNDO" in registry.names()


def test_names_and_aliases_are_unique_and_plain() -> None:
    def nothing(ctx: CommandContext) -> CommandScript:
        return
        yield

    registry = CommandRegistry([Command("LINE", nothing, ("L",))])
    with pytest.raises(ValueError, match="already"):
        registry.register(Command("LIST", nothing, ("l",)))
    for bad in ("_X", "LÍNEA", "TWO WORDS", ""):
        with pytest.raises(ValueError, match="not a valid"):
            registry.register(Command(bad, nothing))


# The runner


def asking(ctx: CommandContext) -> CommandScript:
    key = yield ctx.choose("Pick", [Option("A", "Apple"), Option("B", "Banana")], default="A")
    text = yield ctx.ask("Say")
    ctx.message(f"{key} {text}")


def test_prompts_answers_defaults_and_invalid_options() -> None:
    h = harness(registry=CommandRegistry([Command("ASK", asking)]))
    h.run("ask")
    assert h.runner.active == "ASK"
    assert h.prompts() == ["Pick [Apple(A)/Banana(B)] <A>:"]
    h.run("cherry")
    assert h.messages() == ["Invalid option."]
    assert h.prompts()[-1] == "Pick [Apple(A)/Banana(B)] <A>:", "asked again"
    h.run("banana")  # the label works too
    h.run("")  # a text prompt without default asks again
    assert h.prompts()[-1] == "Say:" and h.runner.active == "ASK"
    h.run("hello")
    assert h.messages()[-1] == "B hello"
    assert h.of(CommandFinished) == [CommandFinished("ASK")]
    assert h.runner.active is None


def test_enter_repeats_the_last_command_and_esc_cancels() -> None:
    h = harness(registry=CommandRegistry([Command("ASK", asking)]))
    h.run("")  # nothing yet: nothing happens
    assert h.events == []
    h.run("ASK")
    h.runner.cancel()
    assert h.messages() == ["*Cancel*"]
    assert h.of(CommandCancelled) == [CommandCancelled("ASK", "cancelled by the user")]
    h.run("")  # repeats ASK
    assert h.of(CommandStarted) == [CommandStarted("ASK"), CommandStarted("ASK")]
    h.run("ask")  # typed while ASK waits: an answer, not a new command
    assert h.messages()[-1] == "Invalid option."


def test_unknown_commands_and_failures() -> None:
    def broken(ctx: CommandContext) -> CommandScript:
        yield ctx.ask("Go")
        raise RuntimeError("a bug")

    def bad_geometry(ctx: CommandContext) -> CommandScript:
        ctx.document().add_line((0, 0), (0, 0))
        return
        yield

    h = harness(registry=CommandRegistry([Command("BROKEN", broken), Command("BAD", bad_geometry)]))
    h.run("FOO")
    assert h.messages() == ['Unknown command "FOO".']
    h.run("BAD")
    assert h.messages()[-1] == "No drawing is open."
    h.session.new_document()
    h.run("BAD")
    assert h.messages()[-1].startswith("Error: invalid geometry")
    cancelled = h.of(CommandCancelled)[-1]
    assert cancelled.name == "BAD" and cancelled.reason.startswith("invalid_geometry: ")
    h.run("BROKEN", "now")
    assert h.messages()[-1] == "The command stopped because of an internal error."
    assert h.runner.active is None


def test_an_internal_error_is_logged(caplog: pytest.LogCaptureFixture) -> None:
    def broken(ctx: CommandContext) -> CommandScript:
        raise RuntimeError("a bug")
        yield

    h = harness(registry=CommandRegistry([Command("BROKEN", broken)]))
    with caplog.at_level(logging.ERROR, logger="arqstudiocad.commands"):
        h.run("BROKEN")
    assert "command BROKEN failed" in caplog.text and "a bug" in caplog.text


# The session and the built-in commands, without an interface


def test_new_save_open_close_from_the_command_line(tmp_path: Path) -> None:
    h = harness()
    h.run("NEW")
    doc = h.session.active
    assert doc is not None and doc.name == "Drawing1"
    assert h.of(DocumentOpened) == [DocumentOpened(doc.id, "Drawing1", None)]
    assert h.of(ActiveDocumentChanged) == [ActiveDocumentChanged(doc.id)]

    doc.add_line((0, 0), (5, 5))
    h.run("SAVE")  # never saved: asks like SAVEAS, DXF by default
    assert h.prompts()[-1] == "Save as <Drawing1.dxf>:"
    h.run(str(tmp_path / "planta"))
    saved = tmp_path / "planta.dxf"
    assert saved.is_file() and doc.path == saved and not doc.is_modified
    assert h.of(DocumentSaved) == [DocumentSaved(doc.id, saved)]

    h.run("OPEN", str(saved))
    opened = h.session.active
    assert opened is not None and opened != doc
    assert h.messages()[-1].startswith("Opened planta.dxf: 1 entities")

    opened.add_circle((0, 0), 1)
    h.run("CLOSE")
    assert h.prompts()[-1] == "Save changes to planta.dxf? [Yes(Y)/No(N)] <Y>:"
    h.run("N")
    assert h.of(DocumentClosed) == [DocumentClosed(opened.id)]
    assert h.session.active == doc, "the previous document becomes active"
    h.run("CLOSE")  # unmodified: closes at once
    assert h.session.active is None and h.session.documents == []
    h.run("SAVE")
    assert h.messages()[-1] == "No drawing is open."


#: LibreDWG's constraint drawing in the 2010 version: its constraint group
#: is kept undecoded (fetched by tools/fetch_codec_samples.py).
CONSTRAINTS_2010 = (
    Path(__file__).resolve().parents[2] / "target/corpus/libredwg/2010/Constraints.dwg"
)


def regression_drawing(path: Path) -> Path:
    """The fetched drawing; CI requires it (ARQ_REQUIRE_SAMPLES), a local
    run without it skips."""
    if not path.is_file():
        if os.environ.get("ARQ_REQUIRE_SAMPLES"):
            pytest.fail(f"{path} is missing: run tools/fetch_codec_samples.py")
        pytest.skip("run `uv run python tools/fetch_codec_samples.py`")
    return path


def test_open_notes_objects_kept_undecoded() -> None:
    h = harness()
    h.run("OPEN", str(regression_drawing(CONSTRAINTS_2010)))
    doc = h.session.active
    assert doc is not None and doc.recovery_problems == []
    # The engine decodes its geometry dependencies; the constraint group in
    # its old layout stays kept as it is.
    assert doc.undecoded_objects == ["ACDBASSOC2DCONSTRAINTGROUP (3E3)"]
    assert h.messages()[-1] == (
        "1 objects the program cannot read yet are kept as they are: "
        "ACDBASSOC2DCONSTRAINTGROUP (3E3)"
    )


def test_a_recovered_drawing_is_reported_and_not_saved_over_unasked(tmp_path: Path) -> None:
    path = recovered_dxf(tmp_path / "danado.dxf")
    original = path.read_bytes()
    h = harness()
    h.run("OPEN", str(path))
    doc = h.session.active
    assert doc is not None and doc.entity_count == LINES - 1
    assert h.messages()[-len(doc.recovery_problems) - 1] == (
        "danado.dxf is damaged: it was recovered and part of it may be missing."
    )
    assert h.messages()[-1] == f"  {doc.recovery_problems[-1]}"

    doc.add_line((0, 0), (1, 1))
    h.run("SAVE")
    assert h.prompts()[-1] == (
        "danado.dxf is the damaged file this drawing was recovered from. Replace it?"
        " [Replace(R)/Save as(S)/Cancel(C)] <S>:"
    )
    h.run("C")
    assert h.of(CommandCancelled) and path.read_bytes() == original and doc.is_modified

    h.run("SAVE", "S", str(tmp_path / "rescatado.dxf"))
    assert doc.path == tmp_path / "rescatado.dxf" and not doc.is_modified
    assert path.read_bytes() == original

    h.run("SAVEAS", str(path), "R")
    assert path.read_bytes() != original and doc.path == path
    h.run("OPEN", str(path))
    assert h.session.active is not None and h.session.active.recovery_problems == []


def test_with_an_interface_recovery_is_shown_and_asked_in_dialogs(tmp_path: Path) -> None:
    path = recovered_dxf(tmp_path / "danado.dxf")
    ui = FakeUi(open_path=path, save_path=tmp_path / "copia.dxf", overwrite_damaged="save_as")
    h = harness(ui)
    h.run("OPEN")
    doc = h.session.active
    assert doc is not None
    assert ui.asked[-1] == f"recovered danado.dxf: {len(doc.recovery_problems)} problems"
    h.run("SAVE")
    assert ui.asked[-2:] == ["overwrite damaged danado.dxf", f"save {path}"]
    assert doc.path == tmp_path / "copia.dxf"


def test_a_save_that_fails_its_check_says_the_original_is_unchanged(tmp_path: Path) -> None:
    class Unverified:
        def save(self, path: Path) -> None:
            raise SaveNotVerifiedError("plano.dxf: not saved; kept as plano.unverified.dxf")

    h = harness()
    script = _write(h.runner.context, cast(CadDocument, Unverified()), tmp_path / "plano.dxf")
    work = next(script)
    assert isinstance(work, Background)
    with pytest.raises(SaveNotVerifiedError) as raised:
        work.task()
    with pytest.raises(CommandCancelledError):
        script.throw(raised.value)
    assert h.messages() == [
        "plano.dxf was not saved: the file written did not read back as the drawing. "
        "The original file is unchanged; what was written is kept beside it.",
        "  plano.dxf: not saved; kept as plano.unverified.dxf",
    ]
    assert h.of(DocumentSaved) == []


class Lossy:
    """A document whose saves would lose a kind of object until the user
    accepts it, as the core does (ADR 0017 D5.3)."""

    id = 1
    path: Path | None = None
    accepted: list[str]
    saved: list[Path]

    def __init__(self, *, ignores_acceptance: bool = False) -> None:
        self.accepted = []
        self.saved = []
        self.ignores_acceptance = ignores_acceptance

    def save(self, path: Path) -> None:
        if self.ignores_acceptance or "object AECS_WALL" not in self.accepted:
            error = LossyConversionError("plano.dxf: not saved, the file would lose data")
            error.loss_kinds = ["object AECS_WALL"]
            error.losses = ["object AECS_WALL: 26 → 0"]
            raise error
        self.saved.append(path)

    def accept_losses(self, kinds: Sequence[str]) -> None:
        self.accepted.extend(kinds)

    @property
    def accepted_losses(self) -> list[str]:
        return sorted(self.accepted)


def test_a_save_refused_although_accepted_stops_instead_of_asking_again(
    tmp_path: Path,
) -> None:
    ui = FakeUi(accept_losses=True)
    h = harness(ui)
    document = Lossy(ignores_acceptance=True)
    with pytest.raises(LossyConversionError):
        run_write(h, document, tmp_path / "plano.dxf")
    assert len(ui.asked) == 1, "asked once, then stopped"


def run_write(h: Harness, document: Lossy, path: Path) -> None:
    """Drives SAVE's writing step as the runner would, answering prompts
    with ``h``'s queued answers."""
    script = _write(h.runner.context, cast(CadDocument, document), path)
    step = next(script)
    while True:
        try:
            if isinstance(step, Background):
                try:
                    value = step.task()
                except Exception as error:
                    step = script.throw(error)
                    continue
                step = script.send(value)
            else:
                step = script.send(h.answers.pop(0))
        except StopIteration:
            return


def test_a_save_that_would_lose_data_asks_first(tmp_path: Path) -> None:
    h = harness()
    document = Lossy()
    h.answers = ["N"]
    with pytest.raises(CommandCancelledError):
        run_write(h, document, tmp_path / "plano.dxf")
    assert h.messages() == [
        "Saving as plano.dxf would lose data the format cannot keep:",
        "  object AECS_WALL: 26 → 0",
    ]
    assert document.saved == [] and document.accepted == []

    h.answers = ["Y"]
    run_write(h, document, tmp_path / "plano.dxf")
    assert document.saved == [tmp_path / "plano.dxf"]
    assert h.messages()[-1] == f"Saved {tmp_path / 'plano.dxf'}"

    h.answers = []
    run_write(h, document, tmp_path / "plano.dxf")
    assert len(document.saved) == 2, "accepted once for the document: not asked again"


def test_with_an_interface_losses_are_asked_in_a_dialog(tmp_path: Path) -> None:
    ui = FakeUi(accept_losses=True)
    h = harness(ui)
    document = Lossy()
    run_write(h, document, tmp_path / "plano.dxf")
    assert ui.asked == ["losses plano.dxf: ['object AECS_WALL: 26 → 0']"]
    assert document.saved == [tmp_path / "plano.dxf"]


def test_undo_and_redo_report_their_changes() -> None:
    h = harness()
    h.run("NEW")
    doc = h.session.active
    assert doc is not None
    h.run("U")
    assert h.messages()[-1] == "Nothing to undo."
    line, _ = doc.add_line((0, 0), (1, 0))
    h.run("UNDO")
    assert h.messages()[-1] == "Undone: LINE"
    assert h.of(DocumentChanged)[-1].changes.deleted == (line,)
    h.run("REDO")
    assert h.messages()[-1] == "Redone: LINE"
    assert h.of(DocumentChanged)[-1].changes.created == (line,)
    h.run("REDO")
    assert h.messages()[-1] == "Nothing to redo."


def test_commands_use_the_interface_when_there_is_one(tmp_path: Path) -> None:
    view = FakeView()
    ui = FakeUi(save_path=tmp_path / "con dialogo.dwg", the_view=view)
    h = harness(ui)
    h.run("OPEN")
    assert ui.asked == ["open"] and h.of(CommandCancelled)[-1].name == "OPEN"
    h.run("NEW", "SAVEAS")
    assert ui.asked[-1] == "save Drawing1.dxf"
    assert (tmp_path / "con dialogo.dwg").is_file()

    h.run("ZOOM", "")
    assert view.extents == 1
    h.run("Z", "I", "Z", "o")
    assert view.zooms == [2.0, 0.5]

    doc = h.session.active
    assert doc is not None
    doc.add_line((0, 0), (1, 1))
    ui.save_changes = "cancel"
    h.run("CLOSE")
    assert h.session.active == doc, "cancelled: still open"
    ui.save_changes = "yes"
    h.run("CLOSE")
    assert h.session.documents == []
    saves = h.of(DocumentSaved)
    assert [s.path for s in saves] == [tmp_path / "con dialogo.dwg"] * 2, "saved, then closed"


def test_zoom_without_a_view() -> None:
    h = harness()
    h.run("NEW", "ZOOM")
    assert h.messages()[-1] == "There is no view to zoom."
    assert h.runner.active is None


def test_layer_opens_the_layers_panel() -> None:
    ui = FakeUi()
    h = harness(ui)
    h.run("NEW", "LA")
    assert ui.asked == ["panel layers"] and h.of(CommandCancelled) == []


def test_layer_without_a_panel_or_a_drawing() -> None:
    h = harness()
    h.run("NEW", "LAYER")
    assert h.of(CommandCancelled)[-1] == CommandCancelled(
        "LAYER", "The layers panel is not available here."
    )
    ui = FakeUi()
    h = harness(ui)
    h.run("LAYER")
    assert ui.asked == [], "no drawing: nothing to show"
    assert h.of(CommandCancelled)[-1].name == "LAYER"


def test_prompt_formatting() -> None:
    assert Prompt("File").shown() == "File:"
    assert Prompt("Name", default="x").shown() == "Name <x>:"


def test_in_spanish_prompts_are_translated_and_english_keys_still_work() -> None:
    from arqstudiocad.i18n import set_language

    assert set_language("es")
    view = FakeView()
    h = harness(FakeUi(the_view=view))
    h.run("_NEW")  # forced English name
    assert h.session.active is not None and h.session.active.name == "Dibujo1"
    h.run("ZOOM")
    assert h.prompts()[-1] == "Zoom [Extensión(E)/Acercar(I)/Alejar(O)] <E>:"
    h.run("i")  # the English key
    h.run("Z", "alejar")  # or the translated word
    assert view.zooms == [2.0, 0.5]
    h.run("PLOF")
    assert h.messages()[-1] == 'Comando desconocido "PLOF".'
