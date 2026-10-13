"""Operations, groups, undo/redo and change sets from Python (ADR 0012, step 4.4).

Expected positions are worked out by hand, not with the core's transforms.
"""

import math
from collections.abc import Callable
from pathlib import Path

import numpy as np
import pytest

from arqstudiocad.core import (
    CadApplication,
    CadArc,
    CadChangeSet,
    CadCircle,
    CadColor,
    CadDocument,
    CadLine,
    EntityNotFoundError,
    InvalidGeometryError,
    InvalidOperationError,
    LayerNotFoundError,
)


@pytest.fixture
def doc() -> CadDocument:
    return CadApplication().create_document("Plano1")


def line_of(doc: CadDocument, entity_id: int) -> CadLine:
    geometry = doc.get_entity(entity_id).geometry
    assert isinstance(geometry, CadLine)
    return geometry


# Drawing


def test_drawing_returns_the_id_and_what_changed(doc: CadDocument) -> None:
    line, made = doc.add_line((0, 0), (3, 4))
    assert isinstance(made, CadChangeSet)
    assert made.created == (line,)
    assert (made.modified, made.deleted, made.is_empty) == ((), (), False)
    assert repr(made) == f"CadChangeSet(created=[{line}])"

    circle, _ = doc.add_circle(np.array([5.0, 5.0]), 2.0)
    arc, _ = doc.add_arc((0, 0), 1.0, 0.0, math.pi / 2)
    copy, _ = doc.add_geometry(CadArc((0, 0), 1.0, 0.0, math.pi / 2))

    assert line_of(doc, line) == CadLine((0, 0), (3, 4))
    assert doc.get_entity(circle).geometry == CadCircle((5, 5), 2.0)
    assert doc.get_entity(arc).geometry == doc.get_entity(copy).geometry
    assert [e.layer for e in doc.get_entities()] == ["0"] * 4, "on the current layer"
    assert doc.undo_name == "ARC"


def test_invalid_input_changes_nothing(doc: CadDocument) -> None:
    with pytest.raises(InvalidGeometryError):
        doc.add_line((1, 1), (1, 1))
    with pytest.raises(InvalidGeometryError):
        doc.add_circle((0, 0), 0.0)
    with pytest.raises(InvalidGeometryError):
        doc.add_line((0, 0), (math.nan, 1))
    with pytest.raises(TypeError, match="CadLine, CadCircle or CadArc"):
        doc.add_geometry((0, 0))  # type: ignore[arg-type]
    assert doc.get_entities() == []
    assert not doc.can_undo and not doc.is_modified


# Editing


def test_move_rotate_scale_mirror(doc: CadDocument) -> None:
    line, _ = doc.add_line((1, 0), (2, 0))

    assert doc.move_entities([line], (1, 2)).modified == (line,)
    assert line_of(doc, line) == CadLine((2, 2), (3, 2))

    # 90° counterclockwise around (2, 2): (x, y) -> (2 - (y - 2), 2 + (x - 2)).
    doc.rotate_entities([line], (2, 2), math.pi / 2)
    rotated = line_of(doc, line)
    assert rotated.start == pytest.approx((2, 2)) and rotated.end == pytest.approx((2, 3))

    doc.scale_entities([line], (2, 0), 2.0)
    scaled = line_of(doc, line)
    assert scaled.start == pytest.approx((2, 4)) and scaled.end == pytest.approx((2, 6))

    # Across the X axis: y -> -y.
    doc.mirror_entities([line], (-1, 0), (1, 0))
    mirrored = line_of(doc, line)
    assert mirrored.start == pytest.approx((2, -4)) and mirrored.end == pytest.approx((2, -6))

    with pytest.raises(InvalidGeometryError):
        doc.mirror_entities([line], (1, 1), (1, 1))
    with pytest.raises(InvalidGeometryError):
        doc.scale_entities([line], (0, 0), -1.0)


def test_ids_are_any_sequence_repeats_count_once_empty_does_nothing(doc: CadDocument) -> None:
    a, _ = doc.add_line((0, 0), (1, 0))
    b, _ = doc.add_line((0, 1), (1, 1))
    moved = doc.move_entities(np.array([a, b, a]), (1, 0))
    assert moved.modified == (a, b)
    assert line_of(doc, a).start == (1.0, 0.0), "moved once"
    steps = doc.undo_name
    assert doc.move_entities((), (5, 5)).is_empty
    assert doc.undo_name == steps, "no step for nothing"


def test_a_failed_operation_changes_nothing(doc: CadDocument) -> None:
    a, _ = doc.add_line((0, 0), (1, 0))
    with pytest.raises(EntityNotFoundError) as caught:
        doc.move_entities([a, 999_999], (1, 1))
    assert caught.value.entity_id == 999_999
    assert line_of(doc, a) == CadLine((0, 0), (1, 0)), "a moved back"
    assert doc.undo_name == "LINE"


