"""A complete drawing through the Python API (contract §12, integration).

Create → draw → save as DWG and DXF → open → compare → edit → save → open →
check → undo back to the opened state. Every check goes through the public
module, as commands and scripts will.
"""

import math
from pathlib import Path

import pytest

import arqstudiocad._core as native
from arqstudiocad import core
from arqstudiocad.core import (
    CadApplication,
    CadArc,
    CadCircle,
    CadColor,
    CadDocument,
    CadEntityInfo,
    CadLayerInfo,
    CadLine,
)

type Shape = CadLine | CadCircle | CadArc | None


def same_shape(a: Shape, b: Shape) -> bool:
    """Equal curves; DXF stores angles in degrees, so arcs come back within
    an ulp or two."""
    if isinstance(a, CadArc) and isinstance(b, CadArc):
        return (
            a.center == b.center
            and a.radius == b.radius
            and math.isclose(a.start_angle, b.start_angle, abs_tol=1e-12)
            and math.isclose(a.end_angle, b.end_angle, abs_tol=1e-12)
        )
    return a == b


def same_entities(a: list[CadEntityInfo], b: list[CadEntityInfo]) -> bool:
    return len(a) == len(b) and all(
        (x.id, x.kind, x.layer, x.color, x.elevation)
        == (y.id, y.kind, y.layer, y.color, y.elevation)
        and same_shape(x.geometry, y.geometry)
        for x, y in zip(a, b, strict=True)
    )


def layer_state(layers: list[CadLayerInfo]) -> list[tuple[object, ...]]:
    return [
        (y.id, y.name, y.color, y.is_visible, y.is_frozen, y.is_locked, y.is_plottable)
        for y in layers
    ]


def draw_house(doc: CadDocument) -> dict[str, int]:
    """A small plan: four walls, a door, two columns, a note layer."""
    walls, _ = doc.create_layer("A-MUROS")
    doors, _ = doc.create_layer("A-PUERTAS")
    columns, _ = doc.create_layer("E-PILARES")
    doc.set_layer_color(walls, CadColor.from_rgb(200, 120, 40))
    doc.set_layer_color(doors, CadColor.from_index(5))
    doc.set_layer_plottable(columns, False)

    doc.set_current_layer(walls)
    with doc.group("WALLS") as group:
        corners = [(0.0, 0.0), (8.0, 0.0), (8.0, 6.0), (0.0, 6.0)]
        wall_ids = [
            doc.add_line(a, b)[0] for a, b in zip(corners, corners[1:] + corners[:1], strict=True)
        ]
    assert group.changes is not None and group.changes.created == tuple(wall_ids)

    doc.set_current_layer(doors)
    door, _ = doc.add_arc((3.0, 0.0), 0.9, 0.0, math.pi / 2)
    doc.set_current_layer(columns)
    column_a, _ = doc.add_circle((0.5, 0.5), 0.2)
    column_b, _ = doc.add_circle((7.5, 5.5), 0.2)
    doc.set_entity_color([column_b], CadColor.from_index(1))
    return {
        "walls": walls,
        "doors": doors,
        "columns": columns,
        "door": door,
        "column_a": column_a,
        "column_b": column_b,
        **{f"wall{i}": w for i, w in enumerate(wall_ids)},
    }


@pytest.mark.parametrize("ext", ["dwg", "dxf"])
def test_a_complete_drawing(tmp_path: Path, ext: str) -> None:
    app = CadApplication()
    doc = app.create_document("Vivienda")
    ids = draw_house(doc)
    drawn = doc.get_entities()
    drawn_layers = doc.get_layers()
    assert [e.kind for e in drawn] == ["LINE"] * 4 + ["ARC", "CIRCLE", "CIRCLE"]

    # Save, open: everything the API shows comes back, with the same ids.
    path = tmp_path / f"vivienda.{ext}"
    doc.save(path)
    assert not doc.is_modified
    opened = app.open_document(path)
    assert same_entities(opened.get_entities(), drawn)
    assert layer_state(opened.get_layers()) == layer_state(drawn_layers)
    assert opened.current_layer.name == "E-PILARES"
    assert not opened.can_undo, "a file opens with an empty history"

    # Edit the opened drawing.
    with opened.group("REFORMA"):
        opened.move_entities([ids["column_a"]], (1.0, 1.0))
        opened.rotate_entities([ids["door"]], (3.0, 0.0), math.pi / 2)
        opened.delete_entities([ids["wall3"]])
    opened.set_entity_layer([ids["column_b"]], ids["walls"])
    opened.set_entity_color([ids["wall0"]], CadColor.from_index(3))
    opened.rename_layer(ids["doors"], "A-CARPINTERIA")
    assert opened.is_modified
    edited = opened.get_entities()

    # Save the edits, open again: they are there, the rest is untouched.
    edited_path = tmp_path / f"reforma.{ext}"
    opened.save(edited_path)
    again = app.open_document(edited_path)
    assert same_entities(again.get_entities(), edited)
    by_id = {e.id: e for e in again.get_entities()}
    assert ids["wall3"] not in by_id
    assert by_id[ids["column_a"]].geometry == CadCircle((1.5, 1.5), 0.2)
    door = by_id[ids["door"]].geometry
    assert isinstance(door, CadArc)
    assert door.start_angle == pytest.approx(math.pi / 2)
    assert door.end_angle == pytest.approx(math.pi)
    assert by_id[ids["column_b"]].layer == "A-MUROS"
    assert by_id[ids["wall0"]].color == CadColor.from_index(3)
    assert by_id[ids["door"]].layer == "A-CARPINTERIA", "renaming moved its entities"
    for key in ("wall1", "wall2"):
        original = next(e for e in drawn if e.id == ids[key])
        assert by_id[ids[key]].geometry == original.geometry, key

    # Undo every edit in the opened drawing: back to the opened state.
    while opened.can_undo:
        opened.undo()
    assert same_entities(opened.get_entities(), drawn)
    assert layer_state(opened.get_layers()) == layer_state(drawn_layers)
    # It was last saved as the edited file, so it now differs from that.
    assert opened.name == f"reforma.{ext}" and opened.is_modified
    opened.redo()
    assert opened.is_modified, "only part of the edits are redone"
    while opened.can_redo:
        opened.redo()
    assert not opened.is_modified, "redone back to the state it was saved in"

    assert app.document_ids() == [doc.id, opened.id, again.id]
    for d in app.documents():
        d.close()
    assert app.document_ids() == []


def test_the_public_module_exports_every_native_class() -> None:
    native_names = {
        name
        for name in dir(native)
        if not name.startswith("_") and isinstance(getattr(native, name), type)
    }
    assert native_names, "the native module defines classes"
    missing = native_names - set(core.__all__)
    assert not missing, f"add to arqstudiocad.core: {sorted(missing)}"
    assert set(native.__all__) - {"__version__"} == native_names, "native __all__ lists every class"
    for name in core.__all__:
        assert hasattr(core, name), name
