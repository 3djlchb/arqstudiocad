"""FILLET and CHAMFER (milestone 02.14, ADR 0035 D4), as AutoCAD's.

Two objects picked one after the other meet at a corner: FILLET by an arc
of the radius set, CHAMFER by a segment its distances (or its length and
angle) set. Each object keeps the side it was picked on and, in Trim mode,
is trimmed or extended to the joint; Shift on the second pick makes a sharp
corner. Radius, distances, length, angle, method and the Trim mode are
remembered (AutoCAD's FILLETRAD, CHAMFERA…D, CHAMMODE, TRIMMODE, the last
shared by both). Multiple goes on to the next pair; Undo takes back the
last corner of the run. The core does every change, each one undo step.
Lines, for FILLET a line and an arc, and lightweight polylines: the
Polyline option cuts every corner between straight segments at once (and
says how many were too short), and two segments of one polyline picked
cut the corner they share.
"""

import math
from collections.abc import Generator
from functools import partial
from typing import Any

from arqstudiocad.commands import (
    Background,
    Command,
    CommandContext,
    CommandScript,
    Option,
    Picked,
    Prompt,
)
from arqstudiocad.core import CadArc, CadDocument, CadError, CadLine, InvalidGeometryError
from arqstudiocad.i18n import tr

FILLET_RADIUS = "FILLETRAD"
#: "T" trims and extends the objects to the joint, "N" leaves them.
TRIM_MODE = "TRIMMODE"
CHAMFER_FIRST = "CHAMFERA"
CHAMFER_SECOND = "CHAMFERB"
CHAMFER_LENGTH = "CHAMFERC"
#: Radians in memory; degrees on the command line (ADR 0010).
CHAMFER_ANGLE = "CHAMFERD"
#: "D" by two distances, "A" by a length and an angle.
CHAMFER_METHOD = "CHAMMODE"
DEFAULTS: dict[str, Any] = {
    FILLET_RADIUS: 0.0,
    TRIM_MODE: "T",
    CHAMFER_FIRST: 0.0,
    CHAMFER_SECOND: 0.0,
    CHAMFER_LENGTH: 0.0,
    CHAMFER_ANGLE: 0.0,
    CHAMFER_METHOD: "D",
}

type Asking[T] = Generator[Prompt | Background, Any, T]


def _shown(value: float) -> str:
    return f"{value:.10g}"


def _mode(memory: dict[str, Any]) -> str:
    return tr("Trim") if memory[TRIM_MODE] == "T" else tr("No trim")


def fillet(ctx: CommandContext) -> CommandScript:
    """FILLET, AutoCAD's (Spanish EMPALME)."""
    document = ctx.document()
    memory = ctx.remembered
    for key, value in DEFAULTS.items():
        memory.setdefault(key, value)
    ctx.message(
        tr(
            "Current settings: Mode={mode}, Radius={radius}",
            mode=_mode(memory),
            radius=_shown(memory[FILLET_RADIUS]),
        )
    )
    multiple, steps = False, 0
    while True:
        first = yield ctx.entity(
            tr("Select first object"),
            [
                Option("U", tr("Undo")),
                Option("P", tr("Polyline")),
                Option("R", tr("Radius")),
                Option("T", tr("Trim")),
                Option("M", tr("Multiple")),
            ],
            optional=True,
        )
        if first is None:
            return
        if first == "U":
            steps = _undo(ctx, document, steps)
            continue
        if first == "R":
            yield from _radius(ctx, memory)
            continue
        if first == "P":
            radius = memory[FILLET_RADIUS]
            steps += yield from _whole_polyline(
                ctx, document, partial(document.fillet_polyline, radius=radius), fillet=True
            )
            if not multiple:
                return
            continue
        if first == "T":
            yield from _trim_mode(ctx, memory)
            continue
        if first == "M":
            multiple = True
            continue
        assert isinstance(first, Picked)
        polyline = _is_polyline(document, first)
        if not (polyline or _fits(document, first, CadLine | CadArc)):
            ctx.message(tr("Cannot fillet that object."))
            continue
        second = yield from _second(
            ctx,
            document,
            first,
            tr("Select second object or shift-select to apply corner"),
            [Option("R", tr("Radius"))],
            lambda answer: _radius(ctx, memory),
            same=polyline,
        )
        if second is None:
            continue
        radius = 0.0 if second.shift else memory[FILLET_RADIUS]
        if polyline or _is_polyline(document, second):
            if second.entity_id != first.entity_id:
                ctx.message(tr("Cannot fillet those objects."))
                continue
            picks = (first.point, second.point)
            steps += _corner_of_polyline(
                ctx, document, partial(document.fillet_polyline, first.entity_id, radius, picks)
            )
            if not multiple:
                return
            continue
        if not _fits(document, second, CadLine | CadArc):
            ctx.message(tr("Cannot fillet that object."))
            continue
        done = _apply(
            ctx,
            document,
            document.fillet_entities,
            (
                first.entity_id,
                first.point,
                second.entity_id,
                second.point,
                radius,
                memory[TRIM_MODE] == "T",
            ),
            tr("Cannot fillet those objects with that radius."),
        )
        steps += done
        if not multiple:
            return


