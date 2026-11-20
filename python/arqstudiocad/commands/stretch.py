"""STRETCH (AutoCAD 2021): crossing-window line, arc and polyline editing.

Geometry and atomic edits belong to Rust. Selection provenance belongs to
Session so noun-verb and verb-noun use the same crossing windows.
"""

from arqstudiocad.commands import (
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
)
from arqstudiocad.commands.modify import done_selecting, select_objects
from arqstudiocad.core import InvalidGeometryError
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr

DISPLACEMENT = "STRETCH_DISPLACEMENT"


def stretch(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        raise CommandCancelledError(tr("STRETCH currently requires the Top view. Use PLAN first."))
    ids = yield from select_objects(ctx, document, windows=True)
    if not ids:
        return
    windows = ctx.session.crossing_selection(document)
    polygons = ctx.session.crossing_polygons(document)
    preview = Drag(
        "stretch", ids, crossing=tuple(windows), polygons=tuple((i, tuple(p)) for i, p in polygons)
    )
    base = yield ctx.point(
        tr("Specify base point"),
        options=[Option("D", tr("Displacement"))],
        default="D",
    )
    if base is None or base == "D":
        offset = yield ctx.point(
            tr("Specify displacement"), base=(0.0, 0.0), optional=True, drag=preview
        )
        if offset is None:
            offset = ctx.remembered.get(DISPLACEMENT, (0.0, 0.0))
    else:
        second = yield ctx.point(
            tr("Specify second point <use first point as displacement>"),
            base=base,
            optional=True,
            drag=preview,
        )
        offset = base if second is None else (second[0] - base[0], second[1] - base[1])
    try:
        changes = document.stretch_entities(ids, offset, windows, polygons)
    except InvalidGeometryError:
        raise CommandCancelledError(
            tr(
                "STRETCH cannot collapse endpoints or create invalid geometry. "
                "No objects were changed."
            )
        ) from None
    ctx.session.report(document, changes)
    ctx.remembered[DISPLACEMENT] = offset
    done_selecting(ctx, document, ids)


STRETCH_COMMAND = Command("STRETCH", stretch, ("S",))
