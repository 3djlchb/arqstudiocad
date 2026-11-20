"""BREAK's initial 2D subset, following AutoCAD 2021's point workflow."""

from arqstudiocad.commands import Command, CommandContext, CommandScript, Option, Picked
from arqstudiocad.core import CadError, InvalidGeometryError
from arqstudiocad.i18n import tr


def break_object(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using BREAK."))
        return
    while True:
        picked = yield ctx.entity(tr("Select object to break"), optional=True)
        if picked is None:
            return
        assert isinstance(picked, Picked)
        entity = document.get_entity(picked.entity_id)
        if entity.kind != "LWPOLYLINE" and document.editable_curve(picked.entity_id) is None:
            ctx.message(
                tr(
                    "BREAK currently supports lines, arcs, circles, ellipses and "
                    "lightweight polylines."
                )
            )
            continue
        first = picked.point
        second = yield ctx.point(
            tr("Specify second break point"),
            base=first,
            options=[Option("F", tr("First point"))],
        )
        if second == "F":
            first = yield ctx.point(tr("Specify first break point"))
            second = yield ctx.point(tr("Specify second break point"), base=first)
        try:
            _, changes = document.break_entity(picked.entity_id, first, second)
        except InvalidGeometryError:
            ctx.message(tr("Those points do not define a valid break."))
            return
        except CadError as error:
            ctx.message(tr("Error: {message}", message=str(error)))
            return
        ctx.session.report(document, changes)
        return


BREAK_COMMAND = Command("BREAK", break_object, ("BR",))
