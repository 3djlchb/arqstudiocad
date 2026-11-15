"""JOIN's public binding: what it returns, kept style, the index, unknown
ids, and the joined geometry through DWG and DXF."""

from math import pi
from pathlib import Path

import pytest

from arqstudiocad.core import (
    CadApplication,
    CadBounds2,
    CadCircle,
    CadColor,
    CadError,
    CadLine,
)


def test_join_binding_keeps_style_updates_the_index_and_round_trips(tmp_path: Path) -> None:
    app = CadApplication()
    document = app.create_document("join")
    source, _ = document.add_line((0, 0), (2, 0))
    ahead, _ = document.add_line((5, 0), (8, 0))
    layer, _ = document.create_layer("join-test")
    document.set_entity_layer([source], layer)
    document.set_entity_color([source], CadColor.from_index(3))
    arc, _ = document.add_arc((20, 0), 5, 0, pi)
    rest, _ = document.add_arc((20, 0), 5, pi, 0)
    document.set_entity_color([arc], CadColor.from_index(4))

    results, joined, discarded, changes = document.join_entities(source, [ahead])
    assert (results, joined, discarded) == ([source], 1, 0)
    assert list(changes.deleted) == [ahead]
    assert document.query_entities(CadBounds2((3, -0.1), (4, 0.1))) == [source]
    entity = document.get_entity(source)
    assert (entity.layer, entity.color) == ("join-test", CadColor.from_index(3))

    (circle,), joined, _, _ = document.join_entities(None, [arc, rest])
    assert joined == 1
    assert document.get_entity(circle).color == CadColor.from_index(4)

    history = document.undo_history
    with pytest.raises(CadError):
        document.join_entities(999_999, [source])
    assert document.join_entities(None, [999_999, source])[1:3] == (0, 2)
    assert document.undo_history == history

    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"join.{suffix}"
        app.save_document(document.id, path)
        reopened = app.open_document(path)
        shapes = [e.geometry for e in reopened.get_entities()]
        lines = [(g.start, g.end) for g in shapes if isinstance(g, CadLine)]
        circles = [(g.center, g.radius) for g in shapes if isinstance(g, CadCircle)]
        assert lines == [((0, 0), (8, 0))]
        assert circles == [((20, 0), 5)]
