"""The editing commands of milestone 02.5 and 02.8 (ADR 0016 D3, D8):
ERASE, MOVE, COPY, ROTATE, SCALE, MIRROR and EXPLODE; OFFSET, in
milestone 02.14 (ADR 0035 D4), which picks one object at a time.

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
from arqstudiocad.core import (
    CadArc,
    CadChangeSet,
    CadCircle,
    CadDocument,
    CadError,
    CadLine,
    InvalidGeometryError,
    InvalidOperationError,
)
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr
from arqstudiocad.points import Point

#: The last rotation angle (radians), remembered as AutoCAD remembers it.
ROTATION = "ROTATE_ANGLE"
#: OFFSET's distance (None for Through), whether it erases the source, and
#: the layer of what it makes ("S" the source's, "C" the current one),
#: kept for the session as AutoCAD keeps them (OFFSETDIST, OFFSETERASE).
OFFSET_DISTANCE = "OFFSETDIST"
OFFSET_ERASE = "OFFSETERASE"
OFFSET_LAYER = "OFFSET_LAYER"

type Ids = tuple[int, ...]
type Selecting = Generator[Prompt | Background, Any, Ids]


def select_objects(ctx: CommandContext, document: CadDocument, *, edits: bool = True) -> Selecting:
    """The entities to edit: the selection, or what the user selects now.
    Those on locked layers are left out when the command ``edits`` them;
    an inquiry keeps them, as AutoCAD's. Empty if nothing is left."""
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
    if not edits:
        return chosen
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
        done_selecting(ctx, document, ids)


def explode(ctx: CommandContext) -> CommandScript:
    """Replaces selected block references with their transformed contents;
    those of a block that does not allow exploding are left, said so
    (ADR 0023 D2a)."""
    document = ctx.document()
    ids = yield from select_objects(ctx, document)
    if not ids:
        return
    selected = document.get_entities(ids)
    if any(entity.kind != "INSERT" for entity in selected):
        raise CommandCancelledError(tr("Only block references can be exploded."))
    kept: set[str] = set()
    for entity in selected:
        name = dict(entity.details).get("Block name", "")
        info = document.block_info(name) if name else None
        if info is not None and not info.is_explodable:
            kept.add(info.name)
    for name in sorted(kept, key=str.casefold):
        ctx.message(tr("{name} does not allow exploding.", name=name))
    ids = tuple(
        entity.id
        for entity in selected
        if dict(entity.details).get("Block name", "").casefold() not in {n.casefold() for n in kept}
    )
    if not ids:
        raise CommandCancelledError()
    try:
        _, changes = document.explode_entities(ids)
    except InvalidOperationError as error:
        raise CommandCancelledError(tr("This block has nothing to explode.")) from error
    ctx.session.report(document, changes)
    done_selecting(ctx, document, ids)


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
    done_selecting(ctx, document, ids)


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
    done_selecting(ctx, document, ids)


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
    done_selecting(ctx, document, ids)


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
    done_selecting(ctx, document, ids)


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
    done_selecting(ctx, document, ids)


type Curve = CadLine | CadCircle | CadArc


def offset(ctx: CommandContext) -> CommandScript:
    """OFFSET, AutoCAD's (Spanish EQDIST): a copy of a line, circle, arc
    or lightweight polyline a distance away, to the side a point shows, or
    through a point (Through); one object picked at a time until Exit or
    Enter. A polyline's segments meet again at its corners, extended
    outside and trimmed inside, as AutoCAD's OFFSETGAPTYPE 0. Multiple
    offsets the last copy again; Undo takes back the last offset. The
    source may be erased (Erase) and the copy go on the current layer
    (Layer). Each offset is one undo step; the settings are remembered."""
    document = ctx.document()
    memory = ctx.remembered
    memory.setdefault(OFFSET_DISTANCE, None)
    memory.setdefault(OFFSET_ERASE, False)
    memory.setdefault(OFFSET_LAYER, "S")
    ctx.message(
        tr(
            "Current settings: Erase source={erase}  Layer={layer}",
            erase=tr("Yes") if memory[OFFSET_ERASE] else tr("No"),
            layer=tr("Source") if memory[OFFSET_LAYER] == "S" else tr("Current"),
        )
    )
    yield from _offset_settings(ctx, memory)
    steps = 0  # this command's offsets, for Undo
    while True:
        picked = yield ctx.entity(
            tr("Select object to offset"),
            [Option("E", tr("Exit")), Option("U", tr("Undo"))],
            default="E",
        )
        if picked in (None, "E"):
            return
        if picked == "U":
            steps = _undo_offset(ctx, document, steps)
            continue
        source, _ = picked
        entity = document.get_entity(source)
        geometry = entity.geometry
        # A polyline is offset by the core as a whole (None here).
        if geometry is None and entity.kind != "LWPOLYLINE":
            ctx.message(tr("Cannot offset that object."))
            continue
        ctx.session.select(document, (source,))
        try:
            ended, steps = yield from _offset_one(ctx, document, memory, source, geometry, steps)
        finally:
            ctx.session.clear_selection(document)
        if ended:
            return


