"""Entity and layer queries from Python (ADR 0012, step 4.3).

The drawing is an R12 DXF written here, as an old program would: it has
every case the queries must describe, and expected values are worked out by
hand from it (DXF stores arc angles in degrees).
"""

import math
from pathlib import Path

import pytest

from arqstudiocad.core import (
    CadApplication,
    CadArc,
    CadBounds2,
    CadCircle,
    CadColor,
    CadDocument,
    CadEntityInfo,
    CadLayerInfo,
    CadLine,
    CadSnap,
    DocumentNotFoundError,
    EntityNotFoundError,
    InvalidGeometryError,
    LayerNotFoundError,
)

LAYERS = [
    # name, flags (1 frozen, 4 locked), color (negative = off)
    ("0", 0, 7),
    ("MUROS", 0, 1),
    ("TEXTOS", 4, 3),
    ("Congelada", 1, 5),
    ("apagada", 0, -6),
]


def _layer(name: str, flags: int, color: int) -> list[str]:
    return ["0", "LAYER", "2", name, "70", str(flags), "62", str(color), "6", "CONTINUOUS"]


def _entities() -> list[list[str]]:
    def xyz(code: int, x: float, y: float, z: float) -> list[str]:
        return [str(code), str(x), str(code + 10), str(y), str(code + 20), str(z)]

    return [
        # 1. A wall in plan.
        ["0", "LINE", "8", "MUROS", *xyz(10, 0.0, 0.0, 0.0), *xyz(11, 10.0, 0.0, 0.0)],
        # 2. The same at Z = 3: still plan, with elevation.
        ["0", "LINE", "8", "MUROS", *xyz(10, 0.0, 2.0, 3.0), *xyz(11, 10.0, 2.0, 3.0)],
        # 3. A sloped line: not in plan, so read-only.
        ["0", "LINE", "8", "0", *xyz(10, 1.0, 1.0, 0.0), *xyz(11, 2.0, 1.0, 5.0)],
        # 4. A circle with its own color (ACI 1).
        ["0", "CIRCLE", "8", "0", "62", "1", *xyz(10, 5.0, 5.0, 0.0), "40", "2.0"],
        # 5. A quarter arc, 0° to 90°.
        ["0", "ARC", "8", "0", *xyz(10, 0.0, 0.0, 0.0), "40", "3.0", "50", "0.0", "51", "90.0"],
        # 6. A text: kept, read-only.
        ["0", "TEXT", "8", "TEXTOS", *xyz(10, 20.0, 20.0, 0.0), "40", "2.5", "1", "NOTA"],
    ]


