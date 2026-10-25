"""BLOCK and INSERT (ADR 0023 D2): make a block from objects selected, and
place a block, following the cursor until its insertion point is picked.
BEDIT, BSAVE and BCLOSE (D3): edit a block's definition alone, in a block
editor of its own, and save it back into the drawing. The Blocks palette
starts INSERT and BEDIT with the block already chosen (ADR 0025 D5), loads
templates and inserts their blocks, copying each first (D6, D7)."""

import math
from collections.abc import Callable, Generator
from pathlib import Path
from typing import Any

from arqstudiocad.commands import (
    Background,
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.commands.modify import select_objects
from arqstudiocad.core import CadDocument, CadError, InvalidOperationError
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr

#: Where INSERT first puts the block it shows following the cursor.
ORIGIN = (0.0, 0.0)


def block(ctx: CommandContext) -> CommandScript:
    """A block named and based at a point, made from objects selected; a
    reference to it takes their place (AutoCAD's "Convert to block")."""
    document = ctx.document()
    while True:
        name = (yield ctx.ask(tr("Block name"))).strip()
        if _named(document, name) is None:
            break
        ctx.message(tr("A block named {name} exists.", name=name))
    base = yield ctx.point(tr("Specify base point"))
    ids = yield from select_objects(ctx, document)
    if not ids:
        return
    try:
        _, changes = document.create_block(name, base, ids)
    except InvalidOperationError:
        raise CommandCancelledError(tr("{name} is not a valid block name.", name=name)) from None
    ctx.session.report(document, changes)
    ctx.message(tr("Block {name} made from {count} objects.", name=name, count=len(ids)))


def insert(ctx: CommandContext) -> CommandScript:
    """A block placed at a point, scaled and turned (Scale and Rotate before
    the point); it follows the cursor until the point is picked. One undo
    step."""
    document = ctx.document()
    names = document.block_names
    if not names:
        raise CommandCancelledError(tr("The drawing has no blocks to insert."))
    if ctx.ui is not None:
        name = ctx.ui.choose_block(names, tr("Insert block"))
        if name is None:
            raise CommandCancelledError()
    else:
        while (name := _named(document, (yield ctx.ask(tr("Block name"))).strip())) is None:
            ctx.message(tr("There is no such block."))
    yield from place_block(ctx, document, name)


def place_block(
    ctx: CommandContext,
    document: CadDocument,
    name: str,
    scale: float = 1.0,
    rotation: float = 0.0,
    on_screen: bool = True,
    explode: bool = False,
    source: CadDocument | None = None,
) -> CommandScript:
    """Places block ``name``: on screen, following the cursor until its
    point is picked, with Scale and Rotate still offered; otherwise at the
    origin at once. With ``explode`` the reference is then replaced by its
    contents, as EXPLODE does (ADR 0025 D5). A block of ``source`` (a
    template) is first copied into the drawing (D6). One undo step either
    way."""
    if not on_screen:
        document.begin_group("INSERT")
        try:
            name = _copied(ctx, document, name, source)
            placed, changes = document.insert_block(name, ORIGIN, scale, rotation)
            ctx.session.report(document, changes)
            if explode:
                ctx.session.report(document, document.explode_entities([placed])[1])
        except BaseException:
            ctx.session.report(document, document.cancel_group())
            raise
        document.end_group()
        return
    document.begin_group("INSERT")
    placed = False
    try:
        name = _copied(ctx, document, name, source)
        # The block, at the origin, follows the cursor as a drag preview.
        shown, changes = document.insert_block(name, ORIGIN, scale, rotation)
        ctx.session.report(document, changes)
        options = [Option("S", tr("Scale")), Option("R", tr("Rotate"))]
        while True:
            answer = yield ctx.point(
                tr("Specify insertion point"),
                base=ORIGIN,
                options=options,
                drag=Drag("move", (shown,), band=False),
            )
            if answer == "S":
                scale = yield from _number(ctx, tr("Specify scale factor"), scale, positive=True)
            elif answer == "R":
                degrees = yield from _number(
                    ctx, tr("Specify rotation angle"), math.degrees(rotation)
                )
                rotation = math.radians(degrees)
            else:
                ctx.session.report(document, document.move_entities([shown], answer))
                if explode:
                    ctx.session.report(document, document.explode_entities([shown])[1])
                placed = True
                return
            # Shown again with the new scale or rotation.
            ctx.session.report(document, document.delete_entities([shown]))
            shown, changes = document.insert_block(name, ORIGIN, scale, rotation)
            ctx.session.report(document, changes)
    finally:
        if placed:
            document.end_group()
        else:
            # Cancelled: nothing was inserted.
            ctx.session.report(document, document.cancel_group())


def bedit(ctx: CommandContext) -> CommandScript:
    """Opens a block of the drawing in a block editor (ADR 0023 D3): its
    definition alone, at its own coordinates, where every tool works."""
    document = ctx.document()
    if ctx.session.block_editor_of(document) is not None:
        raise CommandCancelledError(tr("Close this block editor before editing another block."))
    names = document.block_names
    if not names:
        raise CommandCancelledError(tr("The drawing has no blocks to edit."))
    if ctx.ui is not None:
        name = ctx.ui.choose_block(names, tr("Edit block"))
        if name is None:
            raise CommandCancelledError()
    else:
        while (name := _named(document, (yield ctx.ask(tr("Block name"))).strip())) is None:
            ctx.message(tr("There is no such block."))
    _edit(ctx, document, name)


def _edit(ctx: CommandContext, document: CadDocument, name: str) -> None:
    """Opens block ``name`` in a block editor, unless it is dynamic."""
    if document.is_dynamic_block(name):
        # Its parameters wait for the parametric engine (D3a).
        raise CommandCancelledError(
            tr("{name} is a dynamic block: it cannot be edited yet.", name=name)
        )
    ctx.session.open_block_editor(document, name)
    ctx.message(tr("Editing block {name}.", name=name))


def insert_named(
    name: str,
    scale: float,
    rotation: float,
    on_screen: bool,
    explode: bool = False,
    source: CadDocument | None = None,
) -> Command:
    """INSERT with block ``name`` already chosen, scaled and turned
    (``rotation`` in radians), exploded if asked, as the Blocks palette
    starts it: from its Insert button or a drop on the drawing. A block of
    ``source`` (a template) is copied into the drawing first, in the same
    undo step (ADR 0025 D6)."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        found = _named(source if source is not None else document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        if source is not None:
            clashes = document.block_clashes(found, source)
            if clashes:
                # Never resolved silently (D6): the choice comes with the
                # clash dialog (step 7c).
                raise CommandCancelledError(
                    tr(
                        "The drawing has other blocks named {names}: "
                        "{name} cannot be inserted from the template yet.",
                        names=", ".join(clashes),
                        name=found,
                    )
                )
        ctx.message(tr("Inserting block {name}.", name=found))
        yield from place_block(ctx, document, found, scale, rotation, on_screen, explode, source)

    return Command("INSERT", run, repeat=False)


def load_template(path: Path | None, loaded: Callable[[CadDocument], None]) -> Command:
    """Opens a template for the Blocks palette in the background, as OPEN
    opens a drawing (ADR 0025 D7): ``path``, or a file the user chooses.
    The template is never shown in a tab nor written; ``loaded`` gets it."""

    def run(ctx: CommandContext) -> CommandScript:
        chosen = path
        if chosen is None:
            if ctx.ui is None:
                chosen = Path((yield ctx.ask(tr("Template to load"))))
            else:
                chosen = ctx.ui.choose_template_path()
                if chosen is None:
                    raise CommandCancelledError()
        app = ctx.session.app
        file = chosen
        try:
            template = yield ctx.background(
                lambda: app.open_document(file),
                tr("Loading template {name}…", name=file.name),
            )
        except CadError as error:
            raise CommandCancelledError(
                tr("{name} cannot be loaded as a template: {reason}", name=file.name, reason=error)
            ) from None
        loaded(template)
        count = len(template.block_names)
        ctx.message(tr("Template {name} loaded: {count} blocks.", name=file.name, count=count))

    return Command("BLOCKSPALETTE", run, repeat=False)


def edit_named(name: str) -> Command:
    """BEDIT with block ``name`` already chosen, as the Blocks palette
    starts it."""

    def run(ctx: CommandContext) -> CommandScript:
        document = ctx.document()
        if ctx.session.block_editor_of(document) is not None:
            raise CommandCancelledError(tr("Close this block editor before editing another block."))
        found = _named(document, name)
        if found is None:
            raise CommandCancelledError(tr("There is no such block."))
        _edit(ctx, document, found)
        return
        yield

    return Command("BEDIT", run, repeat=False)


def bsave(ctx: CommandContext) -> CommandScript:
    """Saves the block editor into its block, as one undo step of the
    drawing; the editor stays open."""
    yield from save_block(ctx, _editor(ctx))


def bclose(ctx: CommandContext) -> CommandScript:
    """Closes the block editor, asking first whether to save the block."""
    yield from close_block_editor(ctx, _editor(ctx))


def save_block(ctx: CommandContext, editor: CadDocument) -> CommandScript:
    """Puts what ``editor`` holds into its block (also SAVE in an editor)."""
    found = ctx.session.block_editor_of(editor)
    assert found is not None
    document, name = found
    try:
        ctx.session.save_block(editor)
    except InvalidOperationError:
        raise CommandCancelledError(
            tr("{name} cannot hold a reference to itself.", name=name)
        ) from None
    ctx.message(tr("Block {name} saved in {drawing}.", name=name, drawing=document.name))
    return
    yield  # a command script, though it asks nothing


def close_block_editor(ctx: CommandContext, editor: CadDocument) -> CommandScript:
    """Closes ``editor`` (also CLOSE in an editor, or closing its drawing),
    asking first whether to save its changes into the block."""
    if editor.is_modified:
        ctx.session.activate(editor)
        if ctx.ui is not None:
            answer = ctx.ui.ask_save_changes(editor.name)
        else:
            key = yield ctx.choose(
                tr("Save changes to {name}?", name=editor.name),
                [Option("Y", tr("Yes")), Option("N", tr("No"))],
                default="Y",
            )
            answer = "yes" if key == "Y" else "no"
        if answer == "cancel":
            raise CommandCancelledError()
        if answer == "yes":
            yield from save_block(ctx, editor)
    name = editor.name
    ctx.session.close(editor)
    ctx.message(tr("Closed {name}", name=name))


def _editor(ctx: CommandContext) -> CadDocument:
    """The active document, which must be a block editor."""
    document = ctx.document()
    if ctx.session.block_editor_of(document) is None:
        raise CommandCancelledError(tr("This is not a block editor."))
    return document


def _copied(
    ctx: CommandContext, document: CadDocument, name: str, source: CadDocument | None
) -> str:
    """Block ``name`` in the drawing: copied from ``source`` first, when
    there is one, within the INSERT group (ADR 0025 D6)."""
    if source is None:
        return name
    copied, changes = document.copy_block(name, source)
    ctx.session.report(document, changes)
    return copied


def _named(document: CadDocument, name: str) -> str | None:
    """The block called ``name``, ignoring case, as the drawing spells it."""
    return next((n for n in document.block_names if n.lower() == name.lower()), None)


def _number(
    ctx: CommandContext, message: str, default: float, positive: bool = False
) -> Generator[Prompt | Background, Any, float]:
    """A number typed at the command line, ``default`` on Enter."""
    while True:
        typed = yield ctx.ask(message, default=f"{default:.10g}")
        try:
            value = float(typed)
        except ValueError:
            ctx.message(tr("{text} is not a number.", text=typed))
            continue
        if not math.isfinite(value) or (positive and value <= 0.0):
            ctx.message(tr("The value must be a number greater than zero."))
            continue
        return value


#: Block commands, with AutoCAD's aliases.
BLOCK_COMMANDS = (
    Command("BLOCK", block, ("B",)),
    Command("INSERT", insert, ("I",)),
    Command("BEDIT", bedit, ("BE",)),
    Command("BSAVE", bsave),
    Command("BCLOSE", bclose),
)
