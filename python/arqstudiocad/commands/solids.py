"""The 3D commands of milestone 02.8 (ADR 0024), in the ribbon's 3D tab:
BOX, WEDGE, CYLINDER, CONE, SPHERE and TORUS, which make solids (steps 4a
and 4b), EXTRUDE, which makes them of closed profiles (step 4c), and
MASSPROP, which measures them.

MASSPROP reports the volume, surface area, bounding box and centroid of the
solids selected, together, as AutoCAD's MASSPROP reports a selection;
block references count the solids inside them (maintainer, 2026-11-01).
Moments of inertia, and writing the report to a file, come later. The
figures show every digit the kernel guarantees: precision is lowered only
for files read from or written for another application (maintainer,
2026-11-01).
"""

import math
from collections.abc import Callable, Generator
from typing import Any

from arqstudiocad.commands import (
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.commands.modify import done_selecting, select_objects
from arqstudiocad.core import CadDocument, CadMassProperties, CadSolidMeasures
from arqstudiocad.i18n import tr

#: Significant digits shown: the kernel refines each measure until it
#: settles to 1e-11 (ADR 0024 step 3), so ten hold.
DIGITS = 10
#: Significant digits a double holds.
DOUBLE_DIGITS = 15


def refusals() -> dict[str, Callable[[int], str]]:
    """Why entities were not measured, by the API's reason, in the order
    said. Written as literal tr() calls so the translation test sees every
    one."""
    return {
        "not_a_solid": lambda n: tr("{count} were not solids.", count=n),
        "untranslated": lambda n: tr("{count} solids cannot be read yet.", count=n),
        "empty": lambda n: tr("{count} solids have no faces.", count=n),
        "not_closed": lambda n: tr("{count} do not close around a volume.", count=n),
        "unsupported": lambda n: tr("{count} have faces that cannot be measured yet.", count=n),
        "not_converged": lambda n: tr("{count} could not be measured precisely.", count=n),
        "uneven_scale": lambda n: tr("{count} block references are scaled unevenly.", count=n),
    }


def massprop(ctx: CommandContext) -> CommandScript:
    """MASSPROP, AutoCAD's: the mass properties of the solids selected."""
    document = ctx.document()
    ids = yield from select_objects(ctx, document, edits=False)
    if not ids:
        return
    measures: CadSolidMeasures = yield ctx.background(
        lambda: document.measure_solids(ids), tr("Measuring solids…")
    )
    done_selecting(ctx, document, ids)
    reasons = [why for _, why, _ in measures.refused]
    for why, said in refusals().items():
        if count := reasons.count(why):
            ctx.message(said(count))
    if measures.total is None:
        raise CommandCancelledError(tr("No solid could be measured."))
    for line in report(measures.total, measures.solids):
        ctx.message(line)


def report(total: CadMassProperties, solids: int) -> list[str]:
    """The lines of MASSPROP's report, its labels aligned."""
    low, high = total.bounds
    size = max(b - a for a, b in zip(low, high, strict=True))
    largest = max(abs(c) for c in (*low, *high))
    # A coordinate's digits count across the solids, not from the origin.
    length, area, volume = _places(size, largest), _places(total.area), _places(total.volume)
    labels = {
        "solids": tr("Solids:"),
        "volume": tr("Volume:"),
        "area": tr("Surface area:"),
        "bounds": tr("Bounding box:"),
        "centroid": tr("Centroid:"),
    }
    width = max(len(label) for label in labels.values()) + 2

    def row(label: str, value: str) -> str:
        return label.ljust(width) + value

    lines = [
        f"{'-' * 16}   {tr('SOLIDS')}   {'-' * 16}",
        row(labels["solids"], str(solids)),
        row(labels["volume"], _fixed(total.volume, volume)),
        row(labels["area"], _fixed(total.area, area)),
    ]
    for index, axis in enumerate("XYZ"):
        span = f"{axis}: {_fixed(low[index], length)}  --  {_fixed(high[index], length)}"
        lines.append(row(labels["bounds"] if index == 0 else "", span))
    for index, axis in enumerate("XYZ"):
        at = f"{axis}: {_fixed(total.centroid[index], length)}"
        lines.append(row(labels["centroid"] if index == 0 else "", at))
    return lines


def _places(scale: float, largest: float = 0.0) -> int:
    """Decimal places that show ``DIGITS`` significant digits of
    ``scale``, but no more than a double keeps of ``largest`` (a coordinate
    far from the origin)."""
    places = DIGITS - 1 - math.floor(math.log10(scale)) if scale > 0.0 else DIGITS
    if largest > 0.0:
        places = min(places, DOUBLE_DIGITS - 1 - math.floor(math.log10(largest)))
    return max(0, places)


def _fixed(value: float, places: int) -> str:
    # Adding zero turns a rounded -0.0 into 0.0.
    return f"{round(value, places) + 0.0:.{places}f}"


#: The last box height, remembered as AutoCAD remembers it.
BOX_HEIGHT = "BOX_HEIGHT"
#: The last wedge height.
WEDGE_HEIGHT = "WEDGE_HEIGHT"
#: The last radius and height of each round solid.
CYLINDER_RADIUS, CYLINDER_HEIGHT = "CYLINDER_RADIUS", "CYLINDER_HEIGHT"
CONE_RADIUS, CONE_HEIGHT = "CONE_RADIUS", "CONE_HEIGHT"
SPHERE_RADIUS = "SPHERE_RADIUS"
TORUS_RADIUS, TUBE_RADIUS = "TORUS_RADIUS", "TUBE_RADIUS"
EXTRUDE_HEIGHT = "EXTRUDE_HEIGHT"

#: Makes a solid in ``document`` between two opposite corners ``(x, y, z)``.
type Maker = Callable[[CadDocument, tuple[float, float, float], tuple[float, float, float]], Any]


def box(ctx: CommandContext) -> CommandScript:
    """BOX, AutoCAD's (Spanish PRISMARECT): a box on the XY plane, its
    sides along the axes, between two corners or about its center, as high
    as typed, up or down (ADR 0024 step 4a)."""
    yield from _between(
        ctx,
        lambda d, a, b: d.add_box(a, b),
        BOX_HEIGHT,
        tr("The box needs a length and a width."),
    )


def wedge(ctx: CommandContext) -> CommandScript:
    """WEDGE, AutoCAD's (Spanish CUÑA): asked for as BOX, upright at its
    first corner and sloping down toward the other (ADR 0024 step 4b)."""
    yield from _between(
        ctx,
        lambda d, a, b: d.add_wedge(a, b),
        WEDGE_HEIGHT,
        tr("The wedge needs a length and a width."),
    )


def _between(ctx: CommandContext, make: Maker, remembered: str, flat: str) -> CommandScript:
    """A solid in the box BOX asks for: two corners or a center, a cube or
    its sides, a height; ``make`` gets the corner it starts at first."""
    document = ctx.document()
    first = yield ctx.point(tr("Specify first corner"), options=[Option("C", tr("Center"))])
    center = None
    if first == "C":
        center = yield ctx.point(tr("Specify center"))
        first = center
    sizes = [Option("C", tr("Cube")), Option("L", tr("Length"))]
    while True:
        message = tr("Specify corner") if center is not None else tr("Specify other corner")
        answer = yield ctx.point(message, base=first, options=sizes)
        height = None
        if answer == "C":
            side = yield ctx.distance(tr("Specify length"), base=first)
            low, high, height = (*_sized(first, side, side, center), side)
        elif answer == "L":
            length = yield ctx.distance(tr("Specify length"), base=first)
            width = yield ctx.distance(tr("Specify width"), base=first)
            low, high = _sized(first, length, width, center)
        elif center is not None:
            low, high = answer, _mirror(answer, center)
        else:
            low, high = first, answer
        if low[0] != high[0] and low[1] != high[1]:
            break
        ctx.message(flat)
    if height is None:
        height = yield from _height(ctx, remembered)
        assert isinstance(height, float)
    ctx.remembered[remembered] = height
    # About its center the solid is centered in height too, as AutoCAD's.
    bottom = -height / 2 if center is not None else 0.0
    _, changes = make(document, (*low, bottom), (*high, bottom + height))
    ctx.session.report(document, changes)


def cylinder(ctx: CommandContext) -> CommandScript:
    """CYLINDER, AutoCAD's (Spanish CILINDRO), its basic options: a base
    circle on the XY plane, by its center and radius or diameter, and a
    height up or down (ADR 0024 step 4b). 3P, 2P, Ttr, Elliptical and Axis
    endpoint wait for the user coordinate systems."""
    document = ctx.document()
    center = yield ctx.point(tr("Specify center point of base"))
    radius = yield from _radius(ctx, center, CYLINDER_RADIUS)
    height = yield from _height(ctx, CYLINDER_HEIGHT)
    assert isinstance(height, float)
    _, changes = document.add_cylinder((*center, 0.0), radius, height)
    ctx.session.report(document, changes)


def cone(ctx: CommandContext) -> CommandScript:
    """CONE, AutoCAD's (Spanish CONO), its basic options: as CYLINDER, to a
    point or, given a top radius, to a smaller or larger circle (a
    frustum; ADR 0024 step 4b)."""
    document = ctx.document()
    center = yield ctx.point(tr("Specify center point of base"))
    base = yield from _radius(ctx, center, CONE_RADIUS)
    top = 0.0
    top_radius = Option("T", tr("Top radius"))
    while (height := (yield from _height(ctx, CONE_HEIGHT, (top_radius,)))) == "T":
        top = yield from _top_radius(ctx, top)
    assert isinstance(height, float)
    _, changes = document.add_cone((*center, 0.0), base, top, height)
    ctx.session.report(document, changes)


def extrusion_refusals() -> dict[str, Callable[[int], str]]:
    """Why profiles were not extruded, by the API's reason, in the order
    said. Written as literal tr() calls so the translation test sees every
    one."""
    return {
        "not_a_profile": lambda n: tr("{count} were not closed polylines or circles.", count=n),
        "open": lambda n: tr("{count} polylines were not closed.", count=n),
        "tilted": lambda n: tr("{count} were not in plan.", count=n),
        "crossing": lambda n: tr("{count} cross themselves.", count=n),
        "flat": lambda n: tr("{count} enclose no area.", count=n),
    }


def extrude(ctx: CommandContext) -> CommandScript:
    """EXTRUDE, AutoCAD's (Spanish EXTRUSION), with its height: the closed
    polylines and circles selected made solids on their layers, and
    erased, as AutoCAD's DELOBJ does (ADR 0024 step 4c). Direction, Path
    and Taper angle come later."""
    document = ctx.document()
    ids = yield from select_objects(ctx, document)
    if not ids:
        return
    height = yield from _height(ctx, EXTRUDE_HEIGHT)
    assert isinstance(height, float)
    made, refused, changes = document.extrude(list(ids), height)
    done_selecting(ctx, document, ids)
    reasons = [why for _, why in refused]
    for why, said in extrusion_refusals().items():
        if count := reasons.count(why):
            ctx.message(said(count))
    if not made:
        raise CommandCancelledError(tr("Nothing could be extruded."))
    ctx.session.report(document, changes)


def sphere(ctx: CommandContext) -> CommandScript:
    """SPHERE, AutoCAD's (Spanish ESFERA), its basic options: a center on
    the XY plane and a radius or diameter (ADR 0024 step 4b). 3P, 2P and
    Ttr wait for the user coordinate systems."""
    document = ctx.document()
    center = yield ctx.point(tr("Specify center point"))
    radius = yield from _radius(ctx, center, SPHERE_RADIUS, tr("Specify radius"))
    _, changes = document.add_sphere((*center, 0.0), radius)
    ctx.session.report(document, changes)


def torus(ctx: CommandContext) -> CommandScript:
    """TORUS, AutoCAD's (Spanish TOROIDE), its basic options: a center on
    the XY plane, the torus's radius and the tube's, each or its diameter
    (ADR 0024 step 4b). The tube must be thinner than the torus is wide:
    a self-crossing torus waits."""
    document = ctx.document()
    center = yield ctx.point(tr("Specify center point"))
    radius = yield from _radius(ctx, center, TORUS_RADIUS, tr("Specify radius"))
    while True:
        tube = yield from _radius(ctx, center, TUBE_RADIUS, tr("Specify tube radius"))
        if tube < radius:
            break
        ctx.message(tr("The tube must be thinner than the torus's radius."))
    _, changes = document.add_torus((*center, 0.0), radius, tube)
    ctx.session.report(document, changes)


def _radius(
    ctx: CommandContext, center: tuple[float, float], remembered: str, message: str | None = None
) -> Generator[Prompt, Any, float]:
    """A radius (``message``; a base radius by default), or a diameter
    halved, from ``center``; the last one, kept as ``remembered``, by
    default."""
    answer = yield ctx.distance(
        message or tr("Specify base radius"),
        base=center,
        options=[Option("D", tr("Diameter"))],
        default=ctx.remembered.get(remembered),
    )
    if answer == "D":
        diameter = yield ctx.distance(tr("Specify diameter"), base=center)
        answer = diameter / 2
    ctx.remembered[remembered] = answer
    return float(answer)


def _top_radius(ctx: CommandContext, last: float) -> Generator[Prompt, Any, float]:
    """A cone's top radius: 0 or more, typed; ``last`` by default."""
    while True:
        typed = yield ctx.ask(tr("Specify top radius"), default=f"{last:.10g}")
        try:
            radius = float(typed)
        except ValueError:
            ctx.message(tr("The radius must be a number."))
            continue
        if math.isfinite(radius) and radius >= 0.0:
            return radius
        ctx.message(tr("The radius cannot be negative."))


def _sized(
    at: tuple[float, float], length: float, width: float, center: tuple[float, float] | None
) -> tuple[tuple[float, float], tuple[float, float]]:
    """Two opposite corners of a base ``length`` along X and ``width``
    along Y, from corner ``at`` or about ``center``."""
    if center is not None:
        x, y = center
        return (x - length / 2, y - width / 2), (x + length / 2, y + width / 2)
    return at, (at[0] + length, at[1] + width)


def _mirror(point: tuple[float, float], center: tuple[float, float]) -> tuple[float, float]:
    """``point`` reflected through ``center``."""
    return (2 * center[0] - point[0], 2 * center[1] - point[1])


def _height(
    ctx: CommandContext, remembered: str, more: tuple[Option, ...] = ()
) -> Generator[Prompt, Any, float | str]:
    """A height, typed (negative: down) or the distance between two points
    (2Point); the last one, kept as ``remembered``, by default; or the key
    of one of ``more`` options."""
    last = ctx.remembered.get(remembered)
    shown = None if last is None else f"{last:.10g}"
    two_points = Option("2P", tr("2Point"))
    while True:
        typed = yield Prompt(tr("Specify height"), "text", (two_points, *more), shown)
        chosen = typed.strip().upper()
        for option in more:
            if chosen in (option.key, option.label.upper()):
                return option.key
        if typed.strip().upper() in (two_points.key, two_points.label.upper()):
            a = yield ctx.point(tr("Specify first point"))
            b = yield ctx.point(tr("Specify second point"), base=a)
            height = math.dist(a, b)
        else:
            try:
                height = float(typed)
            except ValueError:
                ctx.message(tr("The height must be a number."))
                continue
        if math.isfinite(height) and height != 0.0:
            ctx.remembered[remembered] = height
            return height
        ctx.message(tr("The height cannot be zero."))


#: The 3D commands.
SOLID_COMMANDS = (
    Command("BOX", box),
    Command("WEDGE", wedge, ("WE",)),
    Command("CYLINDER", cylinder, ("CYL",)),
    Command("CONE", cone),
    Command("SPHERE", sphere),
    Command("TORUS", torus, ("TOR",)),
    Command("EXTRUDE", extrude, ("EXT",)),
    Command("MASSPROP", massprop),
)

__all__ = ["SOLID_COMMANDS", "report"]
