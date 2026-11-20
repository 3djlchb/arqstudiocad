"""Independent rectangular and polar ARRAY copies; geometry and atomicity belong to Rust."""

import math
from collections.abc import Generator
from typing import Any

from arqstudiocad.commands import (
    Background,
    Command,
    CommandContext,
    CommandScript,
    Option,
    Picked,
    Prompt,
)
from arqstudiocad.commands.modify import done_selecting, select_objects
from arqstudiocad.core import CadError
from arqstudiocad.events import Drag
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
    default_angle = ctx.remembered.get("ARRAY_ANGLE", 0.0)
    angle = yield ctx.angle(
        tr("Specify array orientation"),
        (0.0, 0.0),
        default=default_angle,
        drag=Drag(
            "array_rect",
            ids,
            rotation=default_angle,
            array=(rows, columns, row_spacing, column_spacing),
        ),
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


def polar_array(ctx: CommandContext) -> CommandScript:
    """An independent polar array; all placement and mutation live in Rust."""
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using ARRAYPOLAR."))
        return
    ctx.message(tr("Polar array: independent copies (non-associative)."))
    ids = yield from select_objects(ctx, document, windows=True)
    if not ids:
        return
    center = yield ctx.point(tr("Specify center point of array"))
    while True:
        count = yield from _count(
            ctx, tr("Enter number of items"), ctx.remembered.get("POLAR_COUNT", 4)
        )
        if count >= 2:
            break
        ctx.message(tr("A polar array must contain between 2 and 100000 items."))
    while True:
        fill = yield ctx.angle(
            tr("Specify angle to fill"),
            center,
            default=ctx.remembered.get("POLAR_FILL_ANGLE", math.tau),
        )
        if fill != 0 and abs(fill) <= math.tau:
            break
        ctx.message(tr("Specify a nonzero fill angle between -360 and 360 degrees."))
    rotate = yield ctx.choose(
        tr("Rotate arrayed objects"),
        [Option("Y", tr("Yes")), Option("N", tr("No"))],
        default="Y" if ctx.remembered.get("POLAR_ROTATE_ITEMS", True) else "N",
    )
    rotate_items = rotate == "Y"
    base = None if rotate_items else (yield ctx.point(tr("Specify array base point")))
    try:
        copies, changes = document.polar_array(ids, center, count, fill, rotate_items, base)
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return
    ctx.session.report(document, changes)
    done_selecting(ctx, document, ids)
    ctx.remembered.update(POLAR_COUNT=count, POLAR_FILL_ANGLE=fill, POLAR_ROTATE_ITEMS=rotate_items)
    ctx.message(tr("Created {count} independent copies.", count=len(copies)))


POLAR_ARRAY_COMMAND = Command("ARRAYPOLAR", polar_array)


def path_array(ctx: CommandContext) -> CommandScript:
    """Independent copies on an open path; all geometry belongs to Rust."""
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using ARRAYPATH."))
        return
    ctx.message(tr("Path array: independent copies (non-associative)."))
    ids = yield from select_objects(ctx, document, windows=True)
    if not ids:
        return
    base = yield ctx.point(tr("Specify array base point"))
    while True:
        picked = yield ctx.entity(tr("Select path curve"), optional=True)
        if picked is None:
            return
        assert isinstance(picked, Picked)
        guide = document.get_entity(picked.entity_id)
        if guide.kind in ("LINE", "ARC", "LWPOLYLINE"):
            break
        ctx.message(tr("ARRAYPATH currently supports lines, arcs and open lightweight polylines."))
    while True:
        count = yield from _count(
            ctx, tr("Enter number of items"), ctx.remembered.get("PATH_COUNT", 4)
        )
        if count >= 2:
            break
        ctx.message(tr("A path array must contain between 2 and 100000 items."))
    choice = yield ctx.choose(
        tr("Align arrayed objects"),
        [Option("Y", tr("Yes")), Option("N", tr("No"))],
        default="Y" if ctx.remembered.get("PATH_ALIGN_ITEMS", True) else "N",
    )
    align = choice == "Y"
    try:
        copies, changes = document.path_array(ids, picked.entity_id, base, count, align)
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return
    ctx.session.report(document, changes)
    done_selecting(ctx, document, ids)
    ctx.remembered.update(PATH_COUNT=count, PATH_ALIGN_ITEMS=align)
    ctx.message(tr("Created {count} independent copies.", count=len(copies)))


PATH_ARRAY_COMMAND = Command("ARRAYPATH", path_array)
