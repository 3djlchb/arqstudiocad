"""The document and view commands of phase 5 (ADR 0013, D5.4).

NEW, OPEN, SAVE, SAVEAS, CLOSE, UNDO, REDO, ZOOM and LAYER; ID (ADR 0016) shows
a point's coordinates. The registry also holds the drawing commands of
:mod:`arqstudiocad.commands.draw` and the editing commands of
:mod:`arqstudiocad.commands.modify`. Without an interface
(scripts, tests), OPEN and SAVEAS ask for the file name on the command line,
as AutoCAD does with FILEDIA=0.
"""

import math
from collections.abc import Generator, Sequence
from pathlib import Path
from typing import Any, Literal

from arqstudiocad.commands import (
    Background,
    Command,
    CommandCancelledError,
    CommandContext,
    CommandRegistry,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.commands.blocks import BLOCK_COMMANDS, close_block_editor, save_block
from arqstudiocad.commands.budget import BUDGET_COMMANDS
from arqstudiocad.commands.draw import DRAW_COMMANDS
from arqstudiocad.commands.modify import MODIFY_COMMANDS
from arqstudiocad.core import (
    CadDocument,
    LossyConversionError,
    OverwritesDamagedError,
    SaveNotVerifiedError,
)
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
    yield from open_file(ctx, path)


def open_command(path: Path) -> Command:
    """OPEN for one file chosen beforehand (File > Open Recent): it asks
    nothing, and Enter on an empty command line does not repeat it."""
    return Command("OPEN", lambda ctx: open_file(ctx, path), repeat=False)


def open_file(ctx: CommandContext, path: Path) -> CommandScript:
    """Opens ``path`` in the background and reports what it holds."""
    app = ctx.session.app
    document = yield ctx.background(
        lambda: app.open_document(path), tr("Opening {name}…", name=path.name)
    )
    ctx.session.adopt(document)
    ctx.message(
        tr(
            "Opened {name}: {count} entities, {version}.",
            name=document.name,
            count=len(document.get_entities()),
            version=document.version,
        )
    )
    undecoded = document.undecoded_objects
    if undecoded:
        # Kept verbatim: a save in the drawing's own version keeps them
        # (ADR 0017 D2); another format asks before losing them.
        ctx.message(
            tr(
                "{count} objects the program cannot read yet are kept as they are: {objects}",
                count=len(undecoded),
                objects=", ".join(undecoded),
            )
        )
    problems = document.recovery_problems
    if problems:
        # A damaged file is never taken for a valid drawing (ADR 0017 D5.1).
        ctx.message(
            tr(
                "{name} is damaged: it was recovered and part of it may be missing.",
                name=document.name,
            )
        )
        for problem in problems:
            ctx.message(f"  {problem}")
        if ctx.ui is not None:
            ctx.ui.report_recovered(document.name, problems)


def save(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    if ctx.session.block_editor_of(document) is not None:
        # A block editor saves into its block, never to a file (ADR 0023 D3).
        yield from save_block(ctx, document)
    elif document.path is None:
        yield from _save_as(ctx, document)
    else:
        yield from _write(ctx, document, document.path)


def save_as(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    if ctx.session.block_editor_of(document) is not None:
        raise CommandCancelledError(
            tr("A block editor is saved into its drawing, not to a file: use Save Block.")
        )
    yield from _save_as(ctx, document)


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
    yield from _write(ctx, document, path)


def _write(ctx: CommandContext, document: CadDocument, path: Path) -> CommandScript:
    try:
        saved = yield from _save_to(ctx, document, path)
    except SaveNotVerifiedError as error:
        # The core read the file back and it was not the drawing (ADR 0017
        # D5.2): nothing was replaced, and what was written is kept aside.
        ctx.message(
            tr(
                "{name} was not saved: the file written did not read back as the drawing. "
                "The original file is unchanged; what was written is kept beside it.",
                name=path.name,
            )
        )
        ctx.message(f"  {error}")
        raise CommandCancelledError() from None
    if saved:
        ctx.session.saved(document, path)
        ctx.message(tr("Saved {path}", path=str(path)))


def _save_to(
    ctx: CommandContext, document: CadDocument, path: Path
) -> Generator[Prompt | Background, Any, bool]:
    """Saves to ``path``; False if the user chose to save elsewhere instead
    (that save reported itself)."""
    while True:
        # The core saves a snapshot taken when it starts: drawing may go on.
        try:
            yield ctx.background(lambda: document.save(path), tr("Saving {name}…", name=path.name))
        except OverwritesDamagedError:
            # The drawing was recovered from this very file: replacing it is
            # the user's decision (ADR 0017 D5.1).
            answer = yield from _ask_overwrite_damaged(ctx, path)
            if answer == "cancel":
                raise CommandCancelledError() from None
            if answer == "save_as":
                yield from _save_as(ctx, document)
                return False
            document.allow_overwriting_damaged()
        except LossyConversionError as error:
            # The file would hold less than the drawing (ADR 0017 D5.3):
            # asked once per document, never silent.
            if set(error.loss_kinds) <= set(document.accepted_losses):
                raise  # refused although accepted: never ask in circles
            if not (yield from _accept_losses(ctx, path, error.losses)):
                raise CommandCancelledError() from None
            document.accept_losses(error.loss_kinds)
        else:
            return True


def _accept_losses(
    ctx: CommandContext, path: Path, losses: Sequence[str]
) -> Generator[Prompt | Background, Any, bool]:
    if ctx.ui is not None:
        return ctx.ui.ask_accept_losses(path.name, losses)
    ctx.message(tr("Saving as {name} would lose data the format cannot keep:", name=path.name))
    for loss in losses:
        ctx.message(f"  {loss}")
    key = yield ctx.choose(
        tr("Save anyway?"),
        [Option("Y", tr("Yes")), Option("N", tr("No"))],
        default="N",
    )
    return bool(key == "Y")


def _ask_overwrite_damaged(
    ctx: CommandContext, path: Path
) -> Generator[Prompt | Background, Any, Literal["overwrite", "save_as", "cancel"]]:
    if ctx.ui is not None:
        return ctx.ui.ask_overwrite_damaged(path.name)
    key = yield ctx.choose(
        tr(
            "{name} is the damaged file this drawing was recovered from. Replace it?",
            name=path.name,
        ),
        [Option("R", tr("Replace")), Option("S", tr("Save as")), Option("C", tr("Cancel"))],
        default="S",
    )
    if key == "R":
        return "overwrite"
    if key == "S":
        return "save_as"
    return "cancel"


def _suggested_name(document: CadDocument) -> str:
    if document.path is not None:
        return str(document.path)
    return document.name + DEFAULT_SAVE_EXTENSION


def close(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    if ctx.session.block_editor_of(document) is not None:
        yield from close_block_editor(ctx, document)
        return
    # Its block editors first, each asked about (ADR 0023 D3).
    for editor in ctx.session.block_editors(document):
        yield from close_block_editor(ctx, editor)
    ctx.session.activate(document)
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


def id_(ctx: CommandContext) -> CommandScript:
    """IDPOINT, AutoCAD's ID: the coordinates of a point, typed or picked."""
    ctx.document()
    point = yield ctx.point(tr("Specify point"))
    x, y = point
    ctx.message(f"X = {x:.4f}     Y = {y:.4f}")


def dist(ctx: CommandContext) -> CommandScript:
    """DISTANCE, AutoCAD's DIST: the distance between two points, typed or picked,
    with a rubber band from the first; the angle in the XY plane (degrees,
    ADR 0010) and the deltas, as AutoCAD reports them."""
    ctx.document()
    first = yield ctx.point(tr("Specify first point"))
    second = yield ctx.point(tr("Specify second point"), base=first)
    dx, dy = second[0] - first[0], second[1] - first[1]
    angle = math.degrees(math.atan2(dy, dx)) % 360.0
    ctx.message(
        tr(
            "Distance = {distance},  Angle in XY plane = {angle}°",
            distance=f"{math.hypot(dx, dy):.4f}",
            angle=f"{angle:.4f}",
        )
    )
    ctx.message(f"Delta X = {dx:.4f},  Delta Y = {dy:.4f}")


def layer(ctx: CommandContext) -> CommandScript:
    """Opens the layers panel, as AutoCAD's LAYER opens its palette."""
    ctx.document()
    if ctx.ui is None or not ctx.ui.show_panel("layers"):
        raise CommandCancelledError(tr("The layers panel is not available here."))
    return
    yield


def properties(ctx: CommandContext) -> CommandScript:
    """Opens the Properties panel, as AutoCAD's PROPERTIES (Ctrl+1), with
    or without a drawing (ADR 0025 D2)."""
    if ctx.ui is None or not ctx.ui.show_panel("properties"):
        raise CommandCancelledError(tr("The properties panel is not available here."))
    return
    yield


def blocks_palette(ctx: CommandContext) -> CommandScript:
    """Opens the Blocks palette (ADR 0025), with or without a drawing."""
    if ctx.ui is None or not ctx.ui.show_panel("blocks"):
        raise CommandCancelledError(tr("The blocks palette is not available here."))
    return
    yield


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
    Command("LAYER", layer, ("LA",)),
    Command("BLOCKSPALETTE", blocks_palette),
    Command("PROPERTIES", properties, ("PR",)),
    Command("IDPOINT", id_, ("ID",)),
    Command("DISTANCE", dist, ("DI", "DIST")),
)


def builtin_registry() -> CommandRegistry:
    """A registry with every built-in command."""
    return CommandRegistry(
        (*BUILTIN_COMMANDS, *DRAW_COMMANDS, *MODIFY_COMMANDS, *BLOCK_COMMANDS, *BUDGET_COMMANDS)
    )
