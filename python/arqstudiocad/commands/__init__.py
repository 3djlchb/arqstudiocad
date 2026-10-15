"""User commands (contract §5, ADR 0006, ADR 0013 D5.4).

A command is a generator: it yields a :class:`Prompt`, receives what the
user typed, and goes on until it returns. It may also yield
:class:`Background` work (opening, saving): the runner hands it to a
:class:`Worker` thread and sends the result back, on the caller's thread,
when it is done; without a worker it runs at once. The :class:`CommandRunner` drives
it from the command window, publishes ``CommandStarted``, ``CommandPrompt``,
``CommandMessage``, ``CommandFinished`` and ``CommandCancelled``, and turns
Esc into cancellation. Commands reach the drawing only through the
:class:`~arqstudiocad.session.Session` and the interface only through the
:class:`UiServices` protocol; they never touch widgets.

Command names, aliases and option keys are English and always work, in
every language; ``_LINE`` forces the English name (ADR 0006).
"""

import logging
from collections.abc import Callable, Generator, Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal, Protocol

from arqstudiocad.core import CadDocument, CadError
from arqstudiocad.events import (
    CommandCancelled,
    CommandFinished,
    CommandMessage,
    CommandPrompt,
    CommandStarted,
    EventBus,
    WorkFinished,
    WorkStarted,
)
from arqstudiocad.i18n import tr
from arqstudiocad.points import PointSyntaxError, parse_point, snap_key
from arqstudiocad.session import Session

log = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class Option:
    """A prompt option: typed by its English ``key``, shown translated with
    the key visible, as ``Extents(E)``."""

    key: str
    label: str

    def shown(self) -> str:
        return f"{self.label}({self.key})"


@dataclass(frozen=True, slots=True)
class Prompt:
    """What a command asks for.

    ``kind`` is ``"option"`` (one of ``options``, by key), ``"text"`` (any
    text, e.g. a file name) or ``"point"`` (typed as AutoCAD reads points,
    or picked in the viewport; the command receives an ``(x, y)`` tuple, or
    an option's key). ``base`` is the last point, for relative input, ORTHO
    and the rubber band. An empty answer takes ``default`` when there is
    one, else the prompt is asked again.
    """

    message: str
    kind: Literal["option", "text", "point"] = "text"
    options: tuple[Option, ...] = ()
    default: str | None = None
    base: tuple[float, float] | None = None

    def shown(self) -> str:
        """The prompt as the command window shows it."""
        text = self.message
        if self.options:
            text += " [" + "/".join(o.shown() for o in self.options) + "]"
        if self.default:
            text += f" <{self.default}>"
        return text + ":"


@dataclass(frozen=True, slots=True)
class Background:
    """Work too long for the interface thread (reading or writing a file).
    Yield it to get what ``task`` returns, or have its exception raised at
    the ``yield``. It cannot be interrupted: Esc waits for it."""

    task: Callable[[], Any]
    message: str


@dataclass(frozen=True, slots=True)
class Outcome:
    """How background work ended: its value, or the exception it raised."""

    value: Any = None
    error: Exception | None = None


class Worker(Protocol):
    """Runs background work and calls ``done`` on the thread that asked."""

    def run(self, task: Callable[[], Any], done: Callable[[Outcome], None]) -> None: ...


type Answer = str
type CommandScript = Generator[Prompt | Background, Any, None]


class ViewControl(Protocol):
    """What commands may do with the view of a document."""

    def zoom_extents(self) -> bool: ...
    def zoom(self, factor: float) -> None: ...


class UiServices(Protocol):
    """Dialogs and views the interface provides; commands never build them."""

    def choose_open_path(self) -> Path | None: ...
    def choose_save_path(self, suggested: str) -> Path | None: ...
    def ask_save_changes(self, name: str) -> Literal["yes", "no", "cancel"]: ...
    def report_recovered(self, name: str, problems: Sequence[str]) -> None:
        """Warns that ``name`` was damaged and opened by recovery."""
        ...

    def ask_overwrite_damaged(self, name: str) -> Literal["overwrite", "save_as", "cancel"]:
        """Whether to replace ``name``, the damaged file a drawing was
        recovered from, save elsewhere, or cancel."""
        ...

    def ask_accept_losses(self, name: str, losses: Sequence[str]) -> bool:
        """Whether to save ``name`` although it would lose ``losses``."""
        ...

    def view(self, document: CadDocument) -> ViewControl | None: ...
    def show_panel(self, name: str) -> bool:
        """Shows and raises a panel (``"layers"``…); False if there is none."""
        ...


class CommandCancelledError(Exception):
    """Raised inside a command to stop it as cancelled, with a reason."""


