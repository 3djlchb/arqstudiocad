"""Polygon STRETCH binding validation and metadata-preserving atomic edits."""

from pathlib import Path

import pytest

from arqstudiocad.core import CadApplication, CadBounds2, CadColor, CadLine, InvalidGeometryError

TRIANGLE = [(9.0, -1.0), (12.0, -1.0), (9.0, 2.0)]


def test_polygon_stretch_binding_validates_before_edit_and_preserves_polyline_style(
    tmp_path: Path,
) -> None:
    app = CadApplication()
    document = app.create_document("polygon stretch")
    line, _ = document.add_line((0, 0), (10, 0))
    polyline, _ = document.add_polyline([((0, 5), 0.5), ((10, 0), 0), ((15, 5), 0)])
    document.set_entity_color([polyline], CadColor.from_index(3))
    selected = document.select_crossing_polygon(TRIANGLE)
    assert line in selected and polyline in selected
    history = document.undo_history
    for invalid in (
        [(0, 0), (1, 1)],
        [(0, 0), (2, 2), (0, 2), (2, 0)],
        [(0, 0), (float("nan"), 1), (1, 0)],
    ):
        with pytest.raises(InvalidGeometryError):
            document.select_crossing_polygon(invalid)
        with pytest.raises(InvalidGeometryError):
            document.stretch_entities(
                [line, polyline], (2, 3), [], [(line, TRIANGLE), (polyline, invalid)]
            )
        assert document.undo_history == history
        assert document.get_entity(line).geometry == CadLine((0, 0), (10, 0))
    document.stretch_entities(
        [line, polyline], (2, 3), [], [(line, TRIANGLE), (polyline, TRIANGLE)]
    )
    assert document.get_entity(line).geometry == CadLine((0, 0), (12, 3))
    assert document.get_entity(polyline).color == CadColor.from_index(3)
    grips = [grip.point for grip in document.grips([polyline])]
    assert grips == [(0, 5), (12, 3), (15, 5)]
    assert document.query_entities(CadBounds2((11, 2), (13, 4)))
    document.undo()
    assert document.get_entity(line).geometry == CadLine((0, 0), (10, 0))
    document.redo()
    for suffix in ("dwg", "dxf"):
        path = tmp_path / f"polygon-stretch.{suffix}"
        app.save_document(document.id, path)
        reopened = app.open_document(path)
        assert reopened.get_entity(line).geometry == CadLine((0, 0), (12, 3))
        assert reopened.get_entity(polyline).color == CadColor.from_index(3)
        assert [grip.point for grip in reopened.grips([polyline])] == grips
