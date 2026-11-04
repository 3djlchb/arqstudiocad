"""The view commands of ADR 0030: -VIEW sees the model from one of
AutoCAD's preset views, PLAN from above again.

The views are wireframe, projected by the core. In every view a picked
point falls on the XY plane, except where the view sees it edge on (Front,
Back, Left, Right): there points are typed. Object snaps, ORTHO and grips
work in plan only, for now.
"""

from arqstudiocad.commands import (
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
    ViewControl,
)
from arqstudiocad.core import CadDocument
from arqstudiocad.i18n import tr

#: The preset views, by the name the core takes, in AutoCAD's order.
VIEWS = ("top", "bottom", "front", "back", "left", "right", "swiso", "seiso", "neiso", "nwiso")
#: The views that see the XY plane edge on, where points are typed.
EDGE_ON = ("front", "back", "left", "right")


def view_options() -> list[Option]:
    """-VIEW's options, AutoCAD's: typed by their English names. Written as
    literal tr() calls so the translation test sees every one."""
    labels = (
        tr("Top"),
        tr("Bottom"),
        tr("Front"),
        tr("Back"),
        tr("Left"),
        tr("Right"),
        tr("SWiso"),
        tr("SEiso"),
        tr("NEiso"),
        tr("NWiso"),
    )
    return [Option(name.upper(), label) for name, label in zip(VIEWS, labels, strict=True)]


def view_titles() -> dict[str, str]:
    """Each view's name in menus, as AutoCAD's 3D Views menu."""
    return {
        "top": tr("&Top"),
        "bottom": tr("&Bottom"),
        "front": tr("&Front"),
        "back": tr("Bac&k"),
        "left": tr("&Left"),
        "right": tr("&Right"),
        "swiso": tr("&SW Isometric"),
        "seiso": tr("S&E Isometric"),
        "neiso": tr("&NE Isometric"),
        "nwiso": tr("N&W Isometric"),
    }


def view(ctx: CommandContext) -> CommandScript:
    """-VIEW, AutoCAD's, its preset views only: the named views of its
    View Manager come later."""
    control = _control(ctx, ctx.document())
    choice = yield ctx.choose(tr("Enter an option"), view_options())
    _show(ctx, control, choice.lower())


def plan(ctx: CommandContext) -> CommandScript:
    """PLAN, AutoCAD's: the plan again, all of it. With no user coordinate
    systems yet it asks nothing."""
    _show(ctx, _control(ctx, ctx.document()), "top")
    return
    yield


def _control(ctx: CommandContext, document: CadDocument) -> ViewControl:
    control = None if ctx.ui is None else ctx.ui.view(document)
    if control is None:
        raise CommandCancelledError(tr("There is no view to change."))
    return control


def _show(ctx: CommandContext, control: ViewControl, name: str) -> None:
    control.set_view(name)
    if name in EDGE_ON:
        ctx.message(tr("Type points in this view: it sees the XY plane edge on."))
    if name != "top":
        ctx.message(tr("Object snaps, ORTHO and grips work in plan only, for now."))


#: The view commands, with AutoCAD's alias.
VIEW_COMMANDS = (Command("-VIEW", view, ("-V",)), Command("PLAN", plan))

__all__ = ["EDGE_ON", "VIEWS", "VIEW_COMMANDS", "view_titles"]