@pytest.fixture
def doc(tmp_path: Path) -> CadDocument:
    codes = ["0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1009"]
    codes += ["9", "$CLAYER", "8", "MUROS", "0", "ENDSEC"]
    codes += ["0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", str(len(LAYERS))]
    for layer in LAYERS:
        codes += _layer(*layer)
    codes += ["0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "ENTITIES"]
    for entity in _entities():
        codes += entity
    codes += ["0", "ENDSEC", "0", "EOF", ""]
    path = tmp_path / "consultas.dxf"
    path.write_text("\n".join(codes), encoding="ascii")
    return CadApplication().open_document(path)


def test_entities_in_drawing_order(doc: CadDocument) -> None:
    entities = doc.get_entities()
    assert [e.kind for e in entities] == ["LINE", "LINE", "LINE", "CIRCLE", "ARC", "TEXT"]
    assert [e.layer for e in entities] == ["MUROS", "MUROS", "0", "0", "0", "TEXTOS"]
    assert len({e.id for e in entities}) == 6, "distinct ids"
    assert [e.is_editable for e in entities] == [True, True, False, True, True, False]
    # A text in plan transforms as a whole (ADR 0022); a sloped line does not.
    assert [e.is_transformable for e in entities] == [True, True, False, True, True, True]


def test_geometry_of_each_kind(doc: CadDocument) -> None:
    wall, raised, sloped, circle, arc, text = doc.get_entities()

    assert wall.geometry == CadLine((0.0, 0.0), (10.0, 0.0))
    assert wall.elevation == 0.0
    assert raised.geometry == CadLine((0.0, 2.0), (10.0, 2.0))
    assert raised.elevation == 3.0

    match circle.geometry:
        case CadCircle(center, radius):
            assert (center, radius) == ((5.0, 5.0), 2.0)
        case other:
            pytest.fail(f"not a circle: {other!r}")
    assert circle.color == CadColor.from_index(1)
    assert wall.color == CadColor.by_layer()

    assert isinstance(arc.geometry, CadArc)
    assert arc.geometry.start_angle == pytest.approx(0.0)
    assert arc.geometry.end_angle == pytest.approx(math.pi / 2), "degrees became radians"
    assert arc.geometry.end_point == pytest.approx((0.0, 3.0))

    for read_only in (sloped, text):
        assert (read_only.geometry, read_only.elevation) == (None, None)
        assert read_only.bounds is not None, "still has an extent"
    assert sloped.bounds == CadBounds2((1.0, 1.0), (2.0, 1.0))
    assert text.bounds is not None
    assert text.bounds.min == pytest.approx((20.0, 20.0))


def test_one_entity_and_several_by_id(doc: CadDocument) -> None:
    all_ = doc.get_entities()
    assert doc.get_entity(all_[3].id) == all_[3]
    picked = doc.get_entities([all_[4].id, all_[0].id])
    assert picked == [all_[4], all_[0]], "in the order asked"
    assert doc.get_entities([]) == []
    missing = max(e.id for e in all_) + 1000
    with pytest.raises(EntityNotFoundError) as caught:
        doc.get_entity(missing)
    assert caught.value.entity_id == missing
    with pytest.raises(EntityNotFoundError):
        doc.get_entities([all_[0].id, missing])


def test_bounds_and_window_query(doc: CadDocument) -> None:
    bounds = doc.get_bounds()
    assert bounds is not None
    assert bounds.min == (0.0, 0.0)
    assert bounds.max[0] > 20.0 and bounds.max[1] > 20.0, "reaches the text"

    ids = [e.id for e in doc.get_entities()]
    wall, raised, sloped, circle, arc, text = ids
    assert doc.query_entities(CadBounds2((4.0, 4.0), (6.0, 6.0))) == [circle]
    assert doc.query_entities(CadBounds2((5.0, 1.9), (6.0, 2.1))) == [raised]
    # Inside the arc's box [0, 3] x [0, 3] and on the sloped line's.
    assert doc.query_entities(CadBounds2((1.5, 0.9), (1.8, 1.1))) == [sloped, arc]
    assert doc.query_entities(CadBounds2((19.0, 19.0), (21.0, 21.0))) == [text]
    assert doc.query_entities(CadBounds2((-1.0, -1.0), (0.5, 0.5))) == [wall, arc]
    assert doc.query_entities(bounds) == ids
    assert doc.query_entities(CadBounds2((100.0, 100.0), (200.0, 200.0))) == []


def test_layers(doc: CadDocument) -> None:
    layers = doc.get_layers()
    assert [layer.name for layer in layers] == ["0", "apagada", "Congelada", "MUROS", "TEXTOS"]
    by_name = {layer.name: layer for layer in layers}
    assert by_name["MUROS"].color == CadColor.from_index(1)
    assert by_name["MUROS"].linetype.upper() == "CONTINUOUS"
    assert by_name["Congelada"].is_frozen and not by_name["Congelada"].is_locked
    assert by_name["TEXTOS"].is_locked and not by_name["TEXTOS"].is_frozen
    assert not by_name["apagada"].is_visible
    assert by_name["apagada"].color == CadColor.from_index(6), "off keeps its color"
    assert all(layer.is_visible for name, layer in by_name.items() if name != "apagada")
    assert doc.current_layer == by_name["MUROS"], "from $CLAYER"


def test_one_layer_by_id_and_by_name(doc: CadDocument) -> None:
    walls = doc.get_layer_by_name("muros")
    assert walls is not None and walls.name == "MUROS", "names ignore case"
    assert doc.get_layer(walls.id) == walls
    assert doc.get_layer_by_name("no-existe") is None
    missing = max(layer.id for layer in doc.get_layers()) + 1000
    with pytest.raises(LayerNotFoundError) as caught:
        doc.get_layer(missing)
    assert caught.value.layer_id == missing


def test_infos_are_immutable_copies(doc: CadDocument, tmp_path: Path) -> None:
    entity = doc.get_entities()[0]
    layer = doc.get_layers()[0]
    assert isinstance(entity, CadEntityInfo) and isinstance(layer, CadLayerInfo)
    with pytest.raises(AttributeError):
        entity.layer = "OTRA"  # type: ignore[misc]
    assert repr(entity) == f"CadEntityInfo(id={entity.id}, kind='LINE', layer='MUROS')"
    assert repr(layer) == f"CadLayerInfo(id={layer.id}, name='0')"
    doc.close()
    assert entity.kind == "LINE", "readable after the document is gone"
    with pytest.raises(DocumentNotFoundError):
        doc.get_entities()


def test_pick_window_and_snaps() -> None:
    """Picking, window selection and object snaps on drawn geometry (ADR 0016 D2, D5)."""
    doc = CadApplication().create_document("vacio")
    line = doc.add_line((0, 0), (10, 0))[0]
    circle = doc.add_circle((20, 0), 2)[0]
    assert doc.pick((5, 0.2), 0.5) == line
    assert doc.pick((5, 1.0), 0.5) is None
    assert doc.pick([20, 2.1], 0.5) == circle, "any sequence of two numbers"
    assert doc.select_window(CadBounds2((-1, -1), (11, 1))) == [line]
    assert doc.select_window(CadBounds2((21.5, -1), (23, 1))) == []
    assert doc.select_window(CadBounds2((21.5, -1), (23, 1)), crossing=True) == [circle]

    snap = doc.snap((9.8, 0.1), 0.5, ["END", "mid"])
    assert snap is not None
    assert (snap.kind, snap.point) == ("END", (10.0, 0.0))
    match snap:
        case CadSnap(kind, point):
            assert (kind, point) == ("END", (10.0, 0.0))
    assert hash(snap) == hash(doc.snap((9.8, 0.1), 0.5, ["END"]))
    center = doc.snap((21.9, 0.6), 0.5, ["CEN"])
    assert center is not None and center.point == (20.0, 0.0)
    perp = doc.snap((2, 0.1), 0.5, ["PER"], last_point=(7, 3))
    assert perp is not None and (perp.kind, perp.point) == ("PER", (7.0, 0.0))
    assert doc.snap((2, 0.1), 0.5, ["PER"]) is None, "PER needs a last point"
    assert doc.snap((50, 50), 0.5, ["END"]) is None


def test_bad_apertures_and_snap_keys_are_refused() -> None:
    doc = CadApplication().create_document("vacio")
    doc.add_line((0, 0), (10, 0))
    for bad in (float("nan"), float("inf"), -1.0):
        with pytest.raises(InvalidGeometryError):
            doc.pick((0, 0), bad)
        with pytest.raises(InvalidGeometryError):
            doc.snap((0, 0), bad, ["END"])
    with pytest.raises(ValueError, match="unknown object snap"):
        doc.snap((0, 0), 1.0, ["ENDPOINT"])
    with pytest.raises(TypeError):
        doc.snap((0, 0), 1.0, "END")  # a str is not a list of keys
