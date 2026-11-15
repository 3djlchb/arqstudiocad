"""JOIN, following AutoCAD 2021's: lines and arcs for now
(``docs/design/join-command.md``).

One object selected is the source, and the lines or arcs selected next are
joined to it; several selected at once are grouped and joined together.
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
from arqstudiocad.core import CadDocument, CadError
from arqstudiocad.i18n import tr

#: What can join in this version, and be a source.
JOINABLE = ("LINE", "ARC")


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
    if entity.kind not in JOINABLE or entity.geometry is None:
        ctx.message(tr("JOIN currently joins lines and arcs only."))
        return
    if entity.kind == "ARC":
        message = tr("Select arcs to join to source")
        options = [Option("L", tr("Close"))]
    else:
        message = tr("Select lines to join to source")
        options = []
    others = yield from _selected(ctx, document, message, options)
    if others == "L":
        try:
            _, changes = document.close_arc(source)
        except CadError as error:
            ctx.message(tr("Error: {message}", message=str(error)))
            return
        ctx.session.report(document, changes)
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