def chamfer(ctx: CommandContext) -> CommandScript:
    """CHAMFER, AutoCAD's (Spanish CHAFLAN)."""
    document = ctx.document()
    memory = ctx.remembered
    for key, value in DEFAULTS.items():
        memory.setdefault(key, value)
    _say_chamfer(ctx, memory)
    multiple, steps = False, 0
    while True:
        first = yield ctx.entity(
            tr("Select first line"),
            [
                Option("U", tr("Undo")),
                Option("P", tr("Polyline")),
                Option("D", tr("Distance")),
                Option("A", tr("Angle")),
                Option("T", tr("Trim")),
                Option("E", tr("Method")),
                Option("M", tr("Multiple")),
            ],
            optional=True,
        )
        if first is None:
            return
        if first == "U":
            steps = _undo(ctx, document, steps)
            continue
        if first in ("D", "A", "E"):
            yield from _chamfer_setting(ctx, memory, first)
            continue
        if first == "P":
            cut = _chamfer_cut(memory)
            steps += yield from _whole_polyline(
                ctx,
                document,
                partial(document.chamfer_polyline, first=cut[0], second=cut[1], by_angle=cut[2]),
                fillet=False,
            )
            if not multiple:
                return
            continue
        if first == "T":
            yield from _trim_mode(ctx, memory)
            continue
        if first == "M":
            multiple = True
            continue
        assert isinstance(first, Picked)
        polyline = _is_polyline(document, first)
        if not (polyline or _fits(document, first, CadLine)):
            ctx.message(tr("Cannot chamfer that object."))
            continue
        second = yield from _second(
            ctx,
            document,
            first,
            tr("Select second line or shift-select to apply corner"),
            [Option("D", tr("Distance")), Option("A", tr("Angle")), Option("E", tr("Method"))],
            lambda answer: _chamfer_setting(ctx, memory, answer),
            same=polyline,
        )
        if second is None:
            continue
        if polyline or _is_polyline(document, second):
            if second.entity_id != first.entity_id:
                ctx.message(tr("Cannot chamfer those objects."))
                continue
            cut = (0.0, 0.0, False) if second.shift else _chamfer_cut(memory)
            picks = (first.point, second.point)
            steps += _corner_of_polyline(
                ctx,
                document,
                partial(document.chamfer_polyline, first.entity_id, cut[0], cut[1], picks, cut[2]),
            )
            if not multiple:
                return
            continue
        if not _fits(document, second, CadLine):
            ctx.message(tr("Cannot chamfer that object."))
            continue
        distances = _chamfer_distances(document, memory, first, second)
        if distances is None:
            ctx.message(tr("Cannot chamfer those lines at that angle."))
            continue
        a, b = (0.0, 0.0) if second.shift else distances
        done = _apply(
            ctx,
            document,
            document.chamfer_entities,
            (
                first.entity_id,
                first.point,
                second.entity_id,
                second.point,
                a,
                b,
                memory[TRIM_MODE] == "T",
            ),
            tr("Cannot chamfer those lines with those distances."),
        )
        steps += done
        if not multiple:
            return


