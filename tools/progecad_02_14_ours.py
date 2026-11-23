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
    CadColor,
    CadDocument,
    CadEllipse,
    CadLine,
    CadRectangle,
    CadSpline,
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


def j3(doc: CadDocument) -> None:
    source = doc.add_polyline([((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0)], width=0.5)[0]
    doc.join_entities(source, [line(doc, (4, 0), (4, 3))])


def j4(doc: CadDocument) -> None:
    source = doc.add_polyline([((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0)], width=0.2)[0]
    other = doc.add_polyline([((8.0, 0.0), 0.0), ((4.0, 0.0), 0.0)], width=0.8)[0]
    doc.join_entities(source, [other])


def j5(doc: CadDocument) -> None:
    sides = [((0, 0), (4, 0)), ((4, 0), (4, 3)), ((0, 3), (4, 3)), ((0, 0), (0, 3))]
    doc.join_entities(None, [line(doc, a, b) for a, b in sides])


def j6(doc: CadDocument) -> None:
    source = doc.add_polyline([((0.0, 0.0), 0.0), ((1.0, 0.0), 0.0)])[0]
    doc.join_entities(source, [line(doc, (1, 0), (2, 1)), line(doc, (1, 0), (2, -1))])


def j7(doc: CadDocument) -> None:
    first = line(doc, (0, 0), (4, 0))
    other = doc.add_arc((4, 2), 2.0, math.radians(270), math.radians(90))[0]
    doc.join_entities(None, [first, other])


EVERY = [
    "color",
    "layer",
    "linetype",
    "ltscale",
    "lineweight",
    "transparency",
    "thickness",
    "text",
    "polyline",
]


def m1(doc: CadDocument) -> None:
    layer = doc.create_layer("A-WALLS")[0]
    source = line(doc, (0, 0), (4, 0))
    doc.set_entity_layer([source], layer)
    doc.set_entity_color([source], CadColor.from_index(1))
    target = doc.add_circle((8, 0), 1.0)[0]
    doc.match_properties(source, [target], EVERY)


def m2(doc: CadDocument) -> None:
    path = [((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0)]
    source = doc.add_polyline(path, width=0.5, thickness=2.0)[0]
    target = doc.add_polyline([((0.0, 2.0), 0.0), ((4.0, 2.0), 0.0)])[0]
    doc.match_properties(source, [target], EVERY)


def m3(doc: CadDocument) -> None:
    # A varying width, as JOIN leaves two widths: 0.2 then 0.8.
    source = doc.add_polyline([((0.0, 0.0), 0.0), ((4.0, 0.0), 0.0)], width=0.2)[0]
    other = doc.add_polyline([((4.0, 0.0), 0.0), ((8.0, 0.0), 0.0)], width=0.8)[0]
    doc.join_entities(source, [other])
    target = doc.add_polyline([((0.0, 2.0), 0.0), ((4.0, 2.0), 0.0)], width=0.25)[0]
    doc.match_properties(source, [target], EVERY)


def m4(doc: CadDocument) -> None:
    source = doc.add_text("A-101", (0, 0), 2.5, 0.0)[0]
    doc.match_properties(source, [doc.add_text("B-202", (0, 5), 1.0, 0.0)[0]], EVERY)


def m5(doc: CadDocument) -> None:
    source = doc.add_text("A-101", (0, 0), 2.5, 0.0)[0]
    target = doc.add_mtext("Notes", (0, 5), 1.0, 0.0, 10.0, "TopLeft")[0]
    doc.match_properties(source, [target], EVERY)


def el1(doc: CadDocument) -> None:
    doc.add_ellipse(CadEllipse.from_axis((5, 0), (0, 0), 3.0))


def el2(doc: CadDocument) -> None:
    doc.add_ellipse(CadEllipse.from_axis((5, 0), (7, 0), 4.0))


def el3(doc: CadDocument) -> None:
    doc.add_ellipse(CadEllipse.from_rotation((5, 0), (0, 0), math.radians(60)))


def el4(doc: CadDocument) -> None:
    whole = CadEllipse.from_axis((5, 0), (0, 0), 3.0)
    doc.add_ellipse(whole.arc((0, 0), math.radians(30), math.radians(120)))


SPLINE_POINTS = [(0.0, 0.0), (3.0, 4.0), (7.0, 1.0), (9.0, 6.0)]


def spline(made: CadSpline) -> Callable[[CadDocument], None]:
    """A case drawing ``made``, as SPLINE draws it."""
    return lambda doc: drop(doc.add_spline(made))


def drop(_: object) -> None:
    """Ignores a result."""


def ell(doc: CadDocument, start: float = 0.0, end: float = 0.0) -> int:
    """About (0,0), semi-axes 5 and 3, from start to end (equal: whole)."""
    return doc.add_ellipse(CadEllipse((0, 0), (5, 0), 0.6, start, end))[0]


def el5(doc: CadDocument) -> None:
    target = ell(doc)
    line(doc, (4, -9), (4, 9))
    doc.trim_entity(target, (5, 0))


def el6(doc: CadDocument) -> None:
    target = ell(doc, 0.0, math.pi)
    line(doc, (0, 0), (0, 9))
    line(doc, (-4, 0), (-4, 9))
    doc.trim_entity(target, (-2.5, 2.598076211))


def el7(doc: CadDocument) -> None:
    target = ell(doc, 0.0, math.pi / 2)
    boundary = line(doc, (-4, -9), (-4, 9))
    doc.extend_entity(target, (0.2, 2.997599), [boundary])


def el8(doc: CadDocument) -> None:
    doc.break_entity(ell(doc), (5, 0), (0, 3))


def el9(doc: CadDocument) -> None:
    target = line(doc, (-10, 1.8), (-8, 1.8))
    boundary = ell(doc)
    doc.extend_entity(target, (-8.5, 1.8), [boundary])


def el10(doc: CadDocument) -> None:
    target = ell(doc)
    # The window crosses the ellipse and holds its center, as ProgeCAD's must
    # to select it.
    doc.stretch_entities([target], (2, 3), [(target, CadBounds2((-1, -1), (6, 1)))])


def el11(doc: CadDocument) -> None:
    target = ell(doc, 0.0, math.pi / 2)
    doc.stretch_entities([target], (1, 0), [(target, CadBounds2((-1, -1), (6, 4)))])


def el12(doc: CadDocument) -> None:
    target = ell(doc, 0.0, math.pi / 2)
    doc.stretch_entities([target], (1, 0), [(target, CadBounds2((4, -1), (6, 1)))])


def el13(doc: CadDocument) -> None:
    source = ell(doc, 0.0, 1.0)
    doc.join_entities(source, [ell(doc, 1.5, 2.5), ell(doc, 2.5, 3.0)])


def el14(doc: CadDocument) -> None:
    doc.join_entities(None, [ell(doc, 0.0, 2.0), ell(doc, 2.0, 4.0), ell(doc, 4.0, 0.0)])


def el15(doc: CadDocument) -> None:
    doc.close_arc(ell(doc, 0.0, 1.0))


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
    ("J3-join-polyline-width-line", j3),
    ("J4-join-polylines-two-widths", j4),
    ("J5-join-rectangle-at-once", j5),
    ("J6-join-polyline-branch", j6),
    ("J7-join-line-arc-at-once", j7),
    ("M1-matchprop-line-to-circle", m1),
    ("M2-matchprop-polyline-width", m2),
    ("M3-matchprop-varying-width", m3),
    ("M4-matchprop-text-height", m4),
    ("M5-matchprop-text-to-mtext", m5),
    ("EL1-ellipse-axis-distance", el1),
    ("EL2-ellipse-center", el2),
    ("EL3-ellipse-rotation", el3),
    ("EL4-ellipse-arc", el4),
    ("EL5-trim-ellipse-whole", el5),
    ("EL6-trim-ellipse-arc-middle", el6),
    ("EL7-extend-ellipse-arc", el7),
    ("EL8-break-ellipse-whole", el8),
    ("EL9-extend-line-to-ellipse", el9),
    ("EL10-stretch-ellipse-whole", el10),
    ("EL11-stretch-ellipse-arc-both-ends", el11),
    ("EL12-stretch-ellipse-arc-one-end", el12),
    ("EL13-join-elliptical-arcs-gap", el13),
    ("EL14-join-elliptical-arcs-whole", el14),
    ("EL15-join-elliptical-arc-close", el15),
    ("SP1-spline-fit", spline(CadSpline.through(SPLINE_POINTS))),
    (
        "SP2-spline-fit-tangents",
        spline(CadSpline.through(SPLINE_POINTS, start_tangent=(0, 5), end_tangent=(3, 0))),
    ),
    (
        "SP2b-spline-fit-tangents-after",
        spline(CadSpline.through(SPLINE_POINTS, start_tangent=(0, 5), end_tangent=(3, 0))),
    ),
    (
        "SP3-spline-fit-closed",
        spline(CadSpline.through([(0, 0), (6, 1), (8, 5), (3, 7)], closed=True)),
    ),
    ("SP4-spline-fit-square-root", spline(CadSpline.through(SPLINE_POINTS, "square_root"))),
    ("SP5-spline-cv", spline(CadSpline.by_vertices(SPLINE_POINTS))),
    (
        "SP6-spline-cv-closed-degree-2",
        spline(CadSpline.by_vertices(SPLINE_POINTS, 2, closed=True)),
    ),
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
