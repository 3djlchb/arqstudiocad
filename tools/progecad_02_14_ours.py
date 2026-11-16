"""Our side of the 02.14 commands' comparison with ProgeCAD (docs/testing/
commands-02-14-windows.md): the same synthetic sources and points as
`arq:compare-commands` in tools/progecad_command_compare.lsp, through our
core's operations, each result saved as DWG and DXF for ProgeCAD to reopen.

    uv run python tools/progecad_02_14_ours.py <folder>
"""

import math
import sys
from collections.abc import Callable
from pathlib import Path

from arqstudiocad._core import (
    CadApplication,
    CadArc,
    CadBounds2,
    CadCircle,
    CadDocument,
    CadLine,
    CadRectangle,
)

SQUARE = [((0.0, 0.0), 0.0), ((10.0, 0.0), 0.0), ((10.0, 6.0), 0.0), ((0.0, 6.0), 0.0)]


def line(doc: CadDocument, a: tuple[float, float], b: tuple[float, float]) -> int:
    return doc.add_line(a, b)[0]


def arc(doc: CadDocument, start: float, end: float) -> int:
    """An arc about the origin, radius 5, angles in degrees."""
    return doc.add_arc((0, 0), 5.0, math.radians(start), math.radians(end))[0]


def offset(doc: CadDocument, entity: int, distance: float, toward: tuple[float, float]) -> None:
    """OFFSET of a line, circle or arc: its geometry offset and added."""
    geometry = doc.get_entities([entity])[0].geometry
    assert isinstance(geometry, CadLine | CadCircle | CadArc)
    doc.add_geometry(geometry.offset(distance, toward))


def rectang(
    doc: CadDocument,
    fillet: float = 0.0,
    chamfer: tuple[float, float] = (0.0, 0.0),
    width: float = 0.0,
    elevation: float = 0.0,
) -> None:
    path = CadRectangle((0, 0), (10, 6)).path(fillet, chamfer)
    doc.add_polyline(path, closed=True, elevation=elevation, width=width)


def o1(doc: CadDocument) -> None:
    offset(doc, line(doc, (0, 0), (10, 0)), 2.0, (5, 5))


def o2(doc: CadDocument) -> None:
    offset(doc, doc.add_circle((0, 0), 5.0)[0], 1.0, (10, 0))


def o3(doc: CadDocument) -> None:
    offset(doc, arc(doc, 0, 90), 1.0, (1, 1))


def o4(doc: CadDocument) -> None:
    doc.offset_polyline(doc.add_polyline(SQUARE, closed=True)[0], 1.0, (-5, -5))


def o5(doc: CadDocument) -> None:
    path = [((0.0, 0.0), 0.0), ((10.0, 0.0), math.tan(math.pi / 8)), ((10.0, 6.0), 0.0)]
    doc.offset_polyline(doc.add_polyline(path)[0], 1.0, (5, 5))


def t1(doc: CadDocument) -> None:
    target = line(doc, (0, 0), (10, 0))
    line(doc, (5, -5), (5, 5))
    doc.trim_entity(target, (8, 0))


def t2(doc: CadDocument) -> None:
    target = doc.add_circle((0, 0), 5.0)[0]
    line(doc, (-10, 0), (10, 0))
    doc.trim_entity(target, (0, 5))


def t3(doc: CadDocument) -> None:
    target = line(doc, (0, 0), (10, 0))
    line(doc, (3, -5), (3, 5))
    line(doc, (7, -5), (7, 5))
    doc.trim_entity(target, (5, 0))


def e1(doc: CadDocument) -> None:
    target = line(doc, (0, 0), (4, 0))
    boundary = line(doc, (10, -5), (10, 5))
    doc.extend_entity(target, (3, 0), [boundary])


def e2(doc: CadDocument) -> None:
    target = arc(doc, 0, 45)
    boundary = line(doc, (0, -10), (0, 10))
    doc.extend_entity(target, (3.9931, 3.0091), [boundary])


def s1(doc: CadDocument) -> None:
    square = doc.add_polyline(SQUARE, closed=True)[0]
    doc.stretch_entities([square], (3, 0), [(square, CadBounds2((8, -1), (12, 7)))])


