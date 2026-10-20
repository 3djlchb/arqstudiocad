"""The drawing commands of milestone 2.5 (ADR 0016 D8): LINE, CIRCLE and ARC;
TEXT and MTEXT, AutoCAD's single-line and multiline texts, in milestone 02.8
(ADR 0022).

They ask for points as AutoCAD does (typed or picked, with snaps and
ORTHO) and build the curve with the core's constructors: Python never
computes geometry itself (contract §5). Everything one run of a command
draws is one undo step; what was drawn before Esc stays, as in AutoCAD.
"""

import math
from collections.abc import Callable

from arqstudiocad.commands import (
    Command,
    CommandCancelledError,
    CommandContext,
    CommandScript,
    Option,
)
from arqstudiocad.core import CadArc, CadCircle, CadDocument, InvalidGeometryError
from arqstudiocad.i18n import tr
from arqstudiocad.points import Point

#: The last circle radius, remembered as AutoCAD's CIRCLERAD.
CIRCLE_RADIUS = "CIRCLERAD"
#: The last text height and rotation, remembered as AutoCAD remembers them.
TEXT_HEIGHT = "TEXTSIZE"
TEXT_ROTATION = "TEXT_ROTATION"
#: Line spacing of single-line texts, in text heights (AutoCAD's).
LINE_SPACING = 5 / 3
#: The last multiline text justification, by its option key.
MTEXT_JUSTIFY = "MTEXT_JUSTIFY"
#: MTEXT justifications: option key → AutoCAD's attachment point, and where
#: it hangs on the rectangle (0 left or bottom, 0.5 middle, 1 right or top).
JUSTIFICATIONS = {
    "TL": ("TopLeft", 0.0, 1.0),
    "TC": ("TopCenter", 0.5, 1.0),
    "TR": ("TopRight", 1.0, 1.0),
    "ML": ("MiddleLeft", 0.0, 0.5),
    "MC": ("MiddleCenter", 0.5, 0.5),
    "MR": ("MiddleRight", 1.0, 0.5),
    "BL": ("BottomLeft", 0.0, 0.0),
    "BC": ("BottomCenter", 0.5, 0.0),
    "BR": ("BottomRight", 1.0, 0.0),
}


def line(ctx: CommandContext) -> CommandScript:
    """Segments from point to point until Enter; Undo takes back the last
    one, Close joins the last point to the first."""
    document = ctx.document()
    document.begin_group("LINE")
    try:
        points: list[Point] = [(yield ctx.point(tr("Specify first point")))]
        segments: list[int] = []
        while True:
            options = [Option("U", tr("Undo"))]
            if len(segments) >= 2:
                options.append(Option("C", tr("Close")))
            answer = yield ctx.point(
                tr("Specify next point"), base=points[-1], options=options, optional=True
            )
            if answer is None:
                return
            if answer == "U":
                if not segments:
                    ctx.message(tr("All segments already undone."))
                    continue
                ctx.session.report(document, document.delete_entities([segments.pop()]))
                points.pop()
                continue
            end = points[0] if answer == "C" else answer
            try:
                entity_id, changes = document.add_line(points[-1], end)
            except InvalidGeometryError:
                ctx.message(tr("The two points must differ."))
                continue
            ctx.session.report(document, changes)
            if answer == "C":
                return
            segments.append(entity_id)
            points.append(end)
    finally:
        # Esc keeps what was drawn, as one undo step.
        document.end_group()


def circle(ctx: CommandContext) -> CommandScript:
    """A circle by center and radius (or Diameter), by the two ends of a
    diameter (2P) or through three points (3P)."""
    document = ctx.document()
    answer = yield ctx.point(
        tr("Specify center point for circle"), options=[Option("3P", "3P"), Option("2P", "2P")]
    )
    if answer == "3P":
        a = yield ctx.point(tr("Specify first point on circle"))
        b = yield ctx.point(tr("Specify second point on circle"), base=a)
        c = yield ctx.point(tr("Specify third point on circle"), base=b)
        geometry = _build(
            lambda: CadCircle.through(a, b, c), tr("Three points on one line define no circle.")
        )
    elif answer == "2P":
        a = yield ctx.point(tr("Specify first end point of circle's diameter"))
        b = yield ctx.point(tr("Specify second end point of circle's diameter"), base=a)
        geometry = _build(lambda: CadCircle.from_diameter(a, b), tr("The two points must differ."))
    else:
        center = answer
        last = ctx.remembered.get(CIRCLE_RADIUS)
        radius = yield ctx.distance(
            tr("Specify radius of circle"),
            base=center,
            options=[Option("D", tr("Diameter"))],
            default=last,
        )
        if radius == "D":
            diameter = yield ctx.distance(
                tr("Specify diameter of circle"),
                base=center,
                default=None if last is None else 2 * last,
            )
            radius = diameter / 2
        geometry = CadCircle(center, radius)
    _add(ctx, document, geometry)
    ctx.remembered[CIRCLE_RADIUS] = geometry.radius


def arc(ctx: CommandContext) -> CommandScript:
    """An arc through three points, or by its start, center and end (the
    end gives only the direction; arcs run counterclockwise)."""
    document = ctx.document()
    answer = yield ctx.point(tr("Specify start point of arc"), options=[Option("C", tr("Center"))])
    if answer == "C":
        center = yield ctx.point(tr("Specify center point of arc"))
        start = yield ctx.point(tr("Specify start point of arc"), base=center)
        end = yield ctx.point(tr("Specify end point of arc"), base=center)
        geometry = _start_center_end(start, center, end)
    else:
        start = answer
        second = yield ctx.point(
            tr("Specify second point of arc"),
            base=start,
            options=[Option("C", tr("Center")), Option("E", tr("End"))],
        )
        if second == "C":
            center = yield ctx.point(tr("Specify center point of arc"), base=start)
            end = yield ctx.point(tr("Specify end point of arc"), base=center)
            geometry = _start_center_end(start, center, end)
        elif second == "E":
            end = yield ctx.point(tr("Specify end point of arc"), base=start)
            center = yield ctx.point(tr("Specify center point of arc"), base=end)
            geometry = _start_center_end(start, center, end)
        else:
            end = yield ctx.point(tr("Specify end point of arc"), base=second)
            geometry = _build(
                lambda: CadArc.through(start, second, end),
                tr("Three points on one line define no arc."),
            )
    _add(ctx, document, geometry)


