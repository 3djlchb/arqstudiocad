"""Public ARRAY binding, ownership of copies, spatial index and persistence."""

from math import pi
from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication, CadBounds2, CadColor, CadError, CadLine


def test_array_binding_copies_blocks_and_bulges_keeps_style_and_round_trips(tmp_path: Path) -> None:
    app = CadApplication()
    doc = app.create_document("array")
    line, _ = doc.add_line((0, 0), (2, 0))
    block, _ = doc.create_block("cell", (0, 0), [line])
    poly, _ = doc.add_polyline([((0, 0), 1), ((2, 0), 0), ((2, 3), 0)], False)
    layer, _ = doc.create_layer("array-layer")
    doc.set_entity_layer([poly], layer)
    doc.set_entity_color([poly], CadColor.from_index(3))
    copies, changes = doc.rectangular_array([block, poly, block], 1, 2, 0, 10, pi / 2)
    assert len(copies) == 2 and list(changes.created) == copies
    assert doc.get_entity(copies[0]).kind == "INSERT"
    copy = doc.get_entity(copies[1])
    assert (copy.layer, copy.color) == ("array-layer", CadColor.from_index(3))
    assert copy.bounds is not None
    assert copy.bounds.min == pytest.approx((0, 9))
    assert copy.bounds.max == pytest.approx((2, 13))
    assert set(doc.query_entities(CadBounds2((-1, 9), (3, 14)))) == set(copies)
    # Editing one independent copy does not affect another or its source.
    doc.move_entities([copies[0]], (20, 0))
    bounds = doc.get_entity(block).bounds
    assert bounds is not None and bounds.min == (0, 0)
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"array.{suffix}"
        app.save_document(doc.id, path)
        reopened = app.open_document(path)
        assert len(reopened.get_entities()) == 4
        assert reopened.get_entity(copies[0]).kind == "INSERT"
        bounds = reopened.get_entity(copies[1]).bounds
        assert bounds is not None and bounds.min == pytest.approx((0, 9))
        assert reopened.get_entity(copies[1]).color == CadColor.from_index(3)


def test_array_binding_errors_preserve_document_and_python_count_types() -> None:
    app = CadApplication()
    doc = app.create_document("array")
    source, _ = doc.add_line((0, 0), (1, 0))
    before = doc.undo_history
    for counts, error in [((1.5, 2), TypeError), ((-1, 2), OverflowError), ((1, 1), CadError)]:
        with pytest.raises(error):
            doc.rectangular_array([source], *counts, 1, 1, 0)  # type: ignore[arg-type]
    with pytest.raises(CadError):
        doc.rectangular_array([source, 999999], 2, 2, 1, 1, 0)
    assert doc.undo_history == before
    shape = doc.get_entity(source).geometry
    assert isinstance(shape, CadLine) and shape.end == (1, 0)
    assert len(doc.get_entities()) == 1
