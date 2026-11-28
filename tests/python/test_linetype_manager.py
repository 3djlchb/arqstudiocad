"""The LINETYPE dialog exposes the simple linetype workflow."""

import struct
from pathlib import Path

import pytest
from PySide6.QtCore import Qt
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication
from arqstudiocad.paths import linetypes_dir
from arqstudiocad.ui.linetype_manager_dialog import LinetypeManagerDialog
from arqstudiocad.ui.linetype_preview import linetype_preview
from arqstudiocad.ui.shx_shapes import _decode_vectors, shape_strokes


def test_shx_shape_library_preview_resolves_name_and_vector_strokes(
    qtbot: QtBot, tmp_path: Path
) -> None:
    source = tmp_path / "symbols.shx"
    signature = b"AutoCAD-86 shapes 1.0\x1a"
    body = b"CIRC1\0\x42\x00"
    source.write_bytes(
        signature + struct.pack("<HHH", 42, 42, 1) + struct.pack("<HH", 42, len(body)) + body
    )

    shape = shape_strokes(str(source), "CIRC1")

    assert shape is not None
    assert shape[0] == 42
    assert shape[1] == "circ1"
    assert len(shape[2]) == 1
    assert shape[2][0][0] == (0.0, 0.0)
    assert shape[2][0][1][0] > 0
    preview = linetype_preview(
        (1.0, -1.0), ((1, "shape", "CIRC1", str(source), 1, 0, "relative", 0, 0),)
    )
    assert not preview.pixmap(64, 16).isNull()


def test_shx_shape_preview_decodes_full_octant_arc() -> None:
    paths = _decode_vectors(bytes((10, 3, 0, 0)))

    assert len(paths) == 1
    assert len(paths[0]) > 8
    assert paths[0][0] == pytest.approx(paths[0][-1])


def test_loading_complex_linetype_resolves_available_shx_shape_number(
    qtbot: QtBot, tmp_path: Path
) -> None:
    document = CadApplication().create_document("Drawing")
    shx = tmp_path / "symbols.shx"
    body = b"CIRC1\0\x42\x00"
    shx.write_bytes(
        b"AutoCAD-86 shapes 1.0\x1a"
        + struct.pack("<HHH", 42, 42, 1)
        + struct.pack("<HH", 42, len(body))
        + body
    )
    source = tmp_path / "shape.lin"
    source.write_text(f"*FENCE,Fence\nA,1,-1,[CIRC1,{shx},S=1],-1\n")
    dialog = LinetypeManagerDialog(document)
    qtbot.addWidget(dialog)
    dialog._load_file_path(source)

    dialog._load()

    definition = next(
        item for item in document.get_linetype_definition_details() if item[0] == "FENCE"
    )
    assert definition[3][0][2] == "#42"


def test_lin_file_directory_is_searched_for_its_shx_files(qtbot: QtBot, tmp_path: Path) -> None:
    document = CadApplication().create_document("Drawing")
    body = b"CIRC1\0\x42\x00"
    (tmp_path / "symbols.shx").write_bytes(
        b"AutoCAD-86 shapes 1.0\x1a"
        + struct.pack("<HHH", 42, 42, 1)
        + struct.pack("<HH", 42, len(body))
        + body
    )
    source = tmp_path / "shape.lin"
    source.write_text("*FENCE,Fence\nA,1,-1,[CIRC1,symbols.shx,S=1],-1\n")
    dialog = LinetypeManagerDialog(document)
    qtbot.addWidget(dialog)
    dialog._load_file_path(source)

    dialog._load()

    definition = next(
        item for item in document.get_linetype_definition_details() if item[0] == "FENCE"
    )
    assert definition[3][0][2] == "#42"


def test_linetype_dialog_loads_selected_lin_definitions(
    qtbot: QtBot, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    document = CadApplication().create_document("Drawing")
    source = tmp_path / "custom.lin"
    source.write_text("*DASHED,Dashed\nA,0.5,-0.25\n*CENTER,Center\nA,1,-0.25,0,-0.25\n")
    dialog = LinetypeManagerDialog(document)
    qtbot.addWidget(dialog)
    monkeypatch.setattr(
        "arqstudiocad.ui.linetype_manager_dialog.QFileDialog.getOpenFileName",
        lambda *_args, **_kwargs: (str(source), ""),
    )

    dialog._choose_file()
    assert dialog._available.count() == 2
    dialog._available.item(0).setCheckState(Qt.CheckState.Unchecked)
    dialog._load()

    assert "CENTER" in document.get_linetypes()
    assert "DASHED" not in document.get_linetypes()


def test_load_dialog_lists_the_bundled_opencadstudio_catalog(qtbot: QtBot) -> None:
    document = CadApplication().create_document("Drawing")
    dialog = LinetypeManagerDialog(document, load_only=True)
    qtbot.addWidget(dialog)

    names = [
        dialog._available.item(index).data(Qt.ItemDataRole.UserRole)
        for index in range(dialog._available.count())
    ]
    assert dialog._source.text() == str(linetypes_dir() / "OpenCADStudio.lin")
    assert len(names) == 50
    simple_names = {"BORDER", "CENTER", "DASHDOT", "DASHED", "DIVIDE", "DOT", "HIDDEN", "PHANTOM"}
    assert simple_names <= set(names)
    assert {f"ISO{index:02d}W100" for index in range(2, 16)} <= set(names)
    assert {"FENCELINE1", "GAS_LINE", "DRAINAGE", "PLANT"} <= set(names)


def test_layer_load_dialog_loads_selected_bundled_linetype(qtbot: QtBot) -> None:
    document = CadApplication().create_document("Drawing")
    dialog = LinetypeManagerDialog(document, load_only=True)
    qtbot.addWidget(dialog)
    for index in range(dialog._available.count()):
        item = dialog._available.item(index)
        if item.data(Qt.ItemDataRole.UserRole) != "DASHED":
            item.setCheckState(Qt.CheckState.Unchecked)

    dialog._load()

    assert "DASHED" in document.get_linetypes()
    assert "CENTER" not in document.get_linetypes()
    assert dialog.result() == LinetypeManagerDialog.DialogCode.Accepted
    assert dialog._loaded.count() == 2


def test_missing_shx_still_loads_complex_linetype_and_warns(
    qtbot: QtBot, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    document = CadApplication().create_document("Drawing")
    source = tmp_path / "complex.lin"
    source.write_text(
        '*TEXT_MARKER,Text marker\nA,1,-0.2,["HW",STANDARD,S=1,R=0,X=0,Y=0],-0.2\n'
        "*SHAPE_MARKER,Shape marker\nA,1,-0.2,[CIRC1,missing.shx,S=1],-0.2\n"
    )
    notices: list[str] = []
    monkeypatch.setattr(
        "arqstudiocad.ui.linetype_manager_dialog.QMessageBox.information",
        lambda _parent, _title, message: notices.append(message),
    )
    dialog = LinetypeManagerDialog(document)
    qtbot.addWidget(dialog)
    dialog._load_file_path(source)
    for index in range(dialog._available.count()):
        item = dialog._available.item(index)
        if item.data(Qt.ItemDataRole.UserRole) != "SHAPE_MARKER":
            item.setCheckState(Qt.CheckState.Unchecked)

    dialog._load()

    assert "SHAPE_MARKER" in document.get_linetypes()
    assert notices and "missing.shx" in notices[0]
