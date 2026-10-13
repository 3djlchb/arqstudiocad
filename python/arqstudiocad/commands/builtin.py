"""The document and view commands of phase 5 (ADR 0013, D5.4).

NEW, OPEN, SAVE, SAVEAS, CLOSE, UNDO, REDO and ZOOM. Without an interface
(scripts, tests), OPEN and SAVEAS ask for the file name on the command line,
as AutoCAD does with FILEDIA=0.
"""

from pathlib import Path

from arqstudiocad.commands import (
    Command,
    CommandCancelledError,
    CommandContext,
    CommandRegistry,
    CommandScript,
    Option,
)
from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr

#: Saving defaults to DXF until the round-trip corpus makes DWG the default
#: (contract §10, phase 8).
DEFAULT_SAVE_EXTENSION = ".dxf"

#: Zoom factor of ZOOM In and Out.
ZOOM_STEP = 2.0


def new(ctx: CommandContext) -> CommandScript:
    document = ctx.session.new_document()
    ctx.message(tr("New drawing: {name}", name=document.name))
    return
    yield  # a generator, though it asks nothing


def open_(ctx: CommandContext) -> CommandScript:
    if ctx.ui is not None:
        path = ctx.ui.choose_open_path()
        if path is None:
            raise CommandCancelledError()
    else:
        path = Path((yield ctx.ask(tr("File to open"))))
    document = ctx.session.open_document(path)
    ctx.message(
        tr(
            "Opened {name}: {count} entities, {version}.",
            name=document.name,
            count=len(document.get_entities()),
            version=document.version,
        )
    )


def save(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    if document.path is None:
        yield from _save_as(ctx, document)
    else:
        _write(ctx, document, document.path)


def save_as(ctx: CommandContext) -> CommandScript:
    yield from _save_as(ctx, ctx.document())


def _save_as(ctx: CommandContext, document: CadDocument) -> CommandScript:
    suggested = _suggested_name(document)
    if ctx.ui is not None:
        path = ctx.ui.choose_save_path(suggested)
        if path is None:
            raise CommandCancelledError()
    else:
        path = Path((yield ctx.ask(tr("Save as"), default=suggested)))
    if not path.suffix:
        path = path.with_suffix(DEFAULT_SAVE_EXTENSION)
    _write(ctx, document, path)


def _write(ctx: CommandContext, document: CadDocument, path: Path) -> None:
    ctx.session.save(document, path)
    ctx.message(tr("Saved {path}", path=str(path)))


def _suggested_name(document: CadDocument) -> str:
    if document.path is not None:
        return str(document.path)
    return document.name + DEFAULT_SAVE_EXTENSION


def close(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    if document.is_modified:
        if ctx.ui is not None:
            answer = ctx.ui.ask_save_changes(document.name)
        else:
            key = yield ctx.choose(
                tr("Save changes to {name}?", name=document.name),
                [Option("Y", tr("Yes")), Option("N", tr("No"))],
                default="Y",
            )
            answer = "yes" if key == "Y" else "no"
        if answer == "cancel":
            raise CommandCancelledError()
        if answer == "yes":
            yield from save(ctx)
            if document.is_modified:  # the save was cancelled
                raise CommandCancelledError()
    name = document.name
    ctx.session.close(document)
    ctx.message(tr("Closed {name}", name=name))


def undo(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    name = document.undo_name
    if name is None:
        raise CommandCancelledError(tr("Nothing to undo."))
    ctx.session.report(document, document.undo())
    ctx.message(tr("Undone: {name}", name=name))
    return
    yield


def redo(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    name = document.redo_name
    if name is None:
        raise CommandCancelledError(tr("Nothing to redo."))
    ctx.session.report(document, document.redo())
    ctx.message(tr("Redone: {name}", name=name))
    return
    yield


def zoom(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is None:
        raise CommandCancelledError(tr("There is no view to zoom."))
    choice = yield ctx.choose(
        tr("Zoom"),
        [Option("E", tr("Extents")), Option("I", tr("In")), Option("O", tr("Out"))],
        default="E",
    )
    if choice == "E":
        if not view.zoom_extents():
            ctx.message(tr("The drawing is empty."))
    else:
        view.zoom(ZOOM_STEP if choice == "I" else 1 / ZOOM_STEP)


#: Every command of this module, with AutoCAD's usual aliases.
BUILTIN_COMMANDS = (
    Command("NEW", new, ("QNEW",)),
    Command("OPEN", open_),
    Command("SAVE", save, ("QSAVE",)),
    Command("SAVEAS", save_as),
    Command("CLOSE", close),
    Command("UNDO", undo, ("U",)),
    Command("REDO", redo),
    Command("ZOOM", zoom, ("Z",)),
)


def builtin_registry() -> CommandRegistry:
    """A registry with every built-in command."""
    return CommandRegistry(BUILTIN_COMMANDS)