def _offset_settings(ctx: CommandContext, memory: dict[str, Any]) -> CommandScript:
    """OFFSET's first prompt: the distance or Through, and the Erase and
    Layer settings, until a distance or Through is given."""
    while True:
        distance = memory[OFFSET_DISTANCE]
        answer = yield ctx.number(
            tr("Specify offset distance"),
            default="T" if distance is None else distance,
            options=[
                Option("T", tr("Through")),
                Option("E", tr("Erase")),
                Option("L", tr("Layer")),
            ],
        )
        if answer == "T":
            memory[OFFSET_DISTANCE] = None
            return
        if answer == "E":
            erase = yield ctx.choose(
                tr("Erase source object after offsetting?"),
                [Option("Y", tr("Yes")), Option("N", tr("No"))],
                default="Y" if memory[OFFSET_ERASE] else "N",
            )
            memory[OFFSET_ERASE] = erase == "Y"
            continue
        if answer == "L":
            memory[OFFSET_LAYER] = yield ctx.choose(
                tr("Enter layer option for offset objects"),
                [Option("C", tr("Current")), Option("S", tr("Source"))],
                default=memory[OFFSET_LAYER],
            )
            continue
        memory[OFFSET_DISTANCE] = float(answer)
        return


def _offset_one(
    ctx: CommandContext,
    document: CadDocument,
    memory: dict[str, Any],
    source: int,
    geometry: Curve | None,
    steps: int,
) -> Generator[Prompt | Background, Any, tuple[bool, int]]:
    """The side (or the point to pass through) for one object, once, or
    again and again from each new copy after Multiple. Returns whether the
    command ends (Exit, or Enter outside Multiple) and the offsets made."""
    distance: float | None = memory[OFFSET_DISTANCE]
    original, multiple = source, False
    while True:
        options = [Option("E", tr("Exit"))]
        if not multiple:
            options.append(Option("M", tr("Multiple")))
        options.append(Option("U", tr("Undo")))
        message = (
            tr("Specify through point")
            if distance is None
            else tr("Specify point on side to offset")
        )
        answer = yield ctx.point(message, options=options, optional=True)
        if answer == "E" or (answer is None and not multiple):
            return True, steps
        if answer is None:  # Enter in Multiple: the next object
            return False, steps
        if answer == "M":
            multiple = True
            continue
        if answer == "U":
            steps = _undo_offset(ctx, document, steps)
            continue
        made: Curve | None = None
        if geometry is not None:
            try:
                made = (
                    geometry.offset_through(answer)
                    if distance is None
                    else geometry.offset(distance, answer)
                )
            except InvalidGeometryError:
                ctx.message(tr("Cannot offset that object to that side."))
                continue
        erase = original if memory[OFFSET_ERASE] and source == original else None
        layer = memory[OFFSET_LAYER] == "C"
        new = _make_offset(ctx, document, source, made, (distance, answer), layer, erase)
        if new is None:
            continue
        steps += 1
        if not multiple:
            return False, steps
        # Multiple goes on from the copy just made.
        source, geometry = new, made


def _make_offset(
    ctx: CommandContext,
    document: CadDocument,
    source: int,
    geometry: Curve | None,
    side: tuple[float | None, Point],
    current_layer: bool,
    erase: int | None,
) -> int | None:
    """A copy of ``source`` with ``geometry``, or, for a polyline (None),
    offset by the core by ``side``'s distance (None: through) toward its
    point; on the current layer if asked, ``erase`` erased, all as one undo
    step named OFFSET. None, with a message, when the core refuses: no
    room on that side, or a locked layer."""
    changes: list[CadChangeSet] = []
    new: int | None = None
    document.begin_group("OFFSET")
    try:
        if geometry is None:
            distance, toward = side
            new, made = document.offset_polyline(source, distance, toward)
            changes.append(made)
        else:
            ids, made = document.copy_entities([source], (0.0, 0.0))
            changes.append(made)
            new = ids[0]
            changes.append(document.set_entity_geometry(new, geometry))
        if current_layer:
            changes.append(document.set_entity_layer([new], document.current_layer.id))
        if erase is not None:
            changes.append(document.delete_entities([erase]))
    except InvalidGeometryError:
        new = None
        ctx.message(tr("Cannot offset that object to that side."))
    except CadError as error:
        new = None
        ctx.message(tr("Error: {message}", message=str(error)))
    finally:
        # An operation failing inside the group rolls the whole group back
        # and closes it (the core's rule): nothing half done stays.
        if document.in_group:
            document.end_group()
    if new is None:
        return None
    # Reported once the step is closed, so the History panel lists it.
    for change in changes:
        ctx.session.report(document, change)
    return new


def _undo_offset(ctx: CommandContext, document: CadDocument, steps: int) -> int:
    """OFFSET's Undo: the last offset of this command taken back."""
    if steps == 0:
        ctx.message(tr("Nothing to undo."))
        return 0
    ctx.session.edit(document, lambda d: d.undo())
    return steps - 1


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


def done_selecting(ctx: CommandContext, document: CadDocument, ids: Ids) -> None:
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
    Command("EXPLODE", explode, ("X",)),
    Command("MOVE", move, ("M",)),
    Command("COPY", copy, ("CO", "CP")),
    Command("ROTATE", rotate, ("RO",)),
    Command("SCALE", scale, ("SC",)),
    Command("MIRROR", mirror, ("MI",)),
    Command("OFFSET", offset, ("O",)),
)

__all__ = ["MODIFY_COMMANDS", "done_selecting", "select_objects"]
