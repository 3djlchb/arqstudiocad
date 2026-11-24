"""SPLINE read from a drawing (ADR 0042): an exact shape, snapped to its
ends and nearest point, moved and mirrored whole, windowed by its curve."""

import itertools
import math
from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication, CadBounds2, CadSpline


def spline_dxf(path: Path) -> Path:
    """A cubic Bézier spline from (0,0) by (2,3) and (4,3) to (6,0)."""
    codes = ["0", "SECTION", "2", "ENTITIES", "0", "SPLINE", "8", "0"]
    codes += ["210", "0", "220", "0", "230", "1", "70", "8", "71", "3"]
    codes += ["72", "8", "73", "4", "74", "0"]
    for k in (0, 0, 0, 0, 1, 1, 1, 1):
        codes += ["40", str(float(k))]
    for x, y in ((0, 0), (2, 3), (4, 3), (6, 0)):
        codes += ["10", str(float(x)), "20", str(float(y)), "30", "0.0"]
    codes += ["0", "ENDSEC", "0", "EOF"]
    path.write_text("\n".join(codes) + "\n")
    return path


def test_a_read_spline_snaps_moves_and_has_its_exact_box(tmp_path: Path) -> None:
    doc = CadApplication().open_document(spline_dxf(tmp_path / "s.dxf"))
    (spline,) = doc.get_entities()
    assert spline.kind == "SPLINE" and spline.is_transformable
    bounds = spline.bounds
    assert bounds is not None
    assert bounds.max == (6.0, 3.0), "its control points' box (ADR 0042 D4)"
    # A window round the curve, under its middle control points, holds it.
    assert doc.select_window(CadBounds2((-0.1, -0.1), (6.1, 2.3)), False) == [spline.id]
    end = doc.snap((6.1, 0.1), 0.3, ["END"])
    assert end is not None and end.point == pytest.approx((6.0, 0.0))
    assert doc.snap((3.0, 2.25), 0.3, ["END", "MID"]) is None, "no end between chords"
    doc.move_entities([spline.id], (10.0, 1.0))
    moved = doc.get_entity(spline.id).bounds
    assert moved is not None and moved.min == pytest.approx((10.0, 1.0))
    doc.mirror_entities([spline.id], (0.0, 0.0), (0.0, 1.0))
    mirrored = doc.get_entity(spline.id).bounds
    assert mirrored is not None and mirrored.max[0] == pytest.approx(-10.0)
    assert doc.select_window(CadBounds2((-16.1, 0.9), (-9.9, 3.3)), False) == [spline.id]


def test_a_spline_through_fit_points_passes_through_them_and_keeps_them(tmp_path: Path) -> None:
    from arqstudiocad.core import CadSpline

    points = [(0.0, 0.0), (3.0, 4.0), (7.0, 1.0), (9.0, 6.0)]
    fit = CadSpline.through(points, "square_root", start_tangent=(0.0, 1.0))
    assert fit.degree == 3 and not fit.is_closed
    assert fit.fit_points == points
    assert len(fit.control_points) == len(points) + 2
    drawn = fit.points()
    assert drawn[0] == points[0] and drawn[-1] == pytest.approx(points[-1])
    # It leaves along the tangent: its first two control points upward.
    (x0, y0), (x1, y1) = fit.control_points[:2]
    assert x1 == pytest.approx(x0) and y1 > y0
    # Its knots the running chord lengths, their square roots, or counts.
    steps = [math.dist(a, b) for a, b in itertools.pairwise(points)]
    chord = list(itertools.accumulate(steps))[:-1]
    root = list(itertools.accumulate(math.sqrt(d) for d in steps))[:-1]
    assert CadSpline.through(points, "chord").knots[4:-4] == pytest.approx(chord)
    assert CadSpline.through(points, "square_root").knots[4:-4] == pytest.approx(root)
    assert CadSpline.through(points, "uniform").knots[4:-4] == [1.0, 2.0]
    closed = CadSpline.through(points, "uniform", closed=True)
    assert closed.is_closed and closed.points()[-1] == closed.points()[0]
    cv = CadSpline.by_vertices(points, degree=2)
    assert cv.degree == 2 and cv.fit_points == [] and cv.control_points == points

    doc = CadApplication().create_document("splines")
    ids = [doc.add_spline(s)[0] for s in (fit, closed, cv)]
    assert [doc.get_entity(i).kind for i in ids] == ["SPLINE"] * 3
    path = tmp_path / "splines.dwg"
    doc.save(path)
    back = CadApplication().open_document(path)
    assert [back.get_entity(i).bounds for i in ids] == [doc.get_entity(i).bounds for i in ids]
    # The fitted ones keep their fit points (DXF group 74, their count), a
    # closed one its first again at its end, as ProgeCAD writes it (SP3).
    text = tmp_path / "splines.dxf"
    doc.save(text)
    lines = [line.strip() for line in text.read_text().splitlines()]
    fits, kind = [], ""
    for code, value in zip(lines[::2], lines[1::2], strict=False):
        kind = value if code == "0" else kind
        if kind == "SPLINE" and code == "74":
            fits.append(int(value))
    assert fits == [4, 5, 0]
    doc.undo()
    assert len(doc.get_entities()) == 2


def test_a_spline_it_cannot_make_is_refused() -> None:
    from arqstudiocad.core import CadSpline, InvalidGeometryError

    with pytest.raises(ValueError, match="knots"):
        CadSpline.through([(0, 0), (1, 1)], "curly")  # type: ignore[arg-type]
    with pytest.raises(InvalidGeometryError, match="fit points"):
        CadSpline.through([(0, 0), (0, 0), (1, 1)])
    with pytest.raises(InvalidGeometryError, match="tangent"):
        CadSpline.through([(0, 0), (1, 1), (2, 0)], start_tangent=(1, 0), closed=True)
    with pytest.raises(InvalidGeometryError, match="degree"):
        CadSpline.by_vertices([(0, 0), (1, 1)], closed=True)


def test_a_read_spline_is_a_curve_to_snap_to_and_trim_at(tmp_path: Path) -> None:
    # The Bezier: x = 6u, y = 9u(1 - u); y = 1 where u(1 - u) = 1/9.
    doc = CadApplication().open_document(spline_dxf(tmp_path / "s.dxf"))
    (spline,) = doc.get_entities()
    root = math.sqrt(5.0) / 3.0
    x1, x2 = 3.0 * (1.0 - root), 3.0 * (1.0 + root)
    line, _ = doc.add_line((-1.0, 1.0), (7.0, 1.0))
    found = doc.snap((x1 + 0.05, 1.05), 0.3, ["INT"])
    assert found is not None and found.kind == "INT"
    assert found.point == pytest.approx((x1, 1.0))
    touch = doc.snap((2.25, 2.15), 0.3, ["TAN"], (-3.0, 0.0))
    u = (math.sqrt(3.0) - 1.0) / 2.0
    assert touch is not None and touch.point == pytest.approx((6.0 * u, 9.0 * u * (1.0 - u)))
    left, _ = doc.trim_entity(line, (3.0, 1.0), None, True)
    boxes = [doc.get_entity(i).bounds for i in left]
    ends = sorted(x for b in boxes if b is not None for x in (b.min[0], b.max[0]))
    assert ends == pytest.approx([-1.0, x1, x2, 7.0])
    geometry = doc.get_entity(spline.id).geometry
    assert isinstance(geometry, CadSpline) and geometry.degree == 3, "plan geometry (ADR 0043)"
