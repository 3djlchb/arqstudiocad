"""SPLINE, following AutoCAD 2021's (``docs/design/spline-command.md``,
ADR 0042 D4): a spline through fit points, its Knots by Chord, Square root
or Uniform, with start and end Tangency, Undo and Close; or by control
vertices (Method CV), of Degree 1 to 10, with Undo and Close. The method,
knots and degree are remembered for the session, as AutoCAD's SPLMETHOD,
SPLKNOTS and SPLDEGREE. What is drawn follows the cursor."""

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
from arqstudiocad.core import CadSpline
from arqstudiocad.events import Drag
from arqstudiocad.i18n import tr
from arqstudiocad.splines import Knots, Method, Point, SplineShape, direction, make

#: AutoCAD's SPLMETHOD, SPLKNOTS and SPLDEGREE.
METHOD = "SPLMETHOD"
KNOTS = "SPLKNOTS"
DEGREE = "SPLDEGREE"

type Asking[T] = Generator[Prompt | Background, Any, T]


def _knots_names() -> dict[Knots, str]:
    return {"chord": tr("Chord"), "square_root": tr("Square root"), "uniform": tr("Uniform")}


def spline(ctx: CommandContext) -> CommandScript:
    document = ctx.document()
    view = None if ctx.ui is None else ctx.ui.view(document)
    if view is not None and not view.in_plan:
        ctx.message(tr("Use PLAN to switch to the Top view before using SPLINE."))
        return
    memory = ctx.remembered
    memory.setdefault(METHOD, "fit")
    memory.setdefault(KNOTS, "chord")
    memory.setdefault(DEGREE, 3)
    while True:
        method: Method = memory[METHOD]
        if method == "fit":
            ctx.message(
                tr(
                    "Current settings: Method={method}  Knots={knots}",
                    method=tr("Fit"),
                    knots=_knots_names()[memory[KNOTS]],
                )
            )
            options = [Option("M", tr("Method")), Option("K", tr("Knots"))]
        else:
            ctx.message(
                tr(
                    "Current settings: Method={method}  Degree={degree}",
                    method=tr("CV"),
                    degree=memory[DEGREE],
                )
            )
            options = [Option("M", tr("Method")), Option("D", tr("Degree"))]
        answer = yield ctx.point(tr("Specify first point"), options=options)
        if answer == "M":
            chosen = yield ctx.choose(
                tr("Enter spline creation method"),
                [Option("F", tr("Fit")), Option("CV", tr("CV"))],
                default="F" if method == "fit" else "CV",
            )
            memory[METHOD] = "fit" if chosen == "F" else "cv"
            continue
        if answer == "K":
            keys: dict[str, Knots] = {"C": "chord", "S": "square_root", "U": "uniform"}
            names = _knots_names()
            chosen = yield ctx.choose(
                tr("Enter knot parameterization"),
                [Option(key, names[knots]) for key, knots in keys.items()],
                default={v: k for k, v in keys.items()}[memory[KNOTS]],
            )
            memory[KNOTS] = keys[chosen]
            continue
        if answer == "D":
            memory[DEGREE] = yield from _degree(ctx, memory[DEGREE])
            continue
        assert isinstance(answer, tuple)
        shape = SplineShape((answer,), method, memory[KNOTS], memory[DEGREE])
        made = yield from (_fit(ctx, shape) if method == "fit" else _vertices(ctx, shape))
        if made is not None:
            _, changes = document.add_spline(made)
            ctx.session.report(document, changes)
        return


def _degree(ctx: CommandContext, current: int) -> Asking[int]:
    """A degree typed, a whole number from 1 to 10."""
    while True:
        value = yield ctx.number(tr("Enter degree of spline"), default=float(current))
        if value.is_integer() and 1 <= value <= 10:
            return int(value)
        ctx.message(tr("Enter a whole number from 1 to 10."))


def _next(ctx: CommandContext, shape: SplineShape, message: str, options: list[Option]) -> Prompt:
    """The prompt for the next point, the spline following the cursor."""
    drag = Drag("spline", band=True, spline=shape)
    return ctx.point(message, base=shape.points[-1], options=options, optional=True, drag=drag)


def _with(shape: SplineShape, points: tuple[Point, ...]) -> SplineShape:
    return SplineShape(
        points, shape.method, shape.knots, shape.degree, shape.start_tangent, shape.end
    )


def _fit(ctx: CommandContext, shape: SplineShape) -> Asking[CadSpline | None]:
    """Fit points until Enter, Close or an end tangent."""
    while True:
        count = len(shape.points)
        if count == 1:
            options = [Option("T", tr("start Tangency"))]
        else:
            options = [Option("T", tr("end Tangency")), Option("U", tr("Undo"))]
            if count >= 3:
                options.append(Option("C", tr("Close")))
        answer = yield _next(ctx, shape, tr("Enter next point"), options)
        if answer is None:
            return _made(ctx, shape) if count >= 2 else None
        if answer == "T" and count == 1:
            tangent = yield from _tangent(ctx, shape, tr("Specify start tangent"))
            shape = SplineShape(shape.points, shape.method, shape.knots, start_tangent=tangent)
            continue
        if answer == "T":
            end = SplineShape(
                shape.points, shape.method, shape.knots, shape.degree, shape.start_tangent, True
            )
            tangent = yield from _tangent(ctx, end, tr("Specify end tangent"))
            return _made(ctx, shape, tangent)
        if answer == "U":
            shape = _with(shape, shape.points[:-1])
            continue
        if answer == "C":
            return _made(ctx, shape, closed=True)
        shape = _added(ctx, shape, answer)


def _vertices(ctx: CommandContext, shape: SplineShape) -> Asking[CadSpline | None]:
    """Control vertices until Enter or Close."""
    while True:
        count = len(shape.points)
        options = []
        if count >= 3:
            options.append(Option("C", tr("Close")))
        if count >= 2:
            options.append(Option("U", tr("Undo")))
        answer = yield _next(ctx, shape, tr("Enter next point"), options)
        if answer is None:
            return _made(ctx, shape) if count >= 2 else None
        if answer == "U":
            shape = _with(shape, shape.points[:-1])
            continue
        if answer == "C":
            return _made(ctx, shape, closed=True)
        shape = _added(ctx, shape, answer)


def _added(ctx: CommandContext, shape: SplineShape, point: Any) -> SplineShape:
    """``shape`` with ``point`` added, unless it is the last point again."""
    assert isinstance(point, tuple)
    if point == shape.points[-1]:
        ctx.message(tr("The two points must differ."))
        return shape
    return _with(shape, (*shape.points, point))


def _tangent(ctx: CommandContext, shape: SplineShape, message: str) -> Asking[Point]:
    """A tangent's direction, shown by a point from the first point (start)
    or the last (end)."""
    origin = shape.points[-1]
    drag = Drag("spline", band=True, spline=shape) if shape.end else None
    while True:
        point = yield ctx.point(message, base=origin, drag=drag)
        tangent = direction(origin, point)
        if tangent is not None:
            return tangent
        ctx.message(tr("A point on the base point shows no direction."))


def _made(
    ctx: CommandContext,
    shape: SplineShape,
    end_tangent: Point | None = None,
    closed: bool = False,
) -> CadSpline | None:
    """The spline to draw, or a message why there is none."""
    made = make(shape, end_tangent, closed)
    if made is None:
        ctx.message(tr("Those points define no spline."))
    return made


SPLINE_COMMAND = Command("SPLINE", spline, ("SPL",))