@dataclass
class CommandContext:
    """What a running command may use (contract §5: a controlled context)."""

    session: Session
    ui: UiServices | None = None
    worker: Worker | None = None

    def message(self, text: str) -> None:
        """A line for the command window's history."""
        self.session.bus.publish(CommandMessage(text))

    def document(self) -> CadDocument:
        """The active document, or stop: there is none."""
        document = self.session.active
        if document is None:
            raise CommandCancelledError(tr("No drawing is open."))
        return document

    def choose(self, message: str, options: Sequence[Option], default: str | None = None) -> Prompt:
        """A prompt for one of ``options``; yield it to get the chosen key."""
        return Prompt(message, "option", tuple(options), default)

    def ask(self, message: str, default: str | None = None) -> Prompt:
        """A prompt for any text; yield it to get the text."""
        return Prompt(message, "text", (), default)

    def point(
        self,
        message: str,
        base: tuple[float, float] | None = None,
        options: Sequence[Option] = (),
        default: str | None = None,
    ) -> Prompt:
        """A prompt for a point (ADR 0016 D6); yield it to get ``(x, y)``,
        or the key of one of ``options``. ``base`` is the last point."""
        return Prompt(message, "point", tuple(options), default, base)

    def background(self, task: Callable[[], Any], message: str) -> Background:
        """Work for a worker thread; yield it to get its result."""
        return Background(task, message)


type CommandFunction = Callable[[CommandContext], CommandScript]


@dataclass(frozen=True, slots=True)
class Command:
    """A command: unique English ``name``, its ``aliases`` and its script."""

    name: str
    run: CommandFunction
    aliases: tuple[str, ...] = ()


class CommandRegistry:
    """Commands by name and alias, ignoring case."""

    def __init__(self, commands: Iterable[Command] = ()) -> None:
        self._commands: dict[str, Command] = {}
        self._lookup: dict[str, Command] = {}
        for command in commands:
            self.register(command)

    def register(self, command: Command) -> None:
        """Adds a command; a name or alias already taken is an error."""
        keys = [command.name, *command.aliases]
        for key in keys:
            if key.upper() in self._lookup:
                raise ValueError(f"{key!r} is already a command name or alias")
            if not key.isascii() or not key.replace("_", "").isalnum() or key.startswith("_"):
                raise ValueError(f"{key!r} is not a valid command name")
        self._commands[command.name.upper()] = command
        for key in keys:
            self._lookup[key.upper()] = command

    def find(self, typed: str) -> Command | None:
        """The command typed, by name or alias; ``_NAME`` forces the English
        name (ADR 0006)."""
        key = typed.strip().upper().removeprefix("_")
        return self._lookup.get(key)

    def names(self) -> list[str]:
        """Every command name and alias, sorted, for completion."""
        return sorted(self._lookup)


