"""BLOCK and INSERT (ADR 0023 D2): make a block from objects selected, and
place a block, following the cursor until its insertion point is picked."""

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
        name = ctx.ui.choose_block(names)
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
)