def s2(doc: CadDocument) -> None:
    target = line(doc, (0, 0), (10, 0))
    doc.stretch_entities([target], (3, 2), [(target, CadBounds2((8, -1), (12, 1)))])


def f1(doc: CadDocument) -> None:
    first, second = line(doc, (0, 0), (8, 0)), line(doc, (10, 2), (10, 10))
    doc.fillet_entities(first, (4, 0), second, (10, 8), 2.0)


def f2(doc: CadDocument) -> None:
    doc.fillet_polyline(doc.add_polyline(SQUARE, closed=True)[0], 1.0)


def c1(doc: CadDocument) -> None:
    first, second = line(doc, (0, 0), (8, 0)), line(doc, (10, 2), (10, 10))
    doc.chamfer_entities(first, (4, 0), second, (10, 8), 2.0, 3.0)


def c2(doc: CadDocument) -> None:
    doc.chamfer_polyline(doc.add_polyline(SQUARE, closed=True)[0], 1.0, 1.0)


def b1(doc: CadDocument) -> None:
    doc.break_entity(line(doc, (0, 0), (10, 0)), (3, 0), (7, 0))


def b2(doc: CadDocument) -> None:
    doc.break_entity(doc.add_circle((0, 0), 5.0)[0], (5, 0), (0, 5))


def b3(doc: CadDocument) -> None:
    doc.break_entity(arc(doc, 0, 180), (4.330127019, 2.5), (2.5, 4.330127019))


def b4(doc: CadDocument) -> None:
    path = [((0.0, 0.0), 0.0), ((10.0, 0.0), 0.0), ((10.0, 10.0), 0.0)]
    doc.break_entity(doc.add_polyline(path)[0], (5, 0), (10, 5))


def j1(doc: CadDocument) -> None:
    source, other = line(doc, (0, 0), (4, 0)), line(doc, (6, 0), (10, 0))
    doc.join_entities(source, [other])


def j2(doc: CadDocument) -> None:
    source, other = arc(doc, 0, 90), arc(doc, 180, 270)
    doc.join_entities(source, [other])


CASES: list[tuple[str, Callable[[CadDocument], None]]] = [
    ("O1-offset-line", o1),
    ("O2-offset-circle-out", o2),
    ("O3-offset-arc-in", o3),
    ("O4-offset-closed-polyline-out", o4),
    ("O5-offset-open-polyline-with-arc", o5),
    ("T1-trim-line-end", t1),
    ("T2-trim-circle-top", t2),
    ("T3-trim-line-middle", t3),
    ("E1-extend-line", e1),
    ("E2-extend-arc", e2),
    ("S1-stretch-polyline-side", s1),
    ("S2-stretch-line-end", s2),
    ("F1-fillet-lines", f1),
    ("F2-fillet-polyline", f2),
    ("C1-chamfer-lines", c1),
    ("C2-chamfer-polyline", c2),
    ("B1-break-line", b1),
    ("B2-break-circle", b2),
    ("B3-break-arc", b3),
    ("B4-break-open-polyline", b4),
    ("J1-join-collinear-lines", j1),
    ("J2-join-arcs", j2),
    ("R1-rectang", rectang),
    ("R2-rectang-fillet", lambda doc: rectang(doc, fillet=1.0)),
    ("R3-rectang-chamfer", lambda doc: rectang(doc, chamfer=(1.0, 2.0))),
    ("R4-rectang-width-elevation", lambda doc: rectang(doc, width=0.5, elevation=2.0)),
]


def main() -> None:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    app = CadApplication()
    for name, run in CASES:
        doc = app.create_document(name)
        try:
            run(doc)
        except Exception as error:  # refused: written down, nothing saved
            print(f"{name}: refused: {type(error).__name__}: {error}")
            continue
        for ext in ("dwg", "dxf"):
            doc.save(out / f"ours-{name}.{ext}")
        print(f"{name}: {len(doc.get_entities())} entities; saved")


if __name__ == "__main__":
    main()
