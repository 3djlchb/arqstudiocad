"""TRIM and EXTEND (milestone 02.14, ADR 0035 D4), as AutoCAD's since 2021.

In Quick mode every other object drawn is an edge: the part of the object
picked between the edges nearest the pick is trimmed away (the whole object
erased when no edge bounds it), or its end reaches the nearest edge. In
Standard mode the edges are chosen first ("Select cutting edges" or
"Select boundary edges"; Enter for all). Mode switches between them and is
remembered (AutoCAD's TRIMEXTENDMODE); Shift on a pick does the other
command. Undo takes back the last trim or extension of the run, and TRIM's
Erase erases objects without leaving it. The core does every change, each
one undo step. Lines, arcs, circles and lightweight polylines: a polyline
trimmed in the middle becomes two, a closed one opens; an open one extends
at its end segment. Fences and crossing rectangles select several objects;
Top-view cursor gestures provide a freehand or two-click fence. Each target
is edited once per gesture, with the existing per-object undo behavior.
"""

from collections.abc import Generator
from typing import Any

from arqstudiocad.commands import (
    Background,
    Command,
    CommandContext,
    CommandScript,
    Option,
    Picked,
    PickedPath,
    Prompt,
)
from arqstudiocad.commands.modify import done_selecting, select_objects
from arqstudiocad.core import CadDocument, CadError, InvalidGeometryError
from arqstudiocad.i18n import tr

#: Quick ("Q") or Standard ("S"), shared by TRIM and EXTEND as AutoCAD's
#: TRIMEXTENDMODE; Quick until changed.
MODE = "TRIMEXTENDMODE"

type Edges = tuple[int, ...] | None


def trim(ctx: CommandContext) -> CommandScript:
    """TRIM, AutoCAD's (Spanish RECORTA)."""
    yield from _trim_or_extend(ctx, extending=False)


def extend(ctx: CommandContext) -> CommandScript:
    """EXTEND, AutoCAD's (Spanish ALARGA)."""
    yield from _trim_or_extend(ctx, extending=True)


def _trim_or_extend(ctx: CommandContext, extending: bool) -> CommandScript:
    document = ctx.document()
    memory = ctx.remembered
    memory.setdefault(MODE, "Q")
    _say_mode(ctx, memory[MODE])
    # The edges: None for every object drawn.
    edges: Edges = None
    if memory[MODE] == "S":
        edges = yield from _choose_edges(ctx, document, extending)
    steps = 0  # this run's changes, for Undo
    while True:
        picked = yield _ask_object(ctx, extending)
        if picked is None:
            return
        if picked == "U":
            steps = _undo(ctx, document, steps)
            continue
        if picked in ("T", "B"):
            # Choosing edges is Standard mode, as in AutoCAD.
            memory[MODE] = "S"
            edges = yield from _choose_edges(ctx, document, extending)
            continue
        if picked == "O":
            memory[MODE] = yield ctx.choose(
                tr("Enter a mode option"),
                [Option("Q", tr("Quick")), Option("S", tr("Standard"))],
                default=memory[MODE],
            )
            edges = None
            if memory[MODE] == "S":
                edges = yield from _choose_edges(ctx, document, extending)
            continue
        if picked == "R":
            if (yield from _erase(ctx, document)):
                steps += 1
            continue
        if picked in ("F", "C"):
            crossing = picked == "C"
            first = yield ctx.point(
                tr("Specify first corner") if crossing else tr("Specify first fence point")
            )
            points = [first]
            while True:
                next_point = yield ctx.point(
                    tr("Specify opposite corner") if crossing else tr("Specify next fence point"),
                    base=points[-1],
                    optional=not crossing,
                )
                if next_point is None:
                    break
                points.append(next_point)
                if crossing:
                    break
            picked = PickedPath(tuple(points), crossing=crossing)
        if isinstance(picked, PickedPath):
            if len(picked.points) >= 2:
                picks = document.fence_picks(picked.points, crossing=picked.crossing)
                locked = {layer.name for layer in document.get_layers() if layer.is_locked}
                for entity_id, point in picks:
                    entity = document.get_entity(entity_id)
                    if entity.layer in locked:
                        ctx.message(tr("{count} were on a locked layer.", count=1))
                        continue
                    target = Picked(entity_id, point, picked.shift)
                    if extending != picked.shift:
                        steps += _extend_one(ctx, document, target, edges)
                    else:
                        steps += _trim_one(ctx, document, target, edges, quick=memory[MODE] == "Q")
            continue
        assert isinstance(picked, Picked)
        # Shift does the other command, as AutoCAD's shift-select.
        if extending != picked.shift:
            done = _extend_one(ctx, document, picked, edges)
        else:
            done = _trim_one(ctx, document, picked, edges, quick=memory[MODE] == "Q")
        steps += done


