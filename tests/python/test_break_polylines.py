"""BREAK open polylines: command flow, unchanged cancellation and file round trips."""

from pathlib import Path

import pytest
from test_trim import Harness

from arqstudiocad.core import CadApplication, CadColor


def test_break_polyline_binding_preserves_bulges_properties_and_round_trips(tmp_path: Path) -> None:
    app = CadApplication()
    doc = app.create_document("break-polyline")
    source, _ = doc.add_polyline([((0, 0), 1), ((2, 0), 0), ((2, 3), 0)], False, 7, 3, 0.5)
    doc.set_entity_color([source], CadColor.from_index(3))
    ids, changes = doc.break_entity(source, (1, -2), (1, -2))
    assert len(ids) == 2 and ids[0] == source and list(changes.created) == [ids[1]]
    for entity_id in ids:
        entity = doc.get_entity(entity_id)
        assert entity.kind == "LWPOLYLINE" and entity.color == CadColor.from_index(3)
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"break-polyline.{suffix}"
        app.save_document(doc.id, path)
        reopened = app.open_document(path)
        assert len(reopened.get_entities()) == 2
        for entity_id in ids:
            entity = reopened.get_entity(entity_id)
            assert entity.kind == "LWPOLYLINE" and entity.color == CadColor.from_index(3)
            assert entity.bounds is not None and entity.bounds.min[1] <= -1
    doc.undo()
    assert len(doc.get_entities()) == 1
    doc.redo()
    assert len(doc.get_entities()) == 2


@pytest.mark.parametrize("second", ["7,0", "@"])
def test_break_polyline_command_and_cancel(second: str) -> None:
    h = Harness()
    source, _ = h.document.add_polyline([((0, 0), 0), ((10, 0), 0)], False)
    before = h.document.undo_history
    h.run("BREAK")
    h.runner.answer_entity(source, (3, 0))
    h.runner.cancel()
    assert h.document.undo_history == before and len(h.document.get_entities()) == 1
    h.run("BR")
    h.runner.answer_entity(source, (1, 0))
    h.run("F", "3,0", second)
    assert h.runner.active is None and len(h.document.get_entities()) == 2
    assert h.document.undo_history[-1] == "BREAK"
    bounds = [e.bounds for e in h.document.get_entities()]
    assert all(b is not None for b in bounds)
    assert sorted(b.max[0] for b in bounds if b is not None) == [3, 10]
