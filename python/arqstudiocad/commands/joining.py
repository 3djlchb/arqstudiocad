"""JOIN, following AutoCAD 2021's (``docs/design/join-command.md``).

One object selected is the source: a line takes collinear lines, an arc the
arcs of its circle, an elliptical arc the arcs of its ellipse (ADR 0041),
an open spline the open splines whose ends touch it (ADR 0043 D4), an open
polyline the lines, arcs and polylines whose ends touch it. Several
selected at once are joined together: chains of touching ends, and lines
and arcs of one line, circle or ellipse.
"""

from collections.abc import Generator
from typing import Any

from arqstudiocad.commands import (
    Background,
    Command,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.commands.modify import Ids, done_selecting
from arqstudiocad.core import CadDocument, CadEllipse, CadError, CadSpline
from arqstudiocad.i18n import tr

#: What can join, and be a source; a polyline or a spline when it is open,
#: an ellipse when it is an arc.
JOINABLE = ("LINE", "ARC", "ELLIPSE", "SPLINE", "LWPOLYLINE")


def join(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using JOIN."))
        return
    chosen = yield from _selected(
        ctx, document, tr("Select source object or multiple objects to join at once")
    )
    assert not isinstance(chosen, str), "no option at the first prompt"
    if not chosen:
        return
    if len(chosen) > 1:
        _join(ctx, document, None, chosen)
        return
    source = chosen[0]
    entity = document.get_entity(source)
    in_plan = entity.kind == "LWPOLYLINE" or entity.geometry is not None
    whole = isinstance(entity.geometry, CadEllipse) and entity.geometry.is_full
    if entity.kind not in JOINABLE or not in_plan or whole:
        ctx.message(tr("JOIN joins lines, arcs, elliptical arcs, splines and open polylines."))
        return
    if entity.kind == "LWPOLYLINE" and dict(entity.details).get("Closed") == "true":
        ctx.message(tr("A closed polyline cannot be joined."))
        return
    if isinstance(entity.geometry, CadSpline) and entity.geometry.is_closed:
        ctx.message(tr("A closed spline cannot be joined."))
        return
    options: list[Option] = []
    if entity.kind == "ARC":
        message = tr("Select arcs to join to source")
        options = [Option("L", tr("Close"))]
    elif entity.kind == "ELLIPSE":
        message = tr("Select elliptical arcs to join to source")
        options = [Option("L", tr("Close"))]
    elif entity.kind == "LINE":
        message = tr("Select lines to join to source")
    elif entity.kind == "SPLINE":
        message = tr("Select splines to join to source")
    else:
        message = tr("Select objects to join to source")
    others = yield from _selected(ctx, document, message, options)
    if others == "L":
        try:
            _, changes = document.close_arc(source)
        except CadError as error:
            ctx.message(tr("Error: {message}", message=str(error)))
            return
        ctx.session.report(document, changes)
        if entity.kind == "ELLIPSE":
            ctx.message(tr("The elliptical arc is now an ellipse."))
        else:
            ctx.message(tr("The arc is now a circle."))
        return
    assert not isinstance(others, str)
    _join(ctx, document, source, others)


def _selected(
    ctx: CommandContext, document: CadDocument, message: str, options: list[Option] | None = None
) -> Generator[Prompt | Background, Any, Ids | str]:
    """What is selected already, or what the user selects now, Enter ending
    it; the key of an option when one is chosen instead."""
    if not ctx.session.selection(document):
        answer = yield ctx.select(message, options or [])
        while answer is not None:
            if options and answer in {o.key for o in options}:
                ctx.session.clear_selection(document)
                return str(answer)
            answer = yield ctx.select(message, options or [])
    chosen = ctx.session.selection(document)
    done_selecting(ctx, document, chosen)
    return chosen


def _join(ctx: CommandContext, document: CadDocument, source: int | None, ids: Ids) -> None:
    try:
        results, joined, discarded, changes = document.join_entities(source, ids)
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return
    if joined:
        ctx.session.report(document, changes)
        if source is None:
            ctx.message(tr("Joined: {count}, into {results}.", count=joined, results=len(results)))
        else:
            ctx.message(tr("Joined to the source: {count}.", count=joined))
    else:
        ctx.message(tr("Nothing joined."))
    if discarded:
        ctx.message(tr("Discarded from the operation: {count}.", count=discarded))


JOIN_COMMAND = Command("JOIN", join, ("J",))