def _say_mode(ctx: CommandContext, mode: str) -> None:
    shown = tr("Quick") if mode == "Q" else tr("Standard")
    ctx.message(tr("Current settings: Mode={mode}", mode=shown))


def _ask_object(ctx: CommandContext, extending: bool) -> Prompt:
    if extending:
        return ctx.entity(
            tr("Select object to extend or shift-select to trim"),
            [
                Option("B", tr("Boundary edges")),
                Option("F", tr("Fence")),
                Option("C", tr("Crossing")),
                Option("O", tr("Mode")),
                Option("U", tr("Undo")),
            ],
            optional=True,
            multiple=True,
        )
    return ctx.entity(
        tr("Select object to trim or shift-select to extend"),
        [
            Option("T", tr("Cutting edges")),
            Option("F", tr("Fence")),
            Option("C", tr("Crossing")),
            Option("O", tr("Mode")),
            Option("R", tr("Erase")),
            Option("U", tr("Undo")),
        ],
        optional=True,
        multiple=True,
    )


def _choose_edges(
    ctx: CommandContext, document: CadDocument, extending: bool
) -> Generator[Prompt | Background, Any, Edges]:
    """The edges selected now (or already); every object drawn for none."""
    ctx.message(tr("Select boundary edges...") if extending else tr("Select cutting edges..."))
    chosen = yield from select_objects(ctx, document, edits=False)
    done_selecting(ctx, document, chosen)
    if not chosen:
        ctx.message(tr("Every object is an edge."))
        return None
    return chosen


def _trim_one(
    ctx: CommandContext, document: CadDocument, picked: Picked, edges: Edges, *, quick: bool
) -> int:
    """Trims the object picked; 1 when it changed, else 0 with why."""
    entity = document.get_entity(picked.entity_id)
    if entity.geometry is None and entity.kind != "LWPOLYLINE":
        ctx.message(tr("Cannot trim that object."))
        return 0
    try:
        _, changes = document.trim_entity(
            picked.entity_id, picked.point, None if edges is None else list(edges), quick
        )
    except InvalidGeometryError:
        ctx.message(tr("No cutting edge crosses that part of the object."))
        return 0
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return 0
    ctx.session.report(document, changes)
    return 1


def _extend_one(ctx: CommandContext, document: CadDocument, picked: Picked, edges: Edges) -> int:
    """Extends the object picked; 1 when it changed, else 0 with why."""
    entity = document.get_entity(picked.entity_id)
    polyline = entity.kind == "LWPOLYLINE"
    closed = polyline and dict(entity.details).get("Closed") == "true"
    if (entity.geometry is None and not polyline) or entity.kind == "CIRCLE" or closed:
        ctx.message(tr("Cannot extend that object."))
        return 0
    try:
        changes = document.extend_entity(
            picked.entity_id, picked.point, None if edges is None else list(edges)
        )
    except InvalidGeometryError:
        ctx.message(tr("Object does not intersect an edge."))
        return 0
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return 0
    ctx.session.report(document, changes)
    return 1


def _erase(ctx: CommandContext, document: CadDocument) -> Generator[Prompt | Background, Any, bool]:
    """TRIM's Erase: objects selected now erased, as one step."""
    chosen = yield from select_objects(ctx, document)
    done_selecting(ctx, document, chosen)
    if not chosen:
        return False
    ctx.session.edit(document, lambda d: d.delete_entities(list(chosen)))
    return True


def _undo(ctx: CommandContext, document: CadDocument, steps: int) -> int:
    """Undo: the run's last change taken back."""
    if steps == 0:
        ctx.message(tr("Nothing to undo."))
        return 0
    ctx.session.edit(document, lambda d: d.undo())
    return steps - 1


#: TRIM and EXTEND, with AutoCAD's aliases.
TRIM_COMMANDS = (
    Command("TRIM", trim, ("TR",)),
    Command("EXTEND", extend, ("EX",)),
)

__all__ = ["TRIM_COMMANDS"]
