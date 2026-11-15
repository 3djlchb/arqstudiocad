"""Independent rectangular ARRAY copies; geometry and atomicity belong to Rust."""

from collections.abc import Generator
from typing import Any

from arqstudiocad.commands import Background, Command, CommandContext, CommandScript, Prompt
from arqstudiocad.commands.modify import done_selecting, select_objects
from arqstudiocad.core import CadError
from arqstudiocad.i18n import tr


def _count(
    ctx: CommandContext, message: str, default: int
) -> Generator[Prompt | Background, Any, int]:
    while True:
        value = yield ctx.number(message, default=default)
        if value.is_integer() and value <= 100_000:
            return int(value)
        ctx.message(tr("Enter a whole number from 1 to 100000."))


def rectangular_array(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using ARRAYRECT."))
        return
    ctx.message(tr("Rectangular array: independent copies (non-associative)."))
    ids = yield from select_objects(ctx, document, windows=True)
    if not ids:
        return
    rows = yield from _count(ctx, tr("Enter number of rows"), ctx.remembered.get("ARRAY_ROWS", 4))
    while True:
        columns = yield from _count(
            ctx, tr("Enter number of columns"), ctx.remembered.get("ARRAY_COLUMNS", 4)
        )
        if 2 <= rows * columns <= 100_000:
            break
        ctx.message(tr("The array must contain between 2 and 100000 cells."))
    row_spacing = 0.0
    column_spacing = 0.0
    if rows > 1:
        while row_spacing == 0:
            row_spacing = yield ctx.number(
                tr("Specify distance between rows"),
                default=ctx.remembered.get("ARRAY_ROW_SPACING", 1.0),
                signed=True,
            )
            if row_spacing == 0:
                ctx.message(tr("Spacing must be nonzero."))
    if columns > 1:
        while column_spacing == 0:
            column_spacing = yield ctx.number(
                tr("Specify distance between columns"),
                default=ctx.remembered.get("ARRAY_COLUMN_SPACING", 1.0),
                signed=True,
            )
            if column_spacing == 0:
                ctx.message(tr("Spacing must be nonzero."))
    angle = yield ctx.angle(
        tr("Specify array orientation"),
        (0.0, 0.0),
        default=ctx.remembered.get("ARRAY_ANGLE", 0.0),
    )
    try:
        copies, changes = document.rectangular_array(
            ids, rows, columns, row_spacing, column_spacing, angle
        )
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return
    ctx.session.report(document, changes)
    done_selecting(ctx, document, ids)
    ctx.remembered.update(ARRAY_ROWS=rows, ARRAY_COLUMNS=columns, ARRAY_ANGLE=angle)
    if rows > 1:
        ctx.remembered["ARRAY_ROW_SPACING"] = row_spacing
    if columns > 1:
        ctx.remembered["ARRAY_COLUMN_SPACING"] = column_spacing
    ctx.message(tr("Created {count} independent copies.", count=len(copies)))


ARRAY_COMMAND = Command("ARRAYRECT", rectangular_array, ("ARRAY", "AR", "-ARRAY"))
