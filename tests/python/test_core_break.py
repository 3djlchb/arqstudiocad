"""BREAK's public binding: validation, spatial changes and saved geometry."""

from pathlib import Path

import pytest

from arqstudiocad.core import (
    CadApplication,
    CadArc,
    CadBounds2,
    CadColor,
    CadLine,
    InvalidGeometryError,
)


def test_break_binding_validates_points_preserves_style_and_round_trips(tmp_path: Path) -> None:
    app = CadApplication()
    document = app.create_document("break")
    line, _ = document.add_line((0, 0), (10, 0))
    layer, _ = document.create_layer("break-test")
    document.set_entity_layer([line], layer)
    document.set_entity_color([line], CadColor.from_index(3))
    history = document.undo_history
    for bad in (float("nan"), float("inf"), -float("inf")):
        with pytest.raises(InvalidGeometryError):
            document.break_entity(line, (bad, 0), (7, 0))
        with pytest.raises(InvalidGeometryError):
            document.break_entity(line, (3, 0), (0, bad))
        assert document.undo_history == history
    pieces, changes = document.break_entity(line, (3, 2), (7, -2))
    assert len(pieces) == 2 and pieces[0] == line
    assert list(changes.created) == [pieces[1]]
    assert document.query_entities(CadBounds2((4, -1), (6, 1))) == []
    for entity_id in pieces:
        entity = document.get_entity(entity_id)
        assert entity.layer == "break-test"
        assert entity.color == CadColor.from_index(3)
    circle, _ = document.add_circle((20, 0), 5)
    arc_ids, _ = document.break_entity(circle, (25, 0), (20, 5))
    assert isinstance(document.get_entity(arc_ids[0]).geometry, CadArc)
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"break.{suffix}"
        app.save_document(document.id, path)
        reopened = app.open_document(path)
        lines = [e.geometry for e in reopened.get_entities() if isinstance(e.geometry, CadLine)]
        assert sorted((g.start, g.end) for g in lines) == [((0, 0), (3, 0)), ((7, 0), (10, 0))]
        assert sum(e.kind == "ARC" for e in reopened.get_entities()) == 1
