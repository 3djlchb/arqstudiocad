"""ELLIPSE's ellipse from what its prompts gave, one computation shared by
the command and the viewport's preview (docs/design/ellipse-command.md).
The geometry itself is the core's (``CadEllipse``)."""

import math

from arqstudiocad.core import CadEllipse, InvalidGeometryError

type Point = tuple[float, float]


def relative_angle(center: Point, first: Point, angle: float) -> float:
    """``angle``, a direction in the world, measured from the first axis
    drawn, from ``center`` toward ``first``, as ELLIPSE's Arc takes it."""
    return angle - math.atan2(first[1] - center[1], first[0] - center[0])


def direction(center: Point, point: Point) -> float:
    """The direction from ``center`` to ``point``, radians."""
    return math.atan2(point[1] - center[1], point[0] - center[0])


def candidate(
    center: Point,
    first: Point,
    other: float | None,
    start: float | None,
    parameters: bool,
    at: Point,
) -> CadEllipse | None:
    """The ellipse ELLIPSE would draw with the cursor at ``at``: the other
    semi-axis its distance from the center while ``other`` is not known;
    then the whole ellipse while no ``start`` is (a start angle from the
    first axis, or a parameter); then the arc from it to ``at``'s
    direction. None while there is no such ellipse."""
    try:
        whole = CadEllipse.from_axis(
            center, first, math.dist(center, at) if other is None else other
        )
        if other is None or start is None:
            return whole
        end = relative_angle(center, first, direction(center, at))
        return whole.arc(first, start, end, parameters)
    except InvalidGeometryError:
        return None
