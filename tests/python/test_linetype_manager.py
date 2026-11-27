"""The LINETYPE dialog exposes the simple linetype workflow."""

from pathlib import Path

import pytest
from PySide6.QtCore import Qt
from pytestqt.qtbot import QtBot

from arqstudiocad.core import CadApplication
from arqstudiocad.paths import linetypes_dir
from arqstudiocad.ui.linetype_manager_dialog import LinetypeManagerDialog


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
    assert len(names) == 38
    simple_names = {"BORDER", "CENTER", "DASHDOT", "DASHED", "DIVIDE", "DOT", "HIDDEN", "PHANTOM"}
    assert simple_names <= set(names)
    assert {f"ISO{index:02d}W100" for index in range(2, 16)} <= set(names)


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