def _say_chamfer(ctx: CommandContext, memory: dict[str, Any]) -> None:
    if memory[CHAMFER_METHOD] == "D":
        ctx.message(
            tr(
                "Current settings: Mode={mode}, Distance 1={first}, Distance 2={second}",
                mode=_mode(memory),
                first=_shown(memory[CHAMFER_FIRST]),
                second=_shown(memory[CHAMFER_SECOND]),
            )
        )
    else:
        ctx.message(
            tr(
                "Current settings: Mode={mode}, Length={length}, Angle={angle}",
                mode=_mode(memory),
                length=_shown(memory[CHAMFER_LENGTH]),
                angle=_shown(math.degrees(memory[CHAMFER_ANGLE])),
            )
        )


def _fits(document: CadDocument, picked: Picked, kinds: Any) -> bool:
    return isinstance(document.get_entity(picked.entity_id).geometry, kinds)


def _second(
    ctx: CommandContext,
    document: CadDocument,
    first: Picked,
    message: str,
    options: list[Option],
    setting: Any,
    same: bool = False,
) -> Asking[Picked | None]:
    """The second object, the first highlighted meanwhile; an option sets
    what it names and asks again; Enter gives up this pair. The first again
    only when ``same`` (another segment of a polyline)."""
    ctx.session.select(document, (first.entity_id,))
    try:
        while True:
            second = yield ctx.entity(message, options, optional=True)
            if second is None:
                return None
            if isinstance(second, Picked):
                if second.entity_id == first.entity_id and not same:
                    ctx.message(tr("Select another object."))
                    continue
                return second
            yield from setting(second)
    finally:
        ctx.session.clear_selection(document)


def _is_polyline(document: CadDocument, picked: Picked) -> bool:
    return document.get_entity(picked.entity_id).kind == "LWPOLYLINE"


def _chamfer_cut(memory: dict[str, Any]) -> tuple[float, float, bool]:
    """The chamfer in force as the core's polyline cut takes it."""
    if memory[CHAMFER_METHOD] == "D":
        return memory[CHAMFER_FIRST], memory[CHAMFER_SECOND], False
    return memory[CHAMFER_LENGTH], memory[CHAMFER_ANGLE], True


def _whole_polyline(
    ctx: CommandContext, document: CadDocument, cut: Any, *, fillet: bool
) -> Asking[int]:
    """The Polyline option: a polyline picked, every corner cut; 1 when
    one was."""
    picked = yield ctx.entity(tr("Select 2D polyline"), optional=True)
    if not isinstance(picked, Picked):
        return 0
    if not _is_polyline(document, picked):
        ctx.message(
            tr("Cannot fillet that object.") if fillet else tr("Cannot chamfer that object.")
        )
        return 0
    try:
        done, short, changes = cut(picked.entity_id)
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return 0
    ctx.session.report(document, changes)
    if fillet:
        ctx.message(tr("{count} lines were filleted", count=done))
    else:
        ctx.message(tr("{count} lines were chamfered", count=done))
    if short:
        ctx.message(tr("{count} were too short", count=short))
    return 1 if done else 0


def _corner_of_polyline(ctx: CommandContext, document: CadDocument, cut: Any) -> int:
    """Two segments of one polyline picked: the corner they share; 1 when
    it was cut."""
    try:
        done, short, changes = cut()
    except InvalidGeometryError:
        ctx.message(tr("Those segments share no corner."))
        return 0
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return 0
    ctx.session.report(document, changes)
    if short:
        ctx.message(tr("The segments are too short for it."))
    return 1 if done else 0