class CommandRunner:
    """Runs one command at a time from typed lines."""

    def __init__(self, registry: CommandRegistry, context: CommandContext) -> None:
        self.registry = registry
        self.context = context
        self._script: CommandScript | None = None
        self._command: Command | None = None
        self._prompt: Prompt | None = None
        self._last: Command | None = None
        self._busy: Background | None = None

    @property
    def bus(self) -> EventBus:
        return self.context.session.bus

    @property
    def active(self) -> str | None:
        """Name of the running command, if any."""
        return None if self._command is None else self._command.name

    @property
    def prompt(self) -> Prompt | None:
        """What the running command is asking for."""
        return self._prompt

    @property
    def busy(self) -> str | None:
        """What background work the running command waits for, if any."""
        return None if self._busy is None else self._busy.message

    def execute(self, line: str) -> None:
        """A line typed in the command window: a command to start, or the
        answer to the running command's prompt. An empty line with no
        command running repeats the last one, as Enter does in AutoCAD."""
        text = line.strip()
        if self._refuse_while_busy():
            return
        if self._command is None:
            if not text:
                if self._last is not None:
                    self.start(self._last)
                return
            command = self.registry.find(text)
            if command is None:
                self.context.message(tr('Unknown command "{name}".', name=text.upper()))
                return
            self.start(command)
            return
        self._answer(text)

    def start(self, command: Command) -> None:
        """Starts ``command``, cancelling any running one."""
        if self._refuse_while_busy():
            return
        if self._command is not None:
            self.cancel()
        self._command = command
        self._last = command
        self.bus.publish(CommandStarted(command.name))
        try:
            self._script = command.run(self.context)
        except Exception as error:  # a command failing to start
            self._fail(error)
            return
        self._advance(next)

    def cancel(self, reason: str = "") -> None:
        """Esc: stops the running command (not while it works in the
        background: that cannot be interrupted)."""
        if self._command is None or self._refuse_while_busy():
            return
        script, name = self._script, self._command.name
        self._clear()
        if script is not None:
            script.close()
        self.context.message(tr("*Cancel*"))
        self.bus.publish(CommandCancelled(name, reason or "cancelled by the user"))

    def answer_point(self, point: tuple[float, float]) -> None:
        """A point picked in the viewport, already snapped (ADR 0016 D5):
        the answer to a point prompt; ignored at any other prompt."""
        prompt = self._prompt
        if prompt is None or prompt.kind != "point" or self._refuse_while_busy():
            return
        self.context.session.drafting.override = None
        self._advance(lambda script: script.send(point))

    def _answer(self, text: str) -> None:
        prompt = self._prompt
        assert prompt is not None
        if prompt.kind == "point":
            self._answer_point(prompt, text)
            return
        answer: str | None = text or prompt.default
        if prompt.kind == "option" and answer is not None:
            answer = _option_key(prompt.options, answer)
            if answer is None:
                self.context.message(tr("Invalid option."))
        if answer is None:
            self._show(prompt)  # ask again
            return
        self._advance(lambda script: script.send(answer))

    def _answer_point(self, prompt: Prompt, text: str) -> None:
        """A typed answer to a point prompt: an option, a snap that applies
        to the next point only, or a point in AutoCAD's syntax."""
        drafting = self.context.session.drafting
        typed = text or prompt.default or ""
        key = _option_key(prompt.options, typed) if typed else None
        if key is not None:
            drafting.override = None
            self._advance(lambda script: script.send(key))
            return
        snap = snap_key(typed)
        if snap is not None:
            drafting.override = snap
            self._show(prompt)
            return
        try:
            point = parse_point(typed, last=prompt.base, toward=drafting.cursor)
        except PointSyntaxError as error:
            self.context.message(str(error))
            self._show(prompt)
            return
        drafting.override = None
        self._advance(lambda script: script.send(point))

    def _advance(self, resume: Callable[[CommandScript], Prompt | Background]) -> None:
        """Runs the command to its next prompt, background work or end."""
        script = self._script
        assert script is not None
        try:
            step = resume(script)
        except StopIteration:
            name = self.active or ""
            self._clear()
            self.bus.publish(CommandFinished(name))
            return
        except Exception as error:
            self._fail(error)
            return
        if isinstance(step, Background):
            self._run_background(step)
            return
        self._prompt = step
        self._show(step)

    def _run_background(self, work: Background) -> None:
        self._busy = work
        self.bus.publish(WorkStarted(work.message))
        worker = self.context.worker
        if worker is None:
            try:
                outcome = Outcome(value=work.task())
            except Exception as error:
                outcome = Outcome(error=error)
            self._background_done(outcome)
        else:
            worker.run(work.task, self._background_done)

    def _background_done(self, outcome: Outcome) -> None:
        self._busy = None
        self.bus.publish(WorkFinished())
        if self._script is None:
            return
        error = outcome.error
        if error is not None:
            self._advance(lambda script: script.throw(error))
        else:
            self._advance(lambda script: script.send(outcome.value))

    def _refuse_while_busy(self) -> bool:
        if self._busy is None:
            return False
        self.context.message(tr("Please wait: {work}", work=self._busy.message))
        return True

    def _show(self, prompt: Prompt) -> None:
        self.bus.publish(
            CommandPrompt(self.active or "", prompt.shown(), prompt.kind == "point", prompt.base)
        )

    def _fail(self, error: Exception) -> None:
        name = self.active or ""
        self._clear()
        if isinstance(error, CommandCancelledError):
            reason = str(error)
            if reason:
                self.context.message(reason)
        elif isinstance(error, CadError):
            reason = f"{error.code}: {error}"
            self.context.message(tr("Error: {message}", message=str(error)))
        else:
            log.exception("command %s failed", name)
            reason = f"internal error: {error!r}"
            self.context.message(tr("The command stopped because of an internal error."))
        self.bus.publish(CommandCancelled(name, reason))

    def _clear(self) -> None:
        self._script = None
        self._command = None
        self._prompt = None
        self.context.session.drafting.override = None


def _option_key(options: Sequence[Option], typed: str) -> str | None:
    """The key of the option typed: its English key, or its shown label."""
    wanted = typed.strip().upper()
    for option in options:
        if wanted in (option.key.upper(), option.label.upper()):
            return option.key
    return None


__all__ = [
    "Answer",
    "Background",
    "Command",
    "CommandCancelledError",
    "CommandContext",
    "CommandFunction",
    "CommandRegistry",
    "CommandRunner",
    "CommandScript",
    "Option",
    "Outcome",
    "Prompt",
    "UiServices",
    "ViewControl",
    "Worker",
]
