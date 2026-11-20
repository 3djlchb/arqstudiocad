"""Polar ARRAY public binding, independent copies and codec persistence."""

from math import pi, tau
from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication, CadBounds2, CadColor, CadError, CadLine


def test_polar_binding_properties_blocks_bulges_index_history_and_roundtrip(tmp_path: Path) -> None:
    app = CadApplication()
    doc = app.create_document("polar")
    line, _ = doc.add_line((10, 0), (12, 0))
    block, _ = doc.create_block("cell", (0, 0), [line])
    poly, _ = doc.add_polyline([((10, 0), 1), ((12, 0), 0), ((12, 3), 0)], False)
    doc.set_entity_color([poly], CadColor.from_index(3))
    copies, changes = doc.polar_array([block, poly, block], (0, 0), 4, tau)
    assert len(copies) == 6 and list(changes.created) == copies
    assert len(doc.get_entities()) == 8
    assert doc.get_entity(copies[0]).kind == "INSERT"
    shape = doc.get_entity(copies[1])
    assert shape.color == CadColor.from_index(3)
    assert shape.bounds is not None
    assert shape.bounds.min == pytest.approx((-3, 10))
    assert shape.bounds.max == pytest.approx((1, 12))
    assert set(doc.query_entities(CadBounds2((-4, 9), (2, 13)))) == set(copies[:2])
    assert doc.undo_history[-1] == "ARRAYPOLAR"
    doc.undo()
    assert len(doc.get_entities()) == 2
    assert doc.query_entities(CadBounds2((-4, 9), (2, 13))) == []
    doc.redo()
    doc.move_entities([copies[0]], (20, 0))
    original = doc.get_entity(block).bounds
    assert original is not None and original.min == (10, 0)
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"polar.{suffix}"
        app.save_document(doc.id, path)
        reopened = app.open_document(path)
        assert len(reopened.get_entities()) == 8
        assert reopened.get_entity(copies[0]).kind == "INSERT"
        copy = reopened.get_entity(copies[1])
        assert copy.color == CadColor.from_index(3)
        assert copy.bounds is not None
        assert copy.bounds.min == pytest.approx((-3, 10))
        assert copy.bounds.max == pytest.approx((1, 12))


@pytest.mark.parametrize("fill,expected", [(pi, (-10, 0)), (-pi, (-10, 0))])
def test_polar_binding_partial_fill_without_rotation_uses_explicit_group_base(
    fill: float, expected: tuple[float, float]
) -> None:
    doc = CadApplication().create_document("polar")
    a, _ = doc.add_line((10, 0), (12, 0))
    b, _ = doc.add_line((10, 2), (12, 2))
    copies, _ = doc.polar_array([b, a], (0, 0), 3, fill, False, (10, 0))
    assert len(copies) == 4
    first = doc.get_entity(copies[1]).geometry
    last = doc.get_entity(copies[3]).geometry
    assert isinstance(first, CadLine) and isinstance(last, CadLine)
    assert first.start == pytest.approx((0, 10 if fill > 0 else -10))
    assert first.end == pytest.approx((2, 10 if fill > 0 else -10))
    assert last.start == pytest.approx(expected)
    assert last.end == pytest.approx((-8, 0))
    partner = doc.get_entity(copies[0]).geometry
    assert isinstance(partner, CadLine)
    assert partner.start == pytest.approx((first.start[0], first.start[1] + 2))


def test_polar_binding_rejects_invalid_inputs_without_editing() -> None:
    doc = CadApplication().create_document("polar")
    source, _ = doc.add_line((10, 0), (12, 0))
    before = doc.undo_history
    for count, error in [
        (1.5, TypeError),
        (-1, OverflowError),
        (0, CadError),
        (1, CadError),
        (100001, CadError),
    ]:
        with pytest.raises(error):
            doc.polar_array([source], (0, 0), count, tau)  # type: ignore[arg-type]
    for fill in (0, tau + 1, float("nan"), float("inf")):
        with pytest.raises(CadError):
            doc.polar_array([source], (0, 0), 4, fill)
    with pytest.raises(CadError):
        doc.polar_array([source], (0, 0), 4, tau, False)
    with pytest.raises(CadError):
        doc.polar_array([source, 999999], (0, 0), 4, tau)
    with pytest.raises(CadError):
        doc.polar_array([source], (float("inf"), 0), 4, tau)
    assert doc.undo_history == before
    assert len(doc.get_entities()) == 1