def _radius(ctx: CommandContext, memory: dict[str, Any]) -> Asking[None]:
    memory[FILLET_RADIUS] = yield ctx.number(
        tr("Specify fillet radius"), default=memory[FILLET_RADIUS], zero=True
    )


def _trim_mode(ctx: CommandContext, memory: dict[str, Any]) -> Asking[None]:
    memory[TRIM_MODE] = yield ctx.choose(
        tr("Enter Trim mode option"),
        [Option("T", tr("Trim")), Option("N", tr("No trim"))],
        default=memory[TRIM_MODE],
    )


def _chamfer_setting(ctx: CommandContext, memory: dict[str, Any], key: str) -> Asking[None]:
    """CHAMFER's Distance, Angle and Method."""
    if key == "D":
        first = yield ctx.number(
            tr("Specify first chamfer distance"), default=memory[CHAMFER_FIRST], zero=True
        )
        second = yield ctx.number(tr("Specify second chamfer distance"), default=first, zero=True)
        memory[CHAMFER_FIRST], memory[CHAMFER_SECOND] = first, second
        memory[CHAMFER_METHOD] = "D"
    elif key == "A":
        memory[CHAMFER_LENGTH] = yield ctx.number(
            tr("Specify chamfer length on the first line"),
            default=memory[CHAMFER_LENGTH],
            zero=True,
        )
        degrees = yield ctx.number(
            tr("Specify chamfer angle from the first line"),
            default=math.degrees(memory[CHAMFER_ANGLE]),
            zero=True,
        )
        memory[CHAMFER_ANGLE] = math.radians(degrees)
        memory[CHAMFER_METHOD] = "A"
    else:
        memory[CHAMFER_METHOD] = yield ctx.choose(
            tr("Enter trim method"),
            [Option("D", tr("Distance")), Option("A", tr("Angle"))],
            default=memory[CHAMFER_METHOD],
        )


def _chamfer_distances(
    document: CadDocument, memory: dict[str, Any], first: Picked, second: Picked
) -> tuple[float, float] | None:
    """The two distances the method in force gives; None when the angle
    never meets the second line."""
    if memory[CHAMFER_METHOD] == "D":
        return memory[CHAMFER_FIRST], memory[CHAMFER_SECOND]
    length, angle = memory[CHAMFER_LENGTH], memory[CHAMFER_ANGLE]
    if length == 0.0:
        return 0.0, 0.0
    a = document.get_entity(first.entity_id).geometry
    b = document.get_entity(second.entity_id).geometry
    assert isinstance(a, CadLine) and isinstance(b, CadLine)
    try:
        return length, CadLine.chamfer_distance(a, first.point, b, second.point, length, angle)
    except InvalidGeometryError:
        return None


def _apply(
    ctx: CommandContext, document: CadDocument, make: Any, args: tuple[Any, ...], refused: str
) -> int:
    """Makes the corner with ``make(*args)``; 1 when it did, else 0 with
    why."""
    try:
        _, changes = make(*args)
    except InvalidGeometryError:
        ctx.message(refused)
        return 0
    except CadError as error:
        ctx.message(tr("Error: {message}", message=str(error)))
        return 0
    ctx.session.report(document, changes)
    return 1


def _undo(ctx: CommandContext, document: CadDocument, steps: int) -> int:
    if steps == 0:
        ctx.message(tr("Nothing to undo."))
        return 0
    ctx.session.edit(document, lambda d: d.undo())
    return steps - 1


#: FILLET and CHAMFER, with AutoCAD's aliases.
CORNER_COMMANDS = (
    Command("FILLET", fillet, ("F",)),
    Command("CHAMFER", chamfer, ("CHA",)),
)

__all__ = ["CORNER_COMMANDS"]
