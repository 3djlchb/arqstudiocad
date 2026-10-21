"""BLOCK and INSERT (ADR 0023 D2): make a block from objects selected, and
place a block, following the cursor until its insertion point is picked.
BEDIT, BSAVE and BCLOSE (D3): edit a block's definition alone, in a block
editor of its own, and save it back into the drawing."""

import math
from collections.abc import Generator
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
from arqstudiocad.core import CadDocument, InvalidOperationError
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
    scale, rotation = 1.0, 0.0
    document.begin_group("INSERT")
    placed = False
    try:
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
    if document.is_dynamic_block(name):
        # Its parameters wait for the parametric engine (D3a).
        raise CommandCancelledError(
            tr("{name} is a dynamic block: it cannot be edited yet.", name=name)
        )
    ctx.session.open_block_editor(document, name)
    ctx.message(tr("Editing block {name}.", name=name))


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