def _start_center_end(start: Point, center: Point, end: Point) -> CadArc:
    return _build(
        lambda: CadArc.from_start_center_end(start, center, end),
        tr("The start, the center and the end must be three different directions."),
    )


def _build[T](make: Callable[[], T], refused: str) -> T:
    """The curve ``make`` builds, or stop with ``refused``: the core said
    the points define none."""
    try:
        return make()
    except InvalidGeometryError:
        raise CommandCancelledError(refused) from None


def _add(ctx: CommandContext, document: CadDocument, geometry: CadCircle | CadArc) -> None:
    _, changes = document.add_geometry(geometry)
    ctx.session.report(document, changes)


def text(ctx: CommandContext) -> CommandScript:
    """Single-line texts from a start point, one per line typed, each below
    the last, until an empty line (ADR 0022 D1). One undo step."""
    document = ctx.document()
    start = yield ctx.point(tr("Specify start point of text"))
    height = yield ctx.distance(
        tr("Specify height"),
        base=start,
        default=ctx.remembered.get(TEXT_HEIGHT, document.text_size),
    )
    rotation = yield ctx.angle(
        tr("Specify rotation angle of text"),
        base=start,
        default=ctx.remembered.get(TEXT_ROTATION, 0.0),
    )
    ctx.remembered[TEXT_HEIGHT] = height
    ctx.remembered[TEXT_ROTATION] = rotation
    # The next line starts this far below, across the reading direction.
    step = LINE_SPACING * height
    down = (math.sin(rotation) * step, -math.cos(rotation) * step)
    at = start
    document.begin_group("TEXT")
    try:
        while line := (yield ctx.ask(tr("Enter text"), optional=True)):
            _, changes = document.add_text(line, at, height, rotation)
            ctx.session.report(document, changes)
            at = (at[0] + down[0], at[1] + down[1])
    finally:
        document.end_group()


def mtext(ctx: CommandContext) -> CommandScript:
    """A multiline text in the rectangle of two corners, its paragraphs
    wrapped to the rectangle's width (ADR 0022 D6). Height, Justify and
    Rotation are options of the second corner. One undo step."""
    document = ctx.document()
    height = ctx.remembered.get(TEXT_HEIGHT, document.text_size)
    rotation = ctx.remembered.get(TEXT_ROTATION, 0.0)
    justify = ctx.remembered.get(MTEXT_JUSTIFY, "TL")
    first = yield ctx.point(tr("Specify first corner"))
    options = [
        Option("H", tr("Height")),
        Option("J", tr("Justify")),
        Option("R", tr("Rotation")),
    ]
    while True:
        answer = yield ctx.point(tr("Specify opposite corner"), base=first, options=options)
        if answer == "H":
            height = yield ctx.distance(tr("Specify height"), base=first, default=height)
        elif answer == "R":
            rotation = yield ctx.angle(tr("Specify rotation angle"), base=first, default=rotation)
        elif answer == "J":
            justify = yield ctx.choose(
                tr("Enter justification"),
                [Option(key, justification_label(key)) for key in JUSTIFICATIONS],
                default=justify,
            )
        else:
            opposite = answer
            break
    # The rectangle, in the reading direction: its width and the point the
    # justification hangs the text from.
    cos, sin = math.cos(rotation), math.sin(rotation)
    dx, dy = opposite[0] - first[0], opposite[1] - first[1]
    along, across = dx * cos + dy * sin, -dx * sin + dy * cos
    name, fx, fy = JUSTIFICATIONS[justify]
    x = min(0.0, along) + fx * abs(along)
    y = min(0.0, across) + fy * abs(across)
    anchor = (first[0] + x * cos - y * sin, first[1] + x * sin + y * cos)
    if ctx.ui is not None:
        content = ctx.ui.edit_text(tr("Multiline text"))
    else:
        lines = []
        while line := (yield ctx.ask(tr("Enter text"), optional=True)):
            lines.append(line)
        content = "\n".join(lines)
    if not content or not content.strip():
        return  # nothing typed: nothing drawn, no step
    _, changes = document.add_mtext(content, anchor, height, rotation, abs(along), name)
    ctx.session.report(document, changes)
    ctx.remembered[TEXT_HEIGHT] = height
    ctx.remembered[TEXT_ROTATION] = rotation
    ctx.remembered[MTEXT_JUSTIFY] = justify


def justification_label(key: str) -> str:
    """An MTEXT justification option, translated."""
    return {
        "TL": tr("Top left"),
        "TC": tr("Top center"),
        "TR": tr("Top right"),
        "ML": tr("Middle left"),
        "MC": tr("Middle center"),
        "MR": tr("Middle right"),
        "BL": tr("Bottom left"),
        "BC": tr("Bottom center"),
        "BR": tr("Bottom right"),
    }[key]


#: The drawing commands, with AutoCAD's aliases.
DRAW_COMMANDS = (
    Command("LINE", line, ("L",)),
    Command("CIRCLE", circle, ("C",)),
    Command("ARC", arc, ("A",)),
    Command("TEXT", text, ("DT",)),
    Command("MTEXT", mtext, ("T", "MT")),
)
