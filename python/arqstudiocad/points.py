"""Typing points as AutoCAD reads them (ADR 0016 D6), and ORTHO.

Angles are in degrees here, the boundary where people type them (ADR 0010);
counterclockwise from the X axis.

- ``x,y``: absolute;
- ``@dx,dy``: relative to the last point; ``@`` alone is the last point;
- ``@distance<angle``: relative polar;
- ``distance<angle``: absolute polar (from the origin);
- a lone ``distance``: that far from the last point toward the cursor
  (direct distance entry).

No Qt here: the command window and the viewport both use it.
"""

import math

from arqstudiocad.i18n import tr

type Point = tuple[float, float]

#: The object snaps by AutoCAD's key, as the core names them.
SNAP_KEYS: tuple[str, ...] = ("END", "MID", "CEN", "QUA", "INT", "PER", "TAN", "NEA")

#: The running snaps on by default (ADR 0016 D5).
DEFAULT_SNAPS: frozenset[str] = frozenset({"END", "MID", "CEN", "INT"})


class PointSyntaxError(ValueError):
    """What was typed is not a point; the message says why, translated."""


def snap_key(text: str) -> str | None:
    """The snap key typed (``end``, ``MID``…), if it is one."""
    key = text.strip().upper()
    return key if key in SNAP_KEYS else None


def parse_point(text: str, last: Point | None = None, toward: Point | None = None) -> Point:
    """The point ``text`` names. ``last`` is the last point (for ``@`` and a
    lone distance), ``toward`` where the cursor is (for a lone distance)."""
    s = text.strip().replace(" ", "")
    if not s:
        raise PointSyntaxError(tr("Enter a point."))
    relative = s.startswith("@")
    if relative:
        if last is None:
            raise PointSyntaxError(tr("There is no last point."))
        s = s[1:]
        if not s:
            return last
    base = last if relative and last is not None else (0.0, 0.0)
    if "<" in s:
        distance_text, angle_text = s.split("<", 1)
        distance, angle = _number(distance_text), _number(angle_text)
        radians = math.radians(angle)
        return _finite(
            (base[0] + distance * math.cos(radians), base[1] + distance * math.sin(radians))
        )
    if "," in s:
        parts = s.split(",")
        if len(parts) not in (2, 3):
            raise PointSyntaxError(tr("A point is x,y."))
        x, y = _number(parts[0]), _number(parts[1])
        return _finite((base[0] + x, base[1] + y))
    if relative:
        raise PointSyntaxError(tr("After @, type dx,dy or distance<angle."))
    distance = _number(s)
    if last is None:
        raise PointSyntaxError(tr("A distance alone needs a last point."))
    if toward is None or toward == last:
        raise PointSyntaxError(tr("Move the cursor to show the direction."))
    dx, dy = toward[0] - last[0], toward[1] - last[1]
    length = math.hypot(dx, dy)
    return _finite((last[0] + dx / length * distance, last[1] + dy / length * distance))


def ortho(base: Point, point: Point) -> Point:
    """``point`` kept horizontal or vertical from ``base``, whichever is
    closer (ORTHO, ADR 0016 D5)."""
    dx, dy = point[0] - base[0], point[1] - base[1]
    return (point[0], base[1]) if abs(dx) >= abs(dy) else (base[0], point[1])


def _number(text: str) -> float:
    try:
        value = float(text)
    except ValueError:
        raise PointSyntaxError(tr("{text} is not a number.", text=text or "?")) from None
    if not math.isfinite(value):
        raise PointSyntaxError(tr("{text} is not a number.", text=text))
    return value


def _finite(point: Point) -> Point:
    if not all(math.isfinite(c) for c in point):
        raise PointSyntaxError(tr("That point is out of range."))
    return point
