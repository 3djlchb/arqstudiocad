"""Builds the sample template shipped with the application (ADR 0025 D7).

    uv run python tools/make_sample_template.py [output.dwt]

Writes `resources/templates/arqstudiocad-sample.dwt` by default: an empty
model space and ten simple architectural block definitions, drawn in
metres on coloured layers, made only from this script, never copied from
another application's templates. The template and its blocks are dedicated
to the public domain (CC0 1.0, see `resources/templates/README.md`).
"""

from __future__ import annotations

import math
import sys
from collections.abc import Callable
from pathlib import Path

from arqstudiocad.core import CadApplication, CadColor, CadDocument

DEFAULT_OUTPUT = Path(__file__).resolve().parents[1] / "resources" / "templates"
SAMPLE_NAME = "arqstudiocad-sample.dwt"

#: The template's layers and their ACI colours.
LAYERS = {
    "A-DOOR": 30,
    "A-WINDOW": 4,
    "A-FURNITURE": 2,
    "A-SANITARY": 5,
    "A-STRUCTURE": 1,
    "A-LANDSCAPE": 3,
    "A-ANNOTATION": 7,
}

Point = tuple[float, float]


def rectangle(doc: CadDocument, a: Point, b: Point) -> list[int]:
    (x0, y0), (x1, y1) = a, b
    corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    polyline, _ = doc.add_polyline([(p, 0.0) for p in corners], closed=True)
    return [polyline]


def lines(doc: CadDocument, *segments: tuple[Point, Point]) -> list[int]:
    return [doc.add_line(a, b)[0] for a, b in segments]


def door(doc: CadDocument) -> list[int]:
    """A 0.90 m door, hinged at the base point, drawn open with its swing."""
    leaf = lines(doc, ((0.0, 0.0), (0.0, 0.9)))
    swing, _ = doc.add_arc((0.0, 0.0), 0.9, 0.0, math.pi / 2)
    return [*leaf, swing]


def window(doc: CadDocument) -> list[int]:
    """A 1.20 m window in a 0.15 m wall, with its two glass lines."""
    return [
        *rectangle(doc, (0.0, 0.0), (1.2, 0.15)),
        *lines(doc, ((0.0, 0.06), (1.2, 0.06)), ((0.0, 0.09), (1.2, 0.09))),
    ]


def table(doc: CadDocument) -> list[int]:
    """A 1.20 x 0.80 m table with four chairs, centred on the base point."""
    ids = rectangle(doc, (-0.6, -0.4), (0.6, 0.4))
    for x, y in ((0.0, 0.6), (0.0, -0.6), (0.85, 0.0), (-0.85, 0.0)):
        ids += rectangle(doc, (x - 0.2, y - 0.2), (x + 0.2, y + 0.2))
    return ids


def bed(doc: CadDocument) -> list[int]:
    """A 1.50 x 2.00 m bed with two pillows and the turned-down sheet."""
    return [
        *rectangle(doc, (0.0, 0.0), (1.5, 2.0)),
        *rectangle(doc, (0.1, 1.7), (0.7, 1.95)),
        *rectangle(doc, (0.8, 1.7), (1.4, 1.95)),
        *lines(doc, ((0.0, 1.5), (1.5, 1.5))),
    ]


def wc(doc: CadDocument) -> list[int]:
    """A toilet against the wall at the base point: tank and bowl."""
    bowl, _ = doc.add_circle((0.0, 0.42), 0.2)
    return [
        *rectangle(doc, (-0.22, 0.0), (0.22, 0.18)),
        *lines(doc, ((-0.15, 0.18), (-0.2, 0.42)), ((0.15, 0.18), (0.2, 0.42))),
        bowl,
    ]


def washbasin(doc: CadDocument) -> list[int]:
    """A 0.60 m washbasin against the wall: counter, basin and drain."""
    basin, _ = doc.add_circle((0.0, 0.25), 0.15)
    drain, _ = doc.add_circle((0.0, 0.25), 0.02)
    return [*rectangle(doc, (-0.3, 0.0), (0.3, 0.45)), basin, drain]


