"""World-coordinate indicator, anchored at the model origin (ADR 0034 D4).

Rust supplies the projected directions. Only screen-space glyph layout lives
here; both viewport painters receive the same strokes and labels.
"""

import math
from collections.abc import Sequence

from arqstudiocad.i18n import tr
from arqstudiocad.ui.overlay import RGBA, XY, Label, Overlay, Stroke, circle

AXIS_LENGTH = 38.0


def world_axes_overlay(
    origin: XY,
    axes: Sequence[XY],
    width: int,
    height: int,
    background: tuple[int, int, int],
) -> Overlay:
    """Positive XYZ arms in logical pixels, never relocated to a corner.

    An axis seen end-on has a ring and label instead of a zero-length arrow.
    The indicator leaves the viewport when its model origin leaves it.
    """
    out = Overlay()
    x, y = origin
    if not (0 <= x < width and 0 <= y < height):
        return out
    light = sum(c * w for c, w in zip(background, (0.299, 0.587, 0.114), strict=True)) > 127.5
    colors: tuple[RGBA, RGBA, RGBA] = (
        (230, 56, 56, 255),
        (56, 153 if light else 217, 56, 255),
        (56, 115, 230, 255),
    )
    scale = AXIS_LENGTH / max(math.hypot(*axis) for axis in axes)
    for (dx, dy), color, label in zip(axes, colors, (tr("X"), tr("Y"), tr("Z")), strict=True):
        dx, dy = dx * scale, dy * scale
        length = math.hypot(dx, dy)
        if length < 1.0:
            out.items.append(Stroke(circle(origin, 3.0), color, 2.0, closed=True))
            out.labels.append(Label((x + 7.0, y + 15.0), label, color))
            continue
        nx, ny = dx / length, dy / length
        tip = (x + dx, y + dy)
        out.items.append(Stroke((origin, tip), color, 2.0))
        out.items.append(
            Stroke(
                (
                    (tip[0] - 7 * nx - 3 * ny, tip[1] - 7 * ny + 3 * nx),
                    tip,
                    (tip[0] - 7 * nx + 3 * ny, tip[1] - 7 * ny - 3 * nx),
                ),
                color,
                2.0,
            )
        )
        out.labels.append(Label((tip[0] + nx * 10 - 3, tip[1] + ny * 10 + 4), label, color))
    return out
