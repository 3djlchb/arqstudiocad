"""SPLINE's spline from what its prompts gave, one computation shared by
the command and the viewport's preview (docs/design/spline-command.md).
The geometry itself is the core's (``CadSpline``, ADR 0042)."""

from dataclasses import dataclass
from typing import Literal

from arqstudiocad.core import CadSpline, InvalidGeometryError

type Point = tuple[float, float]
type Method = Literal["fit", "cv"]
type Knots = Literal["chord", "square_root", "uniform"]


@dataclass(frozen=True, slots=True)
class SplineShape:
    """What SPLINE knows of its spline before the cursor: the points so
    far, its method, its knots (Fit) or degree (CV), its start tangent once
    given, and whether the cursor shows the end tangent rather than the
    next point."""

    points: tuple[Point, ...]
    method: Method = "fit"
    knots: Knots = "chord"
    degree: int = 3
    start_tangent: Point | None = None
    end: bool = False


def direction(origin: Point, to: Point) -> Point | None:
    """The vector from ``origin`` to ``to``; none when they coincide."""
    d = (to[0] - origin[0], to[1] - origin[1])
    return None if d == (0.0, 0.0) else d


def make(
    shape: SplineShape, end_tangent: Point | None = None, closed: bool = False
) -> CadSpline | None:
    """The spline of ``shape``'s points, ending along ``end_tangent`` when
    given, closed when ``closed`` (a closed spline takes no tangent); none
    when the core says there is none."""
    points = list(shape.points)
    try:
        if shape.method == "cv":
            return CadSpline.by_vertices(points, shape.degree, closed)
        if closed:
            return CadSpline.through(points, shape.knots, closed=True)
        return CadSpline.through(points, shape.knots, shape.start_tangent, end_tangent)
    except InvalidGeometryError:
        return None


def candidate(shape: SplineShape, at: Point) -> CadSpline | None:
    """The spline SPLINE would draw with the cursor at ``at``: through it
    as the next point, or ending along the direction from the last point
    to it when ``shape.end``. None while there is no such spline."""
    if shape.end:
        tangent = direction(shape.points[-1], at)
        return None if tangent is None else make(shape, tangent)
    return make(
        SplineShape(
            (*shape.points, at), shape.method, shape.knots, shape.degree, shape.start_tangent
        )
    )