def column(doc: CadDocument) -> list[int]:
    """A 0.30 x 0.30 m column centred on the base point, crossed."""
    return [
        *rectangle(doc, (-0.15, -0.15), (0.15, 0.15)),
        *lines(doc, ((-0.15, -0.15), (0.15, 0.15)), ((-0.15, 0.15), (0.15, -0.15))),
    ]


def north_arrow(doc: CadDocument) -> list[int]:
    """A north arrow in a 1.00 m circle, pointing up."""
    ring, _ = doc.add_circle((0.0, 0.0), 0.5)
    letter, _ = doc.add_text("N", (-0.07, 0.58), 0.2, 0.0)
    return [
        ring,
        *lines(
            doc,
            ((0.0, 0.5), (-0.15, -0.3)),
            ((0.0, 0.5), (0.15, -0.3)),
            ((-0.15, -0.3), (0.0, -0.15)),
            ((0.15, -0.3), (0.0, -0.15)),
        ),
        letter,
    ]


def level_mark(doc: CadDocument) -> list[int]:
    """A level mark: its triangle points at the base point."""
    label, _ = doc.add_text("+0.00", (0.15, 0.2), 0.1, 0.0)
    return [
        *lines(
            doc,
            ((0.0, 0.0), (-0.1, 0.15)),
            ((0.0, 0.0), (0.1, 0.15)),
            ((-0.1, 0.15), (0.8, 0.15)),
        ),
        label,
    ]


def tree(doc: CadDocument) -> list[int]:
    """A tree in plan: a 3.00 m crown around its trunk."""
    crown, _ = doc.add_circle((0.0, 0.0), 1.5)
    trunk, _ = doc.add_circle((0.0, 0.0), 0.15)
    rays = lines(
        doc,
        *(
            (
                (0.15 * math.cos(a), 0.15 * math.sin(a)),
                (1.2 * math.cos(a), 1.2 * math.sin(a)),
            )
            for a in (k * math.pi / 3 for k in range(6))
        ),
    )
    return [crown, trunk, *rays]


#: The blocks: name, layer, how they are drawn, base point (0, 0).
BLOCKS: list[tuple[str, str, Callable[[CadDocument], list[int]]]] = [
    ("Door_090", "A-DOOR", door),
    ("Window_120", "A-WINDOW", window),
    ("Table_4_Chairs", "A-FURNITURE", table),
    ("Bed_150", "A-FURNITURE", bed),
    ("WC", "A-SANITARY", wc),
    ("Washbasin_060", "A-SANITARY", washbasin),
    ("Column_30x30", "A-STRUCTURE", column),
    ("Tree", "A-LANDSCAPE", tree),
    ("North_Arrow", "A-ANNOTATION", north_arrow),
    ("Level_Mark", "A-ANNOTATION", level_mark),
]


def build(output: Path) -> Path:
    """Writes the sample template to ``output`` and returns it."""
    app = CadApplication()
    doc = app.create_document("ArqStudioCAD sample template")
    layer_ids: dict[str, int] = {}
    for name, color in LAYERS.items():
        layer_id, _ = doc.create_layer(name)
        doc.set_layer_color(layer_id, CadColor.from_index(color))
        layer_ids[name] = layer_id
    zero = doc.get_layer_by_name("0")
    assert zero is not None
    references = []
    for name, layer, draw in BLOCKS:
        doc.set_current_layer(layer_ids[layer])
        ids = draw(doc)
        reference, _ = doc.create_block(name, (0.0, 0.0), ids)
        references.append(reference)
    # A template holds definitions only: nothing in model space.
    doc.delete_entities(references)
    doc.set_current_layer(zero.id)
    output.parent.mkdir(parents=True, exist_ok=True)
    doc.save(output)
    doc.close()
    return output


def main(argv: list[str]) -> int:
    output = Path(argv[0]) if argv else DEFAULT_OUTPUT / SAMPLE_NAME
    print(build(output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
