"""SPLINE read from a drawing (ADR 0042): an exact shape, snapped to its
ends and nearest point, moved and mirrored whole, windowed by its curve."""

from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication, CadBounds2


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
