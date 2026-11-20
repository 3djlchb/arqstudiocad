"""Path-array binding, independent copies and synthetic persistence."""

from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication, CadBounds2, CadColor, CadError, CadLine


def test_path_array_binding_blocks_bulges_styles_index_undo_and_roundtrip(tmp_path: Path) -> None:
    app = CadApplication()
    doc = app.create_document("path")
    guide, _ = doc.add_line((10, 10), (30, 10))
    line, _ = doc.add_line((0, 0), (2, 0))
    block, _ = doc.create_block("cell", (0, 0), [line])
    poly, _ = doc.add_polyline([((0, 0), 1), ((2, 0), 0), ((2, 3), 0)], False)
    doc.set_entity_color([poly], CadColor.from_index(3))
    copies, changes = doc.path_array([block, poly, block], guide, (0, 0), 3)
    assert len(copies) == 6 and list(changes.created) == copies
    assert not changes.modified and not changes.deleted
    assert len(doc.get_entities()) == 9
    assert doc.get_entity(copies[0]).kind == "INSERT"
    copied = doc.get_entity(copies[1])
    assert copied.color == CadColor.from_index(3)
    assert copied.bounds is not None
    assert copied.bounds.min == pytest.approx((10, 9))
    assert copied.bounds.max == pytest.approx((12, 13))
    assert copies[4] in doc.query_entities(CadBounds2((29, 9), (33, 14)))
    assert doc.undo_history[-1] == "ARRAYPATH"
    doc.undo()
    assert len(doc.get_entities()) == 3
    doc.redo()
    doc.move_entities([copies[0]], (100, 0))
    bounds = doc.get_entity(block).bounds
    assert bounds is not None and bounds.min == (0, 0)
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"path-array.{suffix}"
        app.save_document(doc.id, path)
        reopened = app.open_document(path)
        assert len(reopened.get_entities()) == 9
        copied = reopened.get_entity(copies[1])
        assert copied.color == CadColor.from_index(3)
        assert copied.bounds is not None
        assert copied.bounds.min == pytest.approx((10, 9))
        assert copied.bounds.max == pytest.approx((12, 13))
        assert reopened.get_entity(copies[0]).kind == "INSERT"


@pytest.mark.parametrize("align,end", [(True, (5, 3)), (False, (7, 5))])
def test_path_array_binding_signed_bulge_and_alignment(align: bool, end: tuple[int, int]) -> None:
    doc = CadApplication().create_document("path")
    source, _ = doc.add_line((20, 20), (22, 20))
    guide, _ = doc.add_polyline([((0, 0), -1), ((10, 0), 0)], False)
    copies, _ = doc.path_array([source], guide, (20, 20), 3, align)
    midpoint = doc.get_entity(copies[1]).geometry
    assert isinstance(midpoint, CadLine)
    assert midpoint.start == pytest.approx((5, 5))
    assert midpoint.end == pytest.approx(end)
    original = doc.get_entity(source).geometry
    assert isinstance(original, CadLine) and original.start == (20, 20)


def test_path_array_binding_invalid_types_closed_guides_and_errors_preserve_state() -> None:
    doc = CadApplication().create_document("path")
    source, _ = doc.add_line((0, 0), (2, 0))
    guide, _ = doc.add_line((10, 10), (30, 10))
    closed, _ = doc.add_polyline([((0, 0), 0), ((5, 0), 0), ((5, 5), 0)], True)
    before = doc.undo_history
    for count, error in [
        (1.5, TypeError),
        (-1, OverflowError),
        (0, CadError),
        (1, CadError),
        (100001, CadError),
    ]:
        with pytest.raises(error):
            doc.path_array([source], guide, (0, 0), count)  # type: ignore[arg-type]
    for path in (closed, 999999):
        with pytest.raises(CadError):
            doc.path_array([source], path, (0, 0), 3)
    with pytest.raises(CadError):
        doc.path_array([source, 999999], guide, (0, 0), 3)
    with pytest.raises(CadError):
        doc.path_array([source], guide, (float("nan"), 0), 3)
    assert doc.undo_history == before
    assert len(doc.get_entities()) == 3


def test_path_array_binding_standalone_arc_and_reversed_line_follow_stored_direction() -> None:
    from math import pi

    doc = CadApplication().create_document("path")
    source, _ = doc.add_line((20, 20), (22, 20))
    guide, _ = doc.add_arc((0, 0), 10, 0, pi / 2)
    copies, _ = doc.path_array([source], guide, (20, 20), 3, True)
    for entity_id, start, end in [
        (copies[0], (10, 0), (12, 0)),
        (copies[1], (50**0.5, 50**0.5), (72**0.5, 72**0.5)),
        (copies[2], (0, 10), (0, 12)),
    ]:
        line = doc.get_entity(entity_id).geometry
        assert isinstance(line, CadLine)
        assert line.start == pytest.approx(start)
        assert line.end == pytest.approx(end)
    reverse, _ = doc.add_line((30, 0), (10, 0))
    copies, _ = doc.path_array([source], reverse, (20, 20), 3, True)
    for entity_id, x in zip(copies, (30, 20, 10), strict=True):
        line = doc.get_entity(entity_id).geometry
        assert isinstance(line, CadLine)
        assert line.start == pytest.approx((x, 0))
        assert line.end == pytest.approx((x + 2, 0))
