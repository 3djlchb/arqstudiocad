"""User commands (contract §5, ADR 0006, ADR 0013 D5.4).

A command is a generator: it yields a :class:`Prompt`, receives what the
user typed, and goes on until it returns. The :class:`CommandRunner` drives
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
from typing import Literal, Protocol

from arqstudiocad.core import CadDocument, CadError
from arqstudiocad.events import (
    CommandCancelled,
    CommandFinished,
    CommandMessage,
    CommandPrompt,
    CommandStarted,
    EventBus,
)
from arqstudiocad.i18n import tr
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

    ``kind`` is ``"option"`` (one of ``options``, by key) or ``"text"`` (any
    text, e.g. a file name). An empty answer takes ``default`` when there is
    one, else the prompt is asked again.
    """

    message: str
    kind: Literal["option", "text"] = "text"
    options: tuple[Option, ...] = ()
    default: str | None = None

    def shown(self) -> str:
        """The prompt as the command window shows it."""
        text = self.message
        if self.options:
            text += " [" + "/".join(o.shown() for o in self.options) + "]"
        if self.default:
            text += f" <{self.default}>"
        return text + ":"


type Answer = str
type CommandScript = Generator[Prompt, Answer, None]


class ViewControl(Protocol):
    """What commands may do with the view of a document."""

    def zoom_extents(self) -> bool: ...
    def zoom(self, factor: float) -> None: ...


class UiServices(Protocol):
    """Dialogs and views the interface provides; commands never build them."""

    def choose_open_path(self) -> Path | None: ...
    def choose_save_path(self, suggested: str) -> Path | None: ...
    def ask_save_changes(self, name: str) -> Literal["yes", "no", "cancel"]: ...
    def view(self, document: CadDocument) -> ViewControl | None: ...


class CommandCancelledError(Exception):
    """Raised inside a command to stop it as cancelled, with a reason."""


@dataclass
class CommandContext:
    """What a running command may use (contract §5: a controlled context)."""

    session: Session
    ui: UiServices | None = None

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

    def execute(self, line: str) -> None:
        """A line typed in the command window: a command to start, or the
        answer to the running command's prompt. An empty line with no
        command running repeats the last one, as Enter does in AutoCAD."""
        text = line.strip()
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
        self._step(None)

    def cancel(self, reason: str = "") -> None:
        """Esc: stops the running command."""
        if self._command is None:
            return
        script, name = self._script, self._command.name
        self._clear()
        if script is not None:
            script.close()
        self.context.message(tr("*Cancel*"))
        self.bus.publish(CommandCancelled(name, reason or "cancelled by the user"))

    def _answer(self, text: str) -> None:
        prompt = self._prompt
        assert prompt is not None
        answer: str | None = text or prompt.default
        if prompt.kind == "option" and answer is not None:
            answer = _option_key(prompt.options, answer)
            if answer is None:
                self.context.message(tr("Invalid option."))
        if answer is None:
            self._show(prompt)  # ask again
            return
        self._step(answer)

    def _step(self, answer: str | None) -> None:
        script = self._script
        assert script is not None
        try:
            prompt = next(script) if answer is None else script.send(answer)
        except StopIteration:
            name = self.active or ""
            self._clear()
            self.bus.publish(CommandFinished(name))
            return
        except Exception as error:
            self._fail(error)
            return
        self._prompt = prompt
        self._show(prompt)

    def _show(self, prompt: Prompt) -> None:
        self.bus.publish(CommandPrompt(self.active or "", prompt.shown()))

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


def _option_key(options: Sequence[Option], typed: str) -> str | None:
    """The key of the option typed: its English key, or its shown label."""
    wanted = typed.strip().upper()
    for option in options:
        if wanted in (option.key.upper(), option.label.upper()):
            return option.key
    return None


__all__ = [
    "Answer",
    "Command",
    "CommandCancelledError",
    "CommandContext",
    "CommandFunction",
    "CommandRegistry",
    "CommandRunner",
    "CommandScript",
    "Option",
    "Prompt",
    "UiServices",
    "ViewControl",
]
