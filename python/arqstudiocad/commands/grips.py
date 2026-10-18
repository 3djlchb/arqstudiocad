"""Multifunction grips (ADR 0016 D7).

Selected entities show grips (:func:`shown_grips`) while no command runs.
Clicking one makes it hot and starts a grip edit (:func:`grip_edit`), which
asks on the command line, as AutoCAD: its first mode is Stretch, and Enter
or Space cycles Stretch → Move → Rotate → Scale → Mirror around the hot
grip. Base point picks another base, Copy makes each pick a copy (Enter
then ends), Undo takes back the last copy, eXit ends. Stretch moves a
line's end, changes a circle's radius by a quadrant, reshapes an arc by an
end (through its midpoint and other end), moves an entity by its midpoint,
center or insertion point, and moves one polyline vertex at a time. Move,
Rotate, Scale and Mirror apply to every selected entity that can be transformed.
A grip edit is one
undo step and keeps the selection.
"""

import math
from typing import Any

from arqstudiocad.commands import (
    Command,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.core import (
    CadArc,
    CadCircle,
    CadDocument,
    CadGrip,
    CadLine,
    InvalidGeometryError,
)
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr
from arqstudiocad.points import Point
from arqstudiocad.session import Session

#: AutoCAD's GRIPOBJLIMIT: with more entities selected, no grips are shown.
GRIP_LIMIT = 100

#: The modes Enter cycles through, in AutoCAD's order.
MODES = ("STRETCH", "MOVE", "ROTATE", "SCALE", "MIRROR")

#: Grips that move their entity when stretched.
MOVING = {"MID", "CEN", "INS", "VTX"}


def shown_grips(session: Session, document: CadDocument) -> list[CadGrip]:
    """The grips of the selection: none past :data:`GRIP_LIMIT` entities,
    and none of entities on locked layers, which cannot be edited."""
    selected = session.selection(document)
    if len(selected) > GRIP_LIMIT:
        return []
    return document.grips(_unlocked(document, selected))


def grip_edit(grip: CadGrip) -> Command:
    """The grip edit a click on ``grip`` starts."""
    return Command("GRIP", lambda ctx: _edit(ctx, grip), repeat=False)


def _edit(ctx: CommandContext, grip: CadGrip) -> CommandScript:
    document = ctx.document()
    selected = _unlocked(document, ctx.session.selection(document))
    movable = tuple(e.id for e in document.get_entities(selected) if e.is_transformable)
    base = grip.point
    mode = 0
    copy = False
    copies: list[list[int]] = []
    document.begin_group("GRIP")
    try:
        while True:
            answer = yield _ask(ctx, MODES[mode], grip, base, movable)
            if answer is None:
                if copy:
                    return  # Enter ends a run of copies
                mode = (mode + 1) % len(MODES)
                continue
            if answer == "X":
                return
            if answer == "B":
                base = yield ctx.point(tr("Specify base point"))
                continue
            if answer == "C":
                copy = True
                continue
            if answer == "U":
                if copies:
                    ctx.session.report(document, document.delete_entities(copies.pop()))
                else:
                    ctx.message(tr("Nothing to undo."))
                continue
            made = _apply(ctx, document, MODES[mode], grip, base, movable, answer, copy)
            if made is None:
                continue  # refused: asked again
            if not copy:
                return
            copies.append(made)
    finally:
        document.end_group()


def _ask(
    ctx: CommandContext, mode: str, grip: CadGrip, base: Point, movable: tuple[int, ...]
) -> Prompt:
    options = [
        Option("B", tr("Base point")),
        Option("C", tr("Copy")),
        Option("U", tr("Undo")),
        Option("X", tr("Exit")),
    ]
    if mode == "STRETCH":
        # The generic drag overlay translates whole entities. A vertex grip
        # edits one point, so do not show a misleading whole-polyline preview.
        drag = Drag("move", (grip.entity_id,)) if grip.kind in MOVING - {"VTX"} else None
        message = tr("** STRETCH ** Specify stretch point")
        return ctx.point(message, base=base, options=options, optional=True, drag=drag)
    if mode == "MOVE":
        message = tr("** MOVE ** Specify move point")
        return ctx.point(message, base, options, optional=True, drag=Drag("move", movable))
    if mode == "ROTATE":
        message = tr("** ROTATE ** Specify rotation angle")
        drag = Drag("rotate", movable)
        return ctx.angle(message, base, drag=drag, options=options, optional=True)
    if mode == "SCALE":
        message = tr("** SCALE ** Specify scale factor")
        drag = Drag("scale", movable)
        return ctx.distance(message, base, options, drag=drag, optional=True)
    message = tr("** MIRROR ** Specify second point")
    return ctx.point(message, base, options, optional=True, drag=Drag("mirror", movable))


def _apply(
    ctx: CommandContext,
    document: CadDocument,
    mode: str,
    grip: CadGrip,
    base: Point,
    movable: tuple[int, ...],
    answer: Any,
    copy: bool,
) -> list[int] | None:
    """Does one edit; returns the copies made (none without Copy), or None
    if it was refused and nothing changed. The grip edit's group makes all
    of it one undo step, so each operation reports its own changes."""
    if mode == "STRETCH":
        target = (grip.point[0] + answer[0] - base[0], grip.point[1] + answer[1] - base[1])
        return _stretch(ctx, document, grip, target, copy)
    if mode == "MIRROR" and answer == base:
        ctx.message(tr("The two points must differ."))
        return None
    if not movable:
        ctx.message(tr("The selected objects cannot be transformed yet."))
        return None
    ids = list(movable)
    if copy:
        ids, changes = document.copy_entities(ids, (0.0, 0.0))
        ctx.session.report(document, changes)
    if mode == "MOVE":
        changes = document.move_entities(ids, (answer[0] - base[0], answer[1] - base[1]))
    elif mode == "ROTATE":
        changes = document.rotate_entities(ids, base, answer)
    elif mode == "SCALE":
        changes = document.scale_entities(ids, base, answer)
    else:
        changes = document.mirror_entities(ids, base, answer)
    ctx.session.report(document, changes)
    return ids if copy else []


def _stretch(
    ctx: CommandContext, document: CadDocument, grip: CadGrip, target: Point, copy: bool
) -> list[int] | None:
    entity_id = grip.entity_id
    shape = None
    if grip.kind not in MOVING:
        geometry = document.get_entity(entity_id).geometry
        # An arc keeps its midpoint, as the core puts it.
        mid = next((g.point for g in document.grips([entity_id]) if g.kind == "MID"), target)
        try:
            shape = _reshaped(geometry, grip, target, mid)
        except InvalidGeometryError:
            ctx.message(tr("The object cannot take that shape."))
            return None
    if copy:
        (entity_id,), changes = document.copy_entities([entity_id], (0.0, 0.0))
        ctx.session.report(document, changes)
    if grip.kind == "VTX":
        assert grip.vertex_index is not None
        changes = document.set_polyline_vertex(entity_id, grip.vertex_index, target)
    elif shape is None:
        offset = (target[0] - grip.point[0], target[1] - grip.point[1])
        changes = document.move_entities([entity_id], offset)
    else:
        changes = document.set_entity_geometry(entity_id, shape)
    ctx.session.report(document, changes)
    return [entity_id] if copy else []


def _reshaped(
    geometry: Any, grip: CadGrip, target: Point, mid: Point
) -> CadLine | CadCircle | CadArc:
    """A line's end, a circle's quadrant or an arc's end moved to
    ``target``; an arc goes on through its midpoint ``mid``."""
    if isinstance(geometry, CadLine):
        start_moves = _same(geometry.start, grip.point)
        other = geometry.end if start_moves else geometry.start
        return CadLine(target, other) if start_moves else CadLine(other, target)
    if isinstance(geometry, CadCircle):
        return CadCircle(geometry.center, math.dist(geometry.center, target))
    assert isinstance(geometry, CadArc)
    if _same(geometry.start_point, grip.point):
        return CadArc.through(target, mid, geometry.end_point)
    return CadArc.through(geometry.start_point, mid, target)


def _same(a: Point, b: Point) -> bool:
    return math.dist(a, b) <= 1e-9 * (1.0 + abs(a[0]) + abs(a[1]))


def _unlocked(document: CadDocument, ids: tuple[int, ...]) -> tuple[int, ...]:
    locked = {layer.name for layer in document.get_layers() if layer.is_locked}
    return tuple(e.id for e in document.get_entities(ids) if e.layer not in locked)
