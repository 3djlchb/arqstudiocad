"""MATCHPROP, following AutoCAD 2021's (``docs/design/matchprop-command.md``):
one object's properties given to the objects selected next.

The source is picked; then each selection, ended by Enter, receives the
properties in one undo step, until Enter with nothing selected. Settings
chooses the properties, remembered for the session.
"""

from arqstudiocad.commands import Command, CommandContext, CommandScript, Option, Picked
from arqstudiocad.commands.modify import done_selecting
from arqstudiocad.core import CadError
from arqstudiocad.i18n import tr

#: The basic properties, as Property Settings lists them.
BASIC = ("color", "layer", "linetype", "ltscale", "lineweight", "transparency", "thickness")
#: The special properties the drawing model holds so far.
SPECIAL = ("polyline",)
#: Where the chosen properties are remembered for the session.
MEMORY = "MATCHPROP_SETTINGS"


def setting_label(name: str) -> str:
    """A property's name as the user reads it."""
    labels = {
        "color": tr("Color"),
        "layer": tr("Layer"),
        "linetype": tr("Linetype"),
        "ltscale": tr("Linetype scale"),
        "lineweight": tr("Lineweight"),
        "transparency": tr("Transparency"),
        "thickness": tr("Thickness"),
        "polyline": tr("Polyline"),
    }
    return labels[name]


def matchprop(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    ctx.session.clear_selection(document)
    picked = yield ctx.entity(tr("Select source object"), optional=True)
    if picked is None:
        return
    assert isinstance(picked, Picked)
    source = picked.entity_id
    settings: frozenset[str] = ctx.remembered.setdefault(MEMORY, frozenset(BASIC + SPECIAL))
    _say_settings(ctx, settings)
    while True:
        answer = yield ctx.select(tr("Select destination object(s)"), [Option("S", tr("Settings"))])
        if answer == "S":
            ctx.session.clear_selection(document)
            chosen = None if ctx.ui is None else ctx.ui.choose_match_settings(settings)
            if chosen is not None:
                settings = ctx.remembered[MEMORY] = frozenset(chosen)
            _say_settings(ctx, settings)
            continue
        targets = ctx.session.selection(document)
        done_selecting(ctx, document, targets)
        if not targets:
            return
        try:
            _, locked, changes = document.match_properties(source, targets, sorted(settings))
        except CadError as error:
            ctx.message(tr("Error: {message}", message=str(error)))
            return
        ctx.session.report(document, changes)
        if locked:
            ctx.message(tr("{count} were on a locked layer.", count=locked))


def _say_settings(ctx: CommandContext, settings: frozenset[str]) -> None:
    names = " ".join(setting_label(n) for n in BASIC + SPECIAL if n in settings)
    ctx.message(tr("Current active settings: {names}", names=names or tr("none")))


MATCHPROP_COMMAND = Command("MATCHPROP", matchprop, ("MA",))
