"""Approved closed BREAK behavior, persistence and command lifecycle."""

import math
from pathlib import Path

import pytest
from test_trim import Harness

from arqstudiocad.core import CadApplication, CadColor, CadDocument, InvalidGeometryError
from arqstudiocad.i18n import set_language


def exported_polyline(
    app: CadApplication, doc: CadDocument, path: Path
) -> tuple[dict[int, str], list[tuple[float, ...]]]:
    app.save_document(doc.id, path)
    lines = path.read_text(encoding="utf-8").splitlines()
    properties: dict[int, str] = {}
    vertices: list[dict[int, float]] = []
    inside = False
    for a, b in zip(lines[::2], lines[1::2], strict=True):
        code = int(a.strip())
        value = b.strip()
        if code == 0:
            if inside:
                break
            inside = value == "LWPOLYLINE"
        elif inside:
            if code == 10:
                vertices.append({10: float(value)})
            elif code in (20, 40, 41, 42) and vertices:
                vertices[-1][code] = float(value)
            else:
                properties[code] = value
    assert vertices, "the check must inspect a real polyline"
    return properties, [tuple(v.get(code, 0.0) for code in (10, 20, 42, 40, 41)) for v in vertices]


@pytest.mark.parametrize("suffix", ["dwg", "dxf"])
@pytest.mark.parametrize("curved", [False, True])
def test_closed_break_binding_preserves_handle_bulge_width_and_round_trips(
    tmp_path: Path,
    suffix: str,
    curved: bool,
) -> None:
    app = CadApplication()
    doc = app.create_document("closed break")
    vertices = [
        ((0, 0), 0.0),
        ((10, 0), 0.0),
        ((10, 5 if curved else 10), 1.0 if curved else 0.0),
        ((0, 5 if curved else 10), 0.0),
    ]
    source, _ = doc.add_polyline(vertices, True, 7, 3, 0.5)
    layer, _ = doc.create_layer("closed-break")
    doc.set_entity_layer([source], layer)
    doc.set_entity_color([source], CadColor.from_index(3))
    first, second = ((10, 2), (5, 10)) if curved else ((10, 5), (5, 10))
    history = doc.undo_history
    for bad in (float("nan"), float("inf")):
        with pytest.raises(InvalidGeometryError):
            doc.break_entity(source, (bad, 0), second)
        assert doc.undo_history == history
    ids, changes = doc.break_entity(source, first, second)
    assert ids == [source] and list(changes.modified) == [source]
    assert not changes.created and not changes.deleted
    path = tmp_path / f"closed-break.{suffix}"
    app.save_document(doc.id, path)
    back = app.open_document(path)
    assert len(back.get_entities()) == 1
    entity = back.get_entity(source)
    assert entity.kind == "LWPOLYLINE" and entity.layer == "closed-break"
    assert entity.color == CadColor.from_index(3)
    props, out = exported_polyline(app, back, tmp_path / f"inspect-{suffix}.dxf")
    assert int(props[70]) & 1 == 0
    assert float(props.get(43, "0")) == 0
    assert float(props[38]) == 7 and float(props[39]) == 3
    expected = [
        (5.0, 10.0, math.sqrt(2) - 1 if curved else 0.0),
        (0.0, 5.0 if curved else 10.0, 0.0),
        (0.0, 0.0, 0.0),
        (10.0, 0.0, 0.0),
        (10.0, 2.0 if curved else 5.0, 0.0),
    ]
    assert len(out) == len(expected)
    for vertex, want in zip(out, expected, strict=True):
        assert vertex[:3] == pytest.approx(want)
    assert [v[3:] for v in out] == [(0.5, 0.5)] * 4 + [(0.0, 0.0)]
    doc.undo()
    props, _ = exported_polyline(app, doc, tmp_path / f"undo-{suffix}.dxf")
    assert int(props[70]) & 1 == 1 and float(props[43]) == 0.5
    doc.redo()
    _, again = exported_polyline(app, doc, tmp_path / f"redo-{suffix}.dxf")
    assert again == out


@pytest.mark.parametrize("language", ["en", "es"])
@pytest.mark.parametrize("second", ["5,10", "@"])
def test_closed_break_command_first_point_equal_point_and_cancel(
    language: str, second: str
) -> None:
    set_language(language)
    h = Harness()
    source, _ = h.document.add_polyline(
        [((0, 0), 0), ((10, 0), 0), ((10, 10), 0), ((0, 10), 0)], True
    )
    before = h.document.undo_history
    h.run("PARTE" if language == "es" else "BREAK")
    h.runner.answer_entity(source, (10, 5))
    h.runner.cancel()
    assert h.document.undo_history == before
    assert len(list(h.document.grips([source]))) == 4
    h.run("PARTE" if language == "es" else "BR")
    h.runner.answer_entity(source, (10, 0))
    h.run("F", "10,5", second)
    assert h.runner.active is None and h.document.undo_history[-1] == "BREAK"
    assert [e.id for e in h.document.get_entities()] == [source]
    grips = list(h.document.grips([source]))
    assert len(grips) == (6 if second == "@" else 5)
    assert grips[0].point == pytest.approx((10, 5) if second == "@" else (5, 10))
    assert grips[-1].point == pytest.approx((10, 5))
