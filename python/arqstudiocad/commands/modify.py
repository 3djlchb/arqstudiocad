"""The editing commands of milestone 2.5 (ADR 0016 D3, D8): ERASE, MOVE,
COPY, ROTATE, SCALE and MIRROR.

Each works on what is selected when it starts (noun-verb) or asks "Select
objects" (verb-noun), where the viewport selects as usual and ``ALL``,
``L`` (the last entity drawn) and ``P`` (the previous selection) add to it;
Enter ends it. Entities on locked layers are left out with a message, as in
AutoCAD. While a point is asked, the selection follows the cursor moved,
rotated, scaled or mirrored. The core does every transformation, all or
nothing; each command is one undo step and empties the selection when it
ends.
"""

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
from arqstudiocad.core import CadDocument, InvalidGeometryError
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr
from arqstudiocad.points import Point

#: The last rotation angle (radians), remembered as AutoCAD remembers it.
ROTATION = "ROTATE_ANGLE"

type Ids = tuple[int, ...]
type Selecting = Generator[Prompt | Background, Any, Ids]


def select_objects(ctx: CommandContext, document: CadDocument) -> Selecting:
    """The entities to edit: the selection, or what the user selects now.
    Those on locked layers are left out; empty if nothing is left."""
    if not ctx.session.selection(document):
        options = [Option("ALL", tr("All")), Option("L", tr("Last")), Option("P", tr("Previous"))]
        while (answer := (yield ctx.select(tr("Select objects"), options))) is not None:
            if answer == "ALL":
                found = _all(document)
            elif answer == "L":
                found = _all(document)[-1:]
            else:
                # What still exists and can be selected.
                selectable = set(_all(document))
                found = tuple(
                    i for i in ctx.remembered.get(_previous(document), ()) if i in selectable
                )
            ctx.session.select(document, found)
            ctx.message(tr("{count} found", count=len(found)))
    chosen = ctx.session.selection(document)
    locked = {layer.name for layer in document.get_layers() if layer.is_locked}
    on_locked = {e.id for e in document.get_entities(chosen) if e.layer in locked}
    if on_locked:
        ctx.message(tr("{count} were on a locked layer.", count=len(on_locked)))
        ctx.session.deselect(document, on_locked)
    return tuple(i for i in chosen if i not in on_locked)


