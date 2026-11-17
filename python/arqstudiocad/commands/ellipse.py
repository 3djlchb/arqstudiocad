"""ELLIPSE, following AutoCAD 2021's (``docs/design/ellipse-command.md``):
an ellipse by an axis and the distance to the other, or by its Center; the
other axis by a Rotation instead; an elliptical Arc by start and end angles,
or Parameters, or an Included angle. What is drawn follows the cursor."""

import math
from collections.abc import Callable, Generator
from typing import Any

from arqstudiocad.commands import (
    Background,
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
    Prompt,
)
from arqstudiocad.core import CadEllipse, InvalidGeometryError
from arqstudiocad.ellipses import Point, direction
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr


def ellipse(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using ELLIPSE."))
        return
    arc = False
    answer = yield ctx.point(
        tr("Specify axis endpoint of ellipse"),
        options=[Option("A", tr("Arc")), Option("C", tr("Center"))],
    )
    if answer == "A":
        arc = True
        answer = yield ctx.point(
            tr("Specify axis endpoint of elliptical arc"), options=[Option("C", tr("Center"))]
        )
    first: Point
    if answer == "C":
        center = yield ctx.point(tr("Specify center of ellipse"))
        first = yield ctx.point(tr("Specify endpoint of axis"), base=center)
    else:
        assert isinstance(answer, tuple)
        first = answer
        other_end = yield ctx.point(tr("Specify other endpoint of axis"), base=first)
        center = ((first[0] + other_end[0]) / 2.0, (first[1] + other_end[1]) / 2.0)
    shape = Drag("ellipse", band=True, ellipse=(center, first, None, None, False))
    distance = yield ctx.distance(
        tr("Specify distance to other axis"),
        base=center,
        options=[Option("R", tr("Rotation"))],
        drag=shape,
    )
    if distance == "R":
        rotation = yield ctx.angle(tr("Specify rotation around major axis"), base=center)
        whole = _make(lambda: CadEllipse.from_rotation(center, first, rotation))
    else:
        whole = _make(lambda: CadEllipse.from_axis(center, first, distance))
    if arc:
        whole = yield from _arc(ctx, whole, center, first)
    _, changes = document.add_ellipse(whole)
    ctx.session.report(document, changes)


def _other(whole: CadEllipse, center: Point, first: Point) -> float:
    """The other semi-axis of ``whole``, the one not toward ``first``."""
    major = math.hypot(*whole.major_axis)
    half = math.dist(center, first)
    return whole.ratio * major if math.isclose(half, major) else major


def _arc(
    ctx: CommandContext, whole: CadEllipse, center: Point, first: Point
) -> Generator[Prompt | Background, Any, CadEllipse]:
    """The elliptical arc of ``whole`` its start and end prompts give: each
    angle or parameter measured from the first axis drawn, typed or shown
    by a point (the prompts' ``angle_base``)."""
    other = _other(whole, center, first)
    axis = direction(center, first)
    parameters = False
    shown = Drag("ellipse", band=True, ellipse=(center, first, other, None, False))
    start = yield ctx.angle(
        tr("Specify start angle"),
        base=center,
        options=[Option("P", tr("Parameter"))],
        drag=shown,
        angle_base=axis,
    )
    if start == "P":
        parameters = True
        start = yield ctx.angle(
            tr("Specify start parameter"), base=center, drag=shown, angle_base=axis
        )
    following = Drag("ellipse", band=True, ellipse=(center, first, other, start, parameters))
    end_options = [Option("I", tr("Included angle"))]
    if not parameters:
        end_options.insert(0, Option("P", tr("Parameter")))
    end = yield ctx.angle(
        tr("Specify end parameter") if parameters else tr("Specify end angle"),
        base=center,
        options=end_options,
        drag=following,
        angle_base=axis,
    )
    if end == "P":
        parameters = True
        end = yield ctx.angle(
            tr("Specify end parameter"), base=center, drag=following, angle_base=axis
        )
    elif end == "I":
        included = yield ctx.angle(tr("Specify included angle for arc"), base=center)
        end = start + included
    return _make(lambda: whole.arc(first, start, end, parameters))


def _make(make: Callable[[], CadEllipse]) -> CadEllipse:
    """The ellipse ``make`` builds, or stop: the core said there is none."""
    try:
        return make()
    except InvalidGeometryError:
        raise CommandCancelledError(tr("Those points and distances define no ellipse.")) from None


ELLIPSE_COMMAND = Command("ELLIPSE", ellipse, ("EL",))