def test_read_only_entities_can_be_erased_not_edited(tmp_path: Path) -> None:
    codes = ["0", "SECTION", "2", "ENTITIES"]
    codes += [
        "0",
        "LINE",
        "8",
        "0",
        "10",
        "0",
        "20",
        "0",
        "30",
        "0",
        "11",
        "1",
        "21",
        "0",
        "31",
        "0",
    ]
    codes += ["0", "TEXT", "8", "0", "10", "5", "20", "5", "30", "0", "40", "1", "1", "NOTA"]
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "texto.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    doc = CadApplication().open_document(path)
    line, text = (e.id for e in doc.get_entities())

    with pytest.raises(InvalidOperationError, match="cannot be transformed"):
        doc.move_entities([line, text], (1, 0))
    assert line_of(doc, line) == CadLine((0, 0), (1, 0))
    assert doc.delete_entities([text]).deleted == (text,)
    assert [e.kind for e in doc.get_entities()] == ["LINE"]


def blocks_and_polylines(tmp_path: Path) -> CadDocument:
    """Block PUERTA, base (1,1), one LINE to (2,1); an INSERT of it at
    (10,5); a polyline (0,0)-(4,0)-(4,2)."""
    codes = ["0", "SECTION", "2", "BLOCKS"]
    codes += ["0", "BLOCK", "8", "0", "2", "PUERTA", "70", "0", "10", "1", "20", "1", "30", "0"]
    codes += ["3", "PUERTA", "0", "LINE", "8", "0", "10", "1", "20", "1", "30", "0"]
    codes += ["11", "2", "21", "1", "31", "0", "0", "ENDBLK", "8", "0", "0", "ENDSEC"]
    codes += ["0", "SECTION", "2", "ENTITIES"]
    codes += ["0", "INSERT", "8", "0", "2", "PUERTA", "10", "10", "20", "5", "30", "0"]
    codes += ["0", "LWPOLYLINE", "8", "0", "90", "3", "70", "0", "10", "0", "20", "0"]
    codes += ["10", "4", "20", "0", "10", "4", "20", "2", "0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "bloques.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    return CadApplication().open_document(path)


def test_blocks_and_polylines_move_and_copy_as_a_whole(tmp_path: Path) -> None:
    doc = blocks_and_polylines(tmp_path)
    door, wall = doc.get_entities()
    assert (door.kind, door.is_transformable, door.is_editable) == ("INSERT", True, False)
    assert (wall.kind, wall.is_transformable, wall.is_editable) == ("LWPOLYLINE", True, False)
    assert wall.geometry is None, "its vertices are not editable yet"

    moved = doc.move_entities([door.id, wall.id], (1, 2))
    assert moved.modified == (door.id, wall.id)
    bounds = doc.get_entity(wall.id).bounds
    assert bounds is not None and (bounds.min, bounds.max) == ((1.0, 2.0), (5.0, 4.0))

    copies, changes = doc.copy_entities([wall.id, wall.id], (0, -10))
    assert len(copies) == 1 and changes.created == tuple(copies)
    copied = doc.get_entity(copies[0]).bounds
    assert copied is not None and (copied.min, copied.max) == ((1.0, -8.0), (5.0, -6.0))
    assert doc.undo_name == "COPY"
    doc.undo()
    assert [e.id for e in doc.get_entities()] == [door.id, wall.id]


def test_copying_what_cannot_be_copied_changes_nothing(doc: CadDocument) -> None:
    line, _ = doc.add_line((0, 0), (1, 0))
    with pytest.raises(EntityNotFoundError):
        doc.copy_entities([line, 999_999], (1, 0))
    assert len(doc.get_entities()) == 1 and doc.undo_name == "LINE"
    copies, _ = doc.copy_entities(np.array([line]), (0, 1))
    assert line_of(doc, copies[0]) == CadLine((0, 1), (1, 1))


def test_locked_layers_protect_their_entities(doc: CadDocument) -> None:
    line, _ = doc.add_line((0, 0), (1, 0))
    zero = doc.current_layer.id
    doc.set_layer_locked(zero, True)
    edits: list[Callable[[], object]] = [
        lambda: doc.move_entities([line], (1, 0)),
        lambda: doc.delete_entities([line]),
        lambda: doc.set_entity_color([line], CadColor.from_index(1)),
    ]
    for edit in edits:
        with pytest.raises(InvalidOperationError, match="locked"):
            edit()
    doc.add_circle((0, 0), 1.0)  # drawing on a locked current layer is allowed
    doc.set_layer_locked(zero, False)
    doc.move_entities([line], (1, 0))


def test_entity_properties(doc: CadDocument) -> None:
    line, _ = doc.add_line((0, 0), (1, 0))
    walls, _ = doc.create_layer("A-MUROS")
    assert doc.set_entity_layer([line], walls).modified == (line,)
    doc.set_entity_color([line], CadColor.from_rgb(10, 20, 30))
    info = doc.get_entity(line)
    assert (info.layer, info.color) == ("A-MUROS", CadColor.from_rgb(10, 20, 30))
    assert doc.undo_name == "CHPROP"
    with pytest.raises(LayerNotFoundError) as caught:
        doc.set_entity_layer([line], 777_777)
    assert caught.value.layer_id == 777_777


# Layers


def test_layer_operations(doc: CadDocument) -> None:
    layer, made = doc.create_layer("A-PUERTAS")
    assert made.layers_created == (layer,)
    assert doc.rename_layer(layer, "A-PUERTA").layers_modified == (layer,)
    doc.set_layer_color(layer, CadColor.from_index(3))
    doc.set_layer_visible(layer, False)
    doc.set_layer_plottable(layer, False)
    info = doc.get_layer(layer)
    assert (info.name, info.color, info.is_visible, info.is_plottable) == (
        "A-PUERTA",
        CadColor.from_index(3),
        False,
        False,
    )
    with pytest.raises(InvalidOperationError):
        doc.set_layer_color(layer, CadColor.by_layer())

    doc.set_layer_frozen(layer, True)
    with pytest.raises(InvalidOperationError):
        doc.set_current_layer(layer)
    doc.set_layer_frozen(layer, False)
    current = doc.set_current_layer(layer)
    assert current.document_changed and repr(current) == "CadChangeSet(document_changed=True)"
    assert doc.current_layer.id == layer
    with pytest.raises(InvalidOperationError):
        doc.delete_layer(layer)  # it is current

    zero = doc.get_layer_by_name("0")
    assert zero is not None
    doc.set_current_layer(zero.id)
    assert doc.delete_layer(layer).layers_deleted == (layer,)
    with pytest.raises(LayerNotFoundError):
        doc.get_layer(layer)
    assert doc.undo_name == "LAYER"


# History


def test_undo_and_redo(doc: CadDocument) -> None:
    assert (doc.can_undo, doc.can_redo, doc.undo_name, doc.redo_name) == (False, False, None, None)
    with pytest.raises(InvalidOperationError, match="nothing to undo"):
        doc.undo()

    line, _ = doc.add_line((0, 0), (1, 0))
    doc.move_entities([line], (0, 5))
    assert doc.is_modified

    assert doc.undo().modified == (line,)
    assert line_of(doc, line) == CadLine((0, 0), (1, 0))
    assert doc.undo().deleted == (line,)
    assert doc.get_entities() == [] and not doc.is_modified
    assert (doc.can_undo, doc.redo_name) == (False, "LINE")

    assert doc.redo().created == (line,)
    doc.redo()
    assert line_of(doc, line) == CadLine((0, 5), (1, 5))
    assert not doc.can_redo

    doc.undo()
    doc.add_circle((0, 0), 1.0)
    assert not doc.can_redo, "a new operation clears redo"


# Groups


def test_a_group_is_one_undo_step(doc: CadDocument) -> None:
    with doc.group("COPY") as group:
        assert doc.in_group and group.changes is None
        a, _ = doc.add_line((0, 0), (1, 0))
        b, _ = doc.add_line((0, 1), (1, 1))
        doc.move_entities([a], (0, 2))
    assert not doc.in_group
    assert group.changes is not None
    assert group.changes.created == (a, b), "created then moved is created"
    assert group.changes.modified == ()
    assert doc.undo_name == "COPY"
    assert doc.undo().deleted == (a, b)
    assert doc.get_entities() == [] and not doc.can_undo


def test_an_exception_cancels_the_group(doc: CadDocument) -> None:
    keep, _ = doc.add_line((0, 0), (1, 0))
    with pytest.raises(ValueError, match="user cancelled"), doc.group("CMD") as group:
        doc.add_line((5, 5), (6, 6))
        doc.move_entities([keep], (9, 9))
        raise ValueError("user cancelled")
    assert [e.id for e in doc.get_entities()] == [keep]
    assert line_of(doc, keep) == CadLine((0, 0), (1, 0))
    assert (doc.in_group, group.changes, doc.undo_name) == (False, None, "LINE")


def test_a_failed_operation_rolls_back_the_whole_group(doc: CadDocument) -> None:
    with pytest.raises(EntityNotFoundError), doc.group("CMD"):
        doc.add_line((5, 5), (6, 6))
        doc.delete_entities([424_242])
    assert doc.get_entities() == []
    assert not doc.in_group and not doc.can_undo


def test_nested_groups_and_the_manual_form(doc: CadDocument) -> None:
    with doc.group("OUTER") as outer:
        doc.add_line((0, 0), (1, 0))
        with doc.group("INNER") as inner:
            doc.add_line((0, 1), (1, 1))
        assert inner.changes is not None and inner.changes.is_empty, "nested end"
        assert doc.in_group
    assert outer.changes is not None and len(outer.changes.created) == 2
    assert doc.undo_name == "OUTER"

    doc.begin_group("MANUAL")
    doc.add_circle((0, 0), 1.0)
    with pytest.raises(InvalidOperationError, match="group is open"):
        doc.undo()
    assert len(doc.end_group().created) == 1
    doc.begin_group("DROPPED")
    doc.add_circle((9, 9), 1.0)
    assert len(doc.cancel_group().deleted) == 1
    with pytest.raises(InvalidOperationError, match="no group"):
        doc.end_group()


def test_an_empty_group_makes_no_step(doc: CadDocument) -> None:
    with doc.group("NOTHING") as group:
        pass
    assert group.changes is not None and group.changes.is_empty
    assert not doc.can_undo
    assert repr(group) == "CadGroup(name='NOTHING')"