def erase(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    ids = yield from select_objects(ctx, document)
    if ids:
        ctx.session.report(document, document.delete_entities(ids))
        _done(ctx, document, ids)


def move(ctx: CommandContext) -> CommandScript:
    """From a base point to a second point; Enter at the second point
    moves by the base point's coordinates, as AutoCAD."""
    document = ctx.document()
    ids = yield from _transformable(ctx, document)
    if not ids:
        return
    base = yield ctx.point(tr("Specify base point"))
    second = yield ctx.point(
        tr("Specify second point <use first point as displacement>"),
        base=base,
        optional=True,
        drag=Drag("move", ids),
    )
    offset = base if second is None else _offset(base, second)
    ctx.session.report(document, document.move_entities(ids, offset))
    _done(ctx, document, ids)


def copy(ctx: CommandContext) -> CommandScript:
    """Copies from a base point to as many second points as are given,
    until Enter; Undo takes back the last copy. One undo step."""
    document = ctx.document()
    ids = yield from _transformable(ctx, document)
    if not ids:
        return
    base = yield ctx.point(tr("Specify base point"))
    copies: list[list[int]] = []
    document.begin_group("COPY")
    try:
        while True:
            answer = yield ctx.point(
                tr("Specify second point"),
                base=base,
                options=[Option("U", tr("Undo"))],
                optional=True,
                drag=Drag("move", ids),
            )
            if answer is None:
                break
            if answer == "U":
                if copies:
                    ctx.session.report(document, document.delete_entities(copies.pop()))
                else:
                    ctx.message(tr("Nothing to undo."))
                continue
            new_ids, changes = document.copy_entities(ids, _offset(base, answer))
            ctx.session.report(document, changes)
            copies.append(new_ids)
    finally:
        document.end_group()
    _done(ctx, document, ids)


def rotate(ctx: CommandContext) -> CommandScript:
    """About a base point, by an angle typed in degrees or shown by a point;
    the last angle is the default."""
    document = ctx.document()
    ids = yield from _transformable(ctx, document)
    if not ids:
        return
    base = yield ctx.point(tr("Specify base point"))
    angle = yield ctx.angle(
        tr("Specify rotation angle"),
        base=base,
        default=ctx.remembered.get(ROTATION),
        drag=Drag("rotate", ids),
    )
    ctx.session.report(document, document.rotate_entities(ids, base, angle))
    ctx.remembered[ROTATION] = angle
    _done(ctx, document, ids)


def scale(ctx: CommandContext) -> CommandScript:
    """About a base point, by a factor typed or shown as the distance from
    the base point to a point."""
    document = ctx.document()
    ids = yield from _transformable(ctx, document)
    if not ids:
        return
    base = yield ctx.point(tr("Specify base point"))
    factor = yield ctx.distance(tr("Specify scale factor"), base=base, drag=Drag("scale", ids))
    ctx.session.report(document, document.scale_entities(ids, base, factor))
    _done(ctx, document, ids)


def mirror(ctx: CommandContext) -> CommandScript:
    """About a line through two points, keeping the source objects unless
    the user says to erase them (AutoCAD's default keeps them)."""
    document = ctx.document()
    ids = yield from _transformable(ctx, document)
    if not ids:
        return
    first = yield ctx.point(tr("Specify first point of mirror line"))
    second = yield ctx.point(
        tr("Specify second point of mirror line"), base=first, drag=Drag("mirror", ids)
    )
    if first == second:
        raise CommandCancelledError(tr("The two points must differ."))
    erase_source = yield ctx.choose(
        tr("Erase source objects?"), [Option("Y", tr("Yes")), Option("N", tr("No"))], default="N"
    )
    try:
        if erase_source == "Y":
            ctx.session.report(document, document.mirror_entities(ids, first, second))
        else:
            with document.group("MIRROR") as group:
                copies, _ = document.copy_entities(ids, (0.0, 0.0))
                document.mirror_entities(copies, first, second)
            if group.changes is not None:
                ctx.session.report(document, group.changes)
    except InvalidGeometryError:
        raise CommandCancelledError(tr("The two points must differ.")) from None
    _done(ctx, document, ids)


def _transformable(ctx: CommandContext, document: CadDocument) -> Selecting:
    """The entities to transform; stop with a message if some kind cannot
    be transformed yet (ADR 0016 D1)."""
    ids = yield from select_objects(ctx, document)
    kinds = sorted({e.kind for e in document.get_entities(ids) if not e.is_transformable})
    if kinds:
        raise CommandCancelledError(
            tr("These objects cannot be transformed yet: {kinds}.", kinds=", ".join(kinds))
        )
    return ids


def _done(ctx: CommandContext, document: CadDocument, ids: Ids) -> None:
    """Remembers the selection for ``P`` and empties it."""
    ctx.remembered[_previous(document)] = ids
    ctx.session.clear_selection(document)


def _all(document: CadDocument) -> Ids:
    """Every entity that can be selected (not on off or frozen layers), in
    the order drawn."""
    bounds = document.get_bounds()
    return () if bounds is None else tuple(sorted(document.select_window(bounds, crossing=True)))


def _previous(document: CadDocument) -> str:
    """Where the previous selection of ``document`` is remembered."""
    return f"PREVIOUS_SELECTION:{document.id}"


def _offset(base: Point, second: Point) -> Point:
    return (second[0] - base[0], second[1] - base[1])


#: The editing commands, with AutoCAD's aliases.
MODIFY_COMMANDS = (
    Command("ERASE", erase, ("E",)),
    Command("MOVE", move, ("M",)),
    Command("COPY", copy, ("CO", "CP")),
    Command("ROTATE", rotate, ("RO",)),
    Command("SCALE", scale, ("SC",)),
    Command("MIRROR", mirror, ("MI",)),
)

__all__ = ["MODIFY_COMMANDS", "select_objects"]
